mod config;
mod init;
mod input;

use anyhow::Context;
use cao_lang::SubProgramType;
use caolo_sim::{
    executor::mp_executor::{self, MpExcError},
    executor::Executor,
    prelude::*,
};
use mp_executor::{MpExecutor, Role};
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
    dep_dotenv::dotenv().unwrap_or_default();
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
            ty: SubProgramType::Instruction.as_str(),
        }))
        .collect::<Vec<_>>();

    let payload = rmp_serde::to_vec_named(&msg)?;

    sqlx::query!(
        r#"
    INSERT INTO scripting_schema (queen_tag, schema_message_packed)
    VALUES ($1, $2)
    ON CONFLICT (queen_tag)
    DO UPDATE SET
    schema_message_packed=$2
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

    let role = match env::var("CAO_ROLE")
        .map(|s| s.to_lowercase())
        .as_ref()
        .map(|s| s.as_str())
    {
        Ok("queen") => Role::Queen,
        Ok("drone") => Role::Drone,
        _ => {
            warn!(
                logger,
                "Env var ROLE not set (or invalid). Defaulting to role 'Drone'"
            );
            Role::Drone
        }
    };

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

    info!(logger, "Creating cao executor");
    let mut executor = sim_rt
        .block_on(MpExecutor::new(
            role,
            &sim_rt,
            logger.clone(),
            mp_executor::ExecutorOptions {
                postgres_url: database_url.clone(),
                script_chunk_size,
                expected_frequency: chrono::Duration::milliseconds(
                    game_conf.target_tick_freq_ms as i64,
                ),
                ..Default::default()
            },
        ))
        .expect("Create executor");
    let logger = executor.logger.clone();
    info!(logger, "Init storage");
    let mut storage = executor
        .initialize(
            None,
            caolo_sim::executor::GameConfig {
                world_radius: game_conf.world_radius,
                room_radius: game_conf.room_radius,
            },
        )
        .expect("Initialize executor");

    let queue_url = std::env::var("MQ_ADDR").unwrap_or_else(|_| "ws://localhost:6942".to_owned());
    let mut queue = sim_rt
        .block_on(caoq_client::connect(queue_url.as_str()))
        .map_err(MpExcError::QueueClientError)
        .expect("failed to connect to msg bus");

    info!(logger, "Starting with {} actors", game_conf.n_actors);

    if executor.is_queen() {
        info!(logger, "Running queen setup");
        init::init_storage(logger.clone(), &mut storage, &game_conf);

        let logger = logger.clone();
        let tag = executor.tag;
        sim_rt
            .block_on(async move {
                let pg_conn = sqlx::postgres::PgPoolOptions::new()
                    .connect(database_url.as_str())
                    .await
                    .expect("Connect to PG");

                send_schema(logger, &mut pg_conn.acquire().await?, tag).await
            })
            .expect("Send schema");
    }

    sentry::capture_message(
        format!(
            "Caolo Worker {} initialization complete! Starting the game loop",
            executor.tag
        )
        .as_str(),
        sentry::Level::Info,
    );

    loop {
        let start = Instant::now();

        tick(logger.clone(), &mut executor, &mut storage);

        let mut sleep_duration = tick_freq
            .checked_sub(Instant::now() - start)
            .unwrap_or_else(|| Duration::from_millis(0));

        if !executor.is_queen() {
            std::thread::sleep(sleep_duration);
            continue;
        }

        sim_rt
            .block_on(output(&*storage, &executor.pool, executor.tag))
            .map_err(|err| {
                error!(logger, "Failed to send world output to storage {:?}", err);
            })
            .unwrap_or(());

        sleep_duration = tick_freq
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
                    &mut queue,
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
