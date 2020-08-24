use crate::components::{Bot, PathCacheComponent};
use crate::indices::EntityId;
use crate::intents::{CachePathIntent, Intents, MutPathCacheIntent, PathCacheIntentAction};
use crate::profile;
use crate::storage::views::{UnsafeView, UnwrapView, UnwrapViewMut, View};
use crate::tables::Table;
use std::mem::replace;

pub fn update(
    (mut path_cache_table, mut cache_intents): (
        UnsafeView<EntityId, PathCacheComponent>,
        UnwrapViewMut<Intents<CachePathIntent>>,
    ),
    (bot_table, mut_cache_intents): (View<EntityId, Bot>, UnwrapView<Intents<MutPathCacheIntent>>),
) {
    profile!("UpdatePathCacheSystem update");

    let cache_intents = replace(&mut cache_intents.0, vec![]);

    for intent in cache_intents.into_iter() {
        let entity_id = intent.bot;
        // check if bot is still alive
        if bot_table.get_by_id(&entity_id).is_none() {
            continue;
        }
        unsafe {
            path_cache_table
                .as_mut()
                .insert_or_update(entity_id, intent.cache);
        }
    }
    for intent in mut_cache_intents.iter() {
        let entity_id = intent.bot;
        match intent.action {
            PathCacheIntentAction::Pop => unsafe {
                if let Some(cache) = path_cache_table.as_mut().get_by_id_mut(&entity_id) {
                    cache.path.pop();
                }
            },
            PathCacheIntentAction::Del => unsafe {
                path_cache_table.as_mut().delete(&entity_id);
            },
        }
    }
}
