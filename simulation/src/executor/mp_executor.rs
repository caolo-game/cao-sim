//! Multiprocess executor.
//!

mod drone;
mod error;
pub mod execute;
mod queen;
pub mod world_state;

pub use self::drone::*;
pub use self::error::*;
pub use self::queen::*;

use caoq_client::{Client as CaoQClient, MessageId};
use capnp::{message::ReaderOptions, message::TypedReader, serialize::try_read_message};
use chrono::{DateTime, Duration, Utc};
use execute::execute_batch_script_update;

use slog::{debug, error, info, o, Drain, Logger};
use std::fmt::Display;
use uuid::Uuid;
use world_state::{update_world, WorldIoOptionFlags};

use crate::{
    job_capnp::{script_batch_job, script_batch_result},
    prelude::World,
    profile,
    world::init_inmemory_storage,
    RuntimeGuard,
};

use super::Executor;

pub const WORLD_ENTITIES: &str = "CAO_WORLD_ENTITIES";
pub const WORLD_CONFIG: &str = "CAO_WORLD_CONFIG";
pub const WORLD_USERS: &str = "CAO_WORLD_USERS";
pub const WORLD_SCRIPTS: &str = "CAO_WORLD_SCIPTS";
pub const WORLD_TERRAIN: &str = "CAO_WORLD_TERRAIN";
pub const JOB_QUEUE: &str = "CAO_JOB_QUEUE";

type BatchScriptInputMsg<'a> = script_batch_job::Reader<'a>;
type BatchScriptInputReader = TypedReader<capnp::serialize::OwnedSegments, script_batch_job::Owned>;
type ScriptBatchResultReader =
    TypedReader<capnp::serialize::OwnedSegments, script_batch_result::Owned>;

/// Multiprocess executor.
///
pub struct MpExecutor {
    pub logger: Logger,
    pub options: ExecutorOptions,
    pub tag: Uuid,

    pub pool: sqlx::postgres::PgPool,

    pub queue: CaoQClient,

    pub role: Role,

    pub runtime: RuntimeGuard,
}

#[derive(Debug, Clone, Copy)]
pub enum Role {
    /// This is the main/coordinator instance
    Queen,
    /// This is a worker instance
    Drone,
}

impl Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Queen => write!(f, "Queen"),
            Role::Drone => write!(f, "Drone"),
        }
    }
}

#[derive(Debug)]
pub struct ExecutorOptions {
    pub postgres_url: String,
    pub queue_url: String,

    pub queen_mutex_expiry_ms: i64,
    pub script_chunk_size: usize,
    /// Expected time to complete a tick
    pub expected_frequency: Duration,
}

impl Default for ExecutorOptions {
    fn default() -> Self {
        let postgres_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:admin@localhost:5432/caolo".to_owned());
        let queue_url =
            std::env::var("MQ_ADDR").unwrap_or_else(|_| "ws://localhost:6942".to_owned());
        Self {
            postgres_url,
            queue_url,
            queen_mutex_expiry_ms: 2000,
            script_chunk_size: 1024,
            expected_frequency: Duration::seconds(1),
        }
    }
}

impl MpExecutor {
    pub async fn new(
        role: Role,
        rt: &RuntimeGuard,
        logger: impl Into<Option<Logger>>,
        options: impl Into<Option<ExecutorOptions>>,
    ) -> Result<Self, MpExcError> {
        async fn _new(
            role: Role,
            rt: &RuntimeGuard,
            logger: Logger,
            options: ExecutorOptions,
        ) -> Result<MpExecutor, MpExcError> {
            info!(
                logger,
                "Connecting to postgres, url {}", &options.postgres_url
            );
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(4)
                .min_connections(2)
                .connect(options.postgres_url.as_str())
                .await?;

            info!(logger, "Connecting to queue, url {}", &options.queue_url);
            let queue = caoq_client::connect(options.queue_url.as_str())
                .await
                .map_err(MpExcError::QueueClientError)?;

            let tag = Uuid::new_v4();
            info!(logger, "Finished setting up Executor, tag {}", tag);

            Ok(MpExecutor {
                runtime: rt.clone(),
                pool,
                tag,
                logger,
                role,
                options,
                queue,
            })
        }

        let logger = logger.into().unwrap_or_else(|| {
            let decorator = slog_term::TermDecorator::new().build();
            let drain = slog_term::FullFormat::new(decorator).build().fuse();
            let drain = slog_envlogger::new(drain).fuse();
            let drain = slog_async::Async::new(drain).build().fuse();
            slog::Logger::root(drain, o!())
        });

        _new(role, rt, logger.clone(), options.into().unwrap_or_default())
            .await
            .map_err(|err| {
                error!(logger, "Failed to initialize Executor {:?}", err);
                err
            })
    }

