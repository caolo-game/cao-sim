use std::collections::HashMap;
use std::convert::TryFrom;

use crate::{
    components::EntityScript,
    intents::{self, BotIntents},
    job_capnp::script_batch_job,
    prelude::{EntityId, World},
    systems::execute_world_update,
    systems::script_execution::execute_scripts,
};

use super::{
    drone::Drone, parse_script_batch_result, MpExcError, MpExecutor, Role, ScriptBatchResultReader,
    ScriptBatchStatus, CAO_JOB_QUEUE_KEY, CAO_JOB_RESULTS_LIST_KEY, CAO_PRIMARY_MUTEX_KEY,
    CAO_WORLD_KEY, CAO_WORLD_TIME_KEY, CHUNK_SIZE,
};

use arrayvec::ArrayVec;
use chrono::{DateTime, TimeZone, Utc};
use redis::{Commands, Connection};
use slog::{debug, error, info, o, trace, warn, Logger};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub struct Primary {
    /// Timestamp of the primary mutex
    pub primary_mutex: DateTime<Utc>,
}

impl Primary {
    pub fn update_role(
        mut self,
        logger: Logger,
        connection: &mut Connection,
        new_expiry: i64,
        mutex_expiry_ms: i64,
    ) -> Result<Role, MpExcError> {
        // add a bit of bias to let the current Primary re-aquire first
        let res: Option<Vec<String>> = redis::pipe()
            .getset(CAO_PRIMARY_MUTEX_KEY, new_expiry)
            .expire(
                CAO_PRIMARY_MUTEX_KEY,
                (mutex_expiry_ms / 1000) as usize + 1, // round up
            )
            .ignore()
            .query(connection)
            .map_err(MpExcError::RedisError)?;
        let res: Option<i64> = res
            .as_ref()
            .and_then(|s| s.get(0))
            .and_then(|s| s.parse().ok());
        let res = match res {
            Some(res) if res != self.primary_mutex.timestamp_millis() => {
                // another process aquired the mutex
                info!(
                    logger,
                    "Another process has been promoted to Primary. Demoting this process to Drone"
                );
                Role::Drone(Drone {
                    primary_mutex: Utc.timestamp_millis(res),
                })
            }
            _ => {
                self.primary_mutex = Utc.timestamp_millis(new_expiry);
                debug!(
                    logger,
                    "Primary mutex has been re-aquired until {}", self.primary_mutex
                );
                Role::Primary(self)
            }
        };
        Ok(res)
    }
}

pub fn forward_primary(executor: &mut MpExecutor, world: &mut World) -> Result<(), MpExcError> {
    executor.logger = world
        .logger
        .new(o!("tick" => world.time(), "role" => format!("{}", executor.role)));
    info!(executor.logger, "Tick starting");

    // broadcast world
    // TODO broadcast changesets instead of the whole state
    debug!(executor.logger, "Sending world state");
    let world_buff =
        rmp_serde::to_vec_named(&world.store).map_err(MpExcError::WorldSerializeError)?;
    redis::pipe()
        .set(CAO_WORLD_KEY, world_buff)
        .ignore()
        .query(&mut executor.connection)
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
            let job = enqueue_job(&executor.logger, &mut executor.connection, msg_id, from, to)?;
            ids.insert(msg_id, job);
            Ok(ids)
        })?;

    // send start signal to drones
    // this 'fence' should ensure that the drones read the correct world state
    // and that the queue is full at this point
    redis::pipe()
        .set(CAO_WORLD_TIME_KEY, world.time())
        .ignore()
        .query(&mut executor.connection)
        .map_err(MpExcError::RedisError)?;

    debug!(executor.logger, "Executing the first chunk");
    let mut intents: Vec<BotIntents> = match executions.chunks(CHUNK_SIZE).next() {
        Some(chunk) => execute_scripts(chunk, world),
        None => {
            warn!(executor.logger, "No scripts to execute");
            return Ok(());
        }
    };
    debug!(executor.logger, "Executing the first chunk done");

    // wait for all messages to return
    'retry: loop {
        // execute jobs while the queue isn't empty
        executor.execute_batch_script_jobs(world)?;
        let new_role = executor.update_role()?;
        assert!(
            matches!(new_role, Role::Primary(_)),
            "Primary role has been lost while executing a tick"
        );

        trace!(executor.logger, "Checking jobs' status");
        while let Some(message) = executor
            .connection
            .rpop::<_, Option<Vec<u8>>>(CAO_JOB_RESULTS_LIST_KEY)
            .map_err(MpExcError::RedisError)
            .and_then::<Option<ScriptBatchResultReader>, _>(|message| {
                parse_script_batch_result(message)
            })?
        {
            use crate::job_capnp::script_batch_result::payload::Which;
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
                        let bot_int =
                            rmp_serde::from_slice(msg).expect("Failed to deserialize BotIntents");
                        intents.push(bot_int);
                    }
                }
                _ => {
                    error!(executor.logger, "Failed to read variant");
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
            debug!(executor.logger, "All jobs have returned");
            break 'retry;
        }
    }
    // TODO
    // on timeout retry failed jobs
    //
    debug!(executor.logger, "Got {} intents", intents.len());
    intents::move_into_storage(world, intents);

    debug!(executor.logger, "Executing systems update");
    execute_world_update(world);

    debug!(executor.logger, "Executing post-processing");
    world.post_process();

    info!(executor.logger, "Tick done");

    Ok(())
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
