use crate::components::{Bot, PathCacheComponent};
use crate::indices::EntityId;
use crate::intents::Intents;
use crate::intents::PathCacheIntentAction;
use crate::profile;
use crate::storage::views::{UnsafeView, UnwrapViewMut, View};
use crate::tables::Table;
use std::mem::replace;

pub fn update(
    (mut path_cache_table, mut intents): (
        UnsafeView<EntityId, PathCacheComponent>,
        UnwrapViewMut<Intents>,
    ),
    (bot_table,): (View<EntityId, Bot>,),
) {
    profile!("UpdatePathCacheSystem update");

    let cache_intents = replace(&mut intents.update_path_cache_intent, vec![]);

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
    let intents = &intents.mut_path_cache_intent;
    for intent in intents {
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
