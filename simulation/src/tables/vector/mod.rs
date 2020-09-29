//! Table with `Vec` back-end. Optimised for dense storage.
//! The storage will allocate memory for N items where `N = the largest id inserted`.
//! Because of this one should use this if the domain of the ids is small or dense.
//!
mod serde;

pub use self::serde::*;

use super::*;
use rayon::prelude::*;
use std::mem;
use thiserror::Error;

#[derive(Default, Debug)]
pub struct DenseVecTable<Id, Row>
where
    Id: SerialId,
    Row: TableRow,
{
    /// the `as_usize` index of the first item in the vector
    offset: usize,
    ids: Vec<Option<Id>>,
    data: Vec<mem::MaybeUninit<Row>>,
}

#[derive(Debug, Error)]
pub enum VecTableError<Id: std::fmt::Debug> {
    #[error("Attempted to insert {0:?} twice")]
    DuplicateEntry(Id),
    #[error("Insertion assumes a sorted range")]
    UnsortedValues,
}

impl<Id, Row> Drop for DenseVecTable<Id, Row>
where
    Id: SerialId,
    Row: TableRow,
{
    fn drop(&mut self) {
        // get the indices of set items
        for i in self
            .ids
            .iter()
            .enumerate()
            .filter_map(|(i, id)| id.map(|_| i))
        {
            let data = mem::replace(&mut self.data[i], mem::MaybeUninit::uninit());
            // drop the data
            let _data = unsafe { data.assume_init() };
        }
    }
}

impl<'a, Id, Row> DenseVecTable<Id, Row>
where
    // TODO: this `Sync` requirement is bullshit, get rid of it
    Id: SerialId + Send + Sync,
    Row: TableRow + Send + Sync,
    // if the underlying vector implements par_iter_mut...
    Vec<mem::MaybeUninit<Row>>:
        rayon::iter::IntoParallelRefMutIterator<'a, Item = mem::MaybeUninit<Row>>,
{
    pub fn par_iter_mut(&'a mut self) -> impl ParallelIterator<Item = (Id, &'a mut Row)> + 'a {
        let keys = self.ids.as_slice();
        self.data[..]
            .par_iter_mut()
            .enumerate()
            .filter_map(move |(i, k)| unsafe {
                let id = *keys.as_ptr().add(i);
                id.map(|id| (id, &mut *k.as_mut_ptr()))
            })
    }
}

impl<'a, Id, Row> DenseVecTable<Id, Row>
where
    Id: SerialId + Send + Sync,
    Row: TableRow + Send + Sync,
    // if the underlying vector implements par_iter...
    Vec<mem::MaybeUninit<Row>>:
        rayon::iter::IntoParallelRefIterator<'a, Item = mem::MaybeUninit<Row>>,
{
    pub fn par_iter(&'a self) -> impl ParallelIterator<Item = (Id, &'a Row)> + 'a {
        let keys = self.ids.as_slice();
        self.data[..]
            .par_iter()
            .enumerate()
            .filter_map(move |(i, k)| unsafe {
                let id = *keys.as_ptr().add(i);
                id.map(|id| (id, &*k.as_ptr()))
            })
    }
}

