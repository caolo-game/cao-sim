use super::System;
use crate::components::{DecayComponent, HpComponent};
use crate::indices::EntityId;
use crate::join;
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

        let decays = unsafe { decays.as_mut() }.iter_mut();
        let hps = unsafe { hps.as_mut() }.iter_mut();
        join!([decays, hps]).for_each(
            |(
                _id,
                (
                    DecayComponent {
                        hp_amount,
                        eta,
                        ref mut t,
                    },
                    HpComponent { ref mut hp, .. },
                ),
            )| match t {
                0 => {
                    *hp -= *hp.min(hp_amount);
                    *t = *eta;
                }
                _ => {
                    *t -= 1;
                }
            },
        );

        debug!("update decay system done");
    }
}
