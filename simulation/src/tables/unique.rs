//! Table for holding a single Row of data.
//! Intended to be used for configurations.
//!
use super::*;
use serde::{Deserialize, Serialize};
use std::mem;

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct UniqueTable<Id, Row>
where
    Row: TableRow,
{
    pub value: Option<Row>,
    #[serde(skip)]
    _m: std::marker::PhantomData<Id>,
}

impl<Id, Row> UniqueTable<Id, Row>
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

impl<Id: TableId, Row> Table for UniqueTable<Id, Row>
where
    Row: TableRow,
{
    type Id = Id;
    type Row = Row;

    fn delete(&mut self, _id: &Self::Id) -> Option<Row> {
        mem::replace(&mut self.value, None)
    }

    fn get_by_id(&self, _id: &Self::Id) -> Option<&Row> {
        self.value.as_ref()
    }
}
