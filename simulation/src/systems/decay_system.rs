use super::System;
use crate::components::{DecayComponent, HpComponent};
use crate::model::EntityId;
use crate::profile;
use crate::storage::views::UnsafeView;
use crate::tables::JoinIterator;
use log::debug;

pub struct DecaySystem;

impl<'a> System<'a> for DecaySystem {
    type Mut = (
        UnsafeView<EntityId, HpComponent>,
        UnsafeView<EntityId, DecayComponent>,
    );
    type Const = ();

    fn update(&mut self, (mut hps, mut decays): Self::Mut, _: Self::Const) {
        profile!("DecaySystem update");
        debug!("update decay system called");

        let iter =
            unsafe { JoinIterator::new(decays.as_mut().iter_mut(), hps.as_mut().iter_mut()) };
        iter.for_each(|(_id, (decay, hp))| match decay.t {
            0 => {
                hp.hp -= hp.hp.min(decay.hp_amount);
                decay.t = decay.eta;
            }
            _ => {
                decay.t -= 1;
            }
        });

        debug!("update decay system done");
    }
}
