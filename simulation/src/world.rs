#[cfg(feature = "serde_json")]
mod json_impl;

use crate::components::*;
use crate::indices::*;
use crate::intents::*;
use crate::storage;
use crate::storage::views::{UnsafeView, View};
use crate::tables::morton_hierarchy::ExtendFailure;
use crate::tables::{Component, TableId};
use crate::Time;
use crate::{components::game_config::GameConfig, prelude::Axial};
use serde::Serialize;
use slog::{debug, o, Drain};
use std::{hash::Hasher, pin::Pin};

storage!(
    module room_store key Room,
    table RoomConnections = room_connections,
    table RoomComponent = rooms,
    table OwnedEntity = owner

    iterby rooms
);

storage!(
    module entity_store key EntityId,

    table Bot = bot,
    table PositionComponent = pos,
    table SpawnBotComponent = spawnbot,
    table CarryComponent = carry,
    table Structure = structure,
    table HpComponent = hp,
    table EnergyRegenComponent = energyregen,
    table EnergyComponent = energy,
    table ResourceComponent = resource,
    table DecayComponent = decay,
    table EntityScript = script,
    table SpawnComponent = spawn,
    table SpawnQueueComponent = spawnqueue,
    table OwnedEntity = owner,
    table MeleeAttackComponent = melee,

    attr serde(skip) table PathCacheComponent = pathcache,
    attr serde(skip) table ScriptHistory = script_history

    iterby bot
    iterby structure
    iterby resource
);

storage!(
    module user_store key UserId,

    table UserComponent = user,
    table EntityScript = user_default_script,
    table Rooms = user_rooms,
    table UserProperties = user_props

    iterby user
);

storage!(
    module resource_store key EmptyKey,

    table Time = time,
    table Intents<MoveIntent> = move_intents,
    table Intents<SpawnIntent> = spawn_intents,
    table Intents<MineIntent> = mine_intents,
    table Intents<DropoffIntent> = dropoff_intents,
    table Intents<LogIntent> = log_intents,
    table Intents<CachePathIntent> = update_path_cache_intents,
    table Intents<MutPathCacheIntent> = mut_path_cache_intents,
    table Intents<MeleeIntent> = melee_intents,
    table Intents<ScriptHistoryEntry> = script_history_intents,
    table Intents<DeleteEntityIntent> = delete_entity_intents
);

storage!(
    module config_store key ConfigKey,

    table RoomProperties = room_properties,
    table GameConfig = game_config
);

storage!(
    module positions_store key WorldPosition,
    // don't forget to implement these in `reset_world_storage`
    table TerrainComponent = point_terrain,
    attr serde(skip) table EntityComponent = point_entity
);

#[derive(Debug, Serialize)]
pub struct World {
    pub entities: entity_store::Storage,
    pub room: room_store::Storage,
    pub user: user_store::Storage,
    pub config: config_store::Storage,
    pub resources: resource_store::Storage,
    pub entity_logs: <LogEntry as Component<EntityTime>>::Table,
    pub scripts: <ScriptComponent as Component<ScriptId>>::Table,
    pub positions: positions_store::Storage,

    #[serde(skip)]
    pub deferred_deletes: entity_store::DeferredDeletes,

    pub next_entity: EntityId,

    #[serde(skip)]
    pub logger: slog::Logger,
}

macro_rules! impl_hastable {
    ($module: ident, $field: ident) => {
        impl<C: Component<$module::Key>> storage::HasTable<$module::Key, C> for World
        where
            $module::Storage: storage::HasTable<$module::Key, C>,
        {
            fn view(&self) -> View<$module::Key, C> {
                self.$field.view()
            }

            fn unsafe_view(&mut self) -> UnsafeView<$module::Key, C> {
                self.$field.unsafe_view()
            }
        }
    };
}

impl_hastable!(entity_store, entities);
impl_hastable!(room_store, room);
impl_hastable!(user_store, user);
impl_hastable!(config_store, config);
impl_hastable!(positions_store, positions);
impl_hastable!(resource_store, resources);

