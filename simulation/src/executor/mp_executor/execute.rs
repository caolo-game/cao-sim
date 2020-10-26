use chrono::Utc;
use redis::Commands;
use slog::{debug, info};

use crate::prelude::World;
use crate::{
    components::EntityScript, job_capnp::script_batch_result, prelude::EntityId,
    systems::script_execution::execute_scripts,
};

use super::{BatchScriptInputMsg, MpExcError, MpExecutor, CAO_JOB_RESULTS_LIST_KEY};

pub fn execute_batch_script_update(
    executor: &mut MpExecutor,
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
    info!(executor.logger, "Got message with id {:?}", msg_id);

    debug!(executor.logger, "Signaling start time");
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
    executor
        .connection
        .lpush(CAO_JOB_RESULTS_LIST_KEY, payload.as_slice())
        .map_err(MpExcError::RedisError)?;

    let scripts_table = world.view::<EntityId, EntityScript>();
    let executions: Vec<(EntityId, EntityScript)> =
        scripts_table.iter().map(|(id, x)| (id, *x)).collect();
    let from = message.get_from_index() as usize;
    let to = message.get_to_index() as usize;
    let executions = &executions[from..to];

    debug!(executor.logger, "Executing scripts");
    let intents = execute_scripts(executions, world);
    debug!(executor.logger, "Executing scripts done");

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

    debug!(executor.logger, "Sending result of message {}", msg_id);
    payload.clear();
    capnp::serialize::write_message(&mut payload, &msg)
        .map_err(MpExcError::MessageSerializeError)?;
    executor
        .connection
        .lpush(CAO_JOB_RESULTS_LIST_KEY, payload.as_slice())
        .map_err(MpExcError::RedisError)?;
    Ok(())
}
