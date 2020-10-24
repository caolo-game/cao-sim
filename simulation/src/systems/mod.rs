pub mod script_execution;

mod attack_system;
mod death_system;
mod decay_system;
mod dropoff_intent_system;
mod energy_system;
mod log_intent_system;
mod log_system;
mod mine_intent_system;
mod mineral_system;
mod move_intent_system;
mod path_cache_intent_system;
mod positions_system;
mod script_history_system;
mod spawn_system;

use crate::storage::views::{FromWorld, FromWorldMut};
use crate::{prelude::World, profile};

pub fn execute_world_update(storage: &mut World) {
    profile!("execute_systems_update");

    execute_intents(storage);
    execute_automated_systems(storage);
}

fn execute_intents(storage: &mut World) {
    profile!("execute_intents");

    // pre processing
    execute_update(spawn_system::update_cont_spawns, storage);

    // main processing
    execute_update(attack_system::update, storage);
    execute_update(move_intent_system::update, storage);
    execute_update(mine_intent_system::update, storage);
    execute_update(dropoff_intent_system::update, storage);
    execute_update(spawn_system::update_spawn_intents, storage);
    execute_update(log_intent_system::update, storage);
    execute_update(path_cache_intent_system::update, storage);
    execute_update(script_history_system::update, storage);
}

/// Execute systems that run regardless of player actions
fn execute_automated_systems(storage: &mut World) {
    profile!("execute_automated_systems");

    execute_update(decay_system::update, storage);
    execute_update(death_system::update, storage);
    execute_update(energy_system::update, storage);
    execute_update(spawn_system::update_spawns, storage);
    execute_update(mineral_system::update, storage);
    execute_update(positions_system::update, storage);
    execute_update(log_system::update, storage);
}

#[inline]
fn execute_update<'a, M, C, Sys>(sys: Sys, storage: &'a mut World)
where
    Sys: Fn(M, C) + 'a,
    M: FromWorldMut + Clone + 'a,
    C: FromWorld<'a> + 'a,
{
    let m = M::new(storage);
    let c = C::new(storage as &_);
    sys(M::clone(&m), c);
    m.log();
}
