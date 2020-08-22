use crate::components::{DecayComponent, HpComponent};
use crate::indices::EntityId;
use crate::join;
use crate::profile;
use crate::storage::views::UnsafeView;
use crate::tables::JoinIterator;
use log::debug;

pub fn update(
    (mut hps, mut decays): (
        UnsafeView<EntityId, HpComponent>,
        UnsafeView<EntityId, DecayComponent>,
    ),
    _: (),
) {
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
