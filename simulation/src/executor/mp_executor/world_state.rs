use redis::Client as RedisClient;
use serde::{de::DeserializeOwned, Serialize};
use slog::{debug, error, Logger};

use crate::{
    prelude::FromWorld, prelude::FromWorldMut, prelude::World, systems::positions_system, Time,
};

use super::{
    MpExcError, MpExecutor, WORLD_CONFIG, WORLD_ENTITIES, WORLD_SCRIPTS, WORLD_TERRAIN, WORLD_USERS,
};

#[derive(serde::Serialize)]
pub struct TimeCodedSer<'a, T> {
    time: u64,
    value: &'a T,
}

#[derive(serde::Deserialize)]
pub struct TimeCodedDe<T> {
    time: u64,
    value: T,
}

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum WorldIoOptions {
    Terrain = 1,
    Config = 1 << 1,
}

#[derive(Default, Clone, Copy)]
pub struct WorldIoOptionFlags(u8);

impl WorldIoOptionFlags {
    #[inline]
    pub fn new() -> Self {
        Self(0)
    }

    #[allow(unused)]
    #[inline]
    pub fn all(&mut self) -> Self {
        self.0 = 0xff;
        *self
    }

    #[allow(unused)]
    #[inline]
    pub fn disable_option(&mut self, opt: WorldIoOptions) -> Self {
        self.0 = self.0 & !(opt as u8);
        *self
    }

    #[allow(unused)]
    #[inline]
    pub fn with_option(&mut self, opt: WorldIoOptions) -> Self {
        self.0 = self.0 | opt as u8;
        *self
    }

    #[inline]
    pub fn has_option(self, opt: WorldIoOptions) -> bool {
        (self.0 & opt as u8) != 0
    }
}

pub async fn get_timed_state<'a, T>(
    client: &'a RedisClient,
    key: &'a str,
    requested_time: Option<u64>,
) -> Result<TimeCodedDe<T>, MpExcError>
where
    T: DeserializeOwned + 'static,
{
    let mut connection = client
        .get_async_connection()
        .await
        .map_err(MpExcError::RedisError)?;
    let store: Vec<Vec<u8>> = redis::pipe()
        .get(key)
        .query_async(&mut connection)
        .await
        .map_err(MpExcError::RedisError)?;
    let data: TimeCodedDe<T> =
        rmp_serde::from_read(&store[0][..]).map_err(MpExcError::WorldDeserializeError)?;
    match requested_time {
        Some(requested_time) if data.time != requested_time => Err(MpExcError::WorldTimeMismatch {
            requested: requested_time,
            actual: data.time,
        }),
        _ => Ok(data),
    }
}

