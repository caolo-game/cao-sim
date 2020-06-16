//! Table for holding a single Row of data.
//! Intended to be used for configurations.
//!
use super::*;
use crate::model::indices::EmptyKey;
use serde_derive::{Deserialize, Serialize};
use std::mem;

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct UniqueTable<Row>
where
    Row: TableRow,
{
    pub value: Option<Row>,
}

impl<Row> Table for UniqueTable<Row>
where
    Row: TableRow,
{
    type Id = EmptyKey;
    type Row = Row;

    fn delete(&mut self, _id: &Self::Id) -> Option<Row> {
        let mut res = None;
        mem::swap(&mut res, &mut self.value);
        res
    }
}
