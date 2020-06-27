use super::*;
use crate::components::{EntityComponent, PositionComponent, ResourceComponent, RoomProperties};
use crate::geometry::Axial;
use crate::model::indices::EmptyKey;
use crate::model::WorldPosition;
use crate::profile;

/// Return OperationResult and an EntityId if the Operation succeeded
pub fn find_closest_resource_by_range(
    vm: &mut VM<ScriptExecutionData>,
    _: (),
) -> Result<(), ExecutionError> {
    profile!(trace "find_closest_resource_by_range");

    let entity_id = vm.get_aux().entity_id;
    let storage = vm.get_aux().storage();

    let position = match storage
        .view::<EntityId, PositionComponent>()
        .reborrow()
        .get_by_id(&entity_id)
    {
        Some(p) => p,
        None => {
            debug!("{:?} has no PositionComponent", entity_id);
            vm.set_value(OperationResult::InvalidInput)?;
            return Ok(());
        }
    };

    let radius = storage
        .view::<EmptyKey, RoomProperties>()
        .unwrap_value()
        .radius;

    let WorldPosition { room, pos } = position.0;

    let mut candidate: Option<(Axial, EntityId)> = None;

    let resources = storage.view::<EntityId, ResourceComponent>();
    storage
        .view::<WorldPosition, EntityComponent>()
        .reborrow()
        .table
        .get_by_id(&room)
        // search the whole room
        .map(|room| {
            let candidate = &mut candidate;
            room.query_range(
                &Axial::new(radius as i32, radius as i32),
                radius,
                &mut |pp, entity| {
                    let resource = resources.get_by_id(&entity.0);
                    // filter only entities that are 'resources'
                    if resource.is_some() {
                        // find the one with min distance
                        *candidate = candidate
                            .map(|current| {
                                if pos.hex_distance(current.0) < pos.hex_distance(pp) {
                                    current
                                } else {
                                    (pp, entity.0)
                                }
                            })
                            .or_else(|| Some((pp, entity.0)));
                    }
                },
            )
        })
        .ok_or_else(|| {
            warn!(
                "find_closest_resource_by_range called on invalid room {:?}",
                position
            );
            ExecutionError::InvalidArgument
        })?;

    match candidate {
        None => {
            debug!("No resource was found");
            vm.set_value(OperationResult::OperationFailed)?;
        }
        Some((_, entity)) => {
            let id = entity.0; // move out of the result to free the storage borrow
            vm.set_value(id)?;
            vm.set_value(OperationResult::Ok)?;
        }
    }
    Ok(())
}
