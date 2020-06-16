use super::System;
use crate::    components::{EnergyComponent, EnergyRegenComponent};
use crate::model::{
    EntityId,
};
use crate::storage::views::{UnsafeView, View};
use crate::tables::JoinIterator;

pub struct EnergySystem;

impl<'a> System<'a> for EnergySystem {
    type Mut = UnsafeView<EntityId, EnergyComponent>;
    type Const = View<'a, EntityId, EnergyRegenComponent>;

    fn update(
        &mut self,
        mut energy: UnsafeView<EntityId, EnergyComponent>,
        energy_regen: View<EntityId, EnergyRegenComponent>,
    ) {
        let energy_it = unsafe { energy.as_mut().iter_mut() };
        let join = JoinIterator::new(energy_it, energy_regen.iter());
        join.for_each(|(_id, (e, er))| {
            e.energy = (e.energy + er.amount).min(e.energy_max);
        });
    }
}
