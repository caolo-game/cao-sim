use crate::model::{self, components, EntityId, OperationResult, UserId};
use crate::World;

#[derive(Debug, Clone)]
pub struct SpawnIntent {
    pub spawn_id: EntityId,
    pub owner_id: Option<UserId>,
}

pub fn check_spawn_intent(
    intent: &SpawnIntent,
    user_id: Option<model::UserId>,
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
                debug!("Structure not found");
                return OperationResult::InvalidInput;
            }
        }
    }

    if let Some(spawn) = storage
        .view::<EntityId, components::SpawnComponent>()
        .get_by_id(&id)
    {
        if spawn.spawning.is_some() {
            debug!("Structure is busy");
            return OperationResult::InvalidInput;
        }
    } else {
        debug!("Structure has no spawn component");
        return OperationResult::InvalidInput;
    }

    OperationResult::Ok
}
