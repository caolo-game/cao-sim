use crate::prelude::World;

use super::{
    queen::Queen, world_state::update_world, world_state::WorldIoOptionFlags, MpExcError,
    MpExecutor, Role,
};

use chrono::{DateTime, Utc};
use slog::{debug, info, o, Logger};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub struct Drone {
    /// Timestamp of the queen mutex
    pub queen_mutex: DateTime<Utc>,
}

impl Drone {
    pub async fn update_role<'a>(
        mut self,
        logger: Logger,
        connection: impl sqlx::Executor<'a, Database = sqlx::Postgres>,
        id: Uuid,
        mutex_expiry_ms: i64,
    ) -> Result<Role, MpExcError> {
        let new_expiry = Utc::now() + chrono::Duration::milliseconds(mutex_expiry_ms);

        struct MxRes {
            f1: Option<Uuid>,
            f2: Option<DateTime<Utc>>,
        }

        let result = sqlx::query_as!(
            MxRes,
            r#"SELECT * FROM caolo_sim_try_aquire_queen_mutex($1, $2)"#,
            id,
            new_expiry
        )
        .fetch_one(connection)
        .await?;

        let queen_id = result.f1.expect("Expected mutex aquire to return a row");
        let queen_cont = result.f2.expect("Expected mutex aquire to return a row");

        let success = id == queen_id;

        Ok(if success {
            info!(
                logger,
                "Aquired Queen mutex. Promoting this process to Queen"
            );
            Role::Queen(Queen {
                queen_mutex: queen_cont,
            })
        } else {
            self.queen_mutex = queen_cont;
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
