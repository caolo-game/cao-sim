use super::{drone::Drone, MpExcError, Role, CAO_PRIMARY_MUTEX_KEY};

use chrono::{DateTime, TimeZone, Utc};
use redis::Connection;
use slog::{debug, info, Logger};

#[derive(Debug, Clone, Copy)]
pub struct Primary {
    /// Timestamp of the primary mutex
    pub primary_mutex: DateTime<Utc>,
}

impl Primary {
    pub fn update_role(
        mut self,
        logger: Logger,
        connection: &mut Connection,
        new_expiry: i64,
        mutex_expiry_ms: i64,
    ) -> Result<Role, MpExcError> {
        // add a bit of bias to let the current Primary re-aquire first
        let res: Option<Vec<String>> = redis::pipe()
            .getset(CAO_PRIMARY_MUTEX_KEY, new_expiry)
            .expire(
                CAO_PRIMARY_MUTEX_KEY,
                (mutex_expiry_ms / 1000) as usize + 1, // round up
            )
            .ignore()
            .query(connection)
            .map_err(MpExcError::RedisError)?;
        let res: Option<i64> = res
            .as_ref()
            .and_then(|s| s.get(0))
            .and_then(|s| s.parse().ok());
        let res = match res {
            Some(res) if res != self.primary_mutex.timestamp_millis() => {
                // another process aquired the mutex
                info!(
                    logger,
                    "Another process has been promoted to Primary. Demoting this process to Drone"
                );
                Role::Drone(Drone {
                    primary_mutex: Utc.timestamp_millis(res),
                })
            }
            _ => {
                self.primary_mutex = Utc.timestamp_millis(new_expiry);
                debug!(
                    logger,
                    "Primary mutex has been re-aquired until {}", self.primary_mutex
                );
                Role::Primary(self)
            }
        };
        Ok(res)
    }
}
