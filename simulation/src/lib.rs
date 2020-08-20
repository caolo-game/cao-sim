pub mod api;
pub mod components;
pub mod geometry;
pub mod map_generation;
pub mod model;
pub mod pathfinding;
pub mod prelude;
pub mod storage;
pub mod tables;

mod data_store;
mod intents;
mod systems;
mod utils;

use log::info;
use systems::execute_world_update;
use systems::script_execution::execute_scripts;

pub use data_store::{init_inmemory_storage, Storage, World};

pub fn forward(storage: &mut World) -> anyhow::Result<()> {
    info!("Executing scripts");
    execute_scripts(storage);
    info!("Executing scripts - done");

    info!("Executing signaling");
    storage.signal_done();
    info!("Executing signaling - done");

    info!("Executing systems update");
    execute_world_update(storage);
    info!("Executing systems update - done");

    info!("-----------Tick {} done-----------", storage.time());
    Ok(())
}
