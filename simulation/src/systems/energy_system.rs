use crate::components::{EnergyComponent, EnergyRegenComponent};
use crate::indices::EntityId;
use crate::profile;
use crate::storage::views::{UnsafeView, View};
use crate::tables::JoinIterator;

pub fn update(
    mut energy: UnsafeView<EntityId, EnergyComponent>,
    energy_regen: View<EntityId, EnergyRegenComponent>,
) {
    profile!("EnergySystem update");
    let energy_it = unsafe { energy.as_mut().iter_mut() };
    let join = JoinIterator::new(energy_it, energy_regen.iter());
    join.for_each(|(_id, (e, er))| {
        e.energy = (e.energy + er.amount).min(e.energy_max);
    });
}
