use rayon::prelude::*;
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
    drone::Drone, parse_script_batch_result, world_state::send_world,
    world_state::WorldIoOptionFlags, MpExcError, MpExecutor, Role, ScriptBatchStatus, JOB_QUEUE,
    JOB_RESULTS_LIST, QUEEN_MUTEX,
};

use arrayvec::ArrayVec;
use chrono::{DateTime, TimeZone, Utc};
use lapin::{
    options::BasicGetOptions, options::BasicPublishOptions, options::QueueDeclareOptions,
    options::QueuePurgeOptions, types::FieldTable, BasicProperties,
};
use slog::{debug, error, info, o, trace, warn, Logger};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub struct Queen {
    /// Timestamp of the queen mutex
    pub queen_mutex: DateTime<Utc>,
}

impl Queen {
    pub async fn update_role(
        mut self,
        logger: Logger,
        connection: &mut redis::aio::Connection,
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
            .query_async(connection)
            .await
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

pub async fn forward_queen(executor: &mut MpExecutor, world: &mut World) -> Result<(), MpExcError> {
    let current_world_time = world.time();
    executor.logger = world
        .logger
        .new(o!("tick" => current_world_time, "role" => format!("{}", executor.role)));
    info!(executor.logger, "Tick starting");

    debug!(executor.logger, "Initializing amqp.amqp_chans");
    // TODO:
    // pls do these in parallel
    let _job_q = executor
        .amqp_chan
        .queue_declare(
            JOB_QUEUE,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .map_err(MpExcError::AmqpError)?;
    let _result_q = executor
        .amqp_chan
        .queue_declare(
            JOB_RESULTS_LIST,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .map_err(MpExcError::AmqpError)?;

    // flush the current messages if any
    // those are left-overs from previous executors
    executor
        .amqp_chan
        .queue_purge(JOB_QUEUE, QueuePurgeOptions { nowait: true })
        .await
        .map_err(MpExcError::AmqpError)?;
    executor
        .amqp_chan
        .queue_purge(JOB_RESULTS_LIST, QueuePurgeOptions { nowait: true })
        .await
        .map_err(MpExcError::AmqpError)?;

    send_world(executor, world, WorldIoOptionFlags::new()).await?;

    let scripts_table = world.view::<EntityId, EntityScript>();
    let executions: Vec<(EntityId, EntityScript)> =
        scripts_table.iter().map(|(i, x)| (i, *x)).collect();
    // split the work (TODO how?)
    // for now let's split it into groups of `chunk_size`
    let chunk_size = executor.options.script_chunk_size;
    let _guard = executor.runtime.tokio_rt.enter();
    let mut message_status: HashMap<_, _> = executions
        .par_chunks(chunk_size)
        .enumerate()
        // skip the first chunk, let's execute it on this node
        .skip(1)
        .map(|(i, chunk)| {
            let from = i * chunk_size;
            let to = from + chunk.len();

            let msg_id = Uuid::new_v4();

            // SAFETY
            // we'll await these tasks in the 'fold' step so this should be fine
            // the compiler can't tell that `executor` lives long enough, so we'll give it a hint
            let executor: &'static MpExecutor = unsafe { std::mem::transmute(&*executor) };
            let msg = build_job_msg(msg_id, from, to, current_world_time);
            let job = executor.runtime.tokio_rt.spawn(enqueue_job(
                &executor.logger,
                &executor.amqp_chan,
                msg,
                msg_id,
                from,
                to,
                current_world_time,
            ));
            (msg_id, job)
        })
        .try_fold(
            || HashMap::with_capacity(executions.len() / chunk_size + 1),
            |mut message_status, (msg_id, job)| {
                let job = executor
                    .runtime
                    .block_on(job)
                    .expect("Failed to join tokio task")?;
                message_status.insert(msg_id, job);
                Ok(message_status)
            },
        )
        .try_reduce(HashMap::new, |a, mut b| {
            b.extend(a);
            Ok(b)
        })?;

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
        executor.execute_batch_script_jobs(world).await?;
        let new_role = executor.update_role().await?;
        assert!(
            matches!(new_role, Role::Queen(_)),
            "Queen role has been lost while executing a tick"
        );

        trace!(executor.logger, "Checking jobs' status");
        while let Some(message) = executor
            .amqp_chan
            .basic_get(JOB_RESULTS_LIST, BasicGetOptions { no_ack: true })
            .await
            .map_err(MpExcError::AmqpError)?
        {
            let delivery = message.delivery;

            let message = parse_script_batch_result(delivery.data)?.unwrap(); // FIXME
            let message = message.get().map_err(MpExcError::MessageDeserializeError)?;
            let msg_id = message
                .get_msg_id()
                .map_err(MpExcError::MessageDeserializeError)?;

            let msg_id = uuid::Uuid::from_fields(
                msg_id.get_d1(),
                msg_id.get_d2(),
                msg_id.get_d3(),
                unsafe { &*(&msg_id.get_d4() as *const u64 as *const [u8; 8]) },
            )
            .expect("Failed to parse msgid");
            let status = message_status
                .entry(msg_id)
                .or_insert_with(|| ScriptBatchStatus::new(msg_id, 0, executions.len()));
            match message.get_intents() {
                Ok(ints) => {
                    status.finished = Some(Utc::now());
                    for int in ints {
                        let msg = int.get_payload().expect("Failed to read payload");
                        let bot_int =
                            rmp_serde::from_slice(msg).expect("Failed to deserialize BotIntents");
                        intents.push(bot_int);
                    }
                }
                Err(err) => {
                    error!(executor.logger, "Failed to read intents {:?}", err);
                    continue;
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
            let timeout = executor.options.expected_frequency;
            if (Utc::now() - status.enqueued) > timeout {
                warn!(executor.logger, "Job {} has timed out.", status.id);
                if timeouts.try_push(*msg_id).is_err() {
                    info!(executor.logger, "Requeueing {}", status.id);
                    let msg = build_job_msg(*msg_id, status.from, status.to, current_world_time);
                    *status = enqueue_job(
                        &executor.logger,
                        &executor.amqp_chan,
                        msg,
                        *msg_id,
                        status.from,
                        status.to,
                        current_world_time,
                    )
                    .await?;
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

fn build_job_msg(
    msg_id: Uuid,
    from: usize,
    to: usize,
    time: u64,
) -> capnp::message::Builder<capnp::message::HeapAllocator> {
    let mut msg = capnp::message::Builder::new_default();
    let mut root = msg.init_root::<script_batch_job::Builder>();

    let mut id_msg = root.reborrow().init_msg_id();
    let (d1, d2, d3, d4) = msg_id.as_fields();
    id_msg.set_d1(d1);
    id_msg.set_d2(d2);
    id_msg.set_d3(d3);
    id_msg.set_d4(unsafe { *(d4 as *const [u8; 8] as *const u64) });
    root.reborrow()
        .set_from_index(u32::try_from(from).expect("Expected index to be convertible to u32"));
    root.reborrow()
        .set_to_index(u32::try_from(to).expect("Expected index to be convertible to u32"));
    root.reborrow().set_world_time(time);
    msg
}

async fn enqueue_job(
    logger: &Logger,
    channel: &lapin::Channel,
    msg: capnp::message::Builder<capnp::message::HeapAllocator>,
    msg_id: Uuid,
    from: usize,
    to: usize,
    time: u64,
) -> Result<ScriptBatchStatus, MpExcError> {
    let mut payload = Vec::with_capacity(64);
    capnp::serialize::write_message(&mut payload, &msg)
        .map_err(MpExcError::MessageSerializeError)?;
    debug!(
        logger,
        "pushing job: msg_id: {}, from: {}, to: {} time: {}; size: {}",
        msg_id,
        from,
        to,
        time,
        payload.len()
    );

    channel
        .basic_publish(
            "",
            JOB_QUEUE,
            BasicPublishOptions::default(),
            payload,
            BasicProperties::default(),
        )
        .await
        .map_err(MpExcError::AmqpError)?;
    Ok(ScriptBatchStatus::new(msg_id, from, to))
}
