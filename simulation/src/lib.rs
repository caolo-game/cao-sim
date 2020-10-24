pub mod components;
pub mod executor;
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

#[derive(Clone, Debug, Copy, serde::Serialize, serde::Deserialize)]
pub struct Time(pub u64);

#[cfg(feature = "mp_executor")]
pub mod job_capnp {
    include!(concat!(env!("OUT_DIR"), "/cpnp/job_capnp.rs"));
}
