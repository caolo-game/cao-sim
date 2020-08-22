use crate::components::HpComponent;
use crate::indices::EntityId;
use crate::profile;
use crate::storage::views::{DeleteEntityView, View};
use log::{debug, trace};

pub fn update<'a>(mut delete: DeleteEntityView, hps: View<'a, EntityId, HpComponent>) {
    profile!("DeathSystem update");
    debug!("update death system called");

    hps.iter().for_each(|(id, hp)| {
        if hp.hp == 0 {
            trace!("Entity {:?} has died, deleting", id);
            unsafe {
                delete.delete_entity(id);
            }
        }
    });

    debug!("update death system done");
}
