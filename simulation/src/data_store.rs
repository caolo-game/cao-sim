pub use self::store_impl::*;

use super::storage;
use crate::components::*;
use crate::indices::*;
use crate::intents::*;
use crate::profile;
use crate::storage::views::{UnsafeView, View};
use crate::tables::morton_hierarchy::ExtendFailure;
use crate::tables::{Component, TableId};
use crate::Time;
use chrono::{DateTime, Duration, Utc};
use serde_derive::Serialize;
use slog::debug;
use slog::{o, Drain};
use std::pin::Pin;

#[cfg(feature = "log_tables")]
use crate::storage::views::logging::LogGuard;

storage!(
    module store_impl

    key EntityId, table Bot = entitybot,
    key EntityId, table PositionComponent = entitypos,
    key EntityId, table SpawnBotComponent = entityspawnbot,
    key EntityId, table CarryComponent = entitycarry,
    key EntityId, table Structure = entitystructure,
    key EntityId, table HpComponent = entityhp,
    key EntityId, table EnergyRegenComponent = entityenergyregen,
    key EntityId, table EnergyComponent = entityenergy,
    key EntityId, table ResourceComponent = entityresource,
    key EntityId, table DecayComponent = entitydecay,
    key EntityId, table EntityScript = entityscript,
    key EntityId, table SpawnComponent = entityspawn,
    key EntityId, table OwnedEntity = entityowner,
    key EntityId, table PathCacheComponent = entitypathcache,

    key EntityTime, table LogEntry = timelog,

    key UserId, table UserComponent = useruser,

    key ScriptId, table ScriptComponent = scriptscript,

    key Room, table RoomConnections = roomconnections,
    key Room, table RoomComponent = rooms,

    // don't forget to implement these in `reset_world_storage`
    key WorldPosition, table TerrainComponent = pointterrain,
    key WorldPosition, table EntityComponent = pointentity,

    // intents
    key EmptyKey, table Intents<MoveIntent> = move_intents,
    key EmptyKey, table Intents<SpawnIntent> = spawn_intents,
    key EmptyKey, table Intents<MineIntent> = mine_intents,
    key EmptyKey, table Intents<DropoffIntent> = dropoff_intents,
    key EmptyKey, table Intents<LogIntent> = log_intents,
    key EmptyKey, table Intents<CachePathIntent> = update_path_cache_intents,
    key EmptyKey, table Intents<MutPathCacheIntent> = mut_path_cache_intents,

    key EmptyKey, table RoomProperties = roomproperties,
);

#[derive(Debug, Serialize)]
pub struct World {
    pub store: Storage,
    #[serde(skip)]
    pub deferred_deletes: DeferredDeletes,

    pub time: u64,
    pub next_entity: EntityId,
    pub last_tick: DateTime<Utc>,
    #[serde(skip)]
    pub dt: Duration,

    #[serde(skip)]
    pub logger: slog::Logger,

    #[cfg(feature = "log_tables")]
    #[serde(skip)]
    _guard: LogGuard,
}

impl<Id: TableId, C: Component<Id>> storage::HasTable<Id, C> for World
where
    Storage: storage::HasTable<Id, C>,
{
    fn view(&self) -> View<Id, C> {
        self.store.view()
    }

    fn unsafe_view(&mut self) -> UnsafeView<Id, C> {
        self.store.unsafe_view()
    }
}

pub fn init_inmemory_storage(logger: impl Into<Option<slog::Logger>>) -> Pin<Box<World>> {
    profile!("init_inmemory_storage");
    let logger = logger.into();
    match logger {
        Some(ref logger) => debug!(logger, "Init Storage"),
        None => println!("Init Storage"),
    }

    let world = World::new(logger);
    let world = Box::pin(world);

    debug!(world.logger, "Init Storage done");
    world
}

unsafe impl Send for World {}

impl Default for World {
    fn default() -> Self {
        Self::new(None)
    }
}

impl World {
    pub fn new(logger: impl Into<Option<slog::Logger>>) -> Self {
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
        let store = Storage::default();
        let deferred_deletes = DeferredDeletes::default();
        Self {
            time: 0,
            store,
            deferred_deletes,
            last_tick: Utc::now(),
            next_entity: EntityId::default(),
            dt: Duration::zero(),

            logger,

            #[cfg(feature = "log_tables")]
            _guard: LogGuard {
                fname: "./tables.log".to_owned(),
            },
        }
    }

    pub fn view<Id: TableId, C: Component<Id>>(&self) -> View<Id, C>
    where
        Storage: storage::HasTable<Id, C>,
    {
        (&self.store as &dyn storage::HasTable<Id, C>).view()
    }

    pub fn unsafe_view<Id: TableId, C: Component<Id>>(&mut self) -> UnsafeView<Id, C>
    where
        Storage: storage::HasTable<Id, C>,
    {
        (&mut self.store as &mut dyn storage::HasTable<Id, C>).unsafe_view()
    }

    pub fn delete<Id: TableId>(&mut self, id: &Id)
    where
        Storage: storage::DeleteById<Id>,
    {
        let storage = &mut self.store as &mut dyn storage::DeleteById<Id>;
        storage.delete(id);
    }

    pub fn delta_time(&self) -> Duration {
        self.dt
    }

    pub fn time(&self) -> u64 {
        self.time
    }

    /// Perform post-tick cleanup on the storage
    pub fn signal_done(&mut self) {
        self.deferred_deletes.execute_all(&mut self.store);
        self.deferred_deletes.clear();

        let now = Utc::now();
        self.dt = now - self.last_tick;
        self.last_tick = now;
        self.time += 1;
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

        macro_rules! table {
            ($component: ty) => {
                let mut table = self.unsafe_view::<WorldPosition, $component>();
                table.clear();
                table.extend_rooms(rooms.iter().cloned())?;
            };
        };

        table!(TerrainComponent);
        table!(EntityComponent);

        Ok(self)
    }
}

impl<Id> storage::DeferredDeleteById<Id> for World
where
    Id: TableId,
    DeferredDeletes: storage::DeferredDeleteById<Id>,
{
    fn deferred_delete(&mut self, key: Id) {
        self.deferred_deletes.deferred_delete(key);
    }

    fn clear_defers(&mut self) {
        self.deferred_deletes.clear_defers();
    }

    fn execute<Store: storage::DeleteById<Id>>(&mut self, store: &mut Store) {
        self.deferred_deletes.execute(store);
    }
}

impl<'a> storage::views::FromWorld<'a> for Time {
    fn new(w: &'a World) -> Self {
        Time(w.time())
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
}
