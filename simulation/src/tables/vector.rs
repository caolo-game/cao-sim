//! Table with `Vec` back-end. Optimised for dense storage.
//! The storage will allocate memory for N items where `N = the largest id inserted`.
//! Because of this one should use this if the domain of the ids is small or dense.
//!
use super::*;
use log::debug;
use rayon::prelude::*;
use serde_derive::{Deserialize, Serialize};
use std::mem;

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct VecTable<Id, Row>
where
    Id: SerialId,
    Row: TableRow,
{
    data: Vec<Option<(Id, Row)>>,
    /// the `as_usize` index of the first item in the vector
    offset: usize,
}

impl<'a, Id, Row> VecTable<Id, Row>
where
    Id: SerialId + Send + Sync,
    Row: TableRow + Send + Sync,
    Vec<Option<(Id, Row)>>: rayon::iter::IntoParallelRefIterator<'a, Item = Option<(Id, Row)>>,
{
    pub fn par_iter(&'a self) -> impl ParallelIterator<Item = (Id, &'a Row)> + 'a {
        self.data[..]
            .par_iter()
            .filter_map(|k| k.as_ref())
            .map(|(id, row)| (*id, row))
    }
}

impl<Id, Row> VecTable<Id, Row>
where
    Id: SerialId,
    Row: TableRow,
{
    pub fn new() -> Self {
        let size = mem::size_of::<(Id, Row)>();
        let size = 1024 / size;
        Self {
            offset: 0,
            data: Vec::with_capacity(size),
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        let size = mem::size_of::<(Id, Row)>();
        let size = 1024 / size;
        Self {
            offset: 0,
            data: Vec::with_capacity(size.min(cap)),
        }
    }

    pub fn insert_or_update(&mut self, id: Id, row: Row) -> bool {
        // Extend the vector if necessary
        let i = id.as_usize();
        let len = self.data.len();
        if i < self.offset {
            self.data.resize(self.offset - i + len, None);
            self.data.rotate_right(self.offset - i);
            self.offset = i;
        }
        let i = i - self.offset;
        if i >= len {
            self.data.resize(i + 1, None);
        }
        if let Some((_, r)) = self.data[i].as_mut() {
            *r = row;
        } else {
            self.data[i] = Some((id, row));
        }
        true
    }

    pub fn get_by_id<'a>(&'a self, id: &Id) -> Option<&'a Row> {
        let ind = id.as_usize();
        if ind < self.offset {
            return None;
        }
        let ind = ind - self.offset;
        self.data
            .get(ind)
            .and_then(|x| x.as_ref().map(|(_, row)| row))
    }

    /// This table might have 'gaps' in the storage
    /// Meaning that a `len` method has to count the non-null elements.
    ///
    pub fn count_set(&self) -> usize {
        self.iter().count()
    }

    pub fn iter<'a>(&'a self) -> impl TableIterator<Id, &'a Row> + 'a {
        self.data
            .iter()
            .skip(self.offset)
            .filter_map(|k| k.as_ref())
            .map(move |(id, row)| (*id, row))
    }

    pub fn iter_mut<'a>(&'a mut self) -> impl TableIterator<Id, &'a mut Row> + 'a {
        self.data
            .iter_mut()
            .skip(self.offset)
            .filter_map(|k| k.as_mut())
            .map(move |(id, row)| (*id, row))
    }

    pub fn contains_id(&self, id: &Id) -> bool {
        let i = id.as_usize();
        if i < self.offset {
            return false;
        }
        let i = i - self.offset;
        // contains if data has this key AND it is Some
        self.data.get(i).and_then(|x| x.as_ref()).is_some()
    }
}

impl<Id, Row> Table for VecTable<Id, Row>
where
    Id: SerialId,
    Row: TableRow,
{
    type Id = Id;
    type Row = Row;

    fn delete(&mut self, id: &Id) -> Option<Row> {
        use std::any::type_name;
        debug!(
            "Deleting id {:?} in VecTable<{}, {}>",
            id,
            type_name::<Id>(),
            type_name::<Row>()
        );
        if !self.contains_id(id) {
            debug!("Table did not contain {:?}", id);
            return None;
        }
        debug!("Table contains {:?}, removing", id);
        let ind = id.as_usize() - self.offset;
        self.data.push(None);
        let res = self.data.swap_remove(ind);
        res.map(|(_, row)| row)
    }
}
