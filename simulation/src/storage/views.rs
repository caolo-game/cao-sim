//! Views are designed to be used as function parameters where functions depend on tables in a
//! Storage. They are intended to be used to display data dependencies in the function signatures.
//!
//! Using tuples of views:
//!
//! ```
//! use caolo_sim::model::{EntityId,components::{Bot, SpawnComponent, PositionComponent,
//! EnergyComponent, EntityComponent, ResourceComponent} ,geometry::Point, self};
//! use caolo_sim::prelude::*;
//! use caolo_sim::tables::{VecTable,BTreeTable, MortonTable};
//!
//! fn update_minerals(
//!     (mut entity_positions, mut energy): (
//!         UnsafeView<EntityId, PositionComponent>,
//!         UnsafeView<EntityId, EnergyComponent>,
//!     ),
//!     (position_entities, resources): (
//!         View<Point, EntityComponent>,
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
use super::{Component, Epic, TableId};
use crate::model::EntityId;
use crate::World;
use std::ops::Deref;

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
}

/// Fetch read-write table reference from a Storage.
/// This is a pretty unsafe way to obtain mutable references. Use with caution.
/// Do not store UnsafeViews for longer than the function scope, that's just asking for trouble.
///
pub struct UnsafeView<Id: TableId, C: Component<Id>>(*mut C::Table);

unsafe impl<Id: TableId, C: Component<Id>> Send for UnsafeView<Id, C> {}
unsafe impl<Id: TableId, C: Component<Id>> Sync for UnsafeView<Id, C> {}

impl<Id: TableId, C: Component<Id>> UnsafeView<Id, C> {
    /// # Safety
    /// This function should only be called if the pointed to Storage is in memory and no other
    /// threads have access to it at this time!
    pub unsafe fn as_mut(&mut self) -> &mut C::Table {
        &mut *self.0
    }

    pub fn from_table(t: &mut C::Table) -> Self {
        Self(t)
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
        unsafe { &*self.0 }
    }
}

pub struct DeleteEntityView {
    storage: *mut World,
}

unsafe impl Send for DeleteEntityView {}
unsafe impl Sync for DeleteEntityView {}

impl DeleteEntityView
where
    crate::data_store::Storage: super::Epic<EntityId>,
{
    /// # Safety
    /// This function should only be called if the pointed to Storage is in memory and no other
    /// threads have access to it at this time!
    pub unsafe fn delete_entity(&mut self, id: &EntityId) {
        let storage: &mut crate::data_store::Storage = &mut (*self.storage).store;
        storage.delete(id);
    }
}

impl FromWorldMut for DeleteEntityView {
    fn new(w: &mut World) -> Self {
        Self {
            storage: w as *mut _,
        }
    }
}

pub struct InsertEntityView {
    storage: *mut World,
}

unsafe impl Send for InsertEntityView {}
unsafe impl Sync for InsertEntityView {}

impl FromWorldMut for InsertEntityView {
    fn new(w: &mut World) -> Self {
        Self {
            storage: w as *mut _,
        }
    }
}

impl InsertEntityView {
    /// # Safety
    /// This function should only be called if the pointed to Storage is in memory and no other
    /// threads have access to it at this time!
    pub unsafe fn insert_entity(&mut self) -> EntityId {
        let storage = &mut *self.storage;
        storage.insert_entity()
    }
}

macro_rules! implement_tuple {
    ($v: ident) => {
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
            }
    };

    ($($vv: ident),*) => {
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
            }
    };
}

implement_tuple!();
implement_tuple!(V1);
implement_tuple!(V1, V2);
implement_tuple!(V1, V2, V3);
implement_tuple!(V1, V2, V3, V4);
implement_tuple!(V1, V2, V3, V4, V5);
implement_tuple!(V1, V2, V3, V4, V5, V6);
implement_tuple!(V1, V2, V3, V4, V5, V6, V7);
implement_tuple!(V1, V2, V3, V4, V5, V6, V7, V8);
implement_tuple!(V1, V2, V3, V4, V5, V6, V7, V8, V9);
implement_tuple!(V1, V2, V3, V4, V5, V6, V7, V8, V9, V10);
implement_tuple!(V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11);
implement_tuple!(V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11, V12);
implement_tuple!(V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11, V12, V13);
implement_tuple!(V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11, V12, V13, V14);
