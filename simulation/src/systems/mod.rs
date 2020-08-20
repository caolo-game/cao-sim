pub mod death_system;
pub mod decay_system;
pub mod dropoff_intent_system;
pub mod energy_system;
pub mod log_intent_system;
pub mod log_system;
pub mod mine_intent_system;
pub mod mineral_system;
pub mod move_intent_system;
pub mod path_cache_intent_system;
pub mod positions_system;
pub mod script_execution;
pub mod spawn_intent_system;
pub mod spawn_system;

use self::dropoff_intent_system::DropoffSystem;
use self::log_intent_system::LogIntentSystem;
use self::mine_intent_system::MineSystem;
use self::move_intent_system::MoveSystem;
use self::path_cache_intent_system::{MutPathCacheSystem, UpdatePathCacheSystem};
use self::spawn_intent_system::SpawnSystem;
use crate::intents::{Intents, MoveIntent};
use crate::model::indices::EmptyKey;
use crate::profile;
use crate::storage::views::{FromWorld, FromWorldMut};
use crate::World;
use log::debug;
use rayon::prelude::*;
use std::mem::replace;

pub trait System<'a> {
    // Requiring these traits instead of From impl disallows Storage as an `update` parameter
    // Thus requiring callers to explicitly state their dependencies
    type Mut: FromWorldMut + Clone;
    type Const: FromWorld<'a>;

    fn update(&mut self, m: Self::Mut, c: Self::Const);

    fn name() -> &'static str {
        use std::any::type_name;
        type_name::<Self>()
    }
}

pub fn execute_world_update(storage: &mut World) {
    profile!("execute_world_update");

    execute_intents(storage);

    let mut decay_sys = decay_system::DecaySystem;
    update(&mut decay_sys, storage);

    let mut death_sys = death_system::DeathSystem;
    update(&mut death_sys, storage);

    let mut energy_sys = energy_system::EnergySystem;
    update(&mut energy_sys, storage);

    let mut spawn_sys = spawn_system::SpawnSystem;
    update(&mut spawn_sys, storage);

    let mut mineral_sys = mineral_system::MineralSystem;
    update(&mut mineral_sys, storage);

    let mut positions_sys = positions_system::PositionSystem;
    update(&mut positions_sys, storage);

    let mut log_sys = log_system::LogSystem;
    update(&mut log_sys, storage);
}

#[inline]
fn update<'a, Sys: System<'a>>(sys: &mut Sys, storage: &'a mut World) {
    let m = Sys::Mut::new(storage);
    let c = Sys::Const::new(storage as &_);
    sys.update(Sys::Mut::clone(&m), c);
    m.log();
}

pub fn execute_intents(storage: &mut World) {
    profile!("execute_intents");

    let log_intent;
    let update_path_cache_intent;
    unsafe {
        let mut intents = storage.unsafe_view::<EmptyKey, Intents>();
        pre_process_move_intents(&mut intents.as_mut().unwrap_mut().move_intent);

        // these will be processed a bit differently than the rest
        log_intent = replace(&mut intents.as_mut().unwrap_mut().log_intent, vec![]);
        update_path_cache_intent = replace(
            &mut intents.as_mut().unwrap_mut().update_path_cache_intent,
            vec![],
        );
    }

    update(&mut MoveSystem, storage);
    update(&mut MineSystem, storage);
    update(&mut DropoffSystem, storage);
    update(&mut SpawnSystem, storage);
    let mut log_sys = LogIntentSystem {
        intents: log_intent,
    };
    update(&mut log_sys, storage);

    let mut update_path_cache_sys = UpdatePathCacheSystem {
        intents: update_path_cache_intent,
    };
    update(&mut update_path_cache_sys, storage);

    update(&mut MutPathCacheSystem, storage);
}

/// Remove duplicate positions.
/// We assume that there are no duplicated entities
fn pre_process_move_intents(move_intents: &mut Vec<MoveIntent>) {
    let len = move_intents.len();
    if len < 2 {
        // 0 and 1 long vectors do not have duplicates
        return;
    }
    move_intents.par_sort_unstable_by_key(|intent| intent.position);
    // move in reverse order because we want to remove invalid intents as we move,
    // swap_remove would change the last position, screwing with the ordering
    for current in (0..=len - 2).rev() {
        let last = current + 1;
        let a = &move_intents[last];
        let b = &move_intents[current];
        if a.position == b.position {
            debug!("Duplicated position in move intents, removing {:?}", a);
            move_intents.swap_remove(last);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Axial;
    use crate::model::EntityId;
    use crate::model::WorldPosition;

    #[test]
    fn pre_process_move_intents_removes_last_dupe() {
        let mut intents = vec![
            MoveIntent {
                bot: EntityId(42),
                position: WorldPosition {
                    room: Default::default(),
                    pos: Axial::new(42, 69),
                },
            },
            MoveIntent {
                bot: EntityId(123),
                position: WorldPosition {
                    room: Default::default(),
                    pos: Axial::new(42, 69),
                },
            },
            MoveIntent {
                bot: EntityId(64),
                position: WorldPosition {
                    room: Default::default(),
                    pos: Axial::new(43, 69),
                },
            },
            MoveIntent {
                bot: EntityId(69),
                position: WorldPosition {
                    room: Default::default(),
                    pos: Axial::new(42, 69),
                },
            },
        ];

        pre_process_move_intents(&mut intents);
        assert_eq!(intents.len(), 2);
        assert_ne!(intents[0].position, intents[1].position);
    }
}
