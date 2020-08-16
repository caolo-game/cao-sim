mod dropoff_intent_system;
mod log_intent_system;
mod mine_intent_system;
mod move_intent_system;
mod path_cache_intent_system;
mod spawn_intent_system;

use self::dropoff_intent_system::DropoffSystem;
use self::log_intent_system::LogSystem;
use self::mine_intent_system::MineSystem;
use self::move_intent_system::MoveSystem;
use self::path_cache_intent_system::{MutPathCacheSystem, UpdatePathCacheSystem};
use self::spawn_intent_system::SpawnSystem;
use crate::components::*;
use crate::intents::{Intents, MoveIntent};
use crate::model::EntityId;
use crate::model::WorldPosition;
use crate::profile;
use crate::storage::views::{FromWorld, FromWorldMut};
use crate::storage::views::{InsertEntityView, UnsafeView, View};
use crate::World;
use log::debug;
use rayon::prelude::*;
use std::mem::replace;

pub trait IntentExecutionSystem<'a> {
    type Mut: FromWorldMut + Clone;
    type Const: FromWorld<'a>;
    type Intents;

    fn execute(&mut self, m: Self::Mut, c: Self::Const, intents: Self::Intents);
}

/// Executes all intents in order of priority (as defined by this system)
pub fn execute_intents(mut intents: Intents, storage: &mut World) {
    profile!("execute_intents");

    pre_process_move_intents(&mut intents.move_intent);

    // these will be processed a bit differently than the rest
    let log_intent = replace(&mut intents.log_intent, vec![]);
    let update_path_cache_intent = replace(&mut intents.update_path_cache_intent, vec![]);

    let intents = &intents;

    rayon::scope(move |s| {
        // we can update systems in parallel that do not use the same tables

        {
            // Explicitly list the dependencies, so we can see what calls can be performed in
            // parallel
            let move_sys = executor::<
                (UnsafeView<EntityId, PositionComponent>,),
                (View<EntityId, Bot>, View<WorldPosition, EntityComponent>),
                _,
                _,
            >(MoveSystem, storage);
            let mine_sys = executor::<
                (
                    UnsafeView<EntityId, EnergyComponent>,
                    UnsafeView<EntityId, CarryComponent>,
                ),
                (View<EntityId, ResourceComponent>,),
                _,
                _,
            >(MineSystem, storage);
            let dropoff_sys = executor::<
                (
                    UnsafeView<EntityId, EnergyComponent>,
                    UnsafeView<EntityId, CarryComponent>,
                ),
                (),
                _,
                _,
            >(DropoffSystem, storage);
            let spawn_sys = executor::<
                (
                    UnsafeView<EntityId, SpawnBotComponent>,
                    UnsafeView<EntityId, SpawnComponent>,
                    UnsafeView<EntityId, OwnedEntity>,
                    InsertEntityView,
                ),
                (View<EntityId, EnergyComponent>,),
                _,
                _,
            >(SpawnSystem, storage);

            s.spawn(move |_| {
                move_sys(intents);
                mine_sys(intents);
                dropoff_sys(intents);
                spawn_sys(intents);
            });
        }

        {
            let m = FromWorldMut::new(storage);
            let c = FromWorld::new(storage as &_);
            s.spawn(move |_| {
                let mut log_sys = LogSystem;
                log_sys.execute(m, c, log_intent);
            });
        }

        {
            let pop_path_cache_sys = executor(MutPathCacheSystem, storage);
            let m = FromWorldMut::new(storage);
            let c = FromWorld::new(storage as &_);
            s.spawn(move |_| {
                let mut update_cache_sys = UpdatePathCacheSystem;
                update_cache_sys.execute(m, c, update_path_cache_intent);
                pop_path_cache_sys(intents);
            });
        }
    });
}

/// Create an executor for an intent system
/// M and C types are used so we're able to explicitly list the dependencies and trigger compile
/// errors when system deps change.
/// This is intended for readers to better visualize the movement of data in their heads.
fn executor<'a, 'b, M, C, T, Sys>(
    mut sys: Sys,
    storage: *mut World,
) -> impl FnOnce(&'b Intents) -> () + 'a
where
    'b: 'a,
    T: 'a,
    M: FromWorldMut + Clone + 'a,
    C: FromWorld<'a> + 'a,
    &'a Intents: Into<&'a [T]>,
    Sys: IntentExecutionSystem<'a, Mut = M, Const = C, Intents = &'a [T]> + 'a,
{
    let storage = unsafe { &mut *storage };
    let mutable = Sys::Mut::new(storage);
    let immutable = Sys::Const::new(storage);

    move |intents| {
        sys.execute(mutable.clone(), immutable, intents.into());
        mutable.log();
    }
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
