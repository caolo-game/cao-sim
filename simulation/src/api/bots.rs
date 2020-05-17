use super::*;
use crate::{
    intents::{
        check_dropoff_intent, check_mine_intent, check_move_intent, CachePathIntent, DropoffIntent,
        MineIntent, MoveIntent, PopPathCacheIntent,
    },
    model::{
        components::{self, PathCacheComponent, Resource, PATH_CACHE_LEN},
        EntityId, OperationResult, UserId, WorldPosition,
    },
    pathfinding, profile,
    storage::views::FromWorld,
    World,
};
use std::convert::TryFrom;

const MAX_PATHFINDING_ITER: usize = 200;

pub fn unload(
    vm: &mut VM<ScriptExecutionData>,
    (amount, ty, structure): (i32, Resource, TPointer),
) -> Result<(), ExecutionError> {
    profile!("unload");

    let amount = TryFrom::try_from(amount).map_err(|e| {
        warn!("unload called with invalid amount: {}", e);
        ExecutionError::InvalidArgument
    })?;
    let structure: EntityId = vm.get_value(structure).ok_or_else(|| {
        warn!("upload called without a structure");
        ExecutionError::InvalidArgument
    })?;

    let aux = vm.get_aux();
    let storage = aux.storage();
    let entity_id = aux.entity_id;
    let user_id = aux.user_id.expect("user_id to be set");

    let dropoff_intent = DropoffIntent {
        bot: entity_id,
        amount,
        ty,
        structure,
    };

    let checkresult = check_dropoff_intent(&dropoff_intent, user_id, FromWorld::new(storage));
    if let OperationResult::Ok = checkresult {
        vm.get_aux_mut()
            .intents
            .dropoff_intents
            .push(dropoff_intent);
    }
    vm.set_value(checkresult)?;
    Ok(())
}

pub fn mine_resource(
    vm: &mut VM<ScriptExecutionData>,
    entity_id: TPointer,
) -> Result<(), ExecutionError> {
    profile!("mine_resource");

    let entity_id: EntityId = vm.get_value(entity_id).ok_or_else(|| {
        warn!("mine_resource called without a target");
        ExecutionError::InvalidArgument
    })?;

    let aux = vm.get_aux();
    let storage = aux.storage();
    let user_id = aux.user_id.expect("user_id to be set");

    let intent = MineIntent {
        bot: aux.entity_id,
        resource: entity_id,
    };

    let checkresult = check_mine_intent(&intent, user_id, FromWorld::new(storage));
    vm.set_value(checkresult)?;
    if let OperationResult::Ok = checkresult {
        vm.get_aux_mut().intents.mine_intents.push(intent);
    }
    Ok(())
}

pub fn approach_entity(
    vm: &mut VM<ScriptExecutionData>,
    target: TPointer,
) -> Result<(), ExecutionError> {
    profile!("approach_entity");

    let target: EntityId = vm.get_value(target).ok_or_else(|| {
        warn!("approach_entity called without a target");
        ExecutionError::InvalidArgument
    })?;

    let aux = vm.get_aux();
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
            warn!("entity {:?} does not have position component!", target);
            vm.set_value(OperationResult::InvalidInput)?;
            return Ok(());
        }
    };

    let checkresult = match move_to_pos(entity, targetpos.0, user_id, storage) {
        Ok((move_intent, pop_cache_intent, update_cache_intent)) => {
            let intents = &mut vm.get_aux_mut().intents;
            intents.move_intents.push(move_intent);
            if let Some(pop_cache_intent) = pop_cache_intent {
                intents.pop_path_cache_intents.push(pop_cache_intent);
            }
            if let Some(update_cache_intent) = update_cache_intent {
                intents.update_path_cache_intents.push(update_cache_intent);
            }

            OperationResult::Ok
        }
        Err(e) => e,
    };
    vm.set_value(checkresult)?;
    Ok(())
}

pub fn move_bot_to_position(
    vm: &mut VM<ScriptExecutionData>,
    point: TPointer,
) -> Result<(), ExecutionError> {
    profile!("move_bot_to_position");

    let aux = vm.get_aux();
    let entity = aux.entity_id;
    let storage = aux.storage();
    let user_id = aux.user_id.expect("user_id to be set");

    let point: WorldPosition = vm.get_value(point).ok_or_else(|| {
        warn!("move_bot called without a point");
        ExecutionError::InvalidArgument
    })?;

    let checkresult = match move_to_pos(entity, point, user_id, storage) {
        Ok((move_intent, pop_cache_intent, update_cache_intent)) => {
            let intents = &mut vm.get_aux_mut().intents;
            intents.move_intents.push(move_intent);
            if let Some(pop_cache_intent) = pop_cache_intent {
                intents.pop_path_cache_intents.push(pop_cache_intent);
            }
            if let Some(update_cache_intent) = update_cache_intent {
                intents.update_path_cache_intents.push(update_cache_intent);
            }
            OperationResult::Ok
        }
        Err(e) => e,
    };
    vm.set_value(checkresult)?;
    Ok(())
}

fn move_to_pos(
    bot: EntityId,
    to: WorldPosition,
    user_id: UserId,
    storage: &World,
) -> Result<
    (
        MoveIntent,
        Option<PopPathCacheIntent>,
        Option<CachePathIntent>,
    ),
    OperationResult,
> {
    // attempt to use the cached path
    // which requires non-empty cache with a valid next step
    if let Some(cache) = storage
        .view::<EntityId, PathCacheComponent>()
        .reborrow()
        .get_by_id(&bot)
    {
        if let Some(position) = cache.0.last().cloned() {
            let intent = MoveIntent {
                bot,
                position: position.0,
            };
            if let OperationResult::Ok =
                check_move_intent(&intent, user_id, FromWorld::new(storage))
            {
                debug!("Bot {:?} path cache hit", bot);
                return Ok((intent, Some(PopPathCacheIntent { bot }), None));
            }
        }
    }
    debug!("Bot {:?} path cache miss", bot);

    let botpos = storage
        .view::<EntityId, components::PositionComponent>()
        .reborrow()
        .get_by_id(&bot)
        .ok_or_else(|| {
            warn!("entity {:?} does not have position component!", bot);
            OperationResult::InvalidInput
        })?;
    let mut path = Vec::with_capacity(MAX_PATHFINDING_ITER);
    if let Err(e) = pathfinding::find_path(
        botpos.0,
        to,
        FromWorld::new(storage),
        MAX_PATHFINDING_ITER as u32,
        &mut path,
    ) {
        debug!("pathfinding failed {:?}", e);
        return Err(OperationResult::InvalidTarget);
    }

    let intent = match path.pop() {
        Some(position) => MoveIntent {
            bot,
            position: position.0,
        },
        None => {
            debug!("Entity {:?} is trying to move to its own position", bot);
            return Err(OperationResult::InvalidTarget);
        }
    };
    let checkresult = check_move_intent(&intent, user_id, FromWorld::new(storage));
    match checkresult {
        OperationResult::Ok => {
            // skip >= 0
            let skip = path.len().max(PATH_CACHE_LEN) - PATH_CACHE_LEN;

            let cache_intent = CachePathIntent {
                bot,
                cache: PathCacheComponent(
                    path.into_iter().skip(skip).take(PATH_CACHE_LEN).collect(),
                ),
            };

            Ok((intent, None, Some(cache_intent)))
        }
        _ => Err(checkresult),
    }
}
