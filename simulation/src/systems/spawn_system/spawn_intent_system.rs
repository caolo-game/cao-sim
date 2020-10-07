use crate::components::{Bot, OwnedEntity, SpawnBotComponent, SpawnQueueComponent};
use crate::indices::EntityId;
use crate::intents::{Intents, SpawnIntent};
use crate::profile;
use crate::storage::views::{InsertEntityView, UnsafeView, UnwrapView, WorldLogger};
use slog::{debug, o, trace};

type Mut = (
    UnsafeView<EntityId, SpawnBotComponent>,
    UnsafeView<EntityId, SpawnQueueComponent>,
    UnsafeView<EntityId, OwnedEntity>,
    InsertEntityView,
);

type Const<'a> = (UnwrapView<'a, Intents<SpawnIntent>>, WorldLogger);

pub fn update(
    (mut spawn_bot_table, mut spawn_queue, mut owner_table, mut insert_entity): Mut,
    (intents, WorldLogger(logger)): Const,
) {
    profile!("SpawnSystem update");
    for intent in intents.iter() {
        let logger = logger.new(o!("spawn_id"=> intent.spawn_id.0));
        trace!(logger, "Spawning bot from structure");

        let spawn = match spawn_queue.get_by_id_mut(&intent.spawn_id) {
            Some(x) => x,
            None => {
                debug!(logger, "structure does not have spawn queue component");
                continue;
            }
        };
        if spawn.queue.len() > 20 { // TODO: config
            debug!(logger, "spawn queue is full");
            continue;
        }

        let bot_id = unsafe { insert_entity.insert_entity() };
        spawn_bot_table.insert_or_update(bot_id, SpawnBotComponent { bot: Bot {} });
        if let Some(owner_id) = intent.owner_id {
            owner_table.insert_or_update(bot_id, OwnedEntity { owner_id });
        }
        spawn.queue.push_back(bot_id);
    }
}
