use super::*;
use crate::{
    components::{self, PathCacheComponent, Resource, TerrainComponent, PATH_CACHE_LEN},
    indices::{EntityId, UserId, WorldPosition},
    intents::{
        check_dropoff_intent, check_melee_intent, check_mine_intent, check_move_intent,
        CachePathIntent, DropoffIntent, MeleeIntent, MineIntent, MoveIntent, MutPathCacheIntent,
        PathCacheIntentAction,
    },
    pathfinding, profile,
    storage::views::FromWorld,
};
use crate::{prelude::World, terrain::TileTerrainType};
use slog::{debug, error, trace, warn};
use std::convert::TryFrom;

pub fn melee_attack(
    vm: &mut VM<ScriptExecutionData>,
    target: Pointer,
) -> Result<(), ExecutionError> {
    profile!("melee-attack");

    let aux = vm.get_aux();
    let logger = &aux.logger;
    trace!(logger, "melee_attack");

    let target: EntityId = vm.get_value(target).ok_or_else(|| {
        warn!(logger, "melee_attack called without a target");
        ExecutionError::invalid_argument("melee_attack called without a target".to_owned())
    })?;

    let storage = aux.storage();
    let entity_id = aux.entity_id;
    let user_id = aux.user_id.expect("user_id to be set");

    let intent = MeleeIntent {
        attacker: entity_id,
        defender: target,
    };

    let res = check_melee_intent(logger, &intent, user_id, FromWorld::new(storage));

    if let OperationResult::Ok = res {
        vm.get_aux_mut().intents.melee_attack_intent = Some(intent);
    }
    vm.stack_push(res)?;
    Ok(())
}

pub fn unload(
    vm: &mut VM<ScriptExecutionData>,
    (amount, ty, target): (i32, Resource, Pointer),
) -> Result<(), ExecutionError> {
    profile!("unload");
    let aux = vm.get_aux();
    let logger = &aux.logger;

    trace!(logger, "unload");

    let amount = TryFrom::try_from(amount).map_err(|e| {
        ExecutionError::invalid_argument(format!("unload called with invalid amount: {}", e))
    })?;
    let target: EntityId = vm.get_value(target).ok_or_else(|| {
        warn!(logger, "unload called without a structure");
        ExecutionError::invalid_argument("unload called without a structure".to_owned())
    })?;

    trace!(
        logger,
        "unload: amount: {} type: {:?} target: {:?}, {}",
        amount,
        ty,
        target,
        aux
    );

    let storage = aux.storage();
    let entity_id = aux.entity_id;
    let user_id = aux.user_id.expect("user_id to be set");

    let dropoff_intent = DropoffIntent {
        bot: entity_id,
        amount,
        ty,
        structure: target,
    };

    let checkresult =
        check_dropoff_intent(logger, &dropoff_intent, user_id, FromWorld::new(storage));
    if let OperationResult::Ok = checkresult {
        vm.get_aux_mut().intents.dropoff_intent = Some(dropoff_intent);
    }
    vm.stack_push(checkresult)?;
    Ok(())
}

pub fn mine_resource(
    vm: &mut VM<ScriptExecutionData>,
    target: Pointer,
) -> Result<(), ExecutionError> {
    profile!("mine_resource");
    let aux = vm.get_aux();
    let logger = &aux.logger;

    let target: EntityId = vm.get_value(target).ok_or_else(|| {
        warn!(logger, "mine_resource called without a target");
        ExecutionError::InvalidArgument { context: None }
    })?;

    trace!(logger, "mine_resource: target: {:?}, {}", target, aux);

    let storage = aux.storage();
    let user_id = aux.user_id.expect("user_id to be set");

    let intent = MineIntent {
        bot: aux.entity_id,
        resource: target,
    };

    let checkresult = check_mine_intent(logger, &intent, user_id, FromWorld::new(storage));
    vm.stack_push(checkresult)?;
    if let OperationResult::Ok = checkresult {
        vm.get_aux_mut().intents.mine_intent = Some(intent);
    }
    Ok(())
}

