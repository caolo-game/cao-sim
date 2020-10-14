mod config;
mod init;
mod input;
mod output;

use anyhow::Context;
use cao_messages::world_capnp::world_state;
use caolo_sim::prelude::*;
use slog::{debug, error, info, o, trace, warn, Drain, Logger};
use sqlx::postgres::PgPool;
use std::time::{Duration, Instant};
use thiserror::Error;

#[cfg(feature = "jemallocator")]
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

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

fn send_config(
    logger: Logger,
    client: &redis::Client,
    storage: &World,
    game_conf: &config::GameConfig,
) -> anyhow::Result<()> {
    debug!(logger, "Sending config");

    let rooms_props = storage.resource::<RoomProperties>();

    let conf = serde_json::json!({
        "roomProperties" : *rooms_props,
        "gameConfig": game_conf
    });

    let payload = serde_json::to_vec(&conf)?;

    let mut con = client.get_connection()?;
    redis::pipe()
        .cmd("SET")
        .arg("SIM_CONFIG")
        .arg(payload)
        .query(&mut con)
        .with_context(|| "Failed to send config")?;

    debug!(logger, "Sending config - done");

    Ok(())
}

fn send_world(
    logger: Logger,
    storage: &World,
    connection: &mut redis::Connection,
) -> anyhow::Result<()> {
    debug!(logger, "Sending world state");

    let mut msg = capnp::message::Builder::new_default();
    let mut root = msg.init_root::<world_state::Builder>();

    output::build_bots(FromWorld::new(storage), &mut root);
    output::build_resources(FromWorld::new(storage), &mut root);
    output::build_structures(FromWorld::new(storage), &mut root);
    output::build_script_history(FromWorld::new(storage), &mut root);
    output::build_logs(FromWorld::new(storage), &mut root);

    let mut payload = Vec::with_capacity(1_000_000);
    capnp::serialize::write_message(&mut payload, &msg)?;

    debug!(logger, "sending {} bytes", payload.len());

    redis::pipe()
        .cmd("SET")
        .arg("WORLD_STATE")
        .arg(payload)
        .query(connection)
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
    use serde_json::json;
    debug!(logger, "sending terrain ");

    let room_properties = storage
        .view::<EmptyKey, RoomProperties>()
        .reborrow()
        .value
        .as_ref()
        .ok_or(TerrainSendFail::RoomPropertiesNotSet)?;

    let room_radius = room_properties.radius;

    let mut tx = client.begin().await?;
    sqlx::query("DELETE FROM world_map WHERE 1=1;")
        .execute(&mut tx)
        .await?;

    for (Room(room), tiles) in output::iter_rooms_terrain(FromWorld::new(storage)) {
        trace!(
            logger,
            "sending room {:?} terrain, len: {}",
            room,
            tiles.len()
        );

        let q = room.q;
        let r = room.r;

        let payload = json!( {
            "tiles": tiles,
            "roomProperties": {
                "roomRadius": room_radius,
                "roomId": room,
            }
        });

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
    use cao_messages::script_capnp::schema;
    debug!(logger, "Sending schema");
    let schema = caolo_sim::scripting_api::make_import();
    let imports = schema.imports();

    let mut msg = capnp::message::Builder::new_default();
    let mut root = msg.init_root::<schema::Builder>();

    let len = imports.len();
    let mut functions = root.reborrow().init_functions(len as u32);
    imports.iter().enumerate().for_each(|(i, import)| {
        let import = &import.desc;
        let mut func = functions.reborrow().get(i as u32);
        func.set_name(import.name);
        func.set_description(import.description);
        func.set_ty(serde_json::to_string(&import.ty).unwrap().as_str());
        {
            let len = import.input.len();
            let mut inputs = func.reborrow().init_input(len as u32);
            import
                .input
                .iter()
                .enumerate()
                .for_each(|(i, inp)| inputs.set(i as u32, inp));
        }
        {
            let len = import.output.len();
            let mut outputs = func.reborrow().init_output(len as u32);
            import
                .output
                .iter()
                .enumerate()
                .for_each(|(i, inp)| outputs.set(i as u32, inp));
        }
        {
            let len = import.constants.len();
            let mut constants = func.reborrow().init_constants(len as u32);
            import
                .constants
                .iter()
                .enumerate()
                .for_each(|(i, inp)| constants.set(i as u32, inp));
        }
    });

    let mut payload = Vec::with_capacity(1_000_000);
    capnp::serialize::write_message(&mut payload, &msg)?;

    let mut con = client.get_connection()?;

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

    let game_conf = config::GameConfig::load();

    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_envlogger::new(drain).fuse();
    let drain = slog_async::Async::new(drain)
        .overflow_strategy(slog_async::OverflowStrategy::Block)
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

    info!(logger, "Starting with {} actors", game_conf.n_actors);

    let mut storage = init::init_storage(logger.clone(), &game_conf);

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379/0".to_owned());
    let redis_client = redis::Client::open(redis_url.as_str()).expect("Redis client");
    let pg_pool = PgPool::new(
        &std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:admin@localhost:5432/caolo".to_owned()),
    )
    .await?;

    send_config(
        logger.clone(),
        &redis_client,
        &*storage.as_ref(),
        &game_conf,
    )
    .expect("Send config");

    send_terrain(&logger, &*storage.as_ref(), &pg_pool)
        .await
        .expect("Send terrain");

    let tick_freq = Duration::from_millis(game_conf.target_tick_freq_ms);

    send_schema(logger.clone(), &redis_client).expect("Send schema");

    sentry::capture_message(
        "Caolo Worker initialization complete! Starting the game loop",
        sentry::Level::Info,
    );
    let mut redis_connection = redis_client
        .get_connection()
        .with_context(|| "Get redis connection failed")?;
    loop {
        let start = Instant::now();

        tick(logger.clone(), &mut storage);

        send_world(logger.clone(), &storage, &mut redis_connection)
            .map_err(|err| {
                error!(logger, "Failed to send world {:?}", err);
            })
            .unwrap_or(());
        let mut sleep_duration = tick_freq
            .checked_sub(Instant::now() - start)
            .unwrap_or_else(|| Duration::from_millis(0));

        // use the sleep time to update inputs
        // this allows faster responses to clients as well as potentially spending less time on
        // inputs because handling them is built into the sleep cycle
        while sleep_duration > Duration::from_millis(0) {
            let start = Instant::now();
            input::handle_messages(logger.clone(), &mut storage, &mut redis_connection)
                .map_err(|err| {
                    error!(logger, "Failed to handle inputs {:?}", err);
                })
                .unwrap_or(());
            sleep_duration = sleep_duration
                .checked_sub(Instant::now() - start)
                .unwrap_or_else(|| Duration::from_millis(0));
        }
    }
}
