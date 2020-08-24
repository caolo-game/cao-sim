use crate::components::{
    Bot, CarryComponent, EnergyComponent, OwnedEntity, PositionComponent, Resource,
};
use crate::indices::{EntityId, UserId};
use crate::scripting_api::OperationResult;
use crate::storage::views::View;
use serde::{Deserialize, Serialize};
use slog::{debug, Logger};

pub const DROPOFF_RANGE: u32 = 1;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DropoffIntent {
    pub bot: EntityId,
    pub structure: EntityId,
    pub amount: u16,
    pub ty: Resource,
}

type CheckInput<'a> = (
    View<'a, EntityId, Bot>,
    View<'a, EntityId, OwnedEntity>,
    View<'a, EntityId, PositionComponent>,
    View<'a, EntityId, CarryComponent>,
    View<'a, EntityId, EnergyComponent>,
);

/// A valid dropoff intent has the following characteristics:
/// - the bot is owned by the user
/// - the bot is carrying resource of type `ty`
/// - the target is not full
/// - the target is within dropoff range
pub fn check_dropoff_intent(
    logger: &Logger,
    intent: &DropoffIntent,
    userid: UserId,
    (bots, owners, positions, carry, energy): CheckInput,
) -> OperationResult {
    let id = intent.bot;
    match bots.get_by_id(&id) {
        Some(_) => {
            let owner_id = owners.get_by_id(&id);
            if owner_id.map(|id| id.owner_id != userid).unwrap_or(true) {
                return OperationResult::NotOwner;
            }
        }
        None => return OperationResult::InvalidInput,
    };

    if carry
        .get_by_id(&id)
        .map(|carry| carry.carry == 0)
        .unwrap_or(true)
    {
        return OperationResult::Empty;
    }

    let target = intent.structure;
    let nearby = positions.get_by_id(&id).and_then(|botpos| {
        positions.get_by_id(&target).map(|targetpos| {
            targetpos.0.room == botpos.0.room
                && targetpos.0.pos.hex_distance(botpos.0.pos) <= DROPOFF_RANGE
        })
    });
    match nearby {
        None => {
            debug!(
                logger,
                "Bot or target has no position components {:?}", intent
            );
            OperationResult::InvalidInput
        }
        Some(false) => OperationResult::NotInRange,
        Some(true) => {
            let capacity = energy.get_by_id(&target);
            if capacity.is_none() {
                debug!(logger, "Target has no energy component {:?}", intent);
                return OperationResult::InvalidInput;
            }
            let capacity = capacity.unwrap();
            if capacity.energy < capacity.energy_max {
                OperationResult::Ok
            } else {
                OperationResult::Full
            }
        }
    }
}
