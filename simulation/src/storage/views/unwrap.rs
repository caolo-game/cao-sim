use super::super::HasTable;
use super::{Component, FromWorld, World};
use crate::model::indices::EmptyKey;
use crate::tables::unique::UniqueTable;
use std::ops::Deref;

/// Fetch read-only tables from a Storage
///
pub struct UnwrapView<'a, C: Component<EmptyKey>>(&'a UniqueTable<C>);

impl<'a, C: Component<EmptyKey>> Clone for UnwrapView<'a, C> {
    fn clone(&self) -> Self {
        UnwrapView(self.0)
    }
}

impl<'a, C: Component<EmptyKey>> Copy for UnwrapView<'a, C> {}

unsafe impl<'a, C: Component<EmptyKey>> Send for UnwrapView<'a, C> {}
unsafe impl<'a, C: Component<EmptyKey>> Sync for UnwrapView<'a, C> {}

impl<'a, C: Component<EmptyKey>> UnwrapView<'a, C> {
    pub fn reborrow(self) -> &'a UniqueTable<C> {
        self.0
    }

    pub fn from_table(t: &'a UniqueTable<C>) -> Self {
        Self(t)
    }
}

impl<'a, C: Component<EmptyKey>> Deref for UnwrapView<'a, C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self
            .0
            .value
            .as_ref()
            .expect("UnwrapView dereferenced with an empty table")
    }
}

impl<'a, C: Default + Component<EmptyKey, Table = UniqueTable<C>>> FromWorld<'a>
    for UnwrapView<'a, C>
where
    crate::data_store::Storage: HasTable<EmptyKey, C>,
{
    fn new(w: &'a World) -> Self {
        let table: &UniqueTable<C> = w.view::<EmptyKey, C>().reborrow();
        UnwrapView(table)
    }
}
