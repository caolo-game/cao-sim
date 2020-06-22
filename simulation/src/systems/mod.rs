pub mod decay_system;
pub mod energy_system;
pub mod intent_system;
pub mod log_system;
pub mod mineral_system;
pub mod positions_system;
pub mod script_execution;
pub mod spawn_system;

use crate::profile;
use crate::storage::views::{FromWorld, FromWorldMut};
use crate::World;

pub trait System<'a> {
    // Requiring these traits instead of From impl disallows Storage as an `update` parameter
    // Thus requiring callers to explicitly state their dependencies
    type Mut: FromWorldMut;
    type Const: FromWorld<'a>;

    fn update(&mut self, m: Self::Mut, c: Self::Const);

    fn name() -> &'static str {
        use std::any::type_name;
        type_name::<Self>()
    }
}

pub fn execute_world_update(storage: &mut World) {
    profile!("execute_world_update");

    let mut energy_sys = energy_system::EnergySystem;
    update(&mut energy_sys, storage);

    let mut spawn_sys = spawn_system::SpawnSystem;
    update(&mut spawn_sys, storage);

    let mut decay_sys = decay_system::DecaySystem;
    update(&mut decay_sys, storage);

    let mut mineral_sys = mineral_system::MineralSystem;
    update(&mut mineral_sys, storage);

    let mut positions_sys = positions_system::PositionSystem;
    update(&mut positions_sys, storage);

    let mut log_sys = log_system::LogSystem;
    update(&mut log_sys, storage);
}

#[inline]
fn update<'a, Sys: System<'a>>(sys: &mut Sys, storage: &'a mut World) {
    sys.update(Sys::Mut::new(storage), Sys::Const::new(storage as &_));
}
