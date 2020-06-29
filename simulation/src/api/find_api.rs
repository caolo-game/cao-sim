use super::*;
use crate::components::{EntityComponent, PositionComponent};
use crate::data_store::World;
use crate::model::WorldPosition;
use crate::profile;
use cao_lang::prelude::*;
use std::convert::TryFrom;

#[derive(Debug, Clone, Copy)]
#[repr(i32)]
pub enum FindConstant {
    Resource = 1,
    Spawn = 2,
}

impl TryFrom<Scalar> for FindConstant {
    type Error = Scalar;
    fn try_from(i: Scalar) -> Result<Self, Scalar> {
        let op = match i {
            Scalar::Integer(1) => FindConstant::Resource,
            Scalar::Integer(2) => FindConstant::Spawn,
            _ => return Err(i),
        };
        Ok(op)
    }
}

impl AutoByteEncodeProperties for FindConstant {}

/// Return OperationResult and an EntityId if the Operation succeeded
pub fn find_closest_by_range(
    vm: &mut VM<ScriptExecutionData>,
    param: FindConstant,
) -> Result<(), ExecutionError> {
    profile!("find_closest_by_range");

    let entity_id = vm.get_aux().entity_id;

    trace!("find_closest_by_range {:?} {:?}", entity_id, param);

    let position = match vm
        .get_aux()
        .storage()
        .view::<EntityId, PositionComponent>()
        .get_by_id(&entity_id)
    {
        Some(p) => p.0,
        None => {
            warn!("{:?} has no PositionComponent", entity_id);
            return Err(ExecutionError::InvalidArgument);
        }
    };

    trace!("Executing find_closest_by_range {:?}", position);

    param.execute(vm, position)
}

impl FindConstant {
    pub fn execute(
        self,
        vm: &mut VM<ScriptExecutionData>,
        position: WorldPosition,
    ) -> Result<(), ExecutionError> {
        let storage = vm.get_aux().storage();
        let candidate = match self {
            FindConstant::Resource => {
                let resources = vm
                    .get_aux()
                    .storage()
                    .view::<EntityId, components::ResourceComponent>();
                find_closest_entity_impl(storage, position, |id| resources.contains(&id))
            }
            FindConstant::Spawn => {
                let spawns = vm
                    .get_aux()
                    .storage()
                    .view::<EntityId, components::SpawnComponent>();
                find_closest_entity_impl(storage, position, |id| spawns.contains(&id))
            }
        }?;
        match candidate {
            Some(entity) => {
                let id = entity.0; // move out of the result to free the storage borrow
                vm.set_value(id)?;
                vm.stack_push(OperationResult::Ok)?;
            }
            None => {
                trace!("No stuff was found");
                vm.stack_push(OperationResult::OperationFailed)?;
            }
        }
        Ok(())
    }
}

