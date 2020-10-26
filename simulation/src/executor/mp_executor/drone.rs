use crate::prelude::World;

use super::{
    queen::{self, Queen},
    MpExcError, MpExecutor, Role, QUEEN_MUTEX, WORLD, WORLD_TIME,
};

use chrono::{DateTime, TimeZone, Utc};
use redis::{Commands, Connection};
use slog::{debug, info, o, trace, Logger};

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

pub fn forward_drone(executor: &mut MpExecutor, world: &mut World) -> Result<(), MpExcError> {
    // wait for the updated world
    loop {
        match executor
            .connection
            .get::<_, Option<u64>>(WORLD_TIME)
            .map_err(MpExcError::RedisError)?
        {
            Some(t) if t > world.time() => break,
            _ => {
                trace!(executor.logger, "World has not been updated. Waiting...");
                if matches!(executor.update_role()?, Role::Queen(_)) {
                    info!(
                            executor.logger,
                            "Assumed role of Queen while waiting for world update. Last world state in this executor: tick {}",
                            world.time()
                        );
                    return queen::forward_queen(executor, world);
                }
            }
        }
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
        .new(o!("tick" => world.time(), "role" => format!("{:?}", executor.role)));
    info!(executor.logger, "Tick starting");

    // execute jobs
    executor.execute_batch_script_jobs(world)
}
