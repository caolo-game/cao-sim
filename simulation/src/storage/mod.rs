mod macros;
pub mod views;

use crate::tables::{Component, TableId};
use views::{UnsafeView, View};

pub trait HasTable<Id: TableId, Row: Component<Id>> {
    fn view(&self) -> View<Id, Row>;
    fn unsafe_view(&mut self) -> UnsafeView<Id, Row>;
}

pub trait DeleteById<Id: TableId> {
    fn delete(&mut self, key: &Id);
}

pub trait DeferredDeleteById<Id: TableId> {
    fn deferred_delete(&mut self, key: Id);
    fn clear_defers(&mut self);
    fn execute<Store: DeleteById<Id>>(&mut self, store: &mut Store);
}
