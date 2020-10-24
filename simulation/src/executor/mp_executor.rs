//! Multiprocess executor.
//!

use chrono::{DateTime, Duration, TimeZone, Utc};
use redis::Client;
use slog::{debug, info, o, Drain, Logger};

use crate::{data_store::init_inmemory_storage, job_capnp, prelude::World};

use super::Executor;

const CAO_PRIMARY_MUTEX_KEY: &str = "CAO_PRIMARY_MUTEX";

/// Multiprocess executor.
///
#[derive(Debug)]
pub struct MpExecutor {
    pub logger: Logger,

    client: Client,
    role: Role,
    /// Timestamp of the primary mutex
    primary_mutex: DateTime<Utc>,

    mutex_expiry_ms: i64,
}

#[derive(Debug, Clone, Copy)]
pub enum Role {
    /// This is the main/coordinator instance
    Primary,
    /// This is a worker instance
    Drone,
}

#[derive(Debug)]
pub struct ExecutorOptions {
    pub redis_url: String,
    pub primary_mutex_expiry_ms: i64,
}

impl Default for ExecutorOptions {
    fn default() -> Self {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379/0".to_owned());
        Self {
            redis_url,
            primary_mutex_expiry_ms: 2000,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum MpExcError {
    #[error("redis error: {0:?}")]
    RedisError(redis::RedisError),
}

impl MpExecutor {
    pub fn new(
        logger: impl Into<Option<Logger>>,
        options: impl Into<Option<ExecutorOptions>>,
    ) -> Result<Self, MpExcError> {
        fn _new(logger: Logger, options: ExecutorOptions) -> Result<MpExecutor, MpExcError> {
            let client =
                Client::open(options.redis_url.as_str()).map_err(MpExcError::RedisError)?;
            let primary_mutex = Utc.timestamp_millis(0);
            Ok(MpExecutor {
                logger,
                role: Role::Drone,
                client,
                primary_mutex,
                mutex_expiry_ms: options.primary_mutex_expiry_ms,
            })
        }

        let logger = logger.into().unwrap_or_else(|| {
            let decorator = slog_term::TermDecorator::new().build();
            let drain = slog_term::FullFormat::new(decorator).build().fuse();
            let drain = slog_envlogger::new(drain).fuse();
            let drain = slog_async::Async::new(drain).build().fuse();
            slog::Logger::root(drain, o!())
        });

        _new(logger, options.into().unwrap_or_default())
    }

    /// Check if this instance is the Primary and if so still holds the mutex.
    pub fn is_primary(&self) -> bool {
        match self.role {
            Role::Primary => Utc::now() < self.primary_mutex,
            Role::Drone => false,
        }
    }

    /// Returns the current role of this instance
    pub fn update_role(&mut self) -> Result<Role, MpExcError> {
        debug!(self.logger, "Updating role of a {:?} process", self.role);
        let now = Utc::now();
        let new_expiry: i64 =
            (now + Duration::milliseconds(self.mutex_expiry_ms)).timestamp_millis();

        let mut connection = self
            .client
            .get_connection()
            .map_err(MpExcError::RedisError)?;
        match self.role {
            Role::Primary => {
                let res: Option<Vec<String>> = redis::pipe()
                    .getset(CAO_PRIMARY_MUTEX_KEY, new_expiry)
                    .expire(
                        CAO_PRIMARY_MUTEX_KEY,
                        (self.mutex_expiry_ms / 1000) as usize,
                    )
                    .ignore()
                    .query(&mut connection)
                    .map_err(MpExcError::RedisError)?;
                let res: Option<i64> = res
                    .as_ref()
                    .and_then(|s| s.get(0))
                    .and_then(|s| s.parse().ok());
                match res {
                    Some(res) if res != self.primary_mutex.timestamp_millis() => {
                        // another process aquired the mutex
                        info!(self.logger, "Another process has been promoted to Primary. Demoting this process to Drone");
                        self.role = Role::Drone;
                        self.primary_mutex = Utc.timestamp_millis(res);
                    }
                    _ => {
                        self.primary_mutex = Utc.timestamp_millis(new_expiry);
                        debug!(
                            self.logger,
                            "Primary mutex has been re-aquired until {}", self.primary_mutex
                        );
                    }
                }
            }
            Role::Drone => {
                // add a bit of bias to let the current Primary re-aquire first
                let primary_expired =
                    now.timestamp_millis() >= (self.primary_mutex.timestamp_millis() + 50);
                if primary_expired {
                    debug!(
                        self.logger,
                        "Primary mutex has expired. Attempting to aquire"
                    );
                    let (success, res) = redis::pipe()
                        .cmd("SET")
                        .arg(CAO_PRIMARY_MUTEX_KEY)
                        .arg(new_expiry)
                        .arg("NX")
                        .arg("PX")
                        .arg(self.mutex_expiry_ms)
                        .get(CAO_PRIMARY_MUTEX_KEY)
                        .query(&mut connection)
                        .map_err(MpExcError::RedisError)?;
                    if success {
                        info!(
                            self.logger,
                            "Aquired Primary mutex. Promoting this process to Primary"
                        );
                        self.role = Role::Primary;
                    } else {
                        debug!(self.logger, "Another process aquired the mutex.");
                    }
                    self.primary_mutex = Utc.timestamp_millis(res);
                }
            }
        }
        Ok(self.role)
    }
}

impl Executor for MpExecutor {
    fn initialize(&mut self, logger: Option<slog::Logger>) -> std::pin::Pin<Box<World>> {
        if let Some(logger) = logger.as_ref() {
            self.logger = logger.clone();
        }
        self.update_role()
            .expect("Failed to set the initial role of this process");
        init_inmemory_storage(logger)
    }

    fn forward(&mut self, world: &mut World) -> anyhow::Result<()> {
        todo!()
    }
}
