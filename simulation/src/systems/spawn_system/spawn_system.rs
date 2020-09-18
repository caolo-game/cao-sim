//!
//!
use crate::components::*;
use crate::indices::{EntityId, UserId};
use crate::join;
use crate::profile;
use crate::storage::views::{UnsafeView, View, WorldLogger};
use crate::tables::{JoinIterator, Table};
use slog::{debug, Logger};

type SpawnSystemMut = (
    UnsafeView<EntityId, SpawnComponent>,
    UnsafeView<EntityId, SpawnQueueComponent>,
    UnsafeView<EntityId, EnergyComponent>,
    (
        UnsafeView<EntityId, SpawnBotComponent>,
        UnsafeView<EntityId, Bot>,
        UnsafeView<EntityId, HpComponent>,
        UnsafeView<EntityId, DecayComponent>,
        UnsafeView<EntityId, CarryComponent>,
        UnsafeView<EntityId, PositionComponent>,
        UnsafeView<EntityId, OwnedEntity>,
        UnsafeView<EntityId, EntityScript>,
    ),
);

pub fn update(
    (mut spawns, mut spawn_queue, mut energy, spawn_views): SpawnSystemMut,
    (WorldLogger(logger), user_default_scripts): (WorldLogger, View<UserId, EntityScript>),
) {
    profile!("SpawnSystem update");

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
        .for_each(|(spawn_id, entity_id)| {
            spawn_bot(
                &logger,
                spawn_id,
                entity_id,
                spawn_views,
                user_default_scripts,
            )
        });

    let ss = spawns.iter_mut().filter(|(_, c)| c.spawning.is_none());
    let en = energy.iter_mut().filter(|(_, e)| e.energy == 500); // TODO: config amount
    let sq = spawn_queue.iter_mut();
    join!([ss, en, sq]).for_each(|(_spawn_id, (spawn, energy, queue))| {
        // spawns with 500 energy and no currently spawning bot
        if let Some(bot) = queue.queue.pop_back() {
            energy.energy -= 500;
            spawn.time_to_spawn = 10;
            spawn.spawning = Some(bot);
        }
    });
}

type SpawnBotMut = (
    UnsafeView<EntityId, SpawnBotComponent>,
    UnsafeView<EntityId, Bot>,
    UnsafeView<EntityId, HpComponent>,
    UnsafeView<EntityId, DecayComponent>,
    UnsafeView<EntityId, CarryComponent>,
    UnsafeView<EntityId, PositionComponent>,
    UnsafeView<EntityId, OwnedEntity>,
    UnsafeView<EntityId, EntityScript>,
);

/// Spawns a bot from a spawn.
/// Removes the spawning bot from the spawn and initializes a bot in the world
fn spawn_bot(
    logger: &Logger,
    spawn_id: EntityId,
    entity_id: EntityId,
    (
        mut spawn_bots,
        mut bots,
        mut hps,
        mut decay,
        mut carry,
        mut positions,
        mut owned,
        mut script_table,
    ): SpawnBotMut,
    user_default_scripts: View<UserId, EntityScript>,
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
        HpComponent {
            hp: 100,
            hp_max: 100,
        },
    );
    decay.insert_or_update(
        entity_id,
        DecayComponent {
            interval: 20,
            time_remaining: 20,
            hp_amount: 100,
        },
    );
    carry.insert_or_update(
        entity_id,
        CarryComponent {
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
        if let Some(script) = user_default_scripts.get_by_id(&owner.owner_id) {
            script_table.insert_or_update(entity_id, script.clone());
        }

        owned.insert_or_update(entity_id, owner);
    }

    debug!(
        logger,
        "spawn_bot spawn_id: {:?} entity_id: {:?} - done", spawn_id, entity_id
    );
}
