use super::{queen::Queen, MpExcError, Role, CAO_QUEEN_MUTEX_KEY};

use chrono::{DateTime, TimeZone, Utc};
use redis::Connection;
use slog::{debug, info, Logger};

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
            .arg(CAO_QUEEN_MUTEX_KEY)
            .arg(new_expiry)
            .arg("NX")
            .arg("PX")
            .arg(mutex_expiry_ms)
            .get(CAO_QUEEN_MUTEX_KEY)
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
