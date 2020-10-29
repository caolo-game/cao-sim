#[cfg(feature = "mp_executor")]
pub mod mp_executor;

use std::{fmt::Debug, pin::Pin};

use slog::{debug, info, o, Logger};

use crate::{
    components::EntityScript, intents, prelude::EntityId, world::init_inmemory_storage,
    world::World,
};
use crate::{profile, systems::execute_world_update, systems::script_execution::execute_scripts};

/// Execute world state updates
pub trait Executor {
    type Error: Debug;

    /// Initialize this executor's state and return the initial world state
    fn initialize(&mut self, logger: Option<Logger>) -> Result<Pin<Box<World>>, Self::Error>;
    /// Forward the world state by 1 tick
    fn forward(&mut self, world: &mut World) -> Result<(), Self::Error>;
}

/// The simplest executor.
///
/// Just runs a world update
pub struct SimpleExecutor;

impl Executor for SimpleExecutor {
    type Error = anyhow::Error;

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

    fn initialize(&mut self, logger: Option<Logger>) -> Result<Pin<Box<World>>, Self::Error> {
        Ok(init_inmemory_storage(logger))
    }
}
