//! Represents the bot's intent to move over to a neighbouring room.
//! The bot must be standing on the connecting bridge to initiate the move.
//!
//! Currently only bots are allowed to move!
//!
use crate::components::{
    self, PositionComponent, RoomConnections, RoomProperties, TerrainComponent,
};
use crate::geometry::Axial;
use crate::model::{
    self, terrain::TileTerrainType, EmptyKey, EntityId, OperationResult, Room, WorldPosition,
};
use crate::storage::views::View;

#[derive(Debug, Clone)]
pub struct RoomTransitIntent {
    pub bot: EntityId,
    pub target_room: Room,
}

type CheckInput<'a> = (
    View<'a, EntityId, components::OwnedEntity>,
    View<'a, EntityId, PositionComponent>,
    View<'a, EntityId, components::Bot>,
    View<'a, WorldPosition, TerrainComponent>,
    View<'a, Room, RoomConnections>,
    View<'a, EmptyKey, RoomProperties>,
);

pub fn check_transit_intent(
    intent: &RoomTransitIntent,
    user_id: model::UserId,
    (
        owners_table,
        positions_table,
        bots_table,
        terrain_table,
        room_connections_table,
        room_properties,
    ): CheckInput,
) -> OperationResult {
    let id = intent.bot;
    match bots_table.get_by_id(&id) {
        Some(_) => {
            let owner_id = owners_table.get_by_id(&id);
            if owner_id.map(|id| id.owner_id != user_id).unwrap_or(true) {
                return OperationResult::NotOwner;
            }
        }
        None => return OperationResult::InvalidInput,
    };

    let pos = match positions_table.get_by_id(&id) {
        Some(pos) => pos,
        None => {
            warn!("Bot has no position");
            return OperationResult::InvalidInput;
        }
    };
    let room_radius = room_properties.unwrap_value().radius;
    let dist = pos
        .0
        .pos
        .hex_distance(Axial::new(room_radius as i32, room_radius as i32));
    if room_radius != dist {
        // bot can not be standing on an edge
        return OperationResult::InvalidInput;
    }

    let is_on_bridge_check = terrain_table
        .get_by_id(&pos.0)
        .ok_or_else(|| {
            error!("Bots' position {:?} was not in terrain", pos);
            OperationResult::OperationFailed
        })
        .and_then(|TerrainComponent(t)| {
            if *t == TileTerrainType::Bridge {
                Ok(())
            } else {
                trace!("Bot {:?} is not standing on a bridge", id);
                Err(OperationResult::InvalidInput)
            }
        });

    if let Err(e) = is_on_bridge_check {
        return e;
    }
    // bot is standing on a bridge

    let delta = intent.target_room.0 - pos.0.room;
    let neighbour_index = match Axial::neighbour_index(delta) {
        Some(i) => i,
        None => {
            trace!("Intended room is not a neighbour");
            return OperationResult::InvalidInput;
        }
    };
    let connections = match room_connections_table.get_by_id(&Room(pos.0.room)) {
        Some(connections) => connections,
        None => {
            error!("Room {:?} has no connections", pos.0.room);
            return OperationResult::OperationFailed;
        }
    };

    if connections.0[neighbour_index].is_none() {
        error!(
            "Rooms {:?} and {:?} are not connected, even tough is_on_bridge_check passed",
            pos.0.room, intent
        );
        return OperationResult::OperationFailed;
    }

    OperationResult::Ok
}
