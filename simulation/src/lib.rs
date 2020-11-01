use prelude::{Component, World};
use tables::{unique::UniqueTable, TableId};

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

mod intents;
mod systems;
mod utils;
mod world;

#[derive(Clone, Debug, Default, Copy, serde::Serialize, serde::Deserialize)]
pub struct Time(pub u64);

#[cfg(feature = "mp_executor")]
#[allow(unknown_lints)]
#[allow(clippy::all)]
pub mod job_capnp {
    include!(concat!(env!("OUT_DIR"), "/cpnp/job_capnp.rs"));
}

impl<'a> storage::views::FromWorld<'a> for Time {
    fn new(w: &'a World) -> Self {
        Time(w.time())
    }
}

impl<Id: TableId> Component<Id> for Time {
    type Table = UniqueTable<Id, Time>;
}

#[derive(Clone)]
pub struct RuntimeGuard {}

#[cfg(feature = "mp_executor")]
impl RuntimeGuard {
    pub fn block_on<F>(&self, f: F) -> F::Output
    where
        F: std::future::Future,
    {
        async_std::task::block_on(f)
    }
}

/// ```
/// let _cao_rt = caolo_sim::init_runtime();
/// ```
pub fn init_runtime() -> RuntimeGuard {
    RuntimeGuard {}
}
