use crate::components::PathCacheComponent;
use crate::model::EntityId;

/// Update the path cache
#[derive(Debug, Clone)]
pub struct CachePathIntent {
    pub bot: EntityId,
    pub cache: PathCacheComponent,
}

/// Remove the top item from the path cache
#[derive(Debug, Clone)]
pub struct MutPathCacheIntent {
    pub bot: EntityId,
    pub action: PathCacheIntentAction,
}

#[derive(Debug, Clone, Copy)]
pub enum PathCacheIntentAction {
    Pop,
    Del,
}
