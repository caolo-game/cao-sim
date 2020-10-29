use super::super::HasTable;
use super::{Component, FromWorld, View, World};
use crate::tables::unique::UniqueTable;
use crate::tables::TableId;
use std::ops::Deref;

/// Fetch read-only tables from a Storage
///
pub struct UnwrapView<'a, Id: TableId, C: Component<Id>>(&'a UniqueTable<Id, C>);

impl<'a, Id: TableId, C: Component<Id>> Clone for UnwrapView<'a, Id, C> {
    fn clone(&self) -> Self {
        UnwrapView(self.0)
    }
}

impl<'a, Id: TableId, C: Component<Id>> Copy for UnwrapView<'a, Id, C> {}

unsafe impl<'a, Id: TableId, C: Component<Id>> Send for UnwrapView<'a, Id, C> {}
unsafe impl<'a, Id: TableId, C: Component<Id>> Sync for UnwrapView<'a, Id, C> {}

impl<'a, Id: TableId, C: Component<Id>> UnwrapView<'a, Id, C> {
    pub fn reborrow(self) -> &'a UniqueTable<Id, C> {
        self.0
    }

    pub fn from_table(t: &'a UniqueTable<Id, C>) -> Self {
        Self(t)
    }
}

impl<'a, Id: TableId, C: Component<Id>> Deref for UnwrapView<'a, Id, C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self
            .0
            .value
            .as_ref()
            .expect("UnwrapView dereferenced with an empty table")
    }
}

impl<'a, Id: TableId, C: Default + Component<Id, Table = UniqueTable<Id, C>>> FromWorld<'a>
    for UnwrapView<'a, Id, C>
where
    crate::world::World: HasTable<Id, C>,
{
    fn new(w: &'a World) -> Self {
        let table: &UniqueTable<Id, C> = View::new(w).reborrow();
        UnwrapView(table)
    }
}
