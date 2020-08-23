use crate::components::{Bot, EntityComponent, PositionComponent};
use crate::indices::{EntityId, WorldPosition};
use crate::intents::{Intents, MoveIntent};
use crate::profile;
use crate::storage::views::{UnsafeView, UnwrapViewMut, View};
use log::{debug, trace};
use rayon::prelude::*;

type Mut = (
    UnsafeView<EntityId, PositionComponent>,
    UnwrapViewMut<Intents<MoveIntent>>,
);
type Const<'a> = (
    View<'a, EntityId, Bot>,
    View<'a, WorldPosition, EntityComponent>,
);

pub fn update((mut positions, mut intents): Mut, (bots, pos_entities): Const) {
    profile!(" MoveSystem update");

    pre_process_move_intents(&mut intents.0);
    for intent in intents.iter() {
        trace!("Moving bot[{:?}] to {:?}", intent.bot, intent.position);

        if bots.get_by_id(&intent.bot).is_none() {
            trace!("Bot by id {:?} does not exist", intent.bot);
            continue;
        }

        if pos_entities.get_by_id(&intent.position).is_some() {
            trace!("Occupied {:?} ", intent.position);
            continue;
        }

        unsafe {
            positions
                .as_mut()
                .insert_or_update(intent.bot, PositionComponent(intent.position));
        }

        trace!("Move successful");
    }
}

/// Remove duplicate positions.
/// We assume that there are no duplicated entities
fn pre_process_move_intents(move_intents: &mut Vec<MoveIntent>) {
    profile!("pre_process_move_intents");

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
    use crate::indices::EntityId;
    use crate::indices::WorldPosition;

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
