use crate::model::components::PathCacheComponent;
use crate::model::EntityId;

/// Update the path cache
#[derive(Debug, Clone)]
pub struct CachePathIntent {
    pub bot: EntityId,
    pub cache: PathCacheComponent,
}

/// Remove the top item from the path cache
#[derive(Debug, Clone)]
pub struct PopPathCacheIntent {
    pub bot: EntityId,
}
