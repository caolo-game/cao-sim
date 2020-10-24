//! Multiprocess executor.
//!

use arrayvec::ArrayVec;
use capnp::{message::ReaderOptions, message::TypedReader, serialize::try_read_message};
use chrono::{DateTime, Duration, TimeZone, Utc};
use redis::{Client, Commands, Connection};
use slog::{debug, error, info, o, trace, warn, Drain, Logger};
use std::{collections::HashMap, convert::TryFrom};
use uuid::Uuid;

use crate::{
    components::EntityScript,
    data_store::init_inmemory_storage,
    intents::{self, BotIntents},
    job_capnp::{self, script_batch_job, script_batch_result},
    prelude::EntityId,
    prelude::World,
    profile,
    systems::execute_world_update,
    systems::script_execution::execute_scripts,
};

use super::Executor;

pub const CAO_PRIMARY_MUTEX_KEY: &str = "CAO_PRIMARY_MUTEX";
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
    /// Timestamp of the primary mutex
    primary_mutex: DateTime<Utc>,

    mutex_expiry_ms: i64,
}

#[derive(Debug, Clone, Copy)]
pub enum Role {
    /// This is the main/coordinator instance
    Primary,
    /// This is a worker instance
    Drone,
}

#[derive(Debug)]
pub struct ExecutorOptions {
    pub redis_url: String,
    pub primary_mutex_expiry_ms: i64,
}

