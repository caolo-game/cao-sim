use super::System;
use crate::components::{Bot, EntityComponent, PositionComponent};
use crate::intents::Intents;
use crate::model::{EntityId, WorldPosition};
use crate::profile;
use crate::storage::views::{UnsafeView, UnwrapView, View};
use log::trace;

pub struct MoveSystem;

impl<'a> System<'a> for MoveSystem {
    type Mut = (UnsafeView<EntityId, PositionComponent>,);
    type Const = (
        View<'a, EntityId, Bot>,
        View<'a, WorldPosition, EntityComponent>,
        UnwrapView<'a, Intents>,
    );

    fn update(&mut self, (mut positions,): Self::Mut, (bots, pos_entities, intents): Self::Const) {
        profile!(" MoveSystem update");
        let intents = &intents.move_intent;
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
