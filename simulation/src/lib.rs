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
use systems::intent_system::execute_intents;
use systems::script_execution::execute_scripts;

pub use data_store::{init_inmemory_storage, Storage, World};

pub fn forward(storage: &mut World) -> anyhow::Result<()> {
    info!("Executing scripts");
    let final_intents = execute_scripts(storage);
    info!("Executing scripts - done");

    storage.signal_done(&final_intents);

    info!("Executing intents");
    execute_intents(final_intents, storage);
    info!("Executing intents - done");
    info!("Executing systems update");
    execute_world_update(storage);
    info!("Executing systems update - done");

    info!("-----------Tick {} done-----------", storage.time());
    Ok(())
}
