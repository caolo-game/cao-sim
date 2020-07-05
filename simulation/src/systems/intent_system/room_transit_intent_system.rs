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

            // the bridge on the other side
            let bridge = match room_connections
                .get_by_id(&intent.target_room)
                .and_then(|c| {
                    let direction = current_pos.0.room - intent.target_room.0;
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
                    current_abs.hex_distance(pos.absolute(props.radius as i32)) <= 1 &&
                    terrain.get_by_id(&pos).map(|t| t.0 == TileTerrainType::Bridge).unwrap_or(false)
                }).collect();

            if candidates.is_empty() {
                error!("Could not find an acceptable bridge candidate");
                continue;
            }

            let new_pos = match get_valid_transits(
                current_pos.0,
                intent.target_room,
                (terrain, pos_entities, room_connections, room_properties),
            ) {
                Ok(candidates) => candidates[0],
                Err(TransitError::InternalError(e)) => {
                    error!("Failed to find transit {:?}", e);
                    continue;
                }
                Err(TransitError::NotFound) => {
                    trace!("{:?} All candidates are occupied", intent.bot);
                    continue;
                }
            };

            unsafe {
                positions
                    .as_mut()
                    .insert_or_update(intent.bot, PositionComponent(new_pos));
            }

            trace!("Transitioning {:?} successful", intent.bot);
        }
    }
}

pub enum TransitError {
    InternalError(anyhow::Error),
    NotFound,
}

/// If the result is `Ok` it will contain at least 1 item
pub fn get_valid_transits(
    current_pos: WorldPosition,
    target_room: Room,
    (terrain, entities, room_connections, room_properties): (
        View<WorldPosition, TerrainComponent>,
        View<WorldPosition, EntityComponent>,
        View<Room, RoomConnections>,
        View<EmptyKey, RoomProperties>,
    ),
) -> Result<ArrayVec<[WorldPosition; 3]>, TransitError> {
    // from a bridge the bot can reach at least 1 and at most 3 tiles
    // try to find an empty one and move the bot there, otherwise the move fails

    // to obtain the edge we require the bot's current pos (room)
    // the room_connection

    // the bridge on the other side
    let bridge = match room_connections.get_by_id(&target_room).and_then(|c| {
        let direction = current_pos.room - target_room.0;
        let ind = Axial::neighbour_index(direction)?;
        c.0[ind].as_ref()
    }) {
        Some(conn) => conn,
        None => {
            return Err(TransitError::InternalError(anyhow::Error::msg(format!(
                "Room {:?} has no (valid) connections",
                target_room
            ))));
        }
    };
    // to obtain the pos we need an edge point that's absolute position is 1 away from
    // current pos and is uncontested.
    let props = room_properties.unwrap_value();

    let current_abs = current_pos.absolute(props.radius as i32);

    // if this fails once it will fail always, so we'll just panic
    let candidates: ArrayVec<[_; 3]> = iter_edge(props.center, props.radius, bridge)
        .expect("Failed to iter the edge")
        .filter(|pos| {
            let pos = WorldPosition {
                room: target_room.0,
                pos: *pos,
            };
            // the candidate terrain must be a Bridge and must be 1 tile away
            current_abs.hex_distance(pos.absolute(props.radius as i32)) == 1
                && terrain
                    .get_by_id(&pos)
                    .map(|t| t.0 == TileTerrainType::Bridge)
                    .unwrap_or(false)
        })
        .map(|pos| WorldPosition {
            room: target_room.0,
            pos,
        })
        .collect();

    if candidates.is_empty() {
        return Err(TransitError::InternalError(anyhow::Error::msg(format!(
            "Could not find an acceptable bridge candidate"
        ))));
    }

    let candidates: ArrayVec<[_; 3]> = candidates
        .into_iter()
        .filter(|p| !entities.contains_key(p))
        .collect();

    if candidates.is_empty() {
        return Err(TransitError::NotFound);
    }

    debug_assert!(candidates.len() >= 1);
    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::*;
    use crate::init_inmemory_storage;
    use crate::query;
    use crate::storage::views::{FromWorld, FromWorldMut};

    #[test]
    fn can_execute_valid_intents() {
        let mut sys = RoomTransitSystem;

        let mut storage = init_inmemory_storage();

        let bot_id = storage.insert_entity();

        let botroom = Axial::new(1, 0);
        let target_room = Axial::new(1, 1);

        let mut connections = RoomConnections::default();
        // this is the bridge in the target_room
        let bridge = RoomConnection {
            direction: botroom - target_room,
            offset_start: 0,
            offset_end: 0,
        };
        connections.0[Axial::neighbour_index(botroom - target_room).unwrap()] =
            Some(bridge.clone());

        query!(
            mutate
            storage
            {
                EmptyKey, RoomProperties,
                    .update(Some(RoomProperties{
                        radius: 16,
                        center: Axial::new(16,16)
                    }));
                EntityId, Bot, .insert_or_update(bot_id, Bot);
                Room, RoomConnections, .insert_or_update(Room(Axial::new(1,1)), connections).unwrap();
                EntityId, PositionComponent,
                    .insert_or_update(bot_id, PositionComponent(WorldPosition{
                        room: botroom,
                        pos: iter_edge(Axial::new(16,16), 16, &RoomConnection {
                            direction: target_room - botroom,
                            offset_start: 0,
                            offset_end: 0,
                        })
                            .unwrap().next().unwrap()
                    }));

                WorldPosition, TerrainComponent,
                    .extend_rooms(
                        [
                        Room(botroom),
                        Room(target_room)
                        ].iter().cloned()
                    )
                    .unwrap();
                WorldPosition, TerrainComponent,
                    .extend_from_slice(
                        &mut
                            iter_edge(Axial::new(16,16), 16, &bridge).expect("Failed to iter the edge")
                            .map(|pos|{
                                (
                                    WorldPosition {
                                        room: target_room,
                                        pos
                                    },
                                    TerrainComponent(TileTerrainType::Bridge)
                                )
                            })
                            .collect::<Vec<_>>())
                    .unwrap();
            }
        );

        let intents = vec![RoomTransitIntent {
            bot: bot_id,
            target_room: Room(target_room),
        }];

        sys.execute(
            FromWorldMut::new(&mut storage),
            FromWorld::new(&storage),
            &intents,
        );

        let PositionComponent(botpos) = storage
            .view::<EntityId, PositionComponent>()
            .get_by_id(&bot_id)
            .cloned()
            .expect("Failed to get the bot's position");

        assert_eq!(botpos.room, target_room);
    }
}
