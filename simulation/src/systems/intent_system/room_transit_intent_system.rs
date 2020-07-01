use super::IntentExecutionSystem;
use crate::components::{
    Bot, EntityComponent, PositionComponent, RoomConnections, RoomProperties, TerrainComponent,
};
use crate::geometry::Axial;
use crate::intents::RoomTransitIntent;
use crate::map_generation::room::iter_edge;
use crate::model::terrain::TileTerrainType;
use crate::model::{EmptyKey, EntityId, Room, WorldPosition};
use crate::storage::views::{UnsafeView, View};
use arrayvec::ArrayVec;

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
                error!("Bot by id {:?} does not exist", intent.bot);
                continue;
            }

            // from a bridge the bot can reach at least 1 and at most 3 tiles
            // try to find an empty one and move the bot there, otherwise the move fails

            // to obtain the edge we require the bot's current pos (room)
            // the room_connection

            let current_pos = match positions.get_by_id(&intent.bot) {
                Some(pos) => pos,
                None => {
                    error!("Bot by id {:?} has no position", intent.bot);
                    continue;
                }
            };

            let bridge = match room_connections
                .get_by_id(&intent.target_room)
                .and_then(|c| {
                    let direction = intent.target_room.0 - current_pos.0.room;
                    let ind = Axial::neighbour_index(direction)?;
                    c.0[ind].as_ref()
                }) {
                Some(conn) => conn,
                None => {
                    error!("Room {:?} has no (valid) connections", intent.target_room);
                    continue;
                }
            };
            // to obtain the pos we need an edge point that's absolute position is 1 away from
            // current pos and is uncontested.
            let props = room_properties.unwrap_value();

            let current_abs = current_pos.0.absolute(props.radius as i32);

            let candidates: ArrayVec<[_; 3]> =
                // if this fails once it will fail always, so we'll just panic
                iter_edge(props.center, props.radius, bridge).expect("Failed to iter the edge")
                .filter(|pos|{
                    let pos = WorldPosition{
                        room: intent.target_room.0,
                        pos: *pos
                    };
                    // the candidate terrain must be a Bridge and must be within 1 tiles
                    terrain.get_by_id(&pos).map(|TerrainComponent(t)| *t == TileTerrainType::Bridge).unwrap_or(false)
                        &&
                    current_abs.hex_distance(pos .absolute(props.radius as i32)) <= 1
                }).collect();

            if candidates.is_empty() {
                error!("Could not find an acceptable bridge candidate");
                continue;
            }

            // find a candidate that is not occupied
            let new_pos = candidates.iter().cloned().find(|pos| {
                pos_entities
                    .get_by_id(&WorldPosition {
                        room: intent.target_room.0,
                        pos: *pos,
                    })
                    .is_none()
            });
            match new_pos {
                Some(new_pos) => unsafe {
                    positions.as_mut().insert_or_update(
                        intent.bot,
                        PositionComponent(WorldPosition {
                            room: intent.target_room.0,
                            pos: new_pos,
                        }),
                    );
                },
                None => {
                    trace!("{:?} All candidates are occupied", intent.bot);
                    continue;
                }
            }

            trace!("Transitioning {:?} successful", intent.bot);
        }
    }
}
