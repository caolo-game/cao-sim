mod config;
mod init;
mod input;

use anyhow::Context;
use caolo_sim::{executor::mp_executor, executor::Executor, prelude::*};
use mp_executor::MpExecutor;
use slog::{debug, error, info, o, trace, warn, Drain, Logger};
use sqlx::postgres::PgPool;
use std::{
    env,
    time::{Duration, Instant},
};

#[cfg(feature = "jemallocator")]
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
) -> anyhow::Result<()> {
    todo!()
    // use cao_messages::script_capnp::schema;
    // debug!(logger, "Sending schema");
    // let schema = caolo_sim::scripting_api::make_import();
    // let imports = schema.imports();
    //
    // let mut msg = capnp::message::Builder::new_default();
    // let mut root = msg.init_root::<schema::Builder>();
    //
    // let len = imports.len();
    // let mut cards = root.reborrow().init_cards(len as u32);
    // imports.iter().enumerate().for_each(|(i, import)| {
    //     let import = &import.desc;
    //     let mut card = cards.reborrow().get(i as u32);
    //     card.set_name(import.name);
    //     card.set_description(import.description);
    //     card.set_ty(
    //         serde_json::to_string(&import.ty)
    //             .expect("Set card type")
    //             .as_str(),
    //     );
    //     {
    //         let len = import.input.len();
    //         let mut inputs = card.reborrow().init_input(len as u32);
    //         import
    //             .input
    //             .iter()
    //             .enumerate()
    //             .for_each(|(i, inp)| inputs.set(i as u32, inp));
    //     }
    //     {
    //         let len = import.output.len();
    //         let mut outputs = card.reborrow().init_output(len as u32);
    //         import
    //             .output
    //             .iter()
    //             .enumerate()
    //             .for_each(|(i, inp)| outputs.set(i as u32, inp));
    //     }
    //     {
    //         let len = import.constants.len();
    //         let mut constants = card.reborrow().init_constants(len as u32);
    //         import
    //             .constants
    //             .iter()
    //             .enumerate()
    //             .for_each(|(i, inp)| constants.set(i as u32, inp));
    //     }
    // });
    //
    // let mut payload = Vec::with_capacity(1_000_000);
    // capnp::serialize::write_message(&mut payload, &msg)?;
    //
    // let mut con = client.get_connection()?;
    //
    // redis::pipe()
    //     .cmd("SET")
    //     .arg("SCHEMA")
    //     .arg(payload)
    //     .query(&mut con)
    //     .with_context(|| "Failed to set SCHEMA")?;
    //
    // debug!(logger, "Sending schema done");
}

fn main() {
    init();
    let sim_rt = caolo_sim::init_runtime();
    let _guard = sim_rt.enter();

    let game_conf = config::GameConfig::load();

    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_envlogger::new(drain).fuse();
    let drain = slog_async::Async::new(drain)
        .overflow_strategy(slog_async::OverflowStrategy::DropAndReport)
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

    let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379/0".to_owned());

    info!(logger, "Loaded Redis Url {:?}", redis_url);

    let queen_mutex_expiry_ms = env::var("CAO_QUEEN_MUTEX_EXPIRY_MS")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(2000);

    let script_chunk_size = env::var("CAO_QUEEN_SCRIPT_CHUNK_SIZE")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(1024);

    let tick_freq = Duration::from_millis(game_conf.target_tick_freq_ms);

    info!(
        logger,
        "Loaded Queen params:\nMutex expiry: {}\nScript chunk size: {}\nTick freq: {:?}",
        queen_mutex_expiry_ms,
        script_chunk_size,
        tick_freq
    );
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:admin@localhost:5432/caolo".to_owned());

    info!(logger, "Creating cao executor");
    let mut executor = sim_rt
        .block_on(MpExecutor::new(
            &sim_rt,
            logger.clone(),
            mp_executor::ExecutorOptions {
                postgres_url: database_url.clone(),
                queen_mutex_expiry_ms,
                script_chunk_size,
                expected_frequency: chrono::Duration::milliseconds(
                    game_conf.target_tick_freq_ms as i64,
                ),
                ..Default::default()
            },
        ))
        .expect("Create executor");
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
    info!(logger, "Starting with {} actors", game_conf.n_actors);

    sim_rt
        .block_on(executor.update_role())
        .expect("Update role");

    if executor.is_queen() {
        init::init_storage(logger.clone(), &mut storage, &game_conf);

        sim_rt
            .block_on(async move {
                let pg_conn = sqlx::postgres::PgPoolOptions::new()
                    .connect(database_url.as_str())
                    .await
                    .expect("Connect to PG");

                send_schema(logger.clone(), &mut pg_conn.acquire().await?).await
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

        // use the sleep time to update inputs
        // this allows faster responses to clients as well as potentially spending less time on
        // inputs because handling them is built into the sleep cycle
        while sleep_duration > Duration::from_millis(0) {
            let start = Instant::now();
            sim_rt
                .block_on(executor.update_role())
                .expect("Failed to update executors role");
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
