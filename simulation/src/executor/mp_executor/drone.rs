use crate::prelude::World;

use super::{
    queen::{self, Queen},
    MpExcError, MpExecutor, Role, QUEEN_MUTEX, UPDATE_FENCE, WORLD, WORLD_TIME_FENCE,
};

use chrono::{DateTime, TimeZone, Utc};
use redis::{Commands, Connection};
use slog::{debug, info, o, trace, warn, Logger};

#[derive(Debug, Clone, Copy)]
pub struct Drone {
    /// Timestamp of the queen mutex
    pub queen_mutex: DateTime<Utc>,
}

impl Drone {
    pub fn update_role(
        mut self,
        logger: Logger,
        connection: &mut Connection,
        now: DateTime<Utc>,
        new_expiry: i64,
        mutex_expiry_ms: i64,
    ) -> Result<Role, MpExcError> {
        // add a bit of bias to let the current Queen re-aquire first
        let queen_expired = now.timestamp_millis() >= (self.queen_mutex.timestamp_millis() + 50);
        if !queen_expired {
            return Ok(Role::Drone(self));
        }
        debug!(logger, "Queen mutex has expired. Attempting to aquire");
        let (success, res) = redis::pipe()
            .cmd("SET")
            .arg(QUEEN_MUTEX)
            .arg(new_expiry)
            .arg("NX")
            .arg("PX")
            .arg(mutex_expiry_ms)
            .get(QUEEN_MUTEX)
            .query(connection)
            .map_err(MpExcError::RedisError)?;
        Ok(if success {
            info!(
                logger,
                "Aquired Queen mutex. Promoting this process to Queen"
            );
            Role::Queen(Queen {
                queen_mutex: Utc.timestamp_millis(res),
            })
        } else {
            self.queen_mutex = Utc.timestamp_millis(res);
            debug!(logger, "Another process aquired the mutex.");
            Role::Drone(self)
        })
    }
}

#[derive(thiserror::Error, Debug)]
enum FenceError {
    #[error("Got a new role while waiting for fence: {0}")]
    NewRole(Role),
    #[error("Error while waiting for fence: {0}")]
    MpExcError(MpExcError),
}

/// Waits until the value at the given key is larger than the given value.
/// Assumes that the current role is `Drone`
///
/// Returns the new value of the fence
fn wait_for_fence(
    executor: &mut MpExecutor,
    key: &str,
    current_value: impl Into<Option<u64>>,
) -> Result<u64, FenceError> {
    fn _wait(
        executor: &mut MpExecutor,
        key: &str,
        current_value: Option<u64>,
    ) -> Result<u64, FenceError> {
        loop {
            match executor
                .connection
                .get::<_, Option<u64>>(key)
                .map_err(MpExcError::RedisError)
                .map_err(FenceError::MpExcError)?
            {
                // if current_value is None than any value will break the loop
                Some(t) if current_value.map(|v| v < t).unwrap_or(true) => break Ok(t),
                _ => {
                    trace!(executor.logger, "World has not been updated. Waiting...");
                    let role = executor.update_role().map_err(FenceError::MpExcError)?;
                    if matches!(role, Role::Queen(_)) {
                        return Err(FenceError::NewRole(*role));
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    _wait(executor, key, current_value.into())
}

pub fn forward_drone(executor: &mut MpExecutor, world: &mut World) -> Result<(), MpExcError> {
    let current_time = world.time();
    info!(executor.logger, "Waiting for {} fence", WORLD_TIME_FENCE);
    match wait_for_fence(executor, WORLD_TIME_FENCE, current_time) {
        Ok(_) => {}
        Err(FenceError::NewRole(Role::Drone(_))) => unreachable!(),
        Err(FenceError::NewRole(Role::Queen(_))) => {
            let logger = &executor.logger;
            warn!(logger, "Assumed role of Queen while waiting for world update. Last world state in this executor: tick {}", world.time());
            return queen::forward_queen(executor, world);
        }
        Err(FenceError::MpExcError(err)) => return Err(err),
    }

    // update world
    let store: Vec<Vec<u8>> = redis::pipe()
        .get(WORLD)
        .query(&mut executor.connection)
        .map_err(MpExcError::RedisError)?;
    let store: crate::data_store::Storage =
        rmp_serde::from_slice(&store[0][..]).map_err(MpExcError::WorldDeserializeError)?;
    world.store = store;
    executor.logger = world
        .logger
        .new(o!("tick" => world.time(), "role" => format!("{}", executor.role)));

    info!(executor.logger, "Waiting for {} fence", UPDATE_FENCE);
    match wait_for_fence(executor, UPDATE_FENCE, current_time) {
        Ok(_) => {}
        Err(FenceError::NewRole(Role::Drone(_))) => unreachable!(),
        Err(FenceError::NewRole(Role::Queen(_))) => {
            let logger = &executor.logger;
            warn!(logger, "Assumed role of Queen while waiting for world update. Last world state in this executor: tick {}", world.time());
            return queen::forward_queen(executor, world);
        }
        Err(FenceError::MpExcError(err)) => return Err(err),
    }

    info!(executor.logger, "Tick starting");

    // execute jobs
    executor.execute_batch_script_jobs(world)
}
