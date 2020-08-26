use crate::components::{Bot, EntityComponent, PositionComponent};
use crate::indices::{EntityId, WorldPosition};
use crate::intents::{Intents, MoveIntent};
use crate::profile;
use crate::storage::views::{UnsafeView, UnwrapViewMut, View, WorldLogger};
use rayon::prelude::*;
use slog::{debug, trace, Logger};

type Mut = (
    UnsafeView<EntityId, PositionComponent>,
    UnwrapViewMut<Intents<MoveIntent>>,
);
type Const<'a> = (
    View<'a, EntityId, Bot>,
    View<'a, WorldPosition, EntityComponent>,
    WorldLogger,
);

pub fn update((mut positions, mut intents): Mut, (bots, pos_entities, WorldLogger(logger)): Const) {
    profile!(" MoveSystem update");

    pre_process_move_intents(&logger, &mut intents.0);
    for intent in intents.iter() {
        trace!(
            logger,
            "Moving bot[{:?}] to {:?}",
            intent.bot,
            intent.position
        );

        if bots.get_by_id(&intent.bot).is_none() {
            trace!(logger, "Bot by id {:?} does not exist", intent.bot);
            continue;
        }

        if pos_entities.get_by_id(&intent.position).is_some() {
            trace!(logger, "Occupied {:?} ", intent.position);
            continue;
        }

        positions.insert_or_update(intent.bot, PositionComponent(intent.position));

        trace!(logger, "Move successful");
    }
}

/// Remove duplicate positions.
/// We assume that there are no duplicated entities
fn pre_process_move_intents(logger: &Logger, move_intents: &mut Vec<MoveIntent>) {
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
            debug!(
                logger,
                "Duplicated position in move intents, removing {:?}", a
            );
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
    use crate::utils::*;

    #[test]
    fn pre_process_move_intents_removes_last_dupe() {
        setup_testing();
        let logger = test_logger();

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

        pre_process_move_intents(&logger, &mut intents);
        assert_eq!(intents.len(), 2);
        assert_ne!(intents[0].position, intents[1].position);
    }
}
