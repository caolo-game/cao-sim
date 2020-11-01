use crate::prelude::World;

use super::{world_state::update_world, world_state::WorldIoOptionFlags, MpExcError, MpExecutor};

use slog::{info, o};

pub async fn forward_drone(executor: &mut MpExecutor, world: &mut World) -> Result<(), MpExcError> {
    update_world(executor, world, None, WorldIoOptionFlags::new().all()).await?;
    executor.logger = world.logger.new(o!(
                "tag" => executor.tag.to_string(),
                "tick" => world.time(),
                "role" => format!("{}", executor.role)));

    info!(executor.logger, "Listening for messages");
    loop {
        // execute jobs
        executor.execute_batch_script_jobs(world).await?;
    }
}
