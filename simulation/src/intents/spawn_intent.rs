use crate::components;
use crate::indices::{EntityId, UserId};
use crate::scripting_api::OperationResult;
use crate::World;
use serde::{Deserialize, Serialize};
use slog::{debug, Logger};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpawnIntent {
    pub spawn_id: EntityId,
    pub owner_id: Option<UserId>,
}

pub fn check_spawn_intent(
    logger: &Logger,
    intent: &SpawnIntent,
    user_id: Option<UserId>,
    storage: &World,
) -> OperationResult {
    let id = intent.spawn_id;

    if let Some(user_id) = user_id {
        match storage
            .view::<EntityId, components::Structure>()
            .get_by_id(&id)
        {
            Some(_) => {
                let owner_id = storage
                    .view::<EntityId, components::OwnedEntity>()
                    .reborrow()
                    .get_by_id(&id);
                if owner_id.map(|id| id.owner_id != user_id).unwrap_or(true) {
                    return OperationResult::NotOwner;
                }
            }
            None => {
                debug!(logger, "Structure not found");
                return OperationResult::InvalidInput;
            }
        }
    }

    if let Some(spawn) = storage
        .view::<EntityId, components::SpawnComponent>()
        .get_by_id(&id)
    {
        if spawn.spawning.is_some() {
            debug!(logger, "Structure is busy");
            return OperationResult::InvalidInput;
        }
    } else {
        debug!(logger, "Structure has no spawn component");
        return OperationResult::InvalidInput;
    }

    OperationResult::Ok
}
