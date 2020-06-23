use super::super::HasTable;
use super::{Component, FromWorld, TableId, World};
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

impl<'a, Id: TableId, C: Component<Id>> FromWorld<'a> for View<'a, Id, C>
where
    crate::data_store::Storage: HasTable<Id, C>,
{
    fn new(w: &'a World) -> Self {
        w.view()
    }
}