pub fn approach_entity(
    vm: &mut VM<ScriptExecutionData>,
    target: Pointer,
) -> Result<(), ExecutionError> {
    profile!("approach_entity");

    let aux = vm.get_aux();
    let logger = &aux.logger;

    let target: EntityId = vm.get_value(target).ok_or_else(|| {
        warn!(logger, "approach_entity called without a target");
        ExecutionError::InvalidArgument { context: None }
    })?;

    trace!(logger, "approach_entity: target: {:?}", target);

    let entity = aux.entity_id;
    let storage = aux.storage();
    let user_id = aux.user_id.expect("user_id to be set");

    let targetpos = match storage
        .view::<EntityId, components::PositionComponent>()
        .reborrow()
        .get_by_id(&target)
    {
        Some(x) => x,
        None => {
            warn!(
                logger,
                "entity {:?} does not have position component!", target
            );
            vm.stack_push(OperationResult::InvalidInput)?;
            return Ok(());
        }
    };

    let checkresult = match move_to_pos(logger, entity, targetpos.0, user_id, storage) {
        Ok(Some((move_intent, pop_cache_intent, update_cache_intent))) => {
            let intents = &mut vm.get_aux_mut().intents;
            intents.move_intent = Some(move_intent);
            if let Some(pop_cache_intent) = pop_cache_intent {
                intents.mut_path_cache_intent = Some(pop_cache_intent);
            }
            if let Some(update_cache_intent) = update_cache_intent {
                intents.update_path_cache_intent = Some(update_cache_intent);
            }

            OperationResult::Ok
        }
        Ok(None) => {
            trace!(logger, "Bot {:?} approach_entity: nothing to do", entity);
            OperationResult::Ok
        }
        Err(e) => e,
    };
    vm.stack_push(checkresult)?;
    Ok(())
}

pub fn move_bot_to_position(
    vm: &mut VM<ScriptExecutionData>,
    point: Pointer,
) -> Result<(), ExecutionError> {
    profile!("move_bot_to_position");

    let aux = vm.get_aux();
    let logger = &aux.logger;

    trace!(logger, "move_bot_to_position");

    let entity = aux.entity_id;
    let storage = aux.storage();
    let user_id = aux.user_id.expect("user_id to be set");

    let point: WorldPosition = vm.get_value(point).ok_or_else(|| {
        warn!(logger, "move_bot called without a point");
        ExecutionError::InvalidArgument { context: None }
    })?;

    let checkresult = match move_to_pos(logger, entity, point, user_id, storage) {
        Ok(Some((move_intent, pop_cache_intent, update_cache_intent))) => {
            let intents = &mut vm.get_aux_mut().intents;
            intents.move_intent = Some(move_intent);
            if let Some(pop_cache_intent) = pop_cache_intent {
                intents.mut_path_cache_intent = Some(pop_cache_intent);
            }
            if let Some(update_cache_intent) = update_cache_intent {
                intents.update_path_cache_intent = Some(update_cache_intent);
            }
            OperationResult::Ok
        }
        Ok(None) => {
            trace!(logger, "{:?} move_to_pos nothing to do", entity);
            OperationResult::Ok
        }
        Err(e) => e,
    };
    vm.stack_push(checkresult)?;
    Ok(())
}

type MoveToPosIntent = (
    MoveIntent,
    Option<MutPathCacheIntent>,
    Option<CachePathIntent>,
);

