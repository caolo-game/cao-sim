mod config;
mod init;
mod input;

use anyhow::Context;
use caolo_sim::{executor::Executor, executor::SimpleExecutor, prelude::*};
use slog::{debug, error, info, o, warn, Drain, Logger};
use std::{
    env,
    time::{Duration, Instant},
};
use uuid::Uuid;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn init() {
    #[cfg(feature = "dotenv")]
    dotenv::dotenv().unwrap_or_default();
}

fn tick(logger: Logger, exc: &mut impl Executor, storage: &mut World) {
    let start = chrono::Utc::now();
    exc.forward(storage)
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

async fn send_schema<'a>(
    logger: Logger,
    connection: impl sqlx::Executor<'a, Database = sqlx::Postgres>,
    queen_tag: Uuid,
) -> anyhow::Result<()> {
    debug!(logger, "Sending schema");
    let schema = caolo_sim::scripting_api::make_import();
    let imports = schema.imports();

    let basic_descs = cao_lang::compiler::description::get_instruction_descriptions();

    #[derive(serde::Serialize)]
    struct Card<'a> {
        name: &'a str,
        description: &'a str,
        ty: &'a str,
        input: &'a [&'a str],
        output: &'a [&'a str],
        constants: &'a [&'a str],
    }

    let msg = imports
        .iter()
        .map(|import| Card {
            name: import.desc.name,
            description: import.desc.description,
            constants: &*import.desc.constants,
            input: &*import.desc.input,
            output: &*import.desc.output,
            ty: import.desc.ty.as_str(),
        })
        .chain(basic_descs.iter().map(|card| Card {
            name: card.name,
            description: card.description,
            input: &*card.input,
            output: &*card.output,
            constants: &*card.constants,
            ty: card.ty.as_str(),
        }))
        .collect::<Vec<_>>();

    let payload = serde_json::to_value(&msg)?;

    sqlx::query!(
        r#"
    INSERT INTO scripting_schema (queen_tag, payload)
    VALUES ($1, $2)
    ON CONFLICT (queen_tag)
    DO UPDATE SET
    payload=$2
        "#,
        queen_tag,
        payload
    )
    .execute(connection)
    .await
    .with_context(|| "Failed to send schema")?;

    debug!(logger, "Sending schema done");
    Ok(())
}

async fn output<'a>(
    world: &'a World,
    connection: impl sqlx::Executor<'a, Database = sqlx::Postgres>,
    queen_tag: Uuid,
) -> anyhow::Result<()> {
    let payload = world.as_json();
    sqlx::query!(
        r#"
        INSERT INTO world_output (queen_tag, world_time, payload)
        VALUES ($1, $2, $3);
        "#,
        queen_tag,
        world.time() as i64,
        payload
    )
    .execute(connection)
    .await
    .with_context(|| "Failed to insert current world state")?;
    Ok(())
}

fn main() {
    init();
    let sim_rt = caolo_sim::init_runtime();

    let game_conf = config::GameConfig::load();

    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_envlogger::new(drain).fuse();
    let drain = slog_async::Async::new(drain)
        .overflow_strategy(slog_async::OverflowStrategy::Drop)
        .build()
        .fuse();
    let logger = slog::Logger::root(drain, o!());

    info!(logger, "Loaded game config {:?}", game_conf);

    let _sentry = env::var("SENTRY_URI")
        .ok()
        .map(|uri| {
            let options: sentry::ClientOptions = uri.as_str().into();
            let integration = sentry_slog::SlogIntegration::default();
            sentry::init(options.add_integration(integration))
        })
        .ok_or_else(|| {
            warn!(logger, "Sentry URI was not provided");
        });

    let script_chunk_size = env::var("CAO_QUEEN_SCRIPT_CHUNK_SIZE")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(1024);

    let tick_freq = Duration::from_millis(game_conf.target_tick_freq_ms);

    info!(
        logger,
        "Loaded Queen params:\nScript chunk size: {}\nTick freq: {:?}",
        script_chunk_size,
        tick_freq
    );
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:admin@localhost:5432/caolo".to_owned());

    let db_pool = sim_rt
        .block_on(sqlx::PgPool::connect(database_url.as_str()))
        .expect("failed to connect to database");

    let tag = uuid::Uuid::new_v4();

    info!(logger, "Creating cao executor");
    let mut executor = SimpleExecutor;
    info!(logger, "Init storage");
    let mut storage = executor
        .initialize(
            Some(logger.clone()),
            caolo_sim::executor::GameConfig {
                world_radius: game_conf.world_radius,
                room_radius: game_conf.room_radius,
            },
        )
        .expect("Initialize executor");

    let queue_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_owned());
    let redis_client = redis::Client::open(queue_url).expect("Failed to connect to redis");

    info!(logger, "Starting with {} actors", game_conf.n_actors);

    sim_rt
        .block_on(send_schema(logger.clone(), &db_pool, tag))
        .expect("Failed to send schema");
    init::init_storage(logger.clone(), &mut storage, &game_conf);

    sentry::capture_message(
        format!(
            "Caolo Worker {} initialization complete! Starting the game loop",
            tag
        )
        .as_str(),
        sentry::Level::Info,
    );

    loop {
        let start = Instant::now();

        tick(logger.clone(), &mut executor, &mut storage);

        sim_rt
            .block_on(output(&*storage, &db_pool, tag))
            .map_err(|err| {
                error!(logger, "Failed to send world output to storage {:?}", err);
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
            sim_rt
                .block_on(input::handle_messages(
                    logger.clone(),
                    &mut storage,
                    &redis_client,
                ))
                .map_err(|err| {
                    error!(logger, "Failed to handle inputs {:?}", err);
                })
                .unwrap_or(());
            sleep_duration = sleep_duration
                .checked_sub(Instant::now() - start)
                // the idea is to sleep for half of the remaining time, then handle messages again
                .and_then(|d| d.checked_div(2))
                .unwrap_or_else(|| Duration::from_millis(0));
            std::thread::sleep(sleep_duration);
        }
    }
}
