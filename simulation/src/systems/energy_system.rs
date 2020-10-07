use crate::components::{EnergyComponent, EnergyRegenComponent};
use crate::indices::EntityId;
use crate::join;
use crate::profile;
use crate::storage::views::{UnsafeView, View};
use crate::tables::JoinIterator;

pub fn update(
    mut energy: UnsafeView<EntityId, EnergyComponent>,
    energy_regen: View<EntityId, EnergyRegenComponent>,
) {
    profile!("EnergySystem update");
    let energy_it = energy.iter_mut();
    let energy_regen_it = energy_regen.iter();
    join!([energy_it, energy_regen_it]).for_each(|(_id, (e, er))| {
        e.energy = (e.energy + er.amount).min(e.energy_max);
    });
}
