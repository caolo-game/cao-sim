use super::parse_uuid;
use anyhow::Context;
use cao_messages::command_capnp::{
    set_default_script_command, update_entity_script_command, update_script_command,
};
use caolo_sim::prelude::*;
use caolo_sim::{self, tables::JoinIterator};
use slog::{debug, error, Logger};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UpdateProgramError {
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Failed to perform the operation {0:?}")]
    Internal(anyhow::Error),
}
type UpdateResult = Result<(), UpdateProgramError>;

/// Update all programs submitted via the PROGRAM field in the Redis storage
pub fn update_program(
    logger: Logger,
    storage: &mut World,
    msg: &update_script_command::Reader,
) -> UpdateResult {
    debug!(logger, "Updating program");

    let user_id = parse_uuid(
        &msg.reborrow()
            .get_user_id()
            .with_context(|| "Failed to get user id")
            .map_err(UpdateProgramError::Internal)?,
    )
    .map_err(UpdateProgramError::Internal)?;
    let script_id = parse_uuid(
        &msg.reborrow()
            .get_script_id()
            .with_context(|| "Failed to get user id")
            .map_err(UpdateProgramError::Internal)?,
    )
    .map_err(UpdateProgramError::Internal)?;

    debug!(
        logger,
        "Inserting new program for user {} {}", user_id, script_id
    );

    let user_id = UserId(user_id);
    let script_id = ScriptId(script_id);

    let program = msg
        .get_compiled_script()
        .with_context(|| "Failed to get compiled script")
        .map_err(UpdateProgramError::Internal)?;

    let bytecode = program
        .reborrow()
        .get_bytecode()
        .with_context(|| "Failed to get bytecode")
        .map_err(UpdateProgramError::Internal)?;

    let program = cao_lang::CompiledProgram {
        bytecode: bytecode.to_vec(),
        labels: program
            .get_labels()
            .with_context(|| "Failed to get bytecode")
            .map_err(UpdateProgramError::Internal)?
            .iter()
            .filter_map(|kv| {
                let id = kv.get_key();
                let label = kv
                    .get_val()
                    .map_err(|err| {
                        error!(logger, "Failed to get value {:?}", err);
                    })
                    .ok()?;
                let label = cao_lang::Label::new(label.get_block() as u32);
                Some((id, label))
            })
            .collect(),
    };

    let program = ScriptComponent(program);
    storage
        .unsafe_view::<ScriptId, ScriptComponent>()
        .insert_or_update(script_id, program);

    update_user_bot_scripts(
        script_id,
        user_id,
        FromWorldMut::new(storage as &mut _),
        FromWorld::new(storage as &_),
    );

    debug!(logger, "Updating program done");
    Ok(())
}

fn update_user_bot_scripts(
    script_id: ScriptId,
    user_id: UserId,
    mut entity_scripts: UnsafeView<EntityId, EntityScript>,
    owned_entities: View<EntityId, OwnedEntity>,
) {
    let entity_scripts = entity_scripts.iter_mut();
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

pub fn update_entity_script(
    storage: &mut World,
    msg: &update_entity_script_command::Reader,
) -> UpdateResult {
    let user_id = parse_uuid(
        &msg.reborrow()
            .get_user_id()
            .with_context(|| "Failed to get user id")
            .map_err(UpdateProgramError::Internal)?,
    )
    .map_err(UpdateProgramError::Internal)?;

    let entity_id = EntityId(msg.get_entity_id());

    let owned_entities_table: View<EntityId, OwnedEntity> = storage.view();

    owned_entities_table
        .get_by_id(&entity_id)
        .ok_or_else(|| UpdateProgramError::Unauthorized)
        .and_then(|owner| {
            if owner.owner_id.0 != user_id {
                Err(UpdateProgramError::Unauthorized)
            } else {
                Ok(owner)
            }
        })?;

    let script_id = parse_uuid(
        &msg.reborrow()
            .get_script_id()
            .with_context(|| "Failed to get script id")
            .map_err(UpdateProgramError::Internal)?,
    )
    .map_err(UpdateProgramError::Internal)?;
    let script_id = ScriptId(script_id);

    let mut scripts_table: UnsafeView<EntityId, EntityScript> = storage.unsafe_view();
    scripts_table.insert_or_update(entity_id, EntityScript { script_id });
    Ok(())
}

pub fn set_default_script(
    storage: &mut World,
    msg: &set_default_script_command::Reader,
) -> UpdateResult {
    let user_id = parse_uuid(
        &msg.reborrow()
            .get_user_id()
            .with_context(|| "Failed to get user id")
            .map_err(UpdateProgramError::Internal)?,
    )
    .map_err(UpdateProgramError::Internal)?;

    let script_id = parse_uuid(
        &msg.reborrow()
            .get_script_id()
            .with_context(|| "Failed to get script id")
            .map_err(UpdateProgramError::Internal)?,
    )
    .map_err(UpdateProgramError::Internal)?;

    let user_id = UserId(user_id);
    let script_id = ScriptId(script_id);

    let script = EntityScript { script_id };

    let mut user_default_script: UnsafeView<UserId, EntityScript> = storage.unsafe_view();
    user_default_script.insert_or_update(user_id, script);

    Ok(())
}
