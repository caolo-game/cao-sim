use crate::indices::EntityId;
use serde::{Deserialize, Serialize};
use cao_lang::vm::HistoryEntry;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScriptHistoryIntent {
    pub entity: EntityId,
    pub payload: Vec<HistoryEntry>,
    pub time: u64,
}
