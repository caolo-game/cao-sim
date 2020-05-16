use super::*;
use crate::model::components::{EntityComponent, PositionComponent, ResourceComponent};
use crate::model::WorldPosition;
use crate::profile;

pub const MAX_SEARCH_RADIUS: u32 = 256;

/// Return OperationResult and an EntityId if the Operation succeeded
pub fn find_closest_resource_by_range(
    vm: &mut VM<ScriptExecutionData>,
    _: (),
) -> Result<(), ExecutionError> {
    profile!("find_closest_resource_by_range");

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

    let mut candidates = Vec::with_capacity(MAX_SEARCH_RADIUS as usize * 2);
    let WorldPosition { room, pos } = position.0;
    storage
        .view::<WorldPosition, EntityComponent>()
        .reborrow()
        .table
        .get_by_id(&room)
        .map(|room| room.find_by_range(&pos, MAX_SEARCH_RADIUS, &mut candidates))
        .ok_or_else(|| {
            warn!(
                "find_closest_resource_by_range called on invalid room {:?}",
                position
            );
            ExecutionError::InvalidArgument
        });

    let resources = storage.view::<EntityId, ResourceComponent>();

    candidates.retain(|(_pos, entity_id)| resources.get_by_id(&entity_id.0).is_some());
    match candidates
        .iter()
        .min_by_key(|(pos, _)| pos.hex_distance(*pos))
    {
        None => {
            vm.set_value(OperationResult::OperationFailed)?;
        }
        Some((_pos, entity)) => {
            let id = entity.0; // move out of the result to free the storage borrow
            vm.set_value(id)?;
            vm.set_value(OperationResult::Ok)?;
        }
    }
    Ok(())
}
