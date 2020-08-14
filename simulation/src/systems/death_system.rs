use super::System;
use crate::components::HpComponent;
use crate::model::EntityId;
use crate::profile;
use crate::storage::views::{DeleteEntityView, View};
use log::{debug, trace};

pub struct DeathSystem;

impl<'a> System<'a> for DeathSystem {
    type Mut = (DeleteEntityView,);
    type Const = (View<'a, EntityId, HpComponent>,);

    fn update(&mut self, (mut delete,): Self::Mut, (hps,): Self::Const) {
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
}
