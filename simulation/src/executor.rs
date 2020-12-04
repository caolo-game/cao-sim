#[cfg(feature = "mp_executor")]
pub mod mp_executor;

use std::{convert::Infallible, fmt::Debug, pin::Pin};

use slog::{debug, info, o, Logger};

use crate::{
    components::EntityScript, intents, map_generation::generate_full_map,
    map_generation::overworld::OverworldGenerationParams,
    map_generation::room::RoomGenerationParams, map_generation::MapGenError, prelude::EntityId,
    prelude::FromWorldMut, world::init_inmemory_storage, world::World,
};
use crate::{profile, systems::execute_world_update, systems::script_execution::execute_scripts};

#[derive(Debug, Clone)]
pub struct GameConfig {
    pub world_radius: u32,
    pub room_radius: u32,
}

/// Execute world state updates
pub trait Executor {
    type Error: Debug;

    /// Initialize this executor's state and return the initial world state
    fn initialize(
        &mut self,
        logger: Option<Logger>,
        config: GameConfig,
    ) -> Result<Pin<Box<World>>, Self::Error>;
    /// Forward the world state by 1 tick
    fn forward(&mut self, world: &mut World) -> Result<(), Self::Error>;
}

/// The simplest executor.
///
/// Just runs a world update
pub struct SimpleExecutor;

impl Executor for SimpleExecutor {
    type Error = Infallible;

    fn forward(&mut self, world: &mut World) -> Result<(), Self::Error> {
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

    fn initialize(
        &mut self,
        logger: Option<Logger>,
        config: GameConfig,
    ) -> Result<Pin<Box<World>>, Self::Error> {
        let mut world = init_inmemory_storage(logger);

        execute_map_generation(world.logger.clone(), &mut *world, &config)
            .expect("Failed to generate world map");

        Ok(world)
    }
}

fn execute_map_generation(
    logger: Logger,
    world: &mut World,
    config: &GameConfig,
) -> Result<(), MapGenError> {
    let world_radius = config.world_radius;
    let radius = config.room_radius;
    assert!(radius > 6);
    let params = OverworldGenerationParams::builder()
        .with_radius(world_radius as u32)
        .with_room_radius(radius)
        .with_min_bridge_len(3)
        .with_max_bridge_len(radius - 3)
        .build()
        .unwrap();
    let room_params = RoomGenerationParams::builder()
        .with_radius(radius)
        .with_chance_plain(0.33)
        .with_chance_wall(0.33)
        .with_plain_dilation(2)
        .build()
        .unwrap();
    debug!(logger, "generating map {:#?} {:#?}", params, room_params);

    generate_full_map(
        logger.clone(),
        &params,
        &room_params,
        None,
        FromWorldMut::new(world),
    )?;
    debug!(logger, "world generation done");
    Ok(())
}
