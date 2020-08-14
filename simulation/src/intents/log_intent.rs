use crate::model::indices::EntityId;

#[derive(Debug, Clone)]
pub struct LogIntent {
    pub entity: EntityId,
    pub payload: Vec<String>,
    pub time: u64,
}
