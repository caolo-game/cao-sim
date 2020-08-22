use crate::components::PathCacheComponent;
use crate::indices::EntityId;
use serde::{Deserialize, Serialize};

/// Update the path cache
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CachePathIntent {
    pub bot: EntityId,
    pub cache: PathCacheComponent,
}

/// Remove the top item from the path cache
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutPathCacheIntent {
    pub bot: EntityId,
    pub action: PathCacheIntentAction,
}
impl Default for MutPathCacheIntent {
    fn default() -> Self {
        MutPathCacheIntent {
            action: PathCacheIntentAction::Del,
            bot: EntityId::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PathCacheIntentAction {
    Pop,
    Del,
}
