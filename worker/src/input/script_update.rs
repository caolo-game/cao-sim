use cao_messages::command::{UpdateEntityScriptCommand, UpdateScriptCommand};
use caolo_sim::prelude::*;
use caolo_sim::{self, tables::JoinIterator};
use log::debug;

#[derive(Debug, Clone)]
pub enum UpdateProgramError {
    Unauthorized,
}
type UpdateResult = Result<(), UpdateProgramError>;

/// Update all programs submitted via the PROGRAM field in the Redis storage
pub fn update_program(storage: &mut World, msg: UpdateScriptCommand) -> UpdateResult {
    debug!("Updating program {:?}", msg);
    debug!(
        "Inserting new program for user {} {}",
        msg.user_id, msg.script_id
    );

    let user_id = UserId(msg.user_id);
    let script_id = ScriptId(msg.script_id);

    let program = msg.compiled_script;
    let program = cao_lang::CompiledProgram {
        bytecode: program.bytecode,
        labels: program
            .labels
            .into_iter()
            .map(|(id, label)| {
                let label = cao_lang::Label::new(label.block, label.myself);
                (id, label)
            })
            .collect(),
    };

    let program = ScriptComponent(program);
    unsafe {
        storage
            .unsafe_view::<ScriptId, ScriptComponent>()
            .as_mut()
            .insert_or_update(script_id, program);
    }

    update_user_bot_scripts(
        script_id,
        user_id,
        FromWorldMut::new(storage as &mut _),
        FromWorld::new(storage as &_),
    );

    debug!("Updating program done");
    Ok(())
}

fn update_user_bot_scripts(
    script_id: ScriptId,
    user_id: UserId,
    mut entity_scripts: UnsafeView<EntityId, EntityScript>,
    owned_entities: View<EntityId, OwnedEntity>,
) {
    let entity_scripts = unsafe { entity_scripts.as_mut().iter_mut() };
    let join = JoinIterator::new(
        owned_entities
            .iter()
            .filter(|(_id, owner)| owner.owner_id == user_id),
        entity_scripts,
    );
    for (_id, (_owner, entity_script)) in join {
        entity_script.script_id = script_id;
    }
}

pub fn update_entity_script(storage: &mut World, msg: UpdateEntityScriptCommand) -> UpdateResult {
    let entity_id = EntityId(msg.entity_id);
    let user_id = UserId(msg.user_id);

    let owned_entities_table: View<EntityId, OwnedEntity> = storage.view();

    owned_entities_table
        .get_by_id(&entity_id)
        .ok_or_else(|| UpdateProgramError::Unauthorized)
        .and_then(|owner| {
            if owner.owner_id != user_id {
                Err(UpdateProgramError::Unauthorized)
            } else {
                Ok(owner)
            }
        })?;

    let mut scripts_table: UnsafeView<EntityId, EntityScript> = storage.unsafe_view();
    let script_id = ScriptId(msg.script_id);
    unsafe {
        scripts_table
            .as_mut()
            .insert_or_update(entity_id, EntityScript { script_id });
    }
    Ok(())
}