pub async fn update_world<'a>(
    executor: &'a mut MpExecutor,
    world: &mut World,
    requested_time: Option<u64>,
    options: WorldIoOptionFlags,
) -> Result<(), MpExcError> {
    // SAFETY
    // we will block until all these tasks complete, so this lifetime should be fine
    let executor = unsafe { &mut *(executor as *mut MpExecutor) as &'static mut MpExecutor };
    let rt = &executor.runtime.tokio_rt;
    let logger = executor.logger.clone();
    //
    // Update world in parallel.
    //
    // Create the get+deserialize task for each Store
    // Spawn the task on the tokio runtime
    //
    let entities = get_timed_state(&executor.client, WORLD_ENTITIES, requested_time);
    let entities = rt.spawn(entities);

    // config isn't updated every tick
    let config = get_timed_state(&executor.client, WORLD_CONFIG, None);
    let config = rt.spawn(config);

    let users = get_timed_state(&executor.client, WORLD_USERS, requested_time);
    let users = rt.spawn(users);

    let scripts = get_timed_state(&executor.client, WORLD_SCRIPTS, requested_time);
    let scripts = rt.spawn(scripts);

    if options.has_option(WorldIoOptions::Terrain) {
        // terrain isn't updated every tick
        let terrain = get_timed_state(&executor.client, WORLD_TERRAIN, None);
        let terrain = rt.spawn(terrain);
        let terrain = terrain
            .await
            .expect("Failed to join terrain")
            .map_err(|err| {
                error!(logger, "Failed to get `terrain`, {:?}", err);
                err
            })?;
        world.positions.point_terrain = terrain.value;
    }

    // Finally wait for all tasks to complete
    let entities = entities
        .await
        .expect("Failed to join entities")
        .map_err(|err| {
            error!(logger, "Failed to get `entities`, {:?}", err);
            err
        })?;
    world.entities = entities.value;
    world.resources.time.value = Some(Time(entities.time));
    // reset the positions storage
    positions_system::update(FromWorldMut::new(world), FromWorld::new(world));

    let users = users.await.expect("Failed to join users").map_err(|err| {
        error!(logger, "Failed to get `users`, {:?}", err);
        err
    })?;
    world.user = users.value;

    let scripts = scripts
        .await
        .expect("Failed to join scripts")
        .map_err(|err| {
            error!(logger, "Failed to get `scripts`, {:?}", err);
            err
        })?;
    world.scripts = scripts.value;

    if options.has_option(WorldIoOptions::Config) {
        let config = config
            .await
            .expect("Failed to join config")
            .map_err(|err| {
                error!(logger, "Failed to get `config`, {:?}", err);
                err
            })?;
        world.config = config.value;
    }

    // TODO: check for time match...

    Ok(())
}

/// TODO broadcast changesets instead of the whole state
/// `options`: bit flags representing `WorldIoOptions`
pub async fn send_world<'a>(
    executor: &'a MpExecutor,
    world: &World,
    options: WorldIoOptionFlags,
) -> Result<(), MpExcError> {
    debug!(executor.logger, "Sending world state");

    let time = world.time();

    // SAFETY
    // We will await all tasks at the end of this function
    // This is just letting the Rust compiler know that the lifetimes are fine
    let world = unsafe { &*(world as *const World) as &'static World };
    let executor = unsafe { &*(executor as *const MpExecutor) as &'static MpExecutor };
    let rt = &executor.runtime.tokio_rt;

    let entities = set_timed_state(
        executor.logger.clone(),
        &executor.client,
        WORLD_ENTITIES,
        time,
        &world.entities,
    );
    let entities = rt.spawn(entities);

    let users = set_timed_state(
        executor.logger.clone(),
        &executor.client,
        WORLD_USERS,
        time,
        &world.user,
    );
    let users = rt.spawn(users);

    let scripts = set_timed_state(
        executor.logger.clone(),
        &executor.client,
        WORLD_SCRIPTS,
        time,
        &world.scripts,
    );
    let scripts = rt.spawn(scripts);

    let config = if options.has_option(WorldIoOptions::Config) {
        let config = set_timed_state(
            executor.logger.clone(),
            &executor.client,
            WORLD_CONFIG,
            time,
            &world.config,
        );
        rt.spawn(async move {
            config.await?;
            Ok(())
        })
    } else {
        rt.spawn(async move { Ok(()) })
    };

    let terrain = if options.has_option(WorldIoOptions::Terrain) {
        let config = set_timed_state(
            executor.logger.clone(),
            &executor.client,
            WORLD_TERRAIN,
            time,
            &world.positions.point_terrain,
        );
        rt.spawn(async move {
            config.await?;
            Ok(())
        })
    } else {
        rt.spawn(async move { Ok(()) })
    };

    entities.await.expect("Failed to join entitites")?;
    users.await.expect("Failed to join users")?;
    scripts.await.expect("Failed to join scripts")?;
    config.await.expect("Failed to join config")?;
    terrain.await.expect("Failed to join terrain")?;

    Ok(())
}

pub async fn set_timed_state<'a, T>(
    logger: Logger,
    client: &'a RedisClient,
    key: &'a str,
    time: u64,
    value: &'a T,
) -> Result<(), MpExcError>
where
    T: Serialize + 'static,
{
    let mut connection = client
        .get_async_connection()
        .await
        .map_err(MpExcError::RedisError)?;

    let payload = TimeCodedSer { time, value };
    let payload: Vec<u8> =
        rmp_serde::to_vec_named(&payload).map_err(MpExcError::WorldSerializeError)?;

    debug!(
        logger,
        "Sending payload of size {} to key {}",
        payload.len(),
        key
    );

    redis::pipe()
        .set(key, payload)
        .ignore()
        .query_async(&mut connection)
        .await
        .map_err(MpExcError::RedisError)?;

    Ok(())
}
