use super::super::HasTable;
use super::{Component, FromWorldMut, TableId};
use crate::prelude::World;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

/// Fetch read-write table reference from a Storage.
/// This is a pretty unsafe way to obtain mutable references. Use with caution.
/// Do not store UnsafeViews for longer than the function scope, that's just asking for trouble.
///
pub struct UnsafeView<Id: TableId, C: Component<Id>>(NonNull<C::Table>);

unsafe impl<Id: TableId, C: Component<Id>> Send for UnsafeView<Id, C> {}
unsafe impl<Id: TableId, C: Component<Id>> Sync for UnsafeView<Id, C> {}

impl<Id: TableId, C: Component<Id>> UnsafeView<Id, C> {
    pub fn as_ptr(&mut self) -> *mut C::Table {
        self.0.as_ptr()
    }

    pub fn from_table(t: &mut C::Table) -> Self {
        let ptr = unsafe { NonNull::new_unchecked(t) };
        let res: UnsafeView<Id, C> = Self(ptr);
        res
    }
}

impl<Id: TableId, C: Component<Id>> FromWorldMut for UnsafeView<Id, C>
where
    crate::world::World: HasTable<Id, C>,
{
    fn new(w: &mut World) -> Self {
        <World as HasTable<Id, C>>::unsafe_view(w)
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

impl<Id: TableId, C: Component<Id>> DerefMut for UnsafeView<Id, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.0.as_mut() }
    }
}
