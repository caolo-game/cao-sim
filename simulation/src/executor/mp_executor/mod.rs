//! Multiprocess executor.
//!

mod drone;
mod queen;

use self::drone::*;
use self::queen::*;

use capnp::{message::ReaderOptions, message::TypedReader, serialize::try_read_message};
use chrono::{DateTime, Duration, TimeZone, Utc};
use redis::{Client, Commands, Connection};
use slog::{debug, error, info, o, trace, Drain, Logger};
use std::fmt::Display;
use uuid::Uuid;

use crate::{
    components::EntityScript,
    data_store::init_inmemory_storage,
    job_capnp::{script_batch_job, script_batch_result},
    prelude::EntityId,
    prelude::World,
    profile,
    systems::script_execution::execute_scripts,
};

use super::Executor;

pub const CAO_QUEEN_MUTEX_KEY: &str = "CAO_QUEEN_MUTEX";
pub const CAO_WORLD_KEY: &str = "CAO_WORLD";
pub const CAO_WORLD_TIME_KEY: &str = "CAO_WORLD_TIME";
pub const CAO_JOB_QUEUE_KEY: &str = "CAO_JOB_QUEUE";
pub const CAO_JOB_RESULTS_LIST_KEY: &str = "CAO_JOB_RESULTS_LIST";
pub const CHUNK_SIZE: usize = 256;

type BatchScriptInputMsg<'a> = script_batch_job::Reader<'a>;
type BatchScriptInputReader = TypedReader<capnp::serialize::OwnedSegments, script_batch_job::Owned>;
type ScriptBatchResultReader =
    TypedReader<capnp::serialize::OwnedSegments, script_batch_result::Owned>;

/// Multiprocess executor.
///
pub struct MpExecutor {
    pub logger: Logger,

    client: Client,
    connection: Connection,
    role: Role,

    mutex_expiry_ms: i64,
}

#[derive(Debug, Clone)]
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
}

