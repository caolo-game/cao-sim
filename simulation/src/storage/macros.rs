#[macro_export(local_inner_macros)]
macro_rules! storage {
    (
        module $module: ident
        $(
            key $id:ty, table $row: ty = $name: ident
        ),*
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
                fn view<'a>(&'a self) -> View<'a, $id, $row>{
                    View::from_table(&self.$name)
                }

                fn unsafe_view(&mut self) -> UnsafeView<$id, $row>{
                    UnsafeView::from_table(&mut self.$name)
                }
            }
        )*
    };
}
