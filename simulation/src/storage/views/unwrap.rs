use super::super::HasTable;
use super::{Component, FromWorld, FromWorldMut, World};
use crate::indices::EmptyKey;
use crate::tables::unique::UniqueTable;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

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

pub struct UnwrapViewMut<C: Component<EmptyKey>>(NonNull<UniqueTable<C>>);

impl<'a, C: Component<EmptyKey>> Clone for UnwrapViewMut<C> {
    fn clone(&self) -> Self {
        UnwrapViewMut(self.0)
    }
}

impl<'a, C: Component<EmptyKey>> Copy for UnwrapViewMut<C> {}

unsafe impl<'a, C: Component<EmptyKey>> Send for UnwrapViewMut<C> {}
unsafe impl<'a, C: Component<EmptyKey>> Sync for UnwrapViewMut<C> {}

impl<'a, C: Component<EmptyKey>> UnwrapViewMut<C> {
    pub fn from_table(t: &mut UniqueTable<C>) -> Self {
        let ptr = unsafe { NonNull::new_unchecked(t) };
        Self(ptr)
    }
}

impl<'a, C: Component<EmptyKey>> Deref for UnwrapViewMut<C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        unsafe { self.0.as_ref() }
            .value
            .as_ref()
            .expect("UnwrapViewMut dereferenced with an empty table")
    }
}

impl<'a, C: Component<EmptyKey>> DerefMut for UnwrapViewMut<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.0.as_mut() }
            .value
            .as_mut()
            .expect("UnwrapViewMut dereferenced with an empty table")
    }
}

impl<'a, C: Default + Component<EmptyKey, Table = UniqueTable<C>>> FromWorldMut for UnwrapViewMut<C>
where
    crate::data_store::Storage: HasTable<EmptyKey, C>,
{
    fn new(w: &mut World) -> Self {
        let table = w.unsafe_view::<EmptyKey, C>().as_ptr();
        UnwrapViewMut(NonNull::new(table).unwrap())
    }

    fn log(&self) {
        #[cfg(feature = "log_tables")]
        {
            use super::logging;
            use crate::tables::traits::Table;

            let key = C::Table::name();

            let table = unsafe { self.0.as_ref() };
            let val = serde_json::to_value(table).expect("Table serialization failed");

            let mut table = logging::TABLE_LOG_HISTORY
                .lock()
                .expect("Failed to aquire TABLE_LOG_HISTORY");
            let logger = table.entry(key).or_insert_with(|| Default::default());
            let logger = unsafe { logger.inserter() };
            logger(val);
        }
    }
}
