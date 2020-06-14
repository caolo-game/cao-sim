//! ## Terminology:
//! - overworld: the large-scale overview of the map.
//! - room: a self-contained slice of the map. Hexagon shaped.
//!
pub mod overworld;
pub mod room;

use self::overworld::{generate_room_layout, OverworldGenerationError, OverworldGenerationParams};
use self::room::{generate_room, RoomGenerationError, RoomGenerationParams};
use crate::model::components::{RoomComponent, RoomConnections, TerrainComponent};
use crate::model::{Room, WorldPosition};
use crate::storage::views::UnsafeView;
use crate::tables::morton::MortonTable;
use arrayvec::ArrayVec;
use rand::{rngs::SmallRng, thread_rng, RngCore, SeedableRng};
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum MapGenError {
    #[error("Failed to generate room: {err}")]
    RoomGenerationError {
        err: RoomGenerationError,
        room: Room,
    },

    #[error("Failed to generate overworld: {err}")]
    OverworldGenerationError { err: OverworldGenerationError },
}

pub fn generate_full_map(
    overworld_params: &OverworldGenerationParams,
    room_params: &RoomGenerationParams,
    seed: Option<[u8; 16]>,
    (mut terrain, rooms, connections): (
        UnsafeView<WorldPosition, TerrainComponent>,
        UnsafeView<Room, RoomComponent>,
        UnsafeView<Room, RoomConnections>,
    ),
) -> Result<(), MapGenError> {
    let seed = seed.unwrap_or_else(|| {
        let mut bytes = [0; 16];
        thread_rng().fill_bytes(&mut bytes);
        bytes
    });
    let mut rng = SmallRng::from_seed(seed);
    generate_room_layout(overworld_params, &mut rng, (rooms, connections))
        .map_err(|err| MapGenError::OverworldGenerationError { err })?;

    let terrain_tables = rooms.iter().try_fold(
        Vec::with_capacity(rooms.len()),
        |mut terrain_tables, (room, _)| {
            // TODO: do this in parallel?
            let mut terrain_table = MortonTable::new();
            let connections = connections
                .get_by_id(&room)
                .expect("Expected just built room to have connections");
            let connections = connections
                .0
                .iter()
                .filter_map(|c| c.as_ref())
                .cloned()
                .collect::<ArrayVec<[_; 6]>>();
            generate_room(
                room_params,
                connections.as_slice(),
                &mut rng,
                (UnsafeView::from_table(&mut terrain_table),),
            )
            .map_err(|err| MapGenError::RoomGenerationError { err, room })?;

            terrain_tables.push((room.0, terrain_table));
            Ok(terrain_tables)
        },
    )?;
    unsafe { &mut terrain.as_mut().table }
        .extend(terrain_tables.into_iter())
        .expect("expected to be able to insert the room terrain tables");
    Ok(())
}
