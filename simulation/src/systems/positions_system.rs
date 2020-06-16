use super::System;
use crate::
    components::{EntityComponent, PositionComponent};
use crate::model::{
    EntityId, WorldPosition,
};
use crate::storage::views::{UnsafeView, View};

pub struct PositionSystem;

impl<'a> System<'a> for PositionSystem {
    type Mut = UnsafeView<WorldPosition, EntityComponent>;
    type Const = View<'a, EntityId, PositionComponent>;

    /// Reset the entity positions table
    fn update(&mut self, mut position_entities: Self::Mut, positions: Self::Const) {
        debug!("update positions system called");

        let mut positions = positions
            .iter()
            .map(|(id, pos)| (pos.0, EntityComponent(id)))
            .collect::<Vec<_>>();

        unsafe {
            position_entities.as_mut().clear();
            position_entities
                .as_mut()
                .extend_from_slice(&mut positions)
                .map_err(|e| {
                    error!("Failed to rebuild position_entities table {:?}", e);
                })
                .ok();
        }

        debug!("update positions system done");
    }
}
