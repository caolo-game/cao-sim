#[macro_use]
extern crate log;

pub mod api;
pub mod model;
pub mod prelude;
pub mod storage;
pub mod tables;

mod data_store;
mod intents;
mod systems;
mod utils;

use systems::execute_world_update;
use systems::intent_system::execute_intents;
use systems::script_execution::execute_scripts;

pub use data_store::{init_inmemory_storage, World};

pub fn forward(storage: &mut World) -> Result<(), Box<dyn std::error::Error>> {
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

    crate::utils::profiler::save_global();
    info!("-----------Tick {} done-----------", storage.time());
    Ok(())
}
