use super::IntentExecutionSystem;
use crate::intents::MoveIntent;
use crate::model::{
    components::{Bot, EntityComponent, PositionComponent},
    EntityId, WorldPosition,
};
use crate::storage::views::{UnsafeView, View};

pub struct MoveSystem;

impl<'a> IntentExecutionSystem<'a> for MoveSystem {
    type Mut = (UnsafeView<EntityId, PositionComponent>,);
    type Const = (
        View<'a, EntityId, Bot>,
        View<'a, WorldPosition, EntityComponent>,
    );
    type Intent = MoveIntent;

    fn execute(
        &mut self,
        (mut positions,): Self::Mut,
        (bots, pos_entities): Self::Const,
        intents: &[Self::Intent],
    ) {
        for intent in intents {
            debug!("Moving bot[{:?}] to {:?}", intent.bot, intent.position);

            if bots.get_by_id(&intent.bot).is_none() {
                debug!("Bot by id {:?} does not exist", intent.bot);
                continue;
            }

            let current_pos = match positions.get_by_id(&intent.bot) {
                Some(current_pos) => current_pos,
                None => {
                    warn!(
                        "Bot {:?} attempts to move but has no position component",
                        intent.bot
                    );
                    continue;
                }
            };

            if pos_entities
                .table
                .get_by_id(&current_pos.0.room)
                .and_then(|room| room.get_by_id(&intent.position))
                .is_some()
            {
                debug!("Occupied {:?} ", intent.position);
                continue;
            }

            unsafe {
                positions.as_mut().insert_or_update(
                    intent.bot,
                    PositionComponent(WorldPosition {
                        room: current_pos.0.room,
                        pos: intent.position,
                    }),
                );
            }

            debug!("Move successful");
        }
    }
}