impl storage::HasTable<EntityTime, LogEntry> for World {
    fn view(&self) -> View<EntityTime, LogEntry> {
        View::from_table(&self.entity_logs)
    }

    fn unsafe_view(&mut self) -> UnsafeView<EntityTime, LogEntry> {
        UnsafeView::from_table(&mut self.entity_logs)
    }
}

impl storage::HasTable<ScriptId, ScriptComponent> for World {
    fn view(&self) -> View<ScriptId, ScriptComponent> {
        View::from_table(&self.scripts)
    }

    fn unsafe_view(&mut self) -> UnsafeView<ScriptId, ScriptComponent> {
        UnsafeView::from_table(&mut self.scripts)
    }
}

pub fn init_inmemory_storage(logger: impl Into<Option<slog::Logger>>) -> Pin<Box<World>> {
    fn _init(logger: Option<slog::Logger>) -> Pin<Box<World>> {
        match logger {
            Some(ref logger) => debug!(logger, "Init Storage"),
            None => println!("Init Storage"),
        }
        let world = World::new(logger);
        debug!(world.logger, "Init Storage done");
        world
    }

    let logger = logger.into();
    _init(logger)
}

impl World {
    /// Moving World around in memory would invalidate views, so let's make sure it doesn't
    /// happen.
    pub fn new(logger: impl Into<Option<slog::Logger>>) -> Pin<Box<Self>> {
        fn _new(logger: slog::Logger) -> Pin<Box<World>> {
            let mut config: config_store::Storage = Default::default();
            config.game_config.value = Some(Default::default());

            let mut res = Box::pin(World {
                entities: Default::default(),
                room: Default::default(),
                config,
                resources: Default::default(),
                entity_logs: Default::default(),
                scripts: Default::default(),
                positions: Default::default(),
                deferred_deletes: Default::default(),
                next_entity: EntityId::default(),

                logger,

                user: Default::default(),
            });

            // initialize the intent tables
            let botints = crate::intents::BotIntents::default();
            crate::intents::move_into_storage(&mut *res, vec![botints]);
            res
        }

        let logger = logger.into().unwrap_or_else(|| {
            let decorator = slog_term::TermDecorator::new().build();
            let drain = slog_term::FullFormat::new(decorator).build().fuse();
            let drain = slog_envlogger::new(drain).fuse();
            let drain = slog_async::Async::new(drain)
                .overflow_strategy(slog_async::OverflowStrategy::DropAndReport)
                .chan_size(16000)
                .build()
                .fuse();
            slog::Logger::root(drain, o!())
        });

        _new(logger)
    }

    pub fn hash_terrain(&self, mut hasher: impl Hasher) {
        for (wp, TerrainComponent(t)) in self.positions.point_terrain.iter() {
            let WorldPosition {
                room: Axial { q: rq, r: rr },
                pos: Axial { q, r },
            } = wp;
            hasher.write_i32(rq);
            hasher.write_i32(rr);
            hasher.write_i32(q);
            hasher.write_i32(r);
            hasher.write_u8(*t as u8);
        }
    }

    pub fn view<Id: TableId, C: Component<Id>>(&self) -> View<Id, C>
    where
        Self: storage::HasTable<Id, C>,
    {
        <Self as storage::HasTable<Id, C>>::view(self)
    }

    pub fn unsafe_view<Id: TableId, C: Component<Id>>(&mut self) -> UnsafeView<Id, C>
    where
        Self: storage::HasTable<Id, C>,
    {
        <Self as storage::HasTable<Id, C>>::unsafe_view(self)
    }

    pub fn time(&self) -> u64 {
        let view = &self.resources.time.value;
        view.map(|Time(t)| t).unwrap_or(0)
    }

    /// Perform post-tick cleanup on the storage
    pub fn post_process(&mut self) {
        self.deferred_deletes.execute_all(&mut self.entities);
        self.deferred_deletes.clear();

        self.resources.time.value = self
            .resources
            .time
            .value
            .map(|x| Time(x.0 + 1))
            .or(Some(Time(1)));
    }

