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
use self::path_cache_intent_system::{PopPathCacheSystem, UpdatePathCacheSystem};
use self::spawn_intent_system::SpawnSystem;
use crate::intents::{Intents, MoveIntent};
use crate::profile;
use crate::storage::views::{FromWorld, FromWorldMut};
use crate::World;
use rayon::prelude::*;

pub trait IntentExecutionSystem<'a> {
    type Mut: FromWorldMut + Clone;
    type Const: FromWorld<'a>;
    type Intent;

    fn execute(&mut self, m: Self::Mut, c: Self::Const, intents: &[Self::Intent]);
}

/// Executes all intents in order of priority (as defined by this system)
pub fn execute_intents(mut intents: Intents, storage: &mut World) {
    profile!("execute_intents");

    pre_process_move_intents(&mut intents.move_intents);

    let intents = &intents;

    rayon::scope(move |s| {
        // we can update systems in parallel that do not use the same tables

        {
            let move_sys = executor(MoveSystem, storage);
            let mine_sys = executor(MineSystem, storage);
            let dropoff_sys = executor(DropoffSystem, storage);
            let spawn_sys = executor(SpawnSystem, storage);

            s.spawn(move |_| {
                move_sys(intents);
                mine_sys(intents);
                dropoff_sys(intents);
                spawn_sys(intents);
            });
        }

        let log_sys = executor(LogSystem, storage);
        s.spawn(move |_| {
            log_sys(intents);
        });

        let update_cache_sys = executor(UpdatePathCacheSystem, storage);
        let pop_path_cache_sys = executor(PopPathCacheSystem, storage);
        s.spawn(move |_| {
            update_cache_sys(intents);
            pop_path_cache_sys(intents);
        });
    });
}

fn executor<'a, 'b, T, Sys>(
    mut sys: Sys,
    storage: *mut World,
) -> impl FnOnce(&'b Intents) -> () + 'a
where
    'b: 'a,
    T: 'a,
    &'a Intents: Into<&'a [T]>,
    Sys: IntentExecutionSystem<'a, Intent = T> + 'a,
{
    let storage = unsafe { &mut *storage };
    let mutable = Sys::Mut::new(storage);
    let immutable = Sys::Const::new(storage);

    move |intents| {
        sys.execute(mutable.clone(), immutable, intents.into());
        mutable.log();
    }
}

/// Remove duplicate positions and entities.
fn pre_process_move_intents(move_intents: &mut Vec<MoveIntent>) {
    macro_rules! dedupe {
        ($field: ident, $intents: ident) => {
            let len = $intents.len();
            if len < 2 {
                // 0 and 1 long vectors do not have duplicates
                return;
            }
            $intents.par_sort_unstable_by_key(|intent| intent.$field);
            // move in reverse order because we want to remove invalid intents as we move, which would be a
            // lot more expensive the other way around
            for current in (0..=len - 2).rev() {
                let last = current + 1;
                let a = &$intents[last];
                let b = &$intents[current];
                if a.$field == b.$field {
                    debug!(
                        concat!(
                            "Duplicated",
                            stringify!($field),
                            "in move intents, removing {:?}"
                        ),
                        a
                    );
                    $intents.remove(last);
                }
            }
        };
    }

    dedupe!(position, move_intents);
    dedupe!(bot, move_intents);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Axial;

    #[test]
    fn pre_process_move_intents_removes_last_dupe() {
        let mut intents = vec![
            MoveIntent {
                bot: Default::default(),
                position: Axial::new(42, 69),
            },
            MoveIntent {
                bot: Default::default(),
                position: Axial::new(42, 69),
            },
        ];

        pre_process_move_intents(&mut intents);
        assert_eq!(intents.len(), 1);
    }

    #[test]
    fn pre_process_move_intents_removes_dupe_entities() {
        let mut intents = vec![
            MoveIntent {
                bot: Default::default(),
                position: Axial::new(42, 42),
            },
            MoveIntent {
                bot: Default::default(),
                position: Axial::new(42, 69),
            },
            MoveIntent {
                bot: Default::default(),
                position: Axial::new(69, 69),
            },
        ];

        pre_process_move_intents(&mut intents);
        assert_eq!(intents.len(), 1);
    }
}
