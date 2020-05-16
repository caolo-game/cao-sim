use super::System;
use crate::model::{
    components::{EntityComponent, PositionComponent},
    geometry::Axial,
    EntityId,
};
use crate::storage::views::{UnsafeView, View};

pub struct PositionSystem;

impl<'a> System<'a> for PositionSystem {
    type Mut = UnsafeView<Axial, EntityComponent>;
    type Const = View<'a, EntityId, PositionComponent>;

    /// Reset the entity positions table
    fn update(&mut self, mut position_entities: Self::Mut, positions: Self::Const) {
        debug!("update positions system called");

        unsafe {
            position_entities.as_mut().clear();
            position_entities
                .as_mut()
                .extend(
                    positions
                        .iter()
                        .map(|(id, pos)| (pos.0, EntityComponent(id))),
                )
                .map_err(|e| {
                    error!("Failed to rebuild position_entities table {:?}", e);
                })
                .unwrap_or_default();
        }

        debug!("update positions system done");
    }
}
