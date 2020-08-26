//! Table for holding a single Row of data.
//! Intended to be used for configurations.
//!
use super::*;
use crate::indices::EmptyKey;
use serde_derive::{Deserialize, Serialize};
use std::mem;

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct UniqueTable<Row>
where
    Row: TableRow,
{
    pub value: Option<Row>,
}

impl<Row> UniqueTable<Row>
where
    Row: TableRow,
{
    pub fn unwrap_value(&self) -> &Row {
        self.value.as_ref().unwrap()
    }

    pub fn unwrap_mut(&mut self) -> &mut Row {
        self.value.as_mut().unwrap()
    }

    pub fn update(&mut self, value: Option<Row>) {
        self.value = value;
    }
}

impl<Row> Table for UniqueTable<Row>
where
    Row: TableRow,
{
    type Id = EmptyKey;
    type Row = Row;

    fn delete(&mut self, _id: &Self::Id) -> Option<Row> {
        mem::replace(&mut self.value, None)
    }

    fn get_by_id(&self, _id: &Self::Id) -> Option<&Row> {
        self.value.as_ref()
    }
}