impl<Id, Row> DenseVecTable<Id, Row>
where
    Id: SerialId,
    Row: TableRow,
{
    pub fn new() -> Self {
        let size = mem::size_of::<(Id, Row)>();
        let size = 1024 / size;
        Self {
            offset: 0,
            ids: Vec::with_capacity(size),
            data: Vec::with_capacity(size),
        }
    }

    /// Create a table from a slice of tuples.
    ///
    /// Requires that every id in the slice is unique and are in sorted order
    pub fn from_sorted_slice(data: &[(Id, Row)]) -> Result<Self, VecTableError<Id>> {
        if data.is_empty() {
            return Ok(Self::new());
        }
        let offset = data[0].0.as_usize();
        let mut res = Self {
            offset,
            ids: vec![None; data.len()],
            data: Vec::with_capacity(data.len()),
        };
        res.data.resize_with(data.len(), mem::MaybeUninit::uninit);
        res.ids[0] = Some(data[0].0);
        unsafe {
            *res.data[0].as_mut_ptr() = data[0].1.clone();
        }
        if data.len() == 1 {
            return Ok(res);
        }
        let mut last = data[0].0;
        for (id, row) in data[1..].iter() {
            if id == &last {
                return Err(VecTableError::DuplicateEntry(last));
            }
            if id < &last {
                return Err(VecTableError::UnsortedValues);
            }
            last = *id;
            let i = id.as_usize() - offset;
            res.ids[i] = Some(*id);
            unsafe {
                *res.data[i].as_mut_ptr() = row.clone();
            }
        }
        Ok(res)
    }

    pub fn with_capacity(cap: usize) -> Self {
        let size = mem::size_of::<(Id, Row)>();
        let size = 1024 / size;
        Self {
            offset: 0,
            ids: Vec::with_capacity(size.min(cap)),
            data: Vec::with_capacity(size.min(cap)),
        }
    }

    pub fn insert_or_update(&mut self, id: Id, row: Row) -> bool {
        // Extend the vector if necessary
        let i = id.as_usize();
        let len = self.data.len();
        if i < self.offset {
            let new_len = self.offset - i + len;
            self.ids.resize(new_len, None);
            self.data.resize_with(new_len, mem::MaybeUninit::uninit);
            self.data.rotate_right(self.offset - i);
            self.offset = i;
        }
        let i = i - self.offset;
        if i >= len {
            self.ids.resize(i + 1, None);
            self.data.resize_with(i + 1, mem::MaybeUninit::uninit);
        }
        if self.ids.get(i).and_then(|x| x.as_ref()).is_some() {
            let _old = mem::replace(&mut self.data[i], mem::MaybeUninit::new(row));
        } else {
            unsafe {
                *self.data[i].as_mut_ptr() = row;
            }
            self.ids[i] = Some(id);
        }
        true
    }

    pub fn get_by_id<'a>(&'a self, id: &Id) -> Option<&'a Row> {
        let ind = id.as_usize();
        if ind < self.offset {
            return None;
        }
        let ind = ind - self.offset;
        self.ids
            .get(ind)
            .and_then(|id| id.map(|_| unsafe { &*self.data[ind].as_ptr() }))
    }

    /// This table might have 'gaps' in the storage
    /// Meaning that a `len` method has to count the non-null elements.
    ///
    pub fn count_set(&self) -> usize {
        self.iter().count()
    }

    pub fn iter<'a>(&'a self) -> impl TableIterator<Id, &'a Row> + 'a {
        let data = &self.data;
        self.ids
            .iter()
            .enumerate()
            .filter_map(|(i, k)| k.map(|id| (i, id)))
            .map(move |(i, id)| {
                let row = unsafe { &*data[i].as_ptr() };
                (id, row)
            })
    }

    pub fn iter_mut<'a>(&'a mut self) -> impl TableIterator<Id, &'a mut Row> + 'a {
        let data = &mut self.data;
        self.ids
            .iter()
            .enumerate()
            .filter_map(|(i, k)| k.map(|id| (i, id)))
            .map(move |(i, id)| {
                let row = unsafe { &mut *data[i].as_mut_ptr() };
                (id, row)
            })
    }

    pub fn contains_id(&self, id: &Id) -> bool {
        let i = id.as_usize();
        if i < self.offset {
            return false;
        }
        let i = i - self.offset;
        // contains if data has this key AND it is Some
        self.ids.get(i).map(|x| x.is_some()).unwrap_or(false)
    }
}

impl<Id, Row> Table for DenseVecTable<Id, Row>
where
    Id: SerialId,
    Row: TableRow,
{
    type Id = Id;
    type Row = Row;

    fn delete(&mut self, id: &Id) -> Option<Row> {
        if !self.contains_id(id) {
            return None;
        }
        let ind = id.as_usize() - self.offset;

        self.ids[ind] = None;
        let res = mem::replace(&mut self.data[ind], mem::MaybeUninit::uninit());
        let res = unsafe { res.assume_init() };
        Some(res)
    }

    fn get_by_id(&self, id: &Id) -> Option<&Row> {
        DenseVecTable::get_by_id(self, id)
    }
}
