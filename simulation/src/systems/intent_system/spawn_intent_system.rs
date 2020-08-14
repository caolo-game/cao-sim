use super::IntentExecutionSystem;
use crate::components::{Bot, EnergyComponent, OwnedEntity, SpawnBotComponent, SpawnComponent};
use crate::intents::SpawnIntent;
use crate::model::EntityId;
use crate::profile;
use crate::storage::views::{InsertEntityView, UnsafeView, View};
use log::{error, trace, warn};

pub struct SpawnSystem;

type Mut = (
    UnsafeView<EntityId, SpawnBotComponent>,
    UnsafeView<EntityId, SpawnComponent>,
    UnsafeView<EntityId, OwnedEntity>,
    InsertEntityView,
);

impl<'a> IntentExecutionSystem<'a> for SpawnSystem {
    type Mut = Mut;
    type Const = (View<'a, EntityId, EnergyComponent>,);
    type Intents =&'a[ SpawnIntent];

    fn execute(
        &mut self,
        (mut spawn_bot_table, mut spawn_table, mut owner_table, mut insert_entity): Self::Mut,
        (entity_table,): Self::Const,
        intents: Self::Intents,
    ) {
        profile!(" SpawnSystem update");
        for intent in intents {
            trace!("Spawning bot from structure {:?}", intent.spawn_id);

            let mut spawn = match spawn_table.get_by_id(&intent.spawn_id).cloned() {
                Some(x) => x,
                None => {
                    error!("structure does not have spawn component");
                    continue;
                }
            };

            if spawn.spawning.is_some() {
                warn!("spawn is busy");
                continue;
            }

            let energy = match entity_table.get_by_id(&intent.spawn_id) {
                Some(x) => x,
                None => {
                    error!("structure does not have energy");
                    continue;
                }
            };

            if energy.energy < 200 {
                error!("not enough energy");
                continue;
            }

            unsafe {
                let bot_id = insert_entity.insert_entity();
                spawn_bot_table
                    .as_mut()
                    .insert_or_update(bot_id, SpawnBotComponent { bot: Bot {} });
                if let Some(owner_id) = intent.owner_id {
                    owner_table
                        .as_mut()
                        .insert_or_update(bot_id, OwnedEntity { owner_id });
                }

                spawn.time_to_spawn = 5;
                spawn.spawning = Some(bot_id);

                spawn_table
                    .as_mut()
                    .insert_or_update(intent.spawn_id, spawn);
            }
        }
    }
}
