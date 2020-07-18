use super::IntentExecutionSystem;
use crate::components::{Bot, PathCacheComponent};
use crate::intents::{CachePathIntent, PopPathCacheIntent};
use crate::model::EntityId;
use crate::profile;
use crate::storage::views::{UnsafeView, View};

pub struct UpdatePathCacheSystem;

impl<'a> IntentExecutionSystem<'a> for UpdatePathCacheSystem {
    type Mut = (UnsafeView<EntityId, PathCacheComponent>,);
    type Const = (View<'a, EntityId, Bot>,);
    type Intent = CachePathIntent;

    fn execute(
        &mut self,
        (mut path_cache_table,): Self::Mut,
        (bot_table,): Self::Const,
        intents: &[Self::Intent],
    ) {
        profile!(" UpdatePathCacheSystem update");
        for intent in intents {
            let entity_id = intent.bot;
            // check if bot is still alive
            if bot_table.get_by_id(&entity_id).is_none() {
                continue;
            }
            unsafe {
                path_cache_table
                    .as_mut()
                    .insert_or_update(entity_id, intent.cache.clone());
            }
        }
    }
}

pub struct PopPathCacheSystem;

impl<'a> IntentExecutionSystem<'a> for PopPathCacheSystem {
    type Mut = (UnsafeView<EntityId, PathCacheComponent>,);
    type Const = ();
    type Intent = PopPathCacheIntent;

    fn execute(
        &mut self,
        (mut path_cache_table,): Self::Mut,
        (): Self::Const,
        intents: &[Self::Intent],
    ) {
        profile!(" PopPathCacheSystem update");
        for intent in intents {
            let entity_id = intent.bot;
            unsafe {
                if let Some(cache) = path_cache_table.as_mut().get_by_id_mut(&entity_id) {
                    cache.path.pop();
                }
            }
        }
    }
}
