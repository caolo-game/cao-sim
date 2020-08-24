mod init;
mod input;
mod output;

use anyhow::Context;
use caolo_sim::prelude::*;
use slog::{debug, error, info, o, trace, warn, Drain, Logger};
use sqlx::postgres::PgPool;
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

#[cfg(feature = "jemallocator")]
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

use cao_messages::{
    Function, RoomProperties as RoomPropertiesMsg, RoomTerrainMessage, Schema, WorldState,
};

fn init() {
    #[cfg(feature = "dotenv")]
    dep_dotenv::dotenv().unwrap_or_default();
}

fn tick(logger: Logger, storage: &mut World) {
    let start = chrono::Utc::now();

    caolo_sim::forward(storage)
        .map(|_| {
            let duration = chrono::Utc::now() - start;

            info!(
                logger,
                "Tick {} has been completed in {} ms",
                storage.time(),
                duration.num_milliseconds()
            );
        })
        .expect("Failed to forward game state")
}

fn send_world(logger: Logger, storage: &World, client: &redis::Client) -> anyhow::Result<()> {
    debug!(logger, "Sending world state");

    let bots: Vec<_> = output::build_bots(FromWorld::new(storage)).collect();

    debug!(logger, "sending {} bots", bots.len());

    let logs: Vec<_> = output::build_logs(FromWorld::new(storage)).collect();

    debug!(logger, "sending {} logs", logs.len());

    let resources: Vec<_> = output::build_resources(FromWorld::new(storage)).collect();

    debug!(logger, "sending {} resources", resources.len());

    let structures: Vec<_> = output::build_structures(FromWorld::new(storage)).collect();

    debug!(logger, "sending {} structures", structures.len());

    let world = WorldState {
        bots,
        logs,
        resources,
        structures,
    };

    let payload = rmp_serde::to_vec_named(&world)?;

    debug!(logger, "sending {} bytes", payload.len());

    let mut con = client.get_connection()?;
    redis::pipe()
        .cmd("SET")
        .arg("WORLD_STATE")
        .arg(payload)
        .query(&mut con)
        .with_context(|| "Failed to send WORLD_STATE")?;

    debug!(logger, "Sending world state done");
    Ok(())
}

#[derive(Debug, Clone, Error)]
pub enum TerrainSendFail {
    #[error("RoomProperties were not set")]
    RoomPropertiesNotSet,
}

async fn send_terrain(logger: &Logger, storage: &World, client: &PgPool) -> anyhow::Result<()> {
    let room_properties = storage
        .view::<EmptyKey, RoomProperties>()
        .reborrow()
        .value
        .as_ref()
        .ok_or_else(|| TerrainSendFail::RoomPropertiesNotSet)?;

    let room_radius = room_properties.radius;

    let mut tx = client.begin().await?;
    sqlx::query("DELETE FROM world_map WHERE 1=1;")
        .execute(&mut tx)
        .await?;

    for (room, tiles) in output::build_terrain(FromWorld::new(storage)) {
        trace!(
            logger,
            "sending room {:?} terrain, len: {}",
            room,
            tiles.len()
        );

        let q = room.q;
        let r = room.r;

        let room_properties = RoomPropertiesMsg {
            room_radius,
            room_id: room,
        };

        let world = RoomTerrainMessage {
            tiles,
            room_properties,
        };

        let payload = serde_json::to_value(&world).unwrap();

        sqlx::query(
            r#"
            INSERT INTO world_map (q, r, payload)
            VALUES ($1, $2, $3)"#,
        )
        .bind(q)
        .bind(r)
        .bind(payload)
        .execute(&mut tx)
        .await?;
    }
    tx.commit().await?;

    debug!(logger, "sending terrain done");
    Ok(())
}

fn send_schema(logger: Logger, client: &redis::Client) -> anyhow::Result<()> {
    debug!(logger, "Sending schema");
    let mut con = client.get_connection()?;

    let schema = caolo_sim::scripting_api::make_import();
    let functions = schema
        .imports()
        .iter()
        .map(|import| {
            let import = &import.desc;
            Function::from_str_parts(
                import.name,
                import.description,
                import.input.as_ref(),
                import.output.as_ref(),
                import.params.as_ref(),
            )
        })
        .collect::<Vec<_>>();

    let schema = Schema { functions };

    let payload = rmp_serde::to_vec_named(&schema).unwrap();

    redis::pipe()
        .cmd("SET")
        .arg("SCHEMA")
        .arg(payload)
        .query(&mut con)
        .with_context(|| "Failed to set SCHEMA")?;

    debug!(logger, "Sending schema done");
    Ok(())
}

#[async_std::main]
async fn main() -> Result<(), anyhow::Error> {
    init();

    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_envlogger::new(drain).fuse();
    let drain = slog_async::Async::new(drain)
        .overflow_strategy(slog_async::OverflowStrategy::DropAndReport)
        .chan_size(16000)
        .build()
        .fuse();
    let logger = slog::Logger::root(drain, o!());

    let _sentry = std::env::var("SENTRY_URI")
        .ok()
        .map(|uri| {
            let options: sentry::ClientOptions = uri.as_str().into();
            sentry::init(options)
        })
        .ok_or_else(|| {
            warn!(logger, "Sentry URI was not provided");
        });

    let n_actors = std::env::var("N_ACTORS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100);

    info!(logger, "Starting with {} actors", n_actors);

    let mut storage = init::init_storage(logger.clone(), n_actors);

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379/0".to_owned());
    let redis_client = redis::Client::open(redis_url.as_str()).expect("Redis client");
    let pg_pool = PgPool::new(
        &std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:admin@localhost:5432/caolo".to_owned()),
    )
    .await?;

    send_terrain(&logger, &*storage.as_ref(), &pg_pool)
        .await
        .expect("Send terrain");

    let tick_freq = std::env::var("TARGET_TICK_FREQUENCY_MS")
        .map(|i| i.parse::<u64>().unwrap())
        .unwrap_or(200);
    let tick_freq = Duration::from_millis(tick_freq);

    send_schema(logger.clone(), &redis_client).expect("Send schema");

    sentry::capture_message(
        "Caolo Worker initialization complete! Starting the game loop",
        sentry::Level::Info,
    );
    loop {
        let start = Instant::now();
        input::handle_messages(logger.clone(), &mut storage, &redis_client);
        tick(logger.clone(), &mut storage);
        send_world(logger.clone(), &storage, &redis_client).expect("Sending world");
        let t = Instant::now() - start;
        let sleep_duration = tick_freq
            .checked_sub(t)
            .unwrap_or_else(|| Duration::from_millis(0));
        thread::sleep(sleep_duration);
    }
}
