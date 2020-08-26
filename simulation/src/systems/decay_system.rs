use crate::components::{DecayComponent, HpComponent};
use crate::indices::EntityId;
use crate::join;
use crate::profile;
use crate::storage::views::{UnsafeView, WorldLogger};
use crate::tables::JoinIterator;
use slog::debug;

pub fn update(
    (mut hps, mut decays): (
        UnsafeView<EntityId, HpComponent>,
        UnsafeView<EntityId, DecayComponent>,
    ),
    WorldLogger(logger): WorldLogger,
) {
    profile!("DecaySystem update");
    debug!(logger, "update decay system called");

    let decays = decays.iter_mut();
    let hps = hps.iter_mut();
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

    debug!(logger, "update decay system done");
}
