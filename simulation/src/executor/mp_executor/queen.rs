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
    ScriptBatchStatus, JOB_QUEUE, JOB_RESULTS_LIST, QUEEN_MUTEX, UPDATE_FENCE, WORLD,
    WORLD_TIME_FENCE,
};

use arrayvec::ArrayVec;
use chrono::{DateTime, Duration, TimeZone, Utc};
use redis::{Commands, Connection};
use slog::{debug, error, info, o, trace, warn, Logger};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub struct Queen {
    /// Timestamp of the queen mutex
    pub queen_mutex: DateTime<Utc>,
}

impl Queen {
    pub fn update_role(
        mut self,
        logger: Logger,
        connection: &mut Connection,
        new_expiry: i64,
        mutex_expiry_ms: i64,
    ) -> Result<Role, MpExcError> {
        // add a bit of bias to let the current Queen re-aquire first
        let res: Option<Vec<String>> = redis::pipe()
            .getset(QUEEN_MUTEX, new_expiry)
            .expire(
                QUEEN_MUTEX,
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
            Some(res) if res != self.queen_mutex.timestamp_millis() => {
                // another process aquired the mutex
                info!(
                    logger,
                    "Another process has been promoted to Queen. Demoting this process to Drone"
                );
                Role::Drone(Drone {
                    queen_mutex: Utc.timestamp_millis(res),
                })
            }
            _ => {
                self.queen_mutex = Utc.timestamp_millis(new_expiry);
                debug!(
                    logger,
                    "Queen mutex has been re-aquired until {}", self.queen_mutex
                );
                Role::Queen(self)
            }
        };
        Ok(res)
    }
}

pub fn forward_queen(executor: &mut MpExecutor, world: &mut World) -> Result<(), MpExcError> {
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
        .set(WORLD, world_buff)
        .ignore()
        .query(&mut executor.connection)
        .map_err(MpExcError::RedisError)?;
    // make sure WORLD is set before setting fence...
    redis::pipe()
        .set(WORLD_TIME_FENCE, world.time())
        .ignore()
        .query(&mut executor.connection)
        .map_err(MpExcError::RedisError)?;

    let scripts_table = world.view::<EntityId, EntityScript>();
    let executions: Vec<(EntityId, EntityScript)> =
        scripts_table.iter().map(|(i, x)| (i, *x)).collect();
    // split the work (TODO how?)
    // for now let's split it into groups of `chunk_size`
    let chunk_size = executor.options.script_chunk_size;
    let mut message_status = executions
        .chunks(chunk_size)
        .enumerate()
        // skip the first chunk, let's execute it on this node
        .skip(1)
        // TODO: this could be done in parallel, however connection has to be mutably borrowed
        // We could open more connections, but that brings its own problems...
        // Maybe upgrade to a pool?
        .try_fold(HashMap::with_capacity(32), |mut ids, (i, chunk)| {
            let from = i * chunk_size;
            let to = from + chunk.len();

            let msg_id = Uuid::new_v4();
            let job = enqueue_job(&executor.logger, &mut executor.connection, msg_id, from, to)?;
            ids.insert(msg_id, job);
            Ok(ids)
        })?;

    redis::pipe()
        .set(UPDATE_FENCE, world.time())
        .ignore()
        .query(&mut executor.connection)
        .map_err(MpExcError::RedisError)?;

    debug!(executor.logger, "Executing the first chunk");
    let mut intents: Vec<BotIntents> = match executions.chunks(chunk_size).next() {
        Some(chunk) => execute_scripts(chunk, world),
        None => {
            warn!(executor.logger, "No scripts to execute");
            return post_script_update(executor, world, vec![]);
        }
    };
    debug!(executor.logger, "Executing the first chunk done");

    // wait for all messages to return
    'retry: loop {
        // execute jobs while the queue isn't empty
        executor.execute_batch_script_jobs(world)?;
        let new_role = executor.update_role()?;
        assert!(
            matches!(new_role, Role::Queen(_)),
            "Queen role has been lost while executing a tick"
        );

        trace!(executor.logger, "Checking jobs' status");
        while let Some(message) = executor
            .connection
            .rpop::<_, Option<Vec<u8>>>(JOB_RESULTS_LIST)
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
        let mut timeouts = ArrayVec::<[_; 3]>::new();
        'stati: for (msg_id, status) in message_status.iter_mut() {
            if status.finished.is_some() {
                count += 1;
                continue 'stati;
            }
            if let Some(start) = status.started {
                let timeout = executor.options.script_chunk_timeout_ms;
                if (Utc::now() - start) > Duration::milliseconds(timeout) {
                    warn!(executor.logger, "Job {} has timed out.", status.id);
                    if timeouts.try_push(*msg_id).is_err() {
                        warn!(executor.logger, "Requeueing {}", status.id);
                        *status = enqueue_job(
                            &executor.logger,
                            &mut executor.connection,
                            *msg_id,
                            status.from,
                            status.to,
                        )?;
                    }
                }
            }
        }
        for msg_id in timeouts {
            info!(executor.logger, "Executing timed out job {}", msg_id);
            let ScriptBatchStatus { from, to, .. } = message_status.remove(&msg_id).unwrap();
            let ints = execute_scripts(&executions[from..to], world);
            intents.extend_from_slice(ints.as_slice());
        }
        if count == message_status.len() {
            debug!(executor.logger, "All jobs have returned");
            break 'retry;
        }
    }
    post_script_update(executor, world, intents)
}

fn post_script_update(
    executor: &mut MpExecutor,
    world: &mut World,
    intents: Vec<BotIntents>,
) -> Result<(), MpExcError> {
    info!(executor.logger, "Got {} intents", intents.len());
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
        .lpush(JOB_QUEUE, payload.as_slice())
        .query(connection)
        .map_err(MpExcError::RedisError)?;
    Ok(ScriptBatchStatus::new(msg_id, from, to))
}
