use slog::{debug, info, o};

use crate::data_store::World;
use crate::{profile, systems::execute_world_update, systems::script_execution::execute_scripts};

/// Execute world state updates
pub trait Executor {
    fn forward(&mut self, world: &mut World) -> anyhow::Result<()>;
}

/// The simplest executor.
///
/// Just runs a world update
pub struct SimpleExecutor;

impl Executor for SimpleExecutor {
    fn forward(&mut self, world: &mut World) -> anyhow::Result<()> {
        profile!("world_forward");

        let logger = world.logger.new(o!("tick" => world.time()));

        info!(logger, "Tick starting");

        debug!(logger, "Executing scripts");
        execute_scripts(world);

        debug!(logger, "Executing systems update");
        execute_world_update(world);

        debug!(logger, "Executing signaling");
        world.post_process();

        info!(logger, "Tick done");
        Ok(())
    }
}
