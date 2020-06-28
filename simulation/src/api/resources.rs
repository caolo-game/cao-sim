use super::*;
use crate::components::{EntityComponent, PositionComponent, ResourceComponent};
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

    let WorldPosition { room, pos } = position.0;

    let resources = storage.view::<EntityId, ResourceComponent>();

    let room = storage
        .view::<WorldPosition, EntityComponent>()
        .reborrow()
        .table
        .get_by_id(&room)
        // search the whole room
        .ok_or_else(|| {
            warn!(
                "find_closest_resource_by_range called on invalid room {:?}",
                position
            );
            ExecutionError::InvalidArgument
        })?;

    let candidate =
        room.find_closest_by_filter(&pos, |_, entity| resources.get_by_id(&entity.0).is_some());
    match candidate {
        Some((_distance, _pos, entity)) => {
            let id = entity.0; // move out of the result to free the storage borrow
            vm.set_value(id)?;
            vm.set_value(OperationResult::Ok)?;
        }
        None => {
            debug!("No resource was found");
            vm.set_value(OperationResult::OperationFailed)?;
        }
    }
    Ok(())
}
