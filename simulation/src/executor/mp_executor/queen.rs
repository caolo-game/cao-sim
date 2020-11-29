use caoq_client::MessageId;
use std::collections::HashMap;
use std::convert::TryFrom;

use crate::{
    components::EntityScript,
    executor::execute_map_generation,
    executor::GameConfig,
    intents::{self, BotIntents},
    job_capnp::script_batch_job,
    prelude::{EntityId, World},
    systems::execute_world_update,
    systems::script_execution::execute_scripts,
};

use super::{
    parse_script_batch_result,
    world_state::WorldIoOptionFlags,
    world_state::{self, send_world},
    MpExcError, MpExecutor, ScriptBatchStatus, JOB_QUEUE,
};

use arrayvec::ArrayVec;
use chrono::Utc;
use slog::{debug, error, info, o, trace, warn, Logger};

pub async fn forward_queen(executor: &mut MpExecutor, world: &mut World) -> Result<(), MpExcError> {
    let current_world_time = world.time();
    executor.logger = world.logger.new(o!(
                "tag" => executor.tag.to_string(),
                "tick" => current_world_time,
                "role" => format!("{}", executor.role)));
    info!(executor.logger, "Tick starting");

    debug!(executor.logger, "Initializing queue");

    let scripts_table = world.view::<EntityId, EntityScript>();

    executor
        .queue
        .active_queue(
            caoq_client::Role::ProdCon,
            JOB_QUEUE.to_owned(),
            Some(caoq_client::QueueOptions {
                capacity: scripts_table.count_set() as u32,
            }),
        )
        .await?;

    send_world(executor, world, WorldIoOptionFlags::new()).await?;

    let executions: Vec<(EntityId, EntityScript)> =
        scripts_table.iter().map(|(i, x)| (i, *x)).collect();
    // split the work (TODO how?)
    // for now let's split it into groups of `chunk_size`
    let chunk_size = executor.options.script_chunk_size;
    let logger = executor.logger.clone();
    let mut message_status = HashMap::new();
    for (i, chunk) in executions
        .chunks(chunk_size)
        .enumerate()
        // skip the first chunk, let's execute it on this node
        .skip(1)
    {
        let from = i * chunk_size;
        let to = from + chunk.len();

        let msg = build_job_msg(from, to, current_world_time);
        let job = enqueue_job(
            &logger,
            &mut executor.queue,
            msg,
            from,
            to,
            current_world_time,
        )
        .await?;
        let msg_id: MessageId = job.id;
        message_status.insert(msg_id, job);
    }

    debug!(logger, "Executing the first chunk");
    let mut intents: Vec<BotIntents> = match executions.chunks(chunk_size).next() {
        Some(chunk) => execute_scripts(chunk, world),
        None => {
            warn!(logger, "No scripts to execute");
            return post_script_update(executor, world, vec![]);
        }
    };
    debug!(logger, "Executing the first chunk done");

    // wait for all messages to return
    'retry: loop {
        // execute jobs while the queue isn't empty
        executor.execute_batch_script_jobs(world).await?;

        trace!(logger, "Checking jobs' status");
        while let Some(delivery) = executor.queue.pop_msg().await? {
            let msg_id = delivery.id;
            let message = parse_script_batch_result(delivery.payload)?.unwrap(); // FIXME
            let message = message.get().map_err(MpExcError::MessageDeserializeError)?;

            let msg_time = message.get_world_time();
            if msg_time != world.time() {
                error!(
                    logger,
                    "Got an intent msg {:?} with invalid timestamp. Expected: {} Actual: {}",
                    msg_id,
                    world.time(),
                    msg_time
                );
                continue;
            }

            let status = message_status
                .entry(msg_id)
                .or_insert_with(|| ScriptBatchStatus::new(msg_id, 0, executions.len()));
            match message.get_intents() {
                Ok(ints) => {
                    status.finished = Some(Utc::now());
                    for int in ints {
                        let msg = int.get_payload().expect("Failed to read payload");
                        let bot_int = match rmp_serde::from_read(msg) {
                            Ok(ints) => ints,
                            Err(err) => {
                                error!(
                                    logger,
                                    "Failed to deserialize intents of message {:?} {:?}. Discarding",
                                    msg_id,
                                    err
                                );
                                continue;
                            }
                        };
                        intents.push(bot_int);
                    }
                }
                Err(err) => {
                    error!(logger, "Failed to read intents {:?}", err);
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
                warn!(logger, "Job {:?} has timed out.", status.id);
                if timeouts.try_push(*msg_id).is_err() {
                    break;
                }
            }
        }
        for msg_id in timeouts {
            info!(logger, "Executing timed out job {:?}", msg_id);
            let ScriptBatchStatus { from, to, .. } = message_status.remove(&msg_id).unwrap();
            let ints = execute_scripts(&executions[from..to], world);
            intents.extend_from_slice(ints.as_slice());
        }
        if count == message_status.len() {
            debug!(logger, "All jobs have returned");
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
    from: usize,
    to: usize,
    time: u64,
) -> capnp::message::Builder<capnp::message::HeapAllocator> {
    let mut msg = capnp::message::Builder::new_default();
    let mut root = msg.init_root::<script_batch_job::Builder>();

    root.reborrow()
        .set_from_index(u32::try_from(from).expect("Expected index to be convertible to u32"));
    root.reborrow()
        .set_to_index(u32::try_from(to).expect("Expected index to be convertible to u32"));
    root.reborrow().set_world_time(time);
    msg
}

async fn enqueue_job(
    logger: &Logger,
    client: &mut caoq_client::Client,
    msg: capnp::message::Builder<capnp::message::HeapAllocator>,
    from: usize,
    to: usize,
    time: u64,
) -> Result<ScriptBatchStatus, MpExcError> {
    let mut payload = Vec::with_capacity(64);
    capnp::serialize::write_message(&mut payload, &msg)
        .map_err(MpExcError::MessageSerializeError)?;
    debug!(
        logger,
        "pushing job: from: {}, to: {} time: {}; size: {}",
        from,
        to,
        time,
        payload.len()
    );

    let msg_id = client.push_msg(payload).await?;
    Ok(ScriptBatchStatus::new(msg_id, from, to))
}

pub async fn initialize_queen(
    executor: &mut MpExecutor,
    world: &mut World,
    config: &GameConfig,
) -> Result<(), MpExcError> {
    // flush the current messages in the job queue if any
    // those are left-overs from previous executors
    info!(executor.logger, "Purging job queues");
    executor
        .queue
        .active_queue(caoq_client::Role::Producer, JOB_QUEUE.to_owned(), None)
        .await
        .unwrap_or(());
    executor.queue.clear_queue().await.unwrap_or_else(|err| {
        warn!(executor.logger, "Failed to purge {}: {:?}", err, JOB_QUEUE);
    });

    info!(executor.logger, "Generating map");
    execute_map_generation(executor.logger.clone(), &mut *world, &config)
        .expect("Failed to generate world map");

    info!(executor.logger, "Sending world state to Drones");
    let opts = WorldIoOptionFlags::new().all();
    world_state::send_world(executor, world, opts)
        .await
        .expect("Failed to send initial world");

    Ok(())
}
