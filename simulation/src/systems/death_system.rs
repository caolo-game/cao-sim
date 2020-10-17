use crate::components::HpComponent;
use crate::indices::EntityId;
use crate::profile;
use crate::storage::views::{DeferredDeleteEntityView, View, WorldLogger};
use slog::{debug, trace};

pub fn update(
    mut delete: DeferredDeleteEntityView,
    (hps, WorldLogger(logger)): (View<EntityId, HpComponent>, WorldLogger),
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

    debug!(logger, "update death system done");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{init_inmemory_storage, query};
    use crate::{
        storage::views::FromWorld,
        storage::views::FromWorldMut,
        utils::{setup_testing, test_logger},
    };

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
