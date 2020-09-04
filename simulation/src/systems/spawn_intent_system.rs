use crate::components::{Bot, EnergyComponent, OwnedEntity, SpawnBotComponent, SpawnComponent};
use crate::indices::EntityId;
use crate::intents::{Intents, SpawnIntent};
use crate::profile;
use crate::storage::views::{InsertEntityView, UnsafeView, UnwrapView, View, WorldLogger};
use slog::{debug, o, trace, warn};

type Mut = (
    UnsafeView<EntityId, SpawnBotComponent>,
    UnsafeView<EntityId, SpawnComponent>,
    UnsafeView<EntityId, OwnedEntity>,
    InsertEntityView,
);

type Const<'a> = (
    View<'a, EntityId, EnergyComponent>,
    UnwrapView<'a, Intents<SpawnIntent>>,
    WorldLogger,
);

pub fn update(
    (mut spawn_bot_table, mut spawn_table, mut owner_table, mut insert_entity): Mut,
    (entity_table, intents, WorldLogger(logger)): Const,
) {
    profile!("SpawnSystem update");
    for intent in intents.iter() {
        let logger = logger.new(o!("spawn_id"=> intent.spawn_id.0));
        trace!(logger, "Spawning bot from structure");

        let spawn = match spawn_table.get_by_id_mut(&intent.spawn_id) {
            Some(x) => x,
            None => {
                debug!(logger, "structure does not have spawn component");
                continue;
            }
        };

        if spawn.spawning.is_some() {
            warn!(logger, "spawn is busy");
            continue;
        }

        let energy = match entity_table.get_by_id(&intent.spawn_id) {
            Some(x) => x,
            None => {
                debug!(logger, "structure does not have energy");
                continue;
            }
        };

        if energy.energy < 200 {
            debug!(logger, "not enough energy {:?}", energy);
            continue;
        }

        unsafe {
            let bot_id = insert_entity.insert_entity();
            spawn_bot_table.insert_or_update(bot_id, SpawnBotComponent { bot: Bot {} });
            if let Some(owner_id) = intent.owner_id {
                owner_table.insert_or_update(bot_id, OwnedEntity { owner_id });
            }

            spawn.time_to_spawn = 5;
            spawn.spawning = Some(bot_id);
        }
    }
}
