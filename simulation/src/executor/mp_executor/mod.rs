//! Multiprocess executor.
//!

mod drone;
pub mod execute;
mod queen;
pub mod world_state;

pub use self::drone::*;
pub use self::queen::*;

use capnp::{message::ReaderOptions, message::TypedReader, serialize::try_read_message};
use chrono::{DateTime, Duration, TimeZone, Utc};
use execute::execute_batch_script_update;
use lapin::options::BasicGetOptions;
use redis::Client as RedisClient;

use slog::{debug, error, info, o, Drain, Logger};
use std::fmt::Display;
use tokio_amqp::*;
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

pub const QUEEN_MUTEX: &str = "CAO_QUEEN_MUTEX";
pub const WORLD_ENTITIES: &str = "CAO_WORLD_ENTITIES";
pub const WORLD_CONFIG: &str = "CAO_WORLD_CONFIG";
pub const WORLD_USERS: &str = "CAO_WORLD_USERS";
pub const WORLD_SCRIPTS: &str = "CAO_WORLD_SCIPTS";
pub const WORLD_TERRAIN: &str = "CAO_WORLD_TERRAIN";
pub const JOB_QUEUE: &str = "CAO_JOB_QUEUE";
pub const JOB_RESULTS_LIST: &str = "CAO_JOB_RESULTS_LIST";

type BatchScriptInputMsg<'a> = script_batch_job::Reader<'a>;
type BatchScriptInputReader = TypedReader<capnp::serialize::OwnedSegments, script_batch_job::Owned>;
type ScriptBatchResultReader =
    TypedReader<capnp::serialize::OwnedSegments, script_batch_result::Owned>;

/// Multiprocess executor.
///
pub struct MpExecutor {
    pub logger: Logger,
    pub options: ExecutorOptions,
    pub tag: String,

    client: RedisClient,

    _amqp_conn: lapin::Connection,
    amqp_chan: lapin::Channel,

    role: Role,

    runtime: RuntimeGuard,
}

#[derive(Debug, Clone, Copy)]
pub enum Role {
    /// This is the main/coordinator instance
    Queen(Queen),
    /// This is a worker instance
    Drone(Drone),
}

impl Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Queen(_) => write!(f, "Queen"),
            Role::Drone(_) => write!(f, "Drone"),
        }
    }
}

#[derive(Debug)]
pub struct ExecutorOptions {
    pub redis_url: String,
    pub amqp_url: String,

    pub queen_mutex_expiry_ms: i64,
    pub script_chunk_size: usize,
    /// Expected time to complete a tick
    pub expected_frequency: Duration,
}

