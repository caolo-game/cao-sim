use super::System;
use crate::components::{Bot, PathCacheComponent};
use crate::intents::Intents;
use crate::intents::{CachePathIntent, PathCacheIntentAction};
use crate::model::EntityId;
use crate::profile;
use crate::storage::views::{UnsafeView, UnwrapView, View};
use crate::tables::Table;

pub struct UpdatePathCacheSystem {
    pub intents: Vec<CachePathIntent>,
}

impl<'a> System<'a> for UpdatePathCacheSystem {
    type Mut = (UnsafeView<EntityId, PathCacheComponent>,);
    type Const = (View<'a, EntityId, Bot>,);

    fn update(&mut self, (mut path_cache_table,): Self::Mut, (bot_table,): Self::Const) {
        profile!("UpdatePathCacheSystem update");

        let intents = std::mem::replace(&mut self.intents, vec![]);
        for intent in intents.into_iter() {
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
    }
}

pub struct MutPathCacheSystem;

impl<'a> System<'a> for MutPathCacheSystem {
    type Mut = (UnsafeView<EntityId, PathCacheComponent>,);
    type Const = (UnwrapView<'a, Intents>,);

    fn update(&mut self, (mut path_cache_table,): Self::Mut, (intents,): Self::Const) {
        profile!(" MutPathCacheSystem update");
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
}
