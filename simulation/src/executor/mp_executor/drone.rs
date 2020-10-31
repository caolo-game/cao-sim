use crate::prelude::World;

use super::{
    queen::Queen, world_state::update_world, world_state::WorldIoOptionFlags, MpExcError,
    MpExecutor, Role, QUEEN_MUTEX,
};

use chrono::{DateTime, TimeZone, Utc};
use slog::{debug, info, o, Logger};

#[derive(Debug, Clone, Copy)]
pub struct Drone {
    /// Timestamp of the queen mutex
    pub queen_mutex: DateTime<Utc>,
}

impl Drone {
    pub async fn update_role(
        mut self,
        logger: Logger,
        connection: &mut redis::aio::Connection,
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
            .query_async(connection)
            .await
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

pub async fn forward_drone(executor: &mut MpExecutor, world: &mut World) -> Result<(), MpExcError> {
    update_world(executor, world, None, WorldIoOptionFlags::new().all()).await?;
    executor.logger = world
        .logger
        .new(o!("tick" => world.time(), "role" => format!("{}", executor.role)));

    info!(executor.logger, "Listening for messages");
    loop {
        // execute jobs
        executor.execute_batch_script_jobs(world).await?;
        let role = executor.update_role().await?;
        if !matches!(role, Role::Drone(_)) {
            info!(executor.logger, "Executor is no longer a Drone!");
            break;
        }
    }
    Ok(())
}