impl Default for ExecutorOptions {
    fn default() -> Self {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379/0".to_owned());
        Self {
            redis_url,
            queen_mutex_expiry_ms: 2000,
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
                mutex_expiry_ms: options.queen_mutex_expiry_ms,
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
            (now + Duration::milliseconds(self.mutex_expiry_ms)).timestamp_millis();

        self.role = match self.role {
            Role::Queen(p) => p.update_role(
                self.logger.clone(),
                &mut self.connection,
                new_expiry,
                self.mutex_expiry_ms,
            )?,
            Role::Drone(d) => d.update_role(
                self.logger.clone(),
                &mut self.connection,
                now,
                new_expiry,
                self.mutex_expiry_ms,
            )?,
        };
        Ok(&self.role)
    }

    fn execute_batch_script_update(
        &mut self,
        message: BatchScriptInputMsg,
        world: &mut World,
    ) -> Result<(), MpExcError> {
        let msg_id_msg = message
            .get_msg_id()
            .map_err(MpExcError::MessageDeserializeError)?;
        let msg_id = uuid::Uuid::from_slice(
            msg_id_msg
                .get_data()
                .map_err(MpExcError::MessageDeserializeError)?,
        )
        .expect("Failed to deserialize msg id");
        info!(self.logger, "Got message with id {:?}", msg_id);

        debug!(self.logger, "Signaling start time");
        let mut msg = capnp::message::Builder::new_default();
        let mut root = msg.init_root::<script_batch_result::Builder>();

        root.reborrow()
            .set_msg_id(msg_id_msg)
            .map_err(MpExcError::MessageSerializeError)?;
        let mut start_time = root.reborrow().init_payload().init_start_time();
        start_time.set_value_ms(Utc::now().timestamp_millis());

        let mut payload = Vec::with_capacity(1_000_000);
        capnp::serialize::write_message(&mut payload, &msg)
            .map_err(MpExcError::MessageSerializeError)?;
        self.connection
            .lpush(CAO_JOB_RESULTS_LIST_KEY, payload.as_slice())
            .map_err(MpExcError::RedisError)?;

        let scripts_table = world.view::<EntityId, EntityScript>();
        let executions: Vec<(EntityId, EntityScript)> =
            scripts_table.iter().map(|(id, x)| (id, *x)).collect();
        let from = message.get_from_index() as usize;
        let to = message.get_to_index() as usize;
        let executions = &executions[from..to];

        debug!(self.logger, "Executing scripts");
        let intents = execute_scripts(executions, world);
        debug!(self.logger, "Executing scripts done");

        let mut root = msg.init_root::<script_batch_result::Builder>();
        root.reborrow()
            .set_msg_id(msg_id_msg)
            .map_err(MpExcError::MessageSerializeError)?;
        let mut intents_msg = root
            .reborrow()
            .init_payload()
            .init_intents(intents.len() as u32);
        for (i, intent) in intents.into_iter().enumerate() {
            let mut intent_msg = intents_msg.reborrow().get(i as u32);
            intent_msg.reborrow().set_entity_id(intent.entity_id.0);
            intent_msg.set_payload(
                rmp_serde::to_vec_named(&intent)
                    .expect("Failed to serialize intents")
                    .as_slice(),
            );
        }

        debug!(self.logger, "Sending result of message {}", msg_id);
        payload.clear();
        capnp::serialize::write_message(&mut payload, &msg)
            .map_err(MpExcError::MessageSerializeError)?;
        self.connection
            .lpush(CAO_JOB_RESULTS_LIST_KEY, payload.as_slice())
            .map_err(MpExcError::RedisError)?;
        Ok(())
    }

    fn forward_drone(&mut self, world: &mut World) -> Result<(), MpExcError> {
        // wait for the updated world
        loop {
            match self
                .connection
                .get::<_, Option<u64>>(CAO_WORLD_TIME_KEY)
                .map_err(MpExcError::RedisError)?
            {
                Some(t) if t > world.time() => break,
                _ => {
                    trace!(self.logger, "World has not been updated. Waiting...");
                    if matches!(self.update_role()?, Role::Queen(_)) {
                        info!(
                            self.logger,
                            "Assumed role of Queen while waiting for world update. Last world state in this executor: tick {}",
                            world.time()
                        );
                        return queen::forward_queen(self, world);
                    }
                }
            }
        }

        // update world
        let store: Vec<Vec<u8>> = redis::pipe()
            .get(CAO_WORLD_KEY)
            .query(&mut self.connection)
            .map_err(MpExcError::RedisError)?;
        let store: crate::data_store::Storage =
            rmp_serde::from_slice(&store[0][..]).map_err(MpExcError::WorldDeserializeError)?;
        world.store = store;
        self.logger = world
            .logger
            .new(o!("tick" => world.time(), "role" => format!("{:?}", self.role)));
        info!(self.logger, "Tick starting");

        // execute jobs
        self.execute_batch_script_jobs(world)
    }

    /// Execute until the queue is empty
    fn execute_batch_script_jobs(&mut self, world: &mut World) -> Result<(), MpExcError> {
        while let Some(message) = self
            .connection
            .rpop::<_, Option<Vec<u8>>>(CAO_JOB_QUEUE_KEY)
            .map_err(MpExcError::RedisError)
            .and_then::<Option<BatchScriptInputReader>, _>(parse_script_batch)?
        {
            let message: BatchScriptInputMsg = message.get().map_err(|err| {
                error!(self.logger, "Failed to 'get' capnp message {:?}", err);
                MpExcError::MessageDeserializeError(err)
            })?;
            self.execute_batch_script_update(message, world)?;
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
                .del(CAO_WORLD_KEY)
                .del(CAO_WORLD_TIME_KEY)
                .del(CAO_JOB_QUEUE_KEY)
                .del(CAO_JOB_RESULTS_LIST_KEY)
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
            Role::Drone(_) => self.forward_drone(world)?,
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
