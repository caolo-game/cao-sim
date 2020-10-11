use crate::components::{HpComponent, MeleeAttackComponent, OwnedEntity, PositionComponent};
use crate::indices::{EntityId, UserId};
use crate::scripting_api::OperationResult;
use crate::storage::views::View;
use serde::{Deserialize, Serialize};
use slog::{debug, trace, Logger};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MeleeIntent {
    pub attacker: EntityId,
    pub defender: EntityId,
}

type CheckInput<'a> = (
    View<'a, EntityId, OwnedEntity>,
    View<'a, EntityId, PositionComponent>,
    View<'a, EntityId, MeleeAttackComponent>,
    View<'a, EntityId, HpComponent>,
);

/// `attacker` must be owned by the user.
/// `attacker` must have `MeleeAttackComponent`
/// `defender` must have `HpComponent`
/// `attacker` and `defender` must be within 1 tiles
pub fn check_melee_intent(
    logger: &Logger,
    intent: &MeleeIntent,
    user_id: UserId,
    (owner_table, pos_table, melee_table, hp_table): CheckInput,
) -> OperationResult {
    let logger = logger.new(slog::o!(
            "attacker" => intent.attacker.0,
            "defender" => intent.defender.0,
    ));
    trace!(logger, "check_melee_intent");

    if owner_table
        .get_by_id(&intent.attacker)
        .map(|o| o.owner_id != user_id)
        .unwrap_or(true)
    {
        // if not owner or the bot has no owner
        return OperationResult::NotOwner;
    }
    if !melee_table.contains(&intent.attacker) {
        debug!(logger, "attacker has no MeleeAttackComponent");
        return OperationResult::InvalidInput;
    }
    if !hp_table.contains_id(&intent.defender) {
        debug!(logger, "defender has no HpComponent");
        return OperationResult::InvalidTarget;
    }
    let attack_pos = match pos_table.get_by_id(&intent.attacker) {
        Some(x) => x,
        None => {
            debug!(logger, "attacker has no PositionComponent");
            return OperationResult::InvalidInput;
        }
    };
    let defend_pos = match pos_table.get_by_id(&intent.defender) {
        Some(x) => x,
        None => {
            debug!(logger, "defender has no PositionComponent");
            return OperationResult::InvalidTarget;
        }
    };
    if attack_pos.0.room != defend_pos.0.room {
        debug!(logger, "Attacker and defender are not in the same room");
        return OperationResult::InvalidTarget;
    }
    if attack_pos.0.pos.hex_distance(defend_pos.0.pos) > 1 {
        debug!(logger, "Attacker is out of melee range");
        return OperationResult::NotInRange;
    }
    OperationResult::Ok
}
