//! Views are designed to be used as function parameters where functions depend on tables in a
//! Storage. They are intended to be used to display data dependencies in the function signatures.
//!
//! Using tuples of views:
//!
//! ```
//! use caolo_sim::components::{Bot, SpawnComponent, PositionComponent,
//! EnergyComponent, EntityComponent, ResourceComponent};
//! use caolo_sim::model::{EntityId, WorldPosition, self};
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
//! let mut storage = World::new();
//! update_minerals(FromWorldMut::new(&mut storage), FromWorld::new(&storage));
//! ```
//!
use super::{Component, DeleteById, TableId};
use crate::model::EntityId;
use crate::World;
use std::ops::Deref;
use std::ptr::NonNull;

/// Fetch read-only tables from a Storage
///
#[derive(Clone, Copy)]
pub struct View<'a, Id: TableId, C: Component<Id>>(&'a C::Table);

unsafe impl<'a, Id: TableId, C: Component<Id>> Send for View<'a, Id, C> {}
unsafe impl<'a, Id: TableId, C: Component<Id>> Sync for View<'a, Id, C> {}

impl<'a, Id: TableId, C: Component<Id>> View<'a, Id, C> {
    pub fn reborrow(self) -> &'a C::Table {
        self.0
    }

    pub fn from_table(t: &'a C::Table) -> Self {
        Self(t)
    }
}

impl<'a, Id: TableId, C: Component<Id>> Deref for View<'a, Id, C> {
    type Target = C::Table;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

pub trait FromWorld<'a> {
    fn new(w: &'a World) -> Self;
}

pub trait FromWorldMut {
    fn new(w: &mut World) -> Self;
    fn log(&self);
}

/// Fetch read-write table reference from a Storage.
/// This is a pretty unsafe way to obtain mutable references. Use with caution.
/// Do not store UnsafeViews for longer than the function scope, that's just asking for trouble.
///
pub struct UnsafeView<Id: TableId, C: Component<Id>>(NonNull<C::Table>);

unsafe impl<Id: TableId, C: Component<Id>> Send for UnsafeView<Id, C> {}
unsafe impl<Id: TableId, C: Component<Id>> Sync for UnsafeView<Id, C> {}

impl<Id: TableId, C: Component<Id>> UnsafeView<Id, C> {
    /// # Safety
    /// This function should only be called if the pointed to Storage is in memory and no other
    /// threads have access to it at this time!
    #[allow(clippy::should_implement_trait)]
    pub unsafe fn as_mut(&mut self) -> &mut C::Table {
        self.0.as_mut()
    }

    pub fn from_table(t: &mut C::Table) -> Self {
        let ptr = unsafe { NonNull::new_unchecked(t) };
        let res: UnsafeView<Id, C> = Self(ptr);
        res.log_table();
        res
    }

    #[inline]
    pub fn log_table(self) {
        #[cfg(feature = "log_tables")]
        {
            trace!(
                "UnsafeView references {:x?}\n{:?}",
                self.0.as_ptr(),
                unsafe { self.0.as_ref() }
            );
        }
    }
}

impl<'a, Id: TableId, C: Component<Id>> FromWorld<'a> for View<'a, Id, C>
where
    crate::data_store::Storage: super::HasTable<Id, C>,
{
    fn new(w: &'a World) -> Self {
        w.view()
    }
}

impl<Id: TableId, C: Component<Id>> FromWorldMut for UnsafeView<Id, C>
where
    crate::data_store::Storage: super::HasTable<Id, C>,
{
    fn new(w: &mut World) -> Self {
        w.unsafe_view()
    }

    fn log(&self) {
        self.log_table();
    }
}

impl<Id: TableId, C: Component<Id>> Clone for UnsafeView<Id, C> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<Id: TableId, C: Component<Id>> Copy for UnsafeView<Id, C> {}

impl<Id: TableId, C: Component<Id>> Deref for UnsafeView<Id, C> {
    type Target = C::Table;

    fn deref(&self) -> &Self::Target {
        unsafe { self.0.as_ref() }
    }
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
        trace!("DeferredDeleteEntityView to storage {:x?}", self.world);
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
        trace!("DeferredDeleteEntityView to storage {:x?}", self.storage);
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
        trace!("InsertEntityView to storage {:x?}", self.storage);
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
