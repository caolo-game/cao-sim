//! Multiprocess executor.
//!

mod drone;
mod execute;
mod queen;

pub use self::drone::*;
pub use self::queen::*;

use capnp::{message::ReaderOptions, message::TypedReader, serialize::try_read_message};
use chrono::{DateTime, Duration, TimeZone, Utc};
use execute::execute_batch_script_update;
use redis::{Client, Commands, Connection};
use slog::{debug, error, o, Drain, Logger};
use std::fmt::Display;
use uuid::Uuid;

use crate::{
    data_store::init_inmemory_storage,
    job_capnp::{script_batch_job, script_batch_result},
    prelude::World,
    profile,
};

use super::Executor;

pub const QUEEN_MUTEX: &str = "CAO_QUEEN_MUTEX";
pub const WORLD: &str = "CAO_WORLD";
pub const JOB_QUEUE: &str = "CAO_JOB_QUEUE";
pub const JOB_RESULTS_LIST: &str = "CAO_JOB_RESULTS_LIST";

pub const UPDATE_FENCE: &str = "CAO_UPDATE_FENCE";
pub const WORLD_TIME_FENCE: &str = "CAO_WORLD_TIME";

type BatchScriptInputMsg<'a> = script_batch_job::Reader<'a>;
type BatchScriptInputReader = TypedReader<capnp::serialize::OwnedSegments, script_batch_job::Owned>;
type ScriptBatchResultReader =
    TypedReader<capnp::serialize::OwnedSegments, script_batch_result::Owned>;

/// Multiprocess executor.
///
pub struct MpExecutor {
    pub logger: Logger,
    pub options: ExecutorOptions,

    client: Client,
    connection: Connection,
    role: Role,
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
    pub queen_mutex_expiry_ms: i64,
    pub script_chunk_size: usize,
    pub script_chunk_timeout_ms: i64,
}

impl Default for ExecutorOptions {
    fn default() -> Self {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379/0".to_owned());
        Self {
            redis_url,
            queen_mutex_expiry_ms: 2000,
            script_chunk_size: 1024,
            script_chunk_timeout_ms: 200,
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
}

impl MpExecutor {
    pub fn new(
        logger: impl Into<Option<Logger>>,
        options: impl Into<Option<ExecutorOptions>>,
    ) -> Result<Self, MpExcError> {
        fn _new(logger: Logger, options: ExecutorOptions) -> Result<MpExecutor, MpExcError> {
            let client =
                Client::open(options.redis_url.as_str()).map_err(MpExcError::RedisError)?;
            let queen_mutex = Utc.timestamp_millis(0);
            let connection = client.get_connection().map_err(MpExcError::RedisError)?;

            Ok(MpExecutor {
                logger,
                role: Role::Drone(Drone { queen_mutex }),
                client,
                connection,
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

        _new(logger, options.into().unwrap_or_default())
    }

    /// Check if this instance is the Queen and if so still holds the mutex.
    pub fn is_queen(&self) -> bool {
        match self.role {
            Role::Queen(Queen { queen_mutex }) => Utc::now() < queen_mutex,
            Role::Drone(_) => false,
        }
    }

    /// Returns the current role of this instance
    pub fn update_role(&mut self) -> Result<&Role, MpExcError> {
        debug!(self.logger, "Updating role of a {:?} process", self.role);
        let now = Utc::now();
        let new_expiry: i64 =
            (now + Duration::milliseconds(self.options.queen_mutex_expiry_ms)).timestamp_millis();

        self.role = match self.role {
            Role::Queen(p) => p.update_role(
                self.logger.clone(),
                &mut self.connection,
                new_expiry,
                self.options.queen_mutex_expiry_ms,
            )?,
            Role::Drone(d) => d.update_role(
                self.logger.clone(),
                &mut self.connection,
                now,
                new_expiry,
                self.options.queen_mutex_expiry_ms,
            )?,
        };
        Ok(&self.role)
    }

    /// Execute until the queue is empty
    fn execute_batch_script_jobs(&mut self, world: &mut World) -> Result<(), MpExcError> {
        while let Some(message) = self
            .connection
            .rpop::<_, Option<Vec<u8>>>(JOB_QUEUE)
            .map_err(MpExcError::RedisError)
            .and_then::<Option<BatchScriptInputReader>, _>(parse_script_batch)?
        {
            let message: BatchScriptInputMsg = message.get().map_err(|err| {
                error!(self.logger, "Failed to 'get' capnp message {:?}", err);
                MpExcError::MessageDeserializeError(err)
            })?;
            execute_batch_script_update(self, message, world)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct ScriptBatchStatus {
    id: Uuid,
    from: usize,
    to: usize,
    enqueued: DateTime<Utc>,
    started: Option<DateTime<Utc>>,
    finished: Option<DateTime<Utc>>,
}

impl ScriptBatchStatus {
    fn new(id: Uuid, from: usize, to: usize) -> Self {
        Self {
            id,
            from,
            to,
            enqueued: Utc::now(),
            started: None,
            finished: None,
        }
    }
}

impl Executor for MpExecutor {
    type Error = MpExcError;

    fn initialize(
        &mut self,
        logger: Option<slog::Logger>,
    ) -> Result<std::pin::Pin<Box<World>>, Self::Error> {
        if let Some(logger) = logger.as_ref() {
            self.logger = logger.clone();
        }
        self.update_role()?;
        if matches!(self.role, Role::Queen(_)) {
            let mut connection = self
                .client
                .get_connection()
                .map_err(MpExcError::RedisError)?;
            redis::pipe()
                .del(WORLD)
                .del(WORLD_TIME_FENCE)
                .del(JOB_QUEUE)
                .del(JOB_RESULTS_LIST)
                .query(&mut connection)
                .map_err(MpExcError::RedisError)?;
        }
        Ok(init_inmemory_storage(self.logger.clone()))
    }

    fn forward(&mut self, world: &mut World) -> Result<(), Self::Error> {
        profile!("world_forward");
        self.update_role()?;
        match self.role {
            Role::Queen(_) => queen::forward_queen(self, world)?,
            Role::Drone(_) => drone::forward_drone(self, world)?,
        }
        Ok(())
    }
}

fn parse_script_batch(
    message: Option<Vec<u8>>,
) -> Result<Option<BatchScriptInputReader>, MpExcError> {
    if let Some(message) = message {
        try_read_message(
            message.as_slice(),
            ReaderOptions {
                traversal_limit_in_words: 512,
                nesting_limit: 64,
            },
        )
        .map_err(MpExcError::MessageDeserializeError)
        .map(|reader| reader.map(|r| r.into_typed()))
    } else {
        Ok(None)
    }
}

fn parse_script_batch_result(
    message: Option<Vec<u8>>,
) -> Result<Option<ScriptBatchResultReader>, MpExcError> {
    if let Some(message) = message {
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
    } else {
        Ok(None)
    }
}
