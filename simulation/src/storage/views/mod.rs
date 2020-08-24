//! Views are designed to be used as function parameters where functions depend on tables in a
//! Storage. They are intended to be used to display data dependencies in the function signatures.
//!
//! Using tuples of views:
//!
//! ```
//! use caolo_sim::prelude::*;
//! use caolo_sim::tables::{vector::VecTable, btree::BTreeTable, morton::MortonTable};
//!
//! fn update_minerals(
//!     (mut entity_positions, mut energy): (
//!         UnsafeView<EntityId, PositionComponent>,
//!         UnsafeView<EntityId, EnergyComponent>,
//!     ),
//!     (position_entities, resources): (
//!         View<WorldPosition, EntityComponent>,
//!         View<EntityId, ResourceComponent>,
//!     ),
//! ) {
//!     // do stuff
//! }
//!
//! let mut storage = World::new(None);
//! update_minerals(FromWorldMut::new(&mut storage), FromWorld::new(&storage));
//! ```
//!
mod unsafe_view;
mod unwrap;
mod unwrap_mut;
mod view;
mod world_logger;

#[cfg(feature = "log_tables")]
pub mod logging;

pub use unsafe_view::*;
pub use unwrap::*;
pub use unwrap_mut::*;
pub use view::*;
pub use world_logger::*;

use super::{Component, DeleteById, TableId};
use crate::indices::EntityId;
use crate::World;
use slog::trace;
use std::ptr::NonNull;

pub trait FromWorld<'a> {
    fn new(w: &'a World) -> Self;
}

pub trait FromWorldMut {
    fn new(w: &mut World) -> Self;
    fn log(&self);
}

#[derive(Clone, Copy)]
pub struct DeferredDeleteEntityView {
    world: NonNull<World>,
}

unsafe impl Send for DeferredDeleteEntityView {}
unsafe impl Sync for DeferredDeleteEntityView {}

impl DeferredDeleteEntityView
where
    crate::data_store::World: super::DeferredDeleteById<EntityId>,
{
    /// # Safety
    /// This function should only be called if the pointed to Storage is in memory and no other
    /// threads have access to it at this time!
    pub unsafe fn delete_entity(&mut self, id: EntityId) {
        use super::DeferredDeleteById;

        let world = self.world.as_mut();
        world.deferred_delete(id);
    }
}

impl FromWorldMut for DeferredDeleteEntityView {
    fn new(w: &mut World) -> Self {
        Self {
            world: unsafe { NonNull::new_unchecked(w) },
        }
    }

    fn log(&self) {
        let logger = unsafe { &self.world.as_ref().logger };
        trace!(
            logger,
            "DeferredDeleteEntityView to storage {:x?}",
            self.world
        );
    }
}

#[derive(Clone, Copy)]
pub struct DeleteEntityView {
    storage: NonNull<World>,
}

unsafe impl Send for DeleteEntityView {}
unsafe impl Sync for DeleteEntityView {}

impl DeleteEntityView
where
    crate::data_store::Storage: super::DeleteById<EntityId>,
{
    /// # Safety
    /// This function should only be called if the pointed to Storage is in memory and no other
    /// threads have access to it at this time!
    pub unsafe fn delete_entity(&mut self, id: EntityId) {
        let storage = &mut self.storage.as_mut().store;
        storage.delete(&id);
    }
}

impl FromWorldMut for DeleteEntityView {
    fn new(w: &mut World) -> Self {
        Self {
            storage: unsafe { NonNull::new_unchecked(w) },
        }
    }

    fn log(&self) {
        let logger = unsafe { &self.storage.as_ref().logger };
        trace!(
            logger,
            "DeferredDeleteEntityView to storage {:x?}",
            self.storage
        );
    }
}

#[derive(Clone, Copy)]
pub struct InsertEntityView {
    storage: NonNull<World>,
}

unsafe impl Send for InsertEntityView {}
unsafe impl Sync for InsertEntityView {}

impl FromWorldMut for InsertEntityView {
    fn new(w: &mut World) -> Self {
        Self {
            storage: unsafe { NonNull::new_unchecked(w) },
        }
    }

    fn log(&self) {
        let logger = unsafe { &self.storage.as_ref().logger };
        trace!(logger, "InsertEntityView to storage {:x?}", self.storage);
    }
}

impl InsertEntityView {
    /// # Safety
    /// This function should only be called if the pointed to Storage is in memory and no other
    /// threads have access to it at this time!
    pub unsafe fn insert_entity(&mut self) -> EntityId {
        let storage = self.storage.as_mut();
        storage.insert_entity()
    }
}

macro_rules! implement_tuple {
    ($id: tt = $v: ident) => {
        impl<'a, $v: FromWorld<'a> >
            FromWorld <'a> for ( $v, )
            {
                #[allow(unused)]
                fn new(storage: &'a World) -> Self {
                    (
                        $v::new(storage) ,
                    )
                }
            }

        impl<$v:FromWorldMut >
            FromWorldMut  for ( $v, )
            {
                #[allow(unused)]
                fn new(storage: &mut World) -> Self {
                    (
                        $v::new(storage),
                    )
                }

                fn log(&self) {
                    self.0.log();
                }
            }
    };

    ($($id: tt = $vv: ident),*) => {
        impl<'a, $($vv:FromWorld<'a>),* >
            FromWorld <'a> for ( $($vv),* )
            {
                #[allow(unused)]
                fn new(storage: &'a World) -> Self {
                    (
                        $($vv::new(storage)),*
                    )
                }
            }

        impl<'a, $($vv:FromWorldMut),* >
            FromWorldMut  for ( $($vv),* )
            {
                #[allow(unused)]
                fn new(storage: &mut World) -> Self {
                    (
                        $($vv::new(storage)),*
                    )
                }

                #[allow(unused)]
                fn log(&self) {
                    let a = self;
                    $(
                       a.$id.log();
                    )*
                }
            }
    };
}

implement_tuple!();
implement_tuple!(0 = V1);
implement_tuple!(0 = V1, 1 = V2);
implement_tuple!(0 = V1, 1 = V2, 2 = V3);
implement_tuple!(0 = V1, 1 = V2, 2 = V3, 3 = V4);
implement_tuple!(0 = V1, 1 = V2, 2 = V3, 3 = V4, 4 = V5);
implement_tuple!(0 = V1, 1 = V2, 2 = V3, 3 = V4, 4 = V5, 5 = V6);
implement_tuple!(0 = V1, 1 = V2, 2 = V3, 3 = V4, 4 = V5, 5 = V6, 6 = V7);
implement_tuple!(
    0 = V1,
    1 = V2,
    2 = V3,
    3 = V4,
    4 = V5,
    5 = V6,
    6 = V7,
    7 = V8
);