impl Default for ExecutorOptions {
    fn default() -> Self {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379/0".to_owned());
        Self {
            redis_url,
            primary_mutex_expiry_ms: 2000,
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
}

impl MpExecutor {
    pub fn new(
        logger: impl Into<Option<Logger>>,
        options: impl Into<Option<ExecutorOptions>>,
    ) -> Result<Self, MpExcError> {
        fn _new(logger: Logger, options: ExecutorOptions) -> Result<MpExecutor, MpExcError> {
            let client =
                Client::open(options.redis_url.as_str()).map_err(MpExcError::RedisError)?;
            let primary_mutex = Utc.timestamp_millis(0);
            let connection = client.get_connection().map_err(MpExcError::RedisError)?;

            Ok(MpExecutor {
                logger,
                role: Role::Drone,
                client,
                connection,
                primary_mutex,
                mutex_expiry_ms: options.primary_mutex_expiry_ms,
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

    /// Check if this instance is the Primary and if so still holds the mutex.
    pub fn is_primary(&self) -> bool {
        match self.role {
            Role::Primary => Utc::now() < self.primary_mutex,
            Role::Drone => false,
        }
    }

    /// Returns the current role of this instance
    pub fn update_role(&mut self) -> Result<Role, MpExcError> {
        debug!(self.logger, "Updating role of a {:?} process", self.role);
        let now = Utc::now();
        let new_expiry: i64 =
            (now + Duration::milliseconds(self.mutex_expiry_ms)).timestamp_millis();

        match self.role {
            Role::Primary => {
                let res: Option<Vec<String>> = redis::pipe()
                    .getset(CAO_PRIMARY_MUTEX_KEY, new_expiry)
                    .expire(
                        CAO_PRIMARY_MUTEX_KEY,
                        (self.mutex_expiry_ms / 1000) as usize + 1, // round up
                    )
                    .ignore()
                    .query(&mut self.connection)
                    .map_err(MpExcError::RedisError)?;
                let res: Option<i64> = res
                    .as_ref()
                    .and_then(|s| s.get(0))
                    .and_then(|s| s.parse().ok());
                match res {
                    Some(res) if res != self.primary_mutex.timestamp_millis() => {
                        // another process aquired the mutex
                        info!(self.logger, "Another process has been promoted to Primary. Demoting this process to Drone");
                        self.role = Role::Drone;
                        self.primary_mutex = Utc.timestamp_millis(res);
                    }
                    _ => {
                        self.primary_mutex = Utc.timestamp_millis(new_expiry);
                        debug!(
                            self.logger,
                            "Primary mutex has been re-aquired until {}", self.primary_mutex
                        );
                    }
                }
            }
            Role::Drone => {
                // add a bit of bias to let the current Primary re-aquire first
                let primary_expired =
                    now.timestamp_millis() >= (self.primary_mutex.timestamp_millis() + 50);
                if primary_expired {
                    debug!(
                        self.logger,
                        "Primary mutex has expired. Attempting to aquire"
                    );
                    let (success, res) = redis::pipe()
                        .cmd("SET")
                        .arg(CAO_PRIMARY_MUTEX_KEY)
                        .arg(new_expiry)
                        .arg("NX")
                        .arg("PX")
                        .arg(self.mutex_expiry_ms)
                        .get(CAO_PRIMARY_MUTEX_KEY)
                        .query(&mut self.connection)
                        .map_err(MpExcError::RedisError)?;
                    if success {
                        info!(
                            self.logger,
                            "Aquired Primary mutex. Promoting this process to Primary"
                        );
                        self.role = Role::Primary;
                    } else {
                        debug!(self.logger, "Another process aquired the mutex.");
                    }
                    self.primary_mutex = Utc.timestamp_millis(res);
                }
            }
        }
        Ok(self.role)
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
                    if matches!(self.update_role()?, Role::Primary) {
                        info!(
                            self.logger,
                            "Assumed role of Primary while waiting for world update. Last world state in this executor: tick {}",
                            world.time()
                        );
                        return self.forward_primary(world);
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
            .map_err(|e| MpExcError::RedisError(e))
            .and_then::<Option<BatchScriptInputReader>, _>(|message| parse_script_batch(message))?
        {
            let message: BatchScriptInputMsg = message.get().map_err(|err| {
                error!(self.logger, "Failed to 'get' capnp message {:?}", err);
                MpExcError::MessageDeserializeError(err)
            })?;
            self.execute_batch_script_update(message, world)?;
        }
        Ok(())
    }

    fn forward_primary(&mut self, world: &mut World) -> Result<(), MpExcError> {
        self.logger = world
            .logger
            .new(o!("tick" => world.time(), "role" => format!("{:?}", self.role)));
        info!(self.logger, "Tick starting");
        let mut connection = self
            .client
            .get_connection()
            .map_err(MpExcError::RedisError)?;

        // broadcast world
        // TODO broadcast changesets instead of the whole state
        debug!(self.logger, "Sending world state");
        let world_buff =
            rmp_serde::to_vec_named(&world.store).map_err(MpExcError::WorldSerializeError)?;
        redis::pipe()
            .set(CAO_WORLD_KEY, world_buff)
            .ignore()
            .query(&mut connection)
            .map_err(MpExcError::RedisError)?;

        let scripts_table = world.view::<EntityId, EntityScript>();
        let executions: Vec<(EntityId, EntityScript)> =
            scripts_table.iter().map(|(i, x)| (i, *x)).collect();
        // split the work (TODO how?)
        // for now let's split it into groups of CHUNK_SIZE
        let mut message_status = executions
            .chunks(CHUNK_SIZE)
            .enumerate()
            // skip the first chunk, let's execute it on this node
            .skip(1)
            // TODO: this could be done in parallel, however connection has to be mutably borrowed
            // We could open more connections, but that brings its own probelms...
            // Maybe upgrade to a pool?
            .try_fold(HashMap::with_capacity(32), |mut ids, (i, chunk)| {
                let from = i * CHUNK_SIZE;
                let to = from + chunk.len();

                let msg_id = Uuid::new_v4();
                let job = enqueue_job(&self.logger, &mut self.connection, msg_id, from, to)?;
                ids.insert(msg_id, job);
                Ok(ids)
            })?;

        // send start signal to drones
        // this 'fence' should ensure that the drones read the correct world state
        // and that the queue is full at this point
        redis::pipe()
            .set(CAO_WORLD_TIME_KEY, world.time())
            .ignore()
            .query(&mut connection)
            .map_err(MpExcError::RedisError)?;

        debug!(self.logger, "Executing the first chunk");
        let mut intents: Vec<BotIntents> = match executions.chunks(CHUNK_SIZE).next() {
            Some(chunk) => execute_scripts(chunk, world),
            None => {
                warn!(self.logger, "No scripts to execute");
                return Ok(());
            }
        };
        debug!(self.logger, "Executing the first chunk done");

        // wait for all messages to return
        'retry: loop {
            // execute jobs while the queue isn't empty
            self.execute_batch_script_jobs(world)?;
            let new_role = self.update_role()?;
            assert!(
                matches!(new_role, Role::Primary),
                "Primary role has been lost while executing a tick"
            );

            trace!(self.logger, "Checking jobs' status");
            while let Some(message) = connection
                .rpop::<_, Option<Vec<u8>>>(CAO_JOB_RESULTS_LIST_KEY)
                .map_err(|e| MpExcError::RedisError(e))
                .and_then::<Option<ScriptBatchResultReader>, _>(|message| {
                    parse_script_batch_result(message)
                })?
            {
                use job_capnp::script_batch_result::payload::Which;
                let message = message.get().map_err(MpExcError::MessageDeserializeError)?;
                let msg_id = message
                    .get_msg_id()
                    .map_err(MpExcError::MessageDeserializeError)?
                    .get_data()
                    .map_err(MpExcError::MessageDeserializeError)?;
                let msg_id = Uuid::from_slice(msg_id).expect("Failed to parse msg id");
                let status = message_status
                    .entry(msg_id)
                    .or_insert_with(|| ScriptBatchStatus::new(msg_id, 0, executions.len()));
                match message
                    .get_payload()
                    .which()
                    .expect("Failed to get payload variant")
                {
                    Which::StartTime(Ok(time)) => {
                        status.started = Some(Utc.timestamp_millis(time.get_value_ms()))
                    }
                    Which::Intents(Ok(ints)) => {
                        status.finished = Some(Utc::now());
                        for int in ints {
                            let msg = int.get_payload().expect("Failed to read payload");
                            let bot_int = rmp_serde::from_slice(msg)
                                .expect("Failed to deserialize BotIntents");
                            intents.push(bot_int);
                        }
                    }
                    _ => {
                        error!(self.logger, "Failed to read variant");
                    }
                }
            }
            let mut count = 0;
            'stati: for (_, status) in message_status.iter() {
                if status.finished.is_some() {
                    count += 1;
                    continue 'stati;
                }
            }
            if count == message_status.len() {
                debug!(self.logger, "All jobs have returned");
                break 'retry;
            }
        }
        // TODO
        // on timeout retry failed jobs
        //
        debug!(self.logger, "Got {} intents", intents.len());
        intents::move_into_storage(world, intents);

        debug!(self.logger, "Executing systems update");
        execute_world_update(world);

        debug!(self.logger, "Executing post-processing");
        world.post_process();

        info!(self.logger, "Tick done");

        Ok(())
    }
}

fn enqueue_job(
    logger: &Logger,
    connection: &mut Connection,
    msg_id: Uuid,
    from: usize,
    to: usize,
) -> Result<ScriptBatchStatus, MpExcError> {
    let mut msg = capnp::message::Builder::new_default();
    let mut root = msg.init_root::<script_batch_job::Builder>();

    let mut id_msg = root.reborrow().init_msg_id();
    id_msg.set_data(msg_id.as_bytes());
    root.reborrow()
        .set_from_index(u32::try_from(from).expect("Expected index to be convertible to u32"));
    root.reborrow()
        .set_to_index(u32::try_from(to).expect("Expected index to be convertible to u32"));

    let mut payload = ArrayVec::<[u8; 64]>::new();
    capnp::serialize::write_message(&mut payload, &msg)
        .map_err(MpExcError::MessageSerializeError)?;
    debug!(
        logger,
        "pushing job: msg_id: {}, from: {}, to: {}; size: {}",
        msg_id,
        from,
        to,
        payload.len()
    );
    redis::pipe()
        .lpush(CAO_JOB_QUEUE_KEY, payload.as_slice())
        .query(connection)
        .map_err(MpExcError::RedisError)?;
    Ok(ScriptBatchStatus::new(msg_id, from, to))
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
        if matches!(self.role, Role::Primary) {
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
            Role::Primary => self.forward_primary(world)?,
            Role::Drone => self.forward_drone(world)?,
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
        .map_err(|err| MpExcError::MessageDeserializeError(err))
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
        .map_err(|err| MpExcError::MessageDeserializeError(err))
        .map(|reader| reader.map(|r| r.into_typed()))
    } else {
        Ok(None)
    }
}
