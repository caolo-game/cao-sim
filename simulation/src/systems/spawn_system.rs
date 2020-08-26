use crate::components;
use crate::indices::EntityId;
use crate::profile;
use crate::storage::views::{UnsafeView, WorldLogger};
use crate::tables::Table;
use slog::{debug, Logger};

type SpawnSystemMut = (
    UnsafeView<EntityId, components::SpawnComponent>,
    UnsafeView<EntityId, components::SpawnBotComponent>,
    UnsafeView<EntityId, components::Bot>,
    UnsafeView<EntityId, components::HpComponent>,
    UnsafeView<EntityId, components::DecayComponent>,
    UnsafeView<EntityId, components::CarryComponent>,
    UnsafeView<EntityId, components::PositionComponent>,
    UnsafeView<EntityId, components::OwnedEntity>,
);

pub fn update(
    (mut spawns, spawn_bots, bots, hps, decay, carry, positions, owned): SpawnSystemMut,
    WorldLogger(logger): WorldLogger,
) {
    profile!("SpawnSystem update");
    let spawn_views = (spawn_bots, bots, hps, decay, carry, positions, owned);
    spawns
        .iter_mut()
        .filter(|(_spawn_id, spawn_component)| spawn_component.spawning.is_some())
        .filter_map(|(spawn_id, spawn_component)| {
            spawn_component.time_to_spawn -= 1;
            if spawn_component.time_to_spawn == 0 {
                let bot = spawn_component.spawning.map(|b| (spawn_id, b));
                spawn_component.spawning = None;
                bot
            } else {
                None
            }
        })
        .for_each(|(spawn_id, entity_id)| spawn_bot(&logger, spawn_id, entity_id, spawn_views));
}

type SpawnBotMut = (
    UnsafeView<EntityId, components::SpawnBotComponent>,
    UnsafeView<EntityId, components::Bot>,
    UnsafeView<EntityId, components::HpComponent>,
    UnsafeView<EntityId, components::DecayComponent>,
    UnsafeView<EntityId, components::CarryComponent>,
    UnsafeView<EntityId, components::PositionComponent>,
    UnsafeView<EntityId, components::OwnedEntity>,
);

/// Spawns a bot from a spawn.
/// Removes the spawning bot from the spawn and initializes a bot in the world
fn spawn_bot(
    logger: &Logger,
    spawn_id: EntityId,
    entity_id: EntityId,
    (mut spawn_bots, mut bots, mut hps, mut decay, mut carry, mut positions, mut owned): SpawnBotMut,
) {
    debug!(
        logger,
        "spawn_bot spawn_id: {:?} entity_id: {:?}", spawn_id, entity_id
    );

    let bot = spawn_bots
        .delete(&entity_id)
        .expect("Spawning bot was not found");
    bots.insert_or_update(entity_id, bot.bot);
    hps.insert_or_update(
        entity_id,
        components::HpComponent {
            hp: 100,
            hp_max: 100,
        },
    );
    decay.insert_or_update(
        entity_id,
        components::DecayComponent {
            eta: 20,
            t: 100,
            hp_amount: 100,
        },
    );
    carry.insert_or_update(
        entity_id,
        components::CarryComponent {
            carry: 0,
            carry_max: 50,
        },
    );

    let pos = positions
        .get_by_id(&spawn_id)
        .cloned()
        .expect("Spawn should have position");
    positions.insert_or_update(entity_id, pos);

    let owner = owned.get_by_id(&spawn_id).cloned();
    if let Some(owner) = owner {
        owned.insert_or_update(entity_id, owner);
    }

    debug!(
        logger,
        "spawn_bot spawn_id: {:?} entity_id: {:?} - done", spawn_id, entity_id
    );
}
