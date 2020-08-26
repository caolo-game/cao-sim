use super::*;
use crate::components::LogEntry;
use crate::indices::EntityTime;
use rayon::prelude::*;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct BTreeTable<Id, Row>
where
    Id: TableId,
    Row: TableRow,
{
    data: BTreeMap<Id, Row>,
}
impl<'a, Id, Row> BTreeTable<Id, Row>
where
    Id: TableId + Send,
    Row: TableRow + Send,
    BTreeMap<Id, Row>: rayon::iter::IntoParallelRefIterator<'a>,
{
    pub fn par_iter(
        &'a self,
    ) -> impl ParallelIterator<
        Item = <BTreeMap<Id, Row> as rayon::iter::IntoParallelRefIterator<'_>>::Item,
    > + 'a {
        self.data.par_iter()
    }
}

impl<Id, Row> BTreeTable<Id, Row>
where
    Id: TableId,
    Row: TableRow,
{
    pub fn new() -> Self {
        Self {
            data: BTreeMap::new(),
        }
    }

    pub fn iter<'a>(&'a self) -> impl TableIterator<Id, &'a Row> + 'a {
        self.data.iter().map(|(id, row)| (*id, row))
    }

    pub fn iter_mut<'a>(&'a mut self) -> impl TableIterator<Id, &'a mut Row> + 'a {
        self.data.iter_mut().map(|(id, row)| (*id, row))
    }

    pub fn get_by_id<'a>(&'a self, id: &Id) -> Option<&'a Row> {
        self.data.get(id)
    }

    pub fn get_by_id_mut<'a>(&'a mut self, id: &Id) -> Option<&'a mut Row> {
        self.data.get_mut(id)
    }

    pub fn get_by_ids<'a>(&'a self, ids: &[Id]) -> Vec<(Id, &'a Row)> {
        self.data
            .iter()
            .filter(move |(i, _)| ids.iter().any(|x| *i == x))
            .map(move |(i, v)| (*i, v))
            .collect()
    }

    pub fn contains(&self, id: &Id) -> bool {
        self.data.get(id).is_some()
    }

    pub fn insert_or_update(&mut self, id: Id, row: Row) -> bool {
        self.data.insert(id, row);
        true
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl<Id, Row> Table for BTreeTable<Id, Row>
where
    Id: TableId,
    Row: TableRow,
{
    type Id = Id;
    type Row = Row;

    fn delete(&mut self, id: &Id) -> Option<Row> {
        self.data.remove(id)
    }

    fn get_by_id(&self, id: &Id) -> Option<&Row> {
        BTreeTable::get_by_id(self, id)
    }
}

impl LogTable for BTreeTable<EntityTime, LogEntry> {
    fn get_logs_by_time(&self, time: u64) -> Vec<(EntityTime, LogEntry)> {
        self.data
            .par_iter()
            .filter(|(t, _)| t.1 == time)
            .map(|(k, v)| (*k, v.clone()))
            .collect()
    }
}
