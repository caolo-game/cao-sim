//! Keeps adding intents
//!

use crate::components::*;
use crate::indices::EntityId;
use crate::intents::{Intents, SpawnIntent};
use crate::join;
use crate::profile;
use crate::storage::views::{UnwrapViewMut, View, WorldLogger};
use crate::tables::JoinIterator;
use slog::debug;

type SpawnSystemMut = (UnwrapViewMut<Intents<SpawnIntent>>,);

type SpawnSystemConsts<'a> = (
    WorldLogger,
    View<'a, EntityId, OwnedEntity>,
    View<'a, EntityId, SpawnQueueComponent>,
);

pub fn update(
    (mut intents,): SpawnSystemMut,
    (WorldLogger(logger), owners, spawn_queues): SpawnSystemConsts,
) {
    profile!("Continous Spawn System update");

    let own_it = owners.iter();
    let spawnq_it = spawn_queues.iter();

    for (spawn_id, (owner, spawn)) in join!([own_it, spawnq_it]) {
        if spawn.queue.is_empty() {
            debug!(
                logger,
                "Adding a spawn intent to the queue of spawn {:?}", spawn_id
            );
            intents.0.push(SpawnIntent {
                spawn_id,
                owner_id: Some(owner.owner_id),
            });
        }
    }
}
