use crate::model::indices::EntityId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LogIntent {
    pub entity: EntityId,
    pub payload: Vec<String>,
    pub time: u64,
}
