use crate::indices::{EntityId, UserId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpawnIntent {
    pub spawn_id: EntityId,
    pub owner_id: Option<UserId>,
}
