use super::super::HasTable;
use super::{Component, FromWorld, TableId, World};
use std::ops::Deref;

/// Fetch read-only tables from a Storage
///
pub struct View<'a, Id: TableId, C: Component<Id>>(&'a C::Table);

impl<'a, Id: TableId, C: Component<Id>> Clone for View<'a, Id, C> {
    fn clone(&self) -> Self {
        View(self.0)
    }
}

impl<'a, Id: TableId, C: Component<Id>> Copy for View<'a, Id, C> {}

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

impl<'a, Id: TableId, C: Component<Id>> FromWorld<'a> for View<'a, Id, C>
where
    crate::world::World: HasTable<Id, C>,
{
    fn new(w: &'a World) -> Self {
        <World as HasTable<Id,C>>::view(w)
    }
}
