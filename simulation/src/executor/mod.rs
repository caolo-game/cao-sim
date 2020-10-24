use std::pin::Pin;

use slog::{debug, info, o, Logger};

use crate::{
    components::EntityScript, data_store::init_inmemory_storage, data_store::World, intents,
    prelude::EntityId,
};
use crate::{profile, systems::execute_world_update, systems::script_execution::execute_scripts};

/// Execute world state updates
pub trait Executor {
    /// Initialize this executor's state and return the initial world state
    fn initialize(&mut self, logger: Option<Logger>) -> Pin<Box<World>>;
    /// Forward the world state by 1 tick
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

        let scripts_table = world.view::<EntityId, EntityScript>();
        let executions: Vec<(EntityId, EntityScript)> =
            scripts_table.iter().map(|(id, x)| (id, *x)).collect();

        debug!(logger, "Executing scripts");
        let intents = execute_scripts(executions.as_slice(), world);

        debug!(logger, "Got {} intents", intents.len());
        intents::move_into_storage(world, intents);

        debug!(logger, "Executing systems update");
        execute_world_update(world);

        debug!(logger, "Executing post-processing");
        world.post_process();

        info!(logger, "Tick done");
        Ok(())
    }

    fn initialize(&mut self, logger: Option<Logger>) -> Pin<Box<World>> {
        init_inmemory_storage(logger)
    }
}
