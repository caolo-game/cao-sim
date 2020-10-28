use crate::indices::EntityId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeleteEntityIntent {
    pub id: EntityId,
}
