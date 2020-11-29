use caoq_client::MessageId;
use slog::debug;

use crate::prelude::World;
use crate::{
    components::EntityScript, job_capnp::script_batch_result, prelude::EntityId,
    systems::script_execution::execute_scripts,
};

use super::{BatchScriptInputMsg, MpExcError, MpExecutor};

pub async fn execute_batch_script_update(
    executor: &mut MpExecutor,
    msg_id: MessageId,
    message: BatchScriptInputMsg<'_>,
    world: &mut World,
) -> Result<(), MpExcError> {
    debug!(executor.logger, "Executing message with id {:?}", msg_id);

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
    root.reborrow().set_world_time(world.time());
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
        "Sending result of {:?}, {} bytes",
        msg_id,
        payload.len()
    );
    executor.queue.msg_response(msg_id, payload).await?;
    Ok(())
}
