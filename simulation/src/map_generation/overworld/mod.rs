//! Generate high level room layout
//!
mod params;
pub use params::*;

use crate::model::components::{RoomComponent, RoomConnection, RoomConnections};
use crate::model::geometry::{Axial, Hexagon};
use crate::model::Room;
use crate::storage::views::UnsafeView;
use crate::tables::morton::{ExtendFailure, MortonTable};
use rand::{rngs::SmallRng, thread_rng, Rng, RngCore, SeedableRng};
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum OverworldGenerationError {
    #[error("Can not place {number_of_rooms} rooms in an area with radius of {radius}")]
    BadRadius { number_of_rooms: u32, radius: u32 },

    #[error("Failed to build Room table: {0:?}")]
    ExtendFail(ExtendFailure<Room>),

    #[error("Failed to build Room weight table: {0:?}")]
    WeightMapInitFail(ExtendFailure<Axial>),
}

/// Insert the given number of rooms in the given radius (where the unit is a room).
/// [ ] TODO: remove some nodes to produce less dense maps?
/// [ ] TODO: resource map?
/// [ ] TODO: political map?
/// [ ] TODO: parallellism?
pub fn generate_room_layout(
    OverworldGenerationParams {
        radius,
        room_radius,
        min_bridge_len,
        max_bridge_len,
        seed,
    }: &OverworldGenerationParams,
    (mut rooms, mut connections): (
        UnsafeView<Room, RoomComponent>,
        UnsafeView<Room, RoomConnections>,
    ),
) -> Result<(), OverworldGenerationError> {
    let seed = seed.unwrap_or_else(|| {
        let mut bytes = [0; 16];
        thread_rng().fill_bytes(&mut bytes);
        bytes
    });

    let mut rng = SmallRng::from_seed(seed);

    let radius = *radius as i32;
    let center = Axial::new(radius, radius);
    let bounds = Hexagon { center, radius };

    // Init the grid
    unsafe {
        let rooms = rooms.as_mut();
        rooms.clear();
        rooms
            .extend(bounds.iter_points().map(|p| {
                (
                    Room(p),
                    RoomComponent {
                        radius: radius as u32,
                        center: Axial::new(radius, radius),
                    },
                )
            }))
            .map_err(OverworldGenerationError::ExtendFail)?;

        let connections = connections.as_mut();
        connections.clear();
        connections
            .extend(bounds.iter_points().map(|p| (Room(p), Default::default())))
            .map_err(OverworldGenerationError::ExtendFail)?;
    }

    debug!("Building connections");

    // loosely running the Erdos - Runyi model
    let connection_weights = MortonTable::from_iterator(bounds.iter_points().map(|p| {
        let weight = rng.gen_range(0.1, 1.0);
        (p, weight)
    }))
    .map_err(OverworldGenerationError::WeightMapInitFail)?;

    for point in bounds.iter_points() {
        update_room_connections(
            *room_radius,
            *min_bridge_len,
            *max_bridge_len,
            point,
            &connection_weights,
            &mut rng,
            connections,
        );
    }
    debug!("Building connections done");

    // TODO: insert more connections if the graph is not fully connected

    Ok(())
}

fn update_room_connections(
    room_radius: u32,
    min_bridge_len: u32,
    max_bridge_len: u32,
    point: Axial,
    connection_weights: &MortonTable<Axial, f32>,
    rng: &mut impl Rng,
    mut connections: UnsafeView<Room, RoomConnections>,
) {
    let w = rng.gen_range(0.0, 0.55);
    // let w = rng.gen_range(0.0, 0.95);
    let mut to_connect = [None; 6];
    connection_weights.query_range(&point, 3, &mut |p, weight| {
        if w <= *weight {
            let n = p - point;
            if let Some(i) = Axial::neighbour_index(n) {
                to_connect[i] = Some(n);
            }
        }
    });

    let current_connections = {
        let to_connect = &mut to_connect[..];
        unsafe { connections.as_mut() }.update_with(
            &Room(point),
            |RoomConnections(ref mut conn)| {
                for (i, c) in to_connect.iter_mut().enumerate() {
                    if conn[i].is_none() && c.is_some() {
                        let bridge_len = rng.gen_range(min_bridge_len, max_bridge_len);
                        let padding = room_radius - bridge_len;

                        let offset_start = rng.gen_range(0, padding);
                        let offset_end = padding - offset_start;

                        // this is a new connection
                        conn[i] = c.map(|c| RoomConnection {
                            direction: c,
                            offset_start,
                            offset_end,
                        });
                    } else {
                        // if we don't have to update this posision then set it to None so we don't
                        // attempt to update the neighbour later.
                        *c = None;
                    }
                }
            },
        )
    }
    .expect("expected the current room to have connection")
    .clone();

    for neighbour in current_connections
        .0
        .iter()
        .filter_map(|n| n.as_ref())
        .cloned()
    {
        unsafe { connections.as_mut() }.update_with(&Room(point + neighbour.direction), |conn| {
            let inverse = neighbour.direction * -1;
            let i = Axial::neighbour_index(inverse)
                .expect("expected neighbour inverse to be a valid neighbour posision");
            // this one's offsets are the current room's inverse
            let offset_end = neighbour.offset_start;
            let offset_start = neighbour.offset_end;
            conn.0[i] = Some(RoomConnection {
                direction: inverse,
                offset_start,
                offset_end,
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Once;

    static INIT: Once = Once::new();

    fn setup() {
        INIT.call_once(|| {
            env_logger::init();
        });
    }

    #[test]
    fn overworld_connections_are_valid() {
        setup();

        let mut rooms = MortonTable::new();
        let mut connections = MortonTable::new();

        let params = OverworldGenerationParams::builder()
            .with_radius(12)
            .with_room_radius(16)
            .with_min_bridge_len(3)
            .with_max_bridge_len(12)
            .build()
            .unwrap();
        generate_room_layout(
            &params,
            (
                UnsafeView::from_table(&mut rooms),
                UnsafeView::from_table(&mut connections),
            ),
        )
        .unwrap();

        assert_eq!(rooms.len(), connections.len());

        // for each connection of the room test if the corresponding connection of the neighbour
        // is valid.
        for (Room(room), RoomConnections(ref room_conn)) in connections.iter() {
            for conn in room_conn.iter().filter_map(|x| x.as_ref()) {
                let RoomConnections(ref conn_pairs) = connections
                    .get_by_id(&Room(room + conn.direction))
                    .expect("Expected the neighbour to be in the connections table");

                let i = Axial::neighbour_index(conn.direction * -1).unwrap();
                let conn_pair = conn_pairs[i]
                    .as_ref()
                    .expect("The pair connection was not found");

                assert_eq!(conn_pair.direction, conn.direction * -1);
                assert_eq!(conn_pair.offset_end, conn.offset_start);
                assert_eq!(conn_pair.offset_start, conn.offset_end);
            }
        }
    }
}