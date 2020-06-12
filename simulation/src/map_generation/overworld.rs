//! Generate high level room layout
//!
use crate::model::components::{RoomComponent, RoomConnection, RoomConnections};
use crate::model::geometry::{Axial, Hexagon};
use crate::model::Room;
use crate::storage::views::{UnsafeView, View};
use crate::tables::morton::{ExtendFailure, MortonTable};
use arrayvec::ArrayVec;
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
pub fn generate_room_layout(
    number_of_rooms: u32,
    radius: u32,
    room_radius: u32,
    min_bridge_len: u32,
    max_bridge_len: u32,
    seed: Option<[u8; 16]>,
    (mut rooms, mut connections): (
        UnsafeView<Room, RoomComponent>,
        UnsafeView<Room, RoomConnections>,
    ),
) -> Result<(), OverworldGenerationError> {
    if number_of_rooms > radius * radius {
        return Err(OverworldGenerationError::BadRadius {
            radius,
            number_of_rooms,
        });
    }

    let seed = seed.unwrap_or_else(|| {
        let mut bytes = [0; 16];
        thread_rng().fill_bytes(&mut bytes);
        bytes
    });

    let mut rng = SmallRng::from_seed(seed);

    let radius = radius as i32;
    let center = Axial::new(radius, radius);
    let bounds = Hexagon { center, radius };

    // Init the grid
    unsafe {
        let rooms = rooms.as_mut();
        rooms.clear();
        rooms
            .extend(bounds.iter_points().map(|p| (Room(p), RoomComponent)))
            .map_err(OverworldGenerationError::ExtendFail)?;

        let connections = connections.as_mut();
        connections.clear();
        connections
            .extend(bounds.iter_points().map(|p| (Room(p), Default::default())))
            .map_err(OverworldGenerationError::ExtendFail)?;
    }

    debug!("Running Erdos - Renyi model");
    let room_weights =
        MortonTable::from_iterator(bounds.iter_points().map(|p| (p, rng.gen_range(0.1, 1.0))))
            .map_err(OverworldGenerationError::WeightMapInitFail)?;

    for point in bounds.iter_points() {
        let w = rng.gen_range(0.0, 0.95);

        let mut to_connect = ArrayVec::<[Axial; 6]>::new();
        let bounds = Hexagon {
            center: point,
            radius: 1,
        };
        room_weights.query_range(&point, 3, &mut |p, weight| {
            if p != point && bounds.contains(&p) && w <= *weight {
                to_connect.push(p - point);
            }
        });
    }
    debug!("Running Erdos - Renyi model done");

    unimplemented!()
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
    fn room_layout_is_not_homogeneous() {
        setup();

        let mut rooms = MortonTable::new();
        let mut connections = MortonTable::new();
        generate_room_layout(
            5,
            12,
            16,
            3,
            12,
            None,
            (
                UnsafeView::from_table(&mut rooms),
                UnsafeView::from_table(&mut connections),
            ),
        )
        .unwrap();
    }
}
