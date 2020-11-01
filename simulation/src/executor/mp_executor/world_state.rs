use std::{collections::hash_map::DefaultHasher, hash::Hasher};

use serde::{de::DeserializeOwned, Serialize};
use slog::{debug, error, info, Logger};

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
    client: impl sqlx::Executor<'a, Database = sqlx::Postgres>,
    key: &'a str,
    requested_time: Option<u64>,
) -> Result<TimeCodedDe<T>, MpExcError>
where
    T: DeserializeOwned + 'static,
{
    struct Foo {
        value_message_packed: Vec<u8>,
    }
    let data = sqlx::query_as!(
        Foo,
        r#"
    SELECT value_message_packed
    FROM world
    WHERE field=$1
        "#,
        key
    )
    .fetch_one(client)
    .await
    .map_err(MpExcError::SqlxError)?;
    let data: TimeCodedDe<T> = rmp_serde::from_read(data.value_message_packed.as_slice())
        .map_err(MpExcError::WorldDeserializeError)?;
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
    let entities = {
        let mut conn = executor.pool.acquire().await?;
        async move { get_timed_state(&mut conn, WORLD_ENTITIES, requested_time).await }
    };
    let entities = rt.spawn(entities);

    // config isn't updated every tick
    let config = {
        let mut conn = executor.pool.acquire().await?;
        async move { get_timed_state(&mut conn, WORLD_CONFIG, None).await }
    };
    let config = rt.spawn(config);

    let users = {
        let mut conn = executor.pool.acquire().await?;
        async move { get_timed_state(&mut conn, WORLD_USERS, requested_time).await }
    };
    let users = rt.spawn(users);

    let scripts = {
        let mut conn = executor.pool.acquire().await?;
        async move { get_timed_state(&mut conn, WORLD_SCRIPTS, requested_time).await }
    };
    let scripts = rt.spawn(scripts);

    if options.has_option(WorldIoOptions::Terrain) {
        // terrain isn't updated every tick
        let terrain = {
            let mut conn = executor.pool.acquire().await?;
            async move { get_timed_state(&mut conn, WORLD_TERRAIN, None).await }
        };
        let terrain = rt.spawn(terrain);
        let terrain = terrain
            .await
            .expect("Failed to join terrain")
            .map_err(|err| {
                error!(logger, "Failed to get `terrain`, {:?}", err);
                err
            })?;
        world.positions.point_terrain = terrain.value;
        let mut hasher = DefaultHasher::new();
        world.hash_terrain(&mut hasher);
        let hash = hasher.finish();
        info!(executor.logger, "Loaded terrain {:0x}", hash);
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
    world.positions.point_entity.deep_clear();
    world
        .positions
        .point_entity
        .extend_rooms(
            world
                .positions
                .point_terrain
                .iter_rooms()
                .map(|(room, _)| room),
        )
        .expect("Failed to initialize the rooms of entity-positions");
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

    let entities = {
        let mut conn = executor.pool.acquire().await?;
        async move {
            set_timed_state(
                executor.logger.clone(),
                &mut conn,
                WORLD_ENTITIES,
                time,
                &world.entities,
            )
            .await
        }
    };
    let entities = rt.spawn(entities);

    let users = {
        let mut conn = executor.pool.acquire().await?;
        async move {
            set_timed_state(
                executor.logger.clone(),
                &mut conn,
                WORLD_USERS,
                time,
                &world.user,
            )
            .await
        }
    };
    let users = rt.spawn(users);

    let scripts = {
        let mut conn = executor.pool.acquire().await?;
        async move {
            set_timed_state(
                executor.logger.clone(),
                &mut conn,
                WORLD_SCRIPTS,
                time,
                &world.scripts,
            )
            .await
        }
    };
    let scripts = rt.spawn(scripts);

    let config = if options.has_option(WorldIoOptions::Config) {
        let conn = executor.pool.acquire().await?;
        rt.spawn(async move {
            let mut conn = conn;
            set_timed_state(
                executor.logger.clone(),
                &mut conn,
                WORLD_CONFIG,
                time,
                &world.config,
            )
            .await
        })
    } else {
        rt.spawn(async move { Ok(()) })
    };

    let terrain = if options.has_option(WorldIoOptions::Terrain) {
        let conn = executor.pool.acquire().await?;
        let f = rt.spawn(async move {
            let mut conn = conn;
            set_timed_state(
                executor.logger.clone(),
                &mut conn,
                WORLD_TERRAIN,
                time,
                &world.positions.point_terrain,
            )
            .await?;
            Ok::<_, MpExcError>(())
        });
        let mut hasher = DefaultHasher::new();
        world.hash_terrain(&mut hasher);
        let hash = hasher.finish();
        info!(executor.logger, "Sending terrain {:0x}", hash);
        f
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
    client: impl sqlx::Executor<'a, Database = sqlx::Postgres>,
    key: &'a str,
    time: u64,
    value: &'a T,
) -> Result<(), MpExcError>
where
    T: Serialize + 'static,
{
    let payload = TimeCodedSer { time, value };
    let payload: Vec<u8> =
        rmp_serde::to_vec_named(&payload).map_err(MpExcError::WorldSerializeError)?;

    debug!(
        logger,
        "Sending payload of size {} to key {}",
        payload.len(),
        key
    );

    sqlx::query!(
        r#"
    INSERT INTO world (field, world_timestamp,value_message_packed)
    VALUES ($1, $2, $3)
    ON CONFLICT(field)
    DO UPDATE
    SET value_message_packed=$3, world_timestamp=$2, updated=now()
        "#,
        key,
        time as i64,
        payload
    )
    .execute(client)
    .await
    .map_err(MpExcError::SqlxError)?;

    Ok(())
}
