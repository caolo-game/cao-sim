use super::IntentExecutionSystem;
use crate::components::{Bot, EntityComponent, PositionComponent};
use crate::intents::MoveIntent;
use crate::model::{EntityId, WorldPosition};
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
}
