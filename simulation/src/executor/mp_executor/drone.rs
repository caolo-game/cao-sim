use super::{primary::Primary, MpExcError, Role, CAO_PRIMARY_MUTEX_KEY};

use chrono::{DateTime, TimeZone, Utc};
use redis::Connection;
use slog::{debug, info, Logger};

#[derive(Debug, Clone, Copy)]
pub struct Drone {
    /// Timestamp of the primary mutex
    pub primary_mutex: DateTime<Utc>,
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
        // add a bit of bias to let the current Primary re-aquire first
        let primary_expired =
            now.timestamp_millis() >= (self.primary_mutex.timestamp_millis() + 50);
        if !primary_expired {
            return Ok(Role::Drone(self));
        }
        debug!(logger, "Primary mutex has expired. Attempting to aquire");
        let (success, res) = redis::pipe()
            .cmd("SET")
            .arg(CAO_PRIMARY_MUTEX_KEY)
            .arg(new_expiry)
            .arg("NX")
            .arg("PX")
            .arg(mutex_expiry_ms)
            .get(CAO_PRIMARY_MUTEX_KEY)
            .query(connection)
            .map_err(MpExcError::RedisError)?;
        Ok(if success {
            info!(
                logger,
                "Aquired Primary mutex. Promoting this process to Primary"
            );
            Role::Primary(Primary {
                primary_mutex: Utc.timestamp_millis(res),
            })
        } else {
            self.primary_mutex = Utc.timestamp_millis(res);
            debug!(logger, "Another process aquired the mutex.");
            Role::Drone(self)
        })
    }
}
