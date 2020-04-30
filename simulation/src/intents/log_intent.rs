use crate::model::indices::EntityId;

#[derive(Debug, Clone)]
pub struct LogIntent {
    pub entity: EntityId,
    pub payload: String,
    pub time: u64,
}