    pub fn insert_entity(&mut self) -> EntityId {
        use crate::tables::SerialId;

        let res = self.next_entity;
        self.next_entity = self.next_entity.next();
        res
    }

    /// # Safety
    /// This function is safe to call if no references obtained via UnsafeView are held.
    pub unsafe fn reset_world_storage(&mut self) -> Result<&mut Self, ExtendFailure> {
        let rooms = self
            .view::<Room, RoomComponent>()
            .iter()
            .map(|(r, _)| r)
            .collect::<Vec<_>>();

        macro_rules! clear_table {
            ($component: ty) => {
                let mut table = self.unsafe_view::<WorldPosition, $component>();
                table.clear();
                table.extend_rooms(rooms.iter().cloned())?;
            };
        }

        clear_table!(TerrainComponent);
        clear_table!(EntityComponent);

        Ok(self)
    }

    #[cfg(feature = "serde_json")]
    pub fn as_json(&self) -> serde_json::Value {
        use rayon::prelude::*;

        // this can be quite a bit of data, so let's parallelize the serialization
        let result = [
            "bots",
            "structures",
            "resources",
            "terrain",
            "roomProperties",
            "gameConfig",
            "users",
            "rooms",
        ]
        .par_iter()
        .cloned()
        .fold(
            Default::default,
            |mut output: serde_json::Map<String, _>, key: &str| {
                let value: serde_json::Value = match key {
                    "bots" => json_impl::json_serialize_bots(&self),
                    "users" => json_impl::json_serialize_users(&self),
                    "structures" => json_impl::json_serialize_structures(&self),
                    "resources" => json_impl::json_serialize_resources(&self),
                    "terrain" => json_impl::json_serialize_terrain(&self),
                    "roomProperties" => {
                        serde_json::to_value(&self.config.room_properties.value).unwrap()
                    }
                    "gameConfig" => serde_json::to_value(&self.config.game_config.value).unwrap(),
                    "rooms" => json_impl::json_serialize_rooms(&self),
                    _ => unreachable!(),
                };
                output.insert(key.to_string(), value);
                output
            },
        )
        .reduce(serde_json::Map::default, |mut output, rhs| {
            for (key, value) in rhs {
                output.insert(key, value);
            }
            output
        });

        result.into()
    }
}

impl storage::DeferredDeleteById<EntityId> for World
where
    entity_store::DeferredDeletes: storage::DeferredDeleteById<EntityId>,
{
    fn deferred_delete(&mut self, key: EntityId) {
        self.deferred_deletes.deferred_delete(key);
    }

    fn clear_defers(&mut self) {
        self.deferred_deletes.clear_defers();
    }

    fn execute<Store: storage::DeleteById<EntityId>>(&mut self, store: &mut Store) {
        self.deferred_deletes.execute(store);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::setup_testing;

    #[test]
    fn check_world_sanity() {
        setup_testing();
        let _world = init_inmemory_storage(None);
    }

    #[test]
    fn test_bot_serialization() {
        setup_testing();
        let mut world = init_inmemory_storage(None);

        for _ in 0..4 {
            let _entity = world.insert_entity(); // produce gaps
            let entity = world.insert_entity();

            world.entities.bot.insert(entity);
            world
                .entities
                .melee
                .insert_or_update(entity, MeleeAttackComponent { strength: 128 });
            world.entities.pos.insert_or_update(
                entity,
                PositionComponent(WorldPosition {
                    room: Axial::new(42, 69),
                    pos: Axial::new(16, 61),
                }),
            );
        }

        for _ in 0..2 {
            let entity = world.insert_entity();

            world.entities.structure.insert(entity);
            world.entities.pos.insert_or_update(
                entity,
                PositionComponent(WorldPosition {
                    room: Axial::new(42, 69),
                    pos: Axial::new(16, 61),
                }),
            );
        }

        let bots: Vec<_> = world.entities.iterby_bot().collect();
        serde_json::to_string_pretty(&bots).unwrap();

        let structures: Vec<_> = world.entities.iterby_structure().collect();
        serde_json::to_string_pretty(&structures).unwrap();
    }
}