    /// Check if this instance is the Queen and if so still holds the mutex.
    pub fn is_queen(&self) -> bool {
        matches!(self.role, Role::Queen)
    }

    /// Execute until the queue is empty
    async fn execute_batch_script_jobs(&mut self, world: &mut World) -> Result<(), MpExcError> {
        debug!(self.logger, "Executing batch script jobs");
        while let Some(delivery) = self.queue.pop_msg().await? {
            let msg_id = delivery.id;
            let message = parse_script_batch(delivery.payload)?.unwrap(); // FIXME
            let message: BatchScriptInputMsg = message.get().map_err(|err| {
                error!(self.logger, "Failed to 'get' capnp message {:?}", err);
                MpExcError::MessageDeserializeError(err)
            })?;
            let expected_time = message.get_world_time();
            if expected_time != world.time() {
                info!(self.logger, "Updating world");
                let mut options = WorldIoOptionFlags::new();
                if world.positions.point_terrain.is_empty() {
                    options = options.all();
                }
                update_world(self, world, Some(expected_time), options).await?;
                self.logger = world.logger.new(o!(
                            "tag" => self.tag.to_string(),
                            "tick" => world.time(),
                            "role" => format!("{}", self.role)));

                info!(self.logger, "Updating world done",);
            }
            if world.time() != expected_time {
                error!(
                    self.logger,
                    "Failed to aquire expected world: {}. Skipping job {:?}", expected_time, msg_id
                );
            }
            execute_batch_script_update(self, msg_id, message, world)
                .await
                .map_err(|err| {
                    error!(self.logger, "Failed to execute message {}", err);
                    err
                })?;
        }
        debug!(self.logger, "Executing batch script jobs done");
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct ScriptBatchStatus {
    id: MessageId,
    from: usize,
    to: usize,
    enqueued: DateTime<Utc>,
    finished: Option<DateTime<Utc>>,
}

impl ScriptBatchStatus {
    fn new(id: MessageId, from: usize, to: usize) -> Self {
        Self {
            id,
            from,
            to,
            enqueued: Utc::now(),
            finished: None,
        }
    }
}

impl Executor for MpExecutor {
    type Error = MpExcError;

    fn initialize(
        &mut self,
        logger: Option<slog::Logger>,
        config: super::GameConfig,
    ) -> Result<std::pin::Pin<Box<World>>, Self::Error> {
        let rt = self.runtime.clone();
        rt.block_on(async move {
            if let Some(logger) = logger.as_ref() {
                self.logger = logger.clone();
            }
            info!(self.logger, "Initializing Storage");
            let mut world = init_inmemory_storage(self.logger.clone());
            if matches!(self.role, Role::Queen) {
                info!(self.logger, "Initializing Queen");
                queen::initialize_queen(self, &mut world, &config).await?;
            }
            Ok(world)
        })
    }

    fn forward(&mut self, world: &mut World) -> Result<(), Self::Error> {
        profile!("world_forward");
        let rt = self.runtime.clone();
        rt.block_on(async move {
            match self.role {
                Role::Queen => queen::forward_queen(self, world).await?,
                Role::Drone => drone::forward_drone(self, world).await?,
            }
            Ok(())
        })
    }
}

fn parse_script_batch(message: Vec<u8>) -> Result<Option<BatchScriptInputReader>, MpExcError> {
    try_read_message(
        message.as_slice(),
        ReaderOptions {
            traversal_limit_in_words: 512,
            nesting_limit: 64,
        },
    )
    .map_err(MpExcError::MessageDeserializeError)
    .map(|reader| reader.map(|r| r.into_typed()))
}

fn parse_script_batch_result(
    message: Vec<u8>,
) -> Result<Option<ScriptBatchResultReader>, MpExcError> {
    try_read_message(
        message.as_slice(),
        ReaderOptions {
            // TODO this limit needs some thinking...
            traversal_limit_in_words: 60_000_000,
            nesting_limit: 64,
        },
    )
    .map_err(MpExcError::MessageDeserializeError)
    .map(|reader| reader.map(|r| r.into_typed()))
}
