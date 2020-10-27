use lapin::{options::BasicPublishOptions, BasicProperties};
use slog::debug;

use crate::prelude::World;
use crate::{
    components::EntityScript, job_capnp::script_batch_result, prelude::EntityId,
    systems::script_execution::execute_scripts,
};

use super::{BatchScriptInputMsg, MpExcError, MpExecutor, JOB_RESULTS_LIST};

pub async fn execute_batch_script_update<'a>(
    executor: &mut MpExecutor,
    message: BatchScriptInputMsg<'a>,
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
    debug!(executor.logger, "Got message with id {:?}", msg_id);

    let scripts_table = world.view::<EntityId, EntityScript>();
    let executions: Vec<(EntityId, EntityScript)> =
        scripts_table.iter().map(|(id, x)| (id, *x)).collect();
    let from = message.get_from_index() as usize;
    let to = message.get_to_index() as usize;
    let executions = &executions[from..to];

    debug!(executor.logger, "Executing scripts");
    let intents = execute_scripts(executions, world);
    debug!(executor.logger, "Executing scripts done");

    let mut msg = capnp::message::Builder::new_default();
    let mut root = msg.init_root::<script_batch_result::Builder>();
    root.reborrow()
        .set_msg_id(msg_id_msg)
        .map_err(MpExcError::MessageSerializeError)?;
    let mut intents_msg = root.reborrow().init_intents(intents.len() as u32);
    for (i, intent) in intents.into_iter().enumerate() {
        let mut intent_msg = intents_msg.reborrow().get(i as u32);
        intent_msg.reborrow().set_entity_id(intent.entity_id.0);
        intent_msg.set_payload(
            rmp_serde::to_vec_named(&intent)
                .expect("Failed to serialize intents")
                .as_slice(),
        );
    }

    let mut payload = Vec::with_capacity(1_000_000);
    capnp::serialize::write_message(&mut payload, &msg)
        .map_err(MpExcError::MessageSerializeError)?;
    debug!(
        executor.logger,
        "Sending result of message {}, {} bytes",
        msg_id,
        payload.len()
    );
    executor
        .amqp_chan
        .basic_publish(
            "",
            JOB_RESULTS_LIST,
            BasicPublishOptions::default(),
            payload,
            BasicProperties::default(),
        )
        .await
        .map_err(MpExcError::AmqpError)?;
    Ok(())
}
