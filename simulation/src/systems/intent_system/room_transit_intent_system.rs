use super::IntentExecutionSystem;
use crate::components::{
    Bot, EntityComponent, PositionComponent, RoomConnections, RoomProperties, TerrainComponent,
};
use crate::map_generation::room::iter_edge;
use crate::intents::RoomTransitIntent;
use crate::model::{EmptyKey, EntityId, Room, WorldPosition};
use crate::storage::views::{UnsafeView, View};

pub struct RoomTransitSystem;

impl<'a> IntentExecutionSystem<'a> for RoomTransitSystem {
    type Mut = (UnsafeView<EntityId, PositionComponent>,);
    type Const = (
        View<'a, EntityId, Bot>,
        View<'a, WorldPosition, EntityComponent>,
        View<'a, WorldPosition, TerrainComponent>,
        View<'a, Room, RoomConnections>,
        View<'a, EmptyKey, RoomProperties>,
    );
    type Intent = RoomTransitIntent;

    fn execute(
        &mut self,
        (mut positions,): Self::Mut,
        (bots, pos_entities, terrain, room_connections, room_properties): Self::Const,
        intents: &[Self::Intent],
    ) {
        for intent in intents {
            trace!(
                "Transitioning bot[{:?}] to {:?}",
                intent.bot,
                intent.target_room
            );

            if bots.get_by_id(&intent.bot).is_none() {
                trace!("Bot by id {:?} does not exist", intent.bot);
                continue;
            }

            // from a bridge the bot can reach at least 1 and at most 3 tiles
            // try to find an empty one and move the bot there, otherwise the move fails

            // to obtain the edge we require the bot's current pos (room)
            // the room_connection

            // to obtain the pos we need an edge point that's absolute position is 1 away from
            // current pos and is uncontested.

            // let candidates = [];
            unimplemented!();

            // if pos_entities.get_by_id(&intent.position).is_some() {
            //     trace!("Occupied {:?} ", intent.position);
            //     continue;
            // }
            //
            // unsafe {
            //     positions
            //         .as_mut()
            //         .insert_or_update(intent.bot, PositionComponent(intent.position));
            // }

            trace!("Transitioning successful");
        }
    }
}
