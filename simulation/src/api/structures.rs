use crate::intents::{check_spawn_intent, SpawnIntent as InnerSpawnIntent};
use crate::model::{EntityId, OperationResult};
use crate::systems::script_execution::ScriptExecutionData;
use cao_lang::prelude::*;

#[derive(Debug, Clone, Copy)]
pub struct SpawnIntent {
    structure_id: EntityId,
}
impl AutoByteEncodeProperties for SpawnIntent {}

/// Given a SpawnIntent as input instructs the current spawn to spawn a new Bot
pub fn spawn(vm: &mut VM<ScriptExecutionData>, intent: TPointer) -> Result<(), ExecutionError> {
    let intent = match vm.get_value::<SpawnIntent>(intent) {
        None => {
            log::error!("spawn intent not set");
            return Err(ExecutionError::MissingArgument);
        }
        Some(i) => i,
    };

    let storage = vm.get_aux().storage();
    let user_id = vm.get_aux().user_id;

    let intent = InnerSpawnIntent {
        spawn_id: intent.structure_id,
        owner_id: user_id,
        // bot: intent.bot,
    };

    let check = check_spawn_intent(&intent, user_id, storage);
    if let OperationResult::Ok = check {
        vm.get_aux_mut().intents.spawn_intents.push(intent);
    }
    vm.set_value(check)?;

    Ok(())
}
