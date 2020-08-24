use crate::components::{EntityComponent, PositionComponent};
use crate::indices::{EntityId, WorldPosition};
use crate::profile;
use crate::storage::views::{UnsafeView, View, WorldLogger};
use slog::{debug, error};

type Mut = UnsafeView<WorldPosition, EntityComponent>;
type Const<'a> = (View<'a, EntityId, PositionComponent>, WorldLogger);

/// Reset the entity positions table
pub fn update(mut position_entities: Mut, (positions, WorldLogger(logger)): Const) {
    profile!("PositionSystem update");
    debug!(logger, "update positions system called");

    let mut positions = positions
        .iter()
        .map(|(id, PositionComponent(pos))| (*pos, EntityComponent(id)))
        .collect::<Vec<_>>();

    unsafe {
        position_entities.as_mut().clear();
        position_entities
            .as_mut()
            .extend_from_slice(positions.as_mut_slice())
            .map_err(|e| {
                error!(logger, "Failed to rebuild position_entities table {:?}", e);
            })
            .ok();
    }

    debug!(logger, "update positions system done");
}
