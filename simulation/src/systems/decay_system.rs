use super::System;
use crate::components::{DecayComponent, HpComponent};
use crate::model::EntityId;
use crate::storage::views::{DeleteEntityView, UnsafeView};
use crate::tables::JoinIterator;

pub struct DecaySystem;

impl<'a> System<'a> for DecaySystem {
    type Mut = (
        UnsafeView<EntityId, HpComponent>,
        UnsafeView<EntityId, DecayComponent>,
        DeleteEntityView,
    );
    type Const = ();

    fn update(&mut self, (mut hps, mut decays, mut delete): Self::Mut, _: Self::Const) {
        debug!("update decay system called");

        let iter =
            unsafe { JoinIterator::new(decays.as_mut().iter_mut(), hps.as_mut().iter_mut()) };
        iter.for_each(|(id, (decay, hp))| {
            if decay.t > 0 {
                decay.t -= 1;
                if decay.t == 0 {
                    hp.hp -= hp.hp.min(decay.hp_amount);
                    decay.t = decay.eta;
                }
            }
            if hp.hp == 0 {
                debug!("Entity {:?} has died, deleting", id);
                unsafe {
                    delete.delete_entity(&id);
                }
            }
        });

        debug!("update decay system done");
    }
}
