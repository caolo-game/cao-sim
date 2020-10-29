use crate::indices::*;
use crate::profile;
use crate::storage::views::{DeferredDeleteEntityView, View, WorldLogger};
use crate::{
    components::HpComponent,
    intents::{DeleteEntityIntent, Intents},
    prelude::UnwrapView,
};
use slog::{debug, trace};

pub fn update(
    mut delete: DeferredDeleteEntityView,
    (hps, delete_intents, WorldLogger(logger)): (
        View<EntityId, HpComponent>,
        UnwrapView<EmptyKey, Intents<DeleteEntityIntent>>,
        WorldLogger,
    ),
) {
    profile!("DeathSystem update");
    debug!(logger, "update death system called");

    hps.iter().for_each(|(id, hp)| {
        if hp.hp == 0 {
            trace!(logger, "Entity {:?} has died, deleting", id);
            unsafe {
                delete.delete_entity(id);
            }
        }
    });

    delete_intents
        .0
        .iter()
        .for_each(|DeleteEntityIntent { id }| unsafe {
            delete.delete_entity(*id);
        });

    debug!(logger, "update death system done");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{intents, query, world::init_inmemory_storage};
    use crate::{
        storage::views::FromWorld,
        storage::views::FromWorldMut,
        utils::{setup_testing, test_logger},
    };

    #[test]
    fn can_kill_or_delete_entity_multiple_times() {
        setup_testing();
        let mut store = init_inmemory_storage(test_logger());

        let entity_1 = store.insert_entity();
        query!(
            mutate
            store
            {
                EntityId, HpComponent, .insert_or_update(entity_1, HpComponent {
                    hp: 0,
                    hp_max: 123
                });
            }
        );
        let entities: Vec<_> = store
            .view::<EntityId, HpComponent>()
            .iter()
            .map(|(id, _)| id)
            .collect();

        assert_eq!(entities, vec![entity_1,]);

        intents::move_into_storage(
            &mut store,
            vec![
                intents::BotIntents {
                    delete_entity_intent: Some(intents::DeleteEntityIntent { id: entity_1 }),
                    ..Default::default()
                };
                3
            ],
        );

        update(FromWorldMut::new(&mut *store), FromWorld::new(&mut *store));
        store.post_process();

        let entities: Vec<_> = store
            .view::<EntityId, HpComponent>()
            .iter()
            .map(|(id, _)| id)
            .collect();

        assert_eq!(entities, vec![]);
    }

    #[test]
    fn test_dead_entity_is_deleted() {
        setup_testing();
        let mut store = init_inmemory_storage(test_logger());

        let entity_1 = store.insert_entity();
        let entity_2 = store.insert_entity();
        query!(
            mutate
            store
            {
                EntityId, HpComponent, .insert_or_update(entity_1, HpComponent {
                    hp: 0,
                    hp_max: 123
                });
                EntityId, HpComponent, .insert_or_update(entity_2, HpComponent {
                    hp: 50,
                    hp_max: 123
                });
            }
        );

        let entities: Vec<_> = store
            .view::<EntityId, HpComponent>()
            .iter()
            .map(|(id, _)| id)
            .collect();

        assert_eq!(entities, vec![entity_1, entity_2]);

        update(FromWorldMut::new(&mut *store), FromWorld::new(&mut *store));
        store.post_process();

        let entities: Vec<_> = store
            .view::<EntityId, HpComponent>()
            .iter()
            .map(|(id, _)| id)
            .collect();

        assert_eq!(entities, vec![entity_2]);
    }
}
