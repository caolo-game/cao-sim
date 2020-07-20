/// # Mutation Queries
///
/// Designed to reduce boilerplate when updating tables.
/// The query does nothing special, just calls the provided methods on the tables
/// obtained by `unsafe_view::<key, value>()`.
///
/// ## Safety
///
/// Updates on Storage are an unsafe operation. Be sure that no other threads have write access to
/// the tables you're mutating!
///
/// ```
/// use caolo_sim::query;
/// use caolo_sim::init_inmemory_storage;
/// use caolo_sim::components::*;
/// use caolo_sim::geometry::Axial;
/// use caolo_sim::model::EntityId;
///
/// let mut store = init_inmemory_storage(None);
///
/// let entity_1 = store.insert_entity();
/// let entity_2 = store.insert_entity();
///
/// query!(
///     mutate
///     store
///     {
///         EntityId, Bot, .insert_or_update(entity_1, Bot);
///         EntityId, Bot, .insert_or_update(entity_2, Bot);
///         EntityId, CarryComponent,
///                  .insert_or_update(entity_1, CarryComponent{carry: 12, carry_max: 69});
///         EntityId, CarryComponent,
///                  .insert_or_update(entity_2, CarryComponent{carry: 0, carry_max: 69});
///     }
/// );
/// ```
#[macro_export]
macro_rules! query {
    (
        mutate
        $store: ident
        {
        $(
            $id: ty, $row: ty, $(.$op: ident ( $($args: expr),* ))*
        );*;
        }
    ) => {
        unsafe {
            $(
                $store.unsafe_view::<$id, $row>().as_mut()
                    . $(
                        $op($($args),*)
                    ).*;
            )*
        }
    };
}

#[macro_export(local_inner_macros)]
macro_rules! storage {
    (
        module $module: ident
        $(
            key $id:ty, table $row: ty = $name: ident
        ),*
        $(,)*
    ) => {
        pub mod $module {
            use super::*;
            use crate::storage::views::{UnsafeView, View};
            use crate::storage::{HasTable, DeleteById, DeferredDeleteById};
            use serde_derive::{Serialize, Deserialize};
            use cao_storage_derive::CaoStorage;
            use crate::tables::Table;

            #[derive(Debug, Serialize, CaoStorage, Default, Deserialize)]
            $(
                #[cao_storage($id, $name)]
            )*
            pub struct Storage {
                $( $name: <$row as crate::tables::Component<$id>>::Table ),+ ,
            }

            storage!(@implement_tables $($name, $id,  $row )*);

            impl Storage {
                #[allow(unused)]
                #[allow(clippy::too_many_arguments)]
                pub fn new(
                    $(
                        $name: <$row as crate::tables::Component<$id>>::Table
                        ),*
                ) -> Self {
                    Self {
                        $( $name ),*
                    }
                }
            }

            unsafe impl Send for Storage {}
        }
    };

    (
        @implement_tables
        $($name: ident, $id: ty,  $row: ty )*
    ) => {
        $(
            impl HasTable<$id, $row> for Storage {
                fn view(&'_ self) -> View<'_, $id, $row>{
                    View::from_table(&self.$name)
                }

                fn unsafe_view(&mut self) -> UnsafeView<$id, $row>{
                    UnsafeView::from_table(&mut self.$name)
                }
            }
        )*
    };
}
