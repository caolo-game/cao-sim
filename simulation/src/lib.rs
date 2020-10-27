use prelude::{Component, EmptyKey, World};
use tables::unique::UniqueTable;

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

#[derive(Clone, Debug, Default, Copy, serde::Serialize, serde::Deserialize)]
pub struct Time(pub u64);

#[cfg(feature = "mp_executor")]
pub mod job_capnp {
    include!(concat!(env!("OUT_DIR"), "/cpnp/job_capnp.rs"));
}

impl<'a> storage::views::FromWorld<'a> for Time {
    fn new(w: &'a World) -> Self {
        Time(w.time())
    }
}

impl Component<EmptyKey> for Time {
    type Table = UniqueTable<Time>;
}

#[derive(Clone)]
pub struct RuntimeGuard {
    /// This underlying executor is subject to change so let's not publish that...
    #[cfg(feature = "mp_executor")]
    pub(crate) tokio_rt: std::sync::Arc<tokio::runtime::Runtime>,
}

#[cfg(feature = "mp_executor")]
impl RuntimeGuard {
    pub fn block_on<F>(&self, f: F) -> F::Output
    where
        F: std::future::Future,
    {
        self.tokio_rt.block_on(f)
    }
}

/// Initializes the global executors.
/// As we rely on both tokio and rayon it's best to tweak their internals a bit.
///
/// ```
/// let _cao_guard = caolo_sim::init_runtime();
/// ```
pub fn init_runtime() -> RuntimeGuard {
    #[cfg(not(feature = "disable-parallelism"))]
    {
        #[cfg(feature = "mp_executor")]
        let tokio_rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .build()
            .expect("Failed to init tokio runtime");

        RuntimeGuard {
            #[cfg(feature = "mp_executor")]
            tokio_rt: std::sync::Arc::new(tokio_rt),
        }
    }
    #[cfg(feature = "disable-parallelism")]
    {
        RuntimeGuard {}
    }
}
