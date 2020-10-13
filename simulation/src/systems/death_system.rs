use crate::components::HpComponent;
use crate::indices::EntityId;
use crate::profile;
use crate::storage::views::{DeleteEntityView, View, WorldLogger};
use slog::{debug, trace};

pub fn update(
    mut delete: DeleteEntityView,
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