impl Default for ExecutorOptions {
    fn default() -> Self {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379/0".to_owned());
        let amqp_url = std::env::var("AMQP_ADDR")
            .or_else(|_| std::env::var("CLOUDAMQP_URL"))
            .unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".to_owned());
        Self {
            redis_url,
            amqp_url,
            queen_mutex_expiry_ms: 2000,
            script_chunk_size: 1024,
            expected_frequency: Duration::seconds(1),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum MpExcError {
    #[error("Redis error: {0:?}")]
    RedisError(redis::RedisError),

    #[error("Failed to serialize the world state: {0:?}")]
    WorldSerializeError(rmp_serde::encode::Error),

    #[error("Failed to deserialize the world state: {0:?}")]
    WorldDeserializeError(rmp_serde::decode::Error),

    #[error("Failed to serialize message {0:?}")]
    MessageSerializeError(capnp::Error),
    #[error("Failed to deserialize message {0:?}")]
    MessageDeserializeError(capnp::Error),

    #[error("The queen node lost its mutex while executing a world update")]
    QueenRoleLost,

    #[error("AmqpError {0:?}")]
    AmqpError(lapin::Error),

    #[error("Time mismatch while updating world. Requested: {requested}. Actual: {actual}")]
    WorldTimeMismatch { requested: u64, actual: u64 },
}

impl MpExecutor {
    pub async fn new(
        rt: &RuntimeGuard,
        logger: impl Into<Option<Logger>>,
        options: impl Into<Option<ExecutorOptions>>,
    ) -> Result<Self, MpExcError> {
        async fn _new(
            rt: &RuntimeGuard,
            logger: Logger,
            options: ExecutorOptions,
        ) -> Result<MpExecutor, MpExcError> {
            let _g = rt.tokio_rt.enter();

            info!(logger, "Connecting to redis, url {}", &options.redis_url);
            let client =
                RedisClient::open(options.redis_url.as_str()).map_err(MpExcError::RedisError)?;
            let queen_mutex = Utc.timestamp_millis(0);

            info!(logger, "Connecting to amqp, url {}", &options.amqp_url);
            let amqp_conn = lapin::Connection::connect(
                options.amqp_url.as_str(),
                lapin::ConnectionProperties::default().with_tokio(rt.tokio_rt.clone()),
            )
            .await
            .map_err(MpExcError::AmqpError)?;

            let channel = amqp_conn
                .create_channel()
                .await
                .map_err(MpExcError::AmqpError)?;

            let tag = format!("{}", uuid::Uuid::new_v4());

            info!(logger, "Finished setting up Executor, tag {}", tag);

            Ok(MpExecutor {
                runtime: rt.clone(),
                tag,
                logger,
                _amqp_conn: amqp_conn,
                amqp_chan: channel,
                role: Role::Drone(Drone { queen_mutex }),
                client,
                options,
            })
        }

        let logger = logger.into().unwrap_or_else(|| {
            let decorator = slog_term::TermDecorator::new().build();
            let drain = slog_term::FullFormat::new(decorator).build().fuse();
            let drain = slog_envlogger::new(drain).fuse();
            let drain = slog_async::Async::new(drain).build().fuse();
            slog::Logger::root(drain, o!())
        });

        _new(rt, logger.clone(), options.into().unwrap_or_default())
            .await
            .map_err(|err| {
                error!(logger, "Failed to initialize Executor {:?}", err);
                err
            })
    }

    /// Check if this instance is the Queen and if so still holds the mutex.
    pub fn is_queen(&self) -> bool {
        match self.role {
            Role::Queen(Queen { queen_mutex }) => Utc::now() < queen_mutex,
            Role::Drone(_) => false,
        }
    }

    /// Returns the current role of this instance
    pub async fn update_role(&mut self) -> Result<&Role, MpExcError> {
        debug!(self.logger, "Updating role of a {:?} process", self.role);
        let now = Utc::now();
        let new_expiry: i64 =
            (now + Duration::milliseconds(self.options.queen_mutex_expiry_ms)).timestamp_millis();

        let mut connection = self
            .client
            .get_async_connection()
            .await
            .map_err(MpExcError::RedisError)?;

        self.role = match self.role {
            Role::Queen(p) => {
                p.update_role(
                    self.logger.clone(),
                    &mut connection,
                    new_expiry,
                    self.options.queen_mutex_expiry_ms,
                )
                .await?
            }
            Role::Drone(d) => {
                d.update_role(
                    self.logger.clone(),
                    &mut connection,
                    now,
                    new_expiry,
                    self.options.queen_mutex_expiry_ms,
                )
                .await?
            }
        };
        Ok(&self.role)
    }

    /// Execute until the queue is empty
    async fn execute_batch_script_jobs(&mut self, world: &mut World) -> Result<(), MpExcError> {
        debug!(self.logger, "Executing batch script jobs");
        while let Some(message) = self
            .amqp_chan
            .basic_get(JOB_QUEUE, BasicGetOptions { no_ack: true })
            .await
            .map_err(MpExcError::AmqpError)?
        {
            let delivery = message.delivery;

            let message = parse_script_batch(delivery.data)?.unwrap(); // FIXME
            let message: BatchScriptInputMsg = message.get().map_err(|err| {
                error!(self.logger, "Failed to 'get' capnp message {:?}", err);
                MpExcError::MessageDeserializeError(err)
            })?;
            let expected_time = message.get_world_time();
            if expected_time != world.time() {
                info!(self.logger, "Updating world");
                let mut options = WorldIoOptionFlags::new().all();
                if world.positions.point_terrain.is_empty() {
                    options = options.all();
                }
                update_world(self, world, Some(expected_time), options).await?;
                self.logger = world
                    .logger
                    .new(o!("tick" => world.time(), "role" => format!("{}", self.role)));

                info!(self.logger, "Updating world done",);
            }
            if world.time() != expected_time {
                let msg_id = message
                    .get_msg_id()
                    .map_err(MpExcError::MessageDeserializeError)?;

                let msg_id = uuid::Uuid::from_fields(
                    msg_id.get_d1(),
                    msg_id.get_d2(),
                    msg_id.get_d3(),
                    unsafe { &*(&msg_id.get_d4() as *const u64 as *const [u8; 8]) },
                )
                .expect("Failed to deserialize msg id");
                error!(
                    self.logger,
                    "Failed to aquire expected world: {}. Skipping job {}", expected_time, msg_id
                );
            }
            execute_batch_script_update(self, message, world)
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
    id: Uuid,
    from: usize,
    to: usize,
    enqueued: DateTime<Utc>,
    finished: Option<DateTime<Utc>>,
}

impl ScriptBatchStatus {
    fn new(id: Uuid, from: usize, to: usize) -> Self {
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
            info!(self.logger, "Initializing cao-sim mp-executor");
            self.update_role().await?;
            info!(self.logger, "Initializing Storage");
            let mut world = init_inmemory_storage(self.logger.clone());
            if matches!(self.role, Role::Queen(_)) {
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
            self.update_role().await?;
            match self.role {
                Role::Queen(_) => queen::forward_queen(self, world).await?,
                Role::Drone(_) => drone::forward_drone(self, world).await?,
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