fn move_to_pos<'a>(
    logger: impl Into<&'a slog::Logger>,
    bot: EntityId,
    to: WorldPosition,
    user_id: UserId,
    storage: &World,
) -> Result<Option<MoveToPosIntent>, OperationResult> {
    profile!("move_to_pos");

    let logger = logger.into();

    let botpos = storage
        .view::<EntityId, components::PositionComponent>()
        .reborrow()
        .get_by_id(&bot)
        .ok_or_else(|| {
            warn!(logger, "entity {:?} does not have position component!", bot);
            OperationResult::InvalidInput
        })?;

    // attempt to use the cached path
    // which requires non-empty cache with a valid next step
    match storage
        .view::<EntityId, PathCacheComponent>()
        .reborrow()
        .get_by_id(&bot)
    {
        Some(cache) if cache.target == to => {
            if let Some(position) = cache.path.last().cloned() {
                let intent = MoveIntent {
                    bot,
                    position: WorldPosition {
                        room: botpos.0.room,
                        pos: position.0,
                    },
                };
                if let OperationResult::Ok =
                    check_move_intent(logger, &intent, user_id, FromWorld::new(storage))
                {
                    trace!(logger, "Bot {:?} path cache hit", bot);
                    return Ok(Some((
                        intent,
                        Some(MutPathCacheIntent {
                            bot,
                            action: PathCacheIntentAction::Pop,
                        }),
                        None,
                    )));
                }
            }
        }
        _ => {}
    }
    trace!(logger, "Bot {:?} path cache miss", bot);

    // TODO: config omponent and read from there
    let max_pathfinding_iter: u32 = std::env::var("MAX_PATHFINDING_ITER")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000);

    let mut path = Vec::with_capacity(max_pathfinding_iter as usize);
    let mut rooms_path = Vec::with_capacity(to.room.hex_distance(botpos.0.room) as usize);
    if let Err(e) = pathfinding::find_path(
        logger,
        botpos.0,
        to,
        FromWorld::new(storage),
        max_pathfinding_iter,
        &mut path,
        &mut rooms_path,
    ) {
        trace!(logger, "pathfinding failed {:?}", e);
        return Err(OperationResult::InvalidTarget);
    }

    match path.pop() {
        Some(position) => {
            let intent = MoveIntent {
                bot,
                position: WorldPosition {
                    room: botpos.0.room,
                    pos: position.0,
                },
            };

            let checkresult = check_move_intent(logger, &intent, user_id, FromWorld::new(storage));
            match checkresult {
                OperationResult::Ok => {
                    let cache_intent = if !path.is_empty() {
                        // skip >= 0
                        let skip = path.len().max(PATH_CACHE_LEN) - PATH_CACHE_LEN;

                        let cache_intent = CachePathIntent {
                            bot,
                            cache: PathCacheComponent {
                                target: to,
                                path: path.into_iter().skip(skip).take(PATH_CACHE_LEN).collect(),
                            },
                        };
                        Some(cache_intent)
                    } else {
                        None
                    };

                    Ok(Some((intent, None, cache_intent)))
                }
                _ => Err(checkresult),
            }
        }
        None => {
            trace!(
                logger,
                "Entity {:?} is trying to move to its own position",
                bot
            );
            match rooms_path.pop() {
                Some(to_room) => {
                    let is_bridge = storage
                        .view::<WorldPosition, TerrainComponent>()
                        .get_by_id(&botpos.0)
                        .map(|TerrainComponent(t)| *t == TileTerrainType::Bridge)
                        .unwrap_or_else(|| {
                            error!(
                                logger,
                                "Bot {:?} is not standing on terrain {:?}", bot, botpos
                            );
                            false
                        });
                    if !is_bridge {
                        return Err(OperationResult::InvalidTarget);
                    }
                    let target_pos = match pathfinding::get_valid_transits(
                        logger,
                        botpos.0,
                        to_room,
                        FromWorld::new(storage),
                    ) {
                        Ok(candidates) => candidates[0],
                        Err(pathfinding::TransitError::NotFound) => {
                            return Err(OperationResult::PathNotFound)
                        }
                        Err(e) => {
                            error!(logger, "Transit failed {:?}", e);
                            return Err(OperationResult::OperationFailed);
                        }
                    };
                    let intent = MoveIntent {
                        bot,
                        position: target_pos,
                    };
                    Ok(Some((
                        intent,
                        Some(MutPathCacheIntent {
                            bot,
                            action: PathCacheIntentAction::Del,
                        }),
                        None,
                    )))
                }
                None => {
                    debug!(logger, "Entity {:?} is trying to move to its own position, but no next room was returned", bot);

                    Ok(None)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;
    use crate::query;
    use crate::terrain::TileTerrainType;
    use crate::{data_store::init_inmemory_storage, map_generation::room::iter_edge};

    #[test]
    fn can_move_to_another_room() {
        crate::utils::setup_testing();

        let logger = crate::utils::test_logger();
        let mut storage = init_inmemory_storage(logger);

        let bot_id = storage.insert_entity();
        let room_radius = 3;
        let room_center = Axial::new(room_radius, room_radius);

        let mut from = WorldPosition {
            room: Axial::new(0, 0),
            pos: Axial::default(),
        };
        let to = WorldPosition {
            room: Axial::new(0, 2),
            pos: Axial::new(2, 1),
        };

        let next_room = Axial::new(0, 1);

        from.pos = iter_edge(
            room_center,
            room_radius as u32,
            &RoomConnection {
                direction: next_room,
                offset_end: 1,
                offset_start: 1,
            },
        )
        .unwrap()
        .next()
        .unwrap();

        let user_id = UserId::default();

        query!(
            mutate
            storage
            {
                EntityId, Bot,
                    .insert_or_update(bot_id, Bot);
                EntityId, PositionComponent,
                    .insert_or_update(bot_id, PositionComponent(from));
                EntityId, OwnedEntity,
                    .insert_or_update(bot_id, OwnedEntity{owner_id:user_id});
                EmptyKey, RoomProperties,
                    .update(Some(RoomProperties{radius:room_radius as u32, center: room_center}));

                WorldPosition, EntityComponent,
                    .extend_rooms([Room(from.room),Room(Axial::new(0,1)), Room(to.room)].iter().cloned())
                    .expect("Failed to add rooms");
                WorldPosition, TerrainComponent,
                    .extend_rooms([Room(from.room),Room(Axial::new(0,1)), Room(to.room)].iter().cloned())
                    .expect("Failed to add rooms");
                WorldPosition, TerrainComponent,
                    .extend_from_slice(&mut [
                        ( from, TerrainComponent(TileTerrainType::Bridge) ),
                        ( WorldPosition{room: Axial::new(0,1), pos: Axial::new(5,0)}
                          , TerrainComponent(TileTerrainType::Bridge) ),
                    ])
                    .expect("Failed to insert terrain");
        });

        let mut init_connections = |room| {
            // init connections...
            let mut connections = RoomConnections::default();
            let neighbour = next_room;
            connections.0[Axial::neighbour_index(neighbour).expect("Bad neighbour")] =
                Some(RoomConnection {
                    direction: neighbour,
                    offset_end: 0,
                    offset_start: 0,
                });
            query!(
                mutate
                storage
                {
                    Room, RoomConnections,
                        .insert( Room(from.room), connections )
                        .expect("Failed to add room connections");
                }
            );
            let mut connections = RoomConnections::default();
            let neighbour = next_room;
            connections.0[Axial::neighbour_index(neighbour).expect("Bad neighbour")] =
                Some(RoomConnection {
                    direction: neighbour,
                    offset_end: 0,
                    offset_start: 0,
                });
            query!(
                mutate
                storage
                {
                Room, RoomConnections,
                    .insert( Room(room), connections )
                    .expect("Failed to add room connections");
                }
            );
        };
        init_connections(next_room);
        init_connections(to.room);

        let (MoveIntent { bot, position }, ..) =
            move_to_pos(&storage.logger, bot_id, to, user_id, &storage)
                .expect("Expected move to succeed")
                .expect("Expected a move intent");

        assert_eq!(bot, bot_id);
        assert_eq!(position.room, next_room);
    }
}
