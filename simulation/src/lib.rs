pub mod components;
pub mod geometry;
pub mod indices;
pub mod map_generation;
pub mod pathfinding;
pub mod prelude;
pub mod scripting_api;
pub mod storage;
pub mod tables;
pub mod terrain;

mod data_store;
mod intents;
mod systems;
mod utils;

use serde_derive::{Deserialize, Serialize};
use slog::{debug, info, o};
use systems::execute_world_update;
use systems::script_execution::execute_scripts;

pub use data_store::{init_inmemory_storage, Storage, World};

#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
pub struct Time(pub u64);

/// Forward the simulation by 1 tick
pub fn forward(storage: &mut World) -> anyhow::Result<()> {
    let logger = storage.logger.new(o!("tick" => storage.time()));

    info!(logger, "Tick starting");

    debug!(logger, "Executing scripts");
    execute_scripts(storage);

    debug!(logger, "Executing systems update");
    execute_world_update(storage);

    debug!(logger, "Executing signaling");
    storage.signal_done();

    info!(logger, "Tick done");
    Ok(())
}