fn find_closest_entity_impl<F>(
    storage: &World,
    position: WorldPosition,
    filter: F,
) -> Result<Option<EntityId>, ExecutionError>
where
    F: Fn(EntityId) -> bool,
{
    let WorldPosition { room, pos } = position;
    let entities_by_pos = storage.view::<WorldPosition, EntityComponent>();

    let room = entities_by_pos.table.get_by_id(&room).ok_or_else(|| {
        warn!(
            "find_closest_resource_by_range called on invalid room {:?}",
            position
        );
        ExecutionError::InvalidArgument
    })?;

    // search the whole room
    let candidate = room.find_closest_by_filter(&pos, |_, entity| filter(entity.0));
    let candidate = candidate.map(|(_, _, id)| id.0);
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_store::{init_inmemory_storage, World};
    use rand::{rngs::SmallRng, thread_rng, Rng, SeedableRng};

    fn init_resource_storage(
        entity_id: EntityId,
        center_pos: WorldPosition,
        expected_id: EntityId,
        expected_pos: WorldPosition,
    ) -> std::pin::Pin<Box<World>> {
        crate::utils::setup_testing();

        let mut seed = [0; 16];
        thread_rng().fill(&mut seed);
        let mut rng = SmallRng::from_seed(seed);

        let mut storage = init_inmemory_storage();

        unsafe {
            let mut entity_positions = storage.unsafe_view::<EntityId, PositionComponent>();
            let mut position_entities = storage.unsafe_view::<WorldPosition, EntityComponent>();

            position_entities
                .as_mut()
                .insert(expected_pos, EntityComponent(expected_id))
                .expect("Initial insert 2");

            for _ in 0..128 {
                let id = storage.insert_entity();
                let pos = loop {
                    let q = rng.gen_range(0, 256);
                    let r = rng.gen_range(0, 256);

                    let pos = Axial::new(q, r);
                    if center_pos.pos.hex_distance(pos)
                        > center_pos.pos.hex_distance(expected_pos.pos)
                    {
                        break pos;
                    }
                };
                position_entities
                    .as_mut()
                    .insert(
                        WorldPosition {
                            room: Axial::new(0, 0),
                            pos,
                        },
                        EntityComponent(id),
                    )
                    .expect("Initial insert 3");
            }

            // make every one of these a resource
            for (_, entity_id) in position_entities.iter() {
                storage
                    .unsafe_view::<EntityId, components::ResourceComponent>()
                    .as_mut()
                    .insert_or_update(
                        entity_id.0,
                        components::ResourceComponent(components::Resource::Energy),
                    );
            }

            // the querying entity is not a resource

            entity_positions
                .as_mut()
                .insert_or_update(entity_id, PositionComponent(center_pos));
            position_entities
                .as_mut()
                .insert(center_pos, EntityComponent(entity_id))
                .expect("Initial insert 1");
        }
        storage
    }

    #[test]
    fn finds_closest_returns_itself_when_appropriate() {
        let entity_id = EntityId(1024);
        let center_pos = WorldPosition {
            room: Axial::new(0, 0),
            pos: Axial::new(14, 14),
        };

        let expected_id = EntityId(2040);
        let expected_pos = WorldPosition {
            room: Axial::new(0, 0),
            pos: Axial::new(69, 69),
        };

        let mut storage = init_resource_storage(entity_id, center_pos, expected_id, expected_pos);
        unsafe {
            storage
                .unsafe_view::<EntityId, components::ResourceComponent>()
                .as_mut()
                .insert_or_update(
                    entity_id,
                    components::ResourceComponent(components::Resource::Energy),
                );
        }
        let data = ScriptExecutionData {
            storage: &*storage.as_ref() as *const _,
            entity_id,
            user_id: Default::default(),
            intents: Default::default(),
        };
        let mut vm = VM::new(data);

        let constant = FindConstant::Resource;

        find_closest_by_range(&mut vm, constant).expect("find_closest_by_range exec");

        let res = vm.stack_pop().expect("Expected an entity id");
        let res =
            OperationResult::try_from(res).expect("Expected res to be a valid OperationResult");
        assert_eq!(res, OperationResult::Ok);

        let res_id = vm.stack_pop().expect("Expected pointer");
        if let Scalar::Pointer(p) = res_id {
            let res_id: EntityId = vm.get_value(p).expect("Expected entity_id to be set");

            assert_eq!(res_id, entity_id);
        } else {
            panic!("Expected pointer, got {:?}", res_id);
        }
    }

    #[test]
    fn finds_closest_resources_as_expected() {
        let entity_id = EntityId(1024);
        let center_pos = WorldPosition {
            room: Axial::new(0, 0),
            pos: Axial::new(14, 14),
        };

        let expected_id = EntityId(2040);
        let expected_pos = WorldPosition {
            room: Axial::new(0, 0),
            pos: Axial::new(69, 69),
        };

        let storage = init_resource_storage(entity_id, center_pos, expected_id, expected_pos);
        let data = ScriptExecutionData {
            storage: &*storage.as_ref() as *const _,
            entity_id,
            user_id: Default::default(),
            intents: Default::default(),
        };
        let mut vm = VM::new(data);

        let constant = FindConstant::Resource;

        find_closest_by_range(&mut vm, constant).expect("find_closest_by_range exec");

        let res = vm.stack_pop().expect("Expected an entity id");
        let res =
            OperationResult::try_from(res).expect("Expected res to be a valid OperationResult");
        assert_eq!(res, OperationResult::Ok);

        let res_id = vm.stack_pop().expect("Expected pointer");
        if let Scalar::Pointer(p) = res_id {
            let res_id: EntityId = vm.get_value(p).expect("Expected entity_id to be set");

            assert_eq!(res_id, expected_id);
        } else {
            panic!("Expected pointer, got {:?}", res_id);
        }
    }
}
