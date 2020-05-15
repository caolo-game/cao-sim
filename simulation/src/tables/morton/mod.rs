//! Linear Quadtree.
//! # Contracts:
//! - Key axis must be in the interval [0, 2^16)
//! This is a severe restriction on the keys that can be used, however dense queries and
//! constructing from iterators is much faster than quadtrees.
//!

mod find_key_partition;
mod litmax_bigmin;
mod morton_key;
mod sorting;
#[cfg(test)]
mod tests;

pub use self::litmax_bigmin::msb_de_bruijn;
use super::*;
use crate::model::{components::EntityComponent, geometry::Point};
use litmax_bigmin::litmax_bigmin;
use morton_key::*;
use serde_derive::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};
use thiserror::Error;

use crate::profile;

// at most 15 bits long non-negative integers
// having the 16th bit set might create problems in find_key
pub const MORTON_POS_MAX: i32 = 0b0111111111111111;

#[derive(Debug, Clone, Error)]
pub enum ExtendFailure<Id: SpatialKey2d> {
    #[error("Failed to insert poision {0:?}")]
    InvalidPosition(Id),
}

const SKIP_LEN: usize = 8;
type SkipList = [u32; SKIP_LEN];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MortonTable<Pos, Row>
where
    Pos: SpatialKey2d,
    Row: TableRow,
{
    skiplist: SkipList,
    skipstep: u32,
    // ---- 9 * 4 bytes so far
    // assuming 64 byte long L1 cache lines we can fit 10 keys
    //
    // keys is 24 bytes in memory
    keys: Vec<MortonKey>,
    positions: Vec<Pos>,
    values: Vec<Row>,
}

impl<Pos, Row> Default for MortonTable<Pos, Row>
where
    Pos: SpatialKey2d + Send,
    Row: TableRow + Send,
{
    fn default() -> Self {
        Self {
            skiplist: [0; SKIP_LEN],
            skipstep: 0,
            keys: Default::default(),
            values: Default::default(),
            positions: Default::default(),
        }
    }
}

unsafe impl<Pos, Row> Send for MortonTable<Pos, Row>
where
    Pos: SpatialKey2d + Send,
    Row: TableRow + Send,
{
}

impl<Pos, Row> MortonTable<Pos, Row>
where
    Pos: SpatialKey2d + Sync,
    Row: TableRow + Send + Sync,
{
    pub fn new() -> Self {
        Self {
            skiplist: Default::default(),
            skipstep: 0,
            keys: vec![],
            values: vec![],
            positions: vec![],
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            skiplist: Default::default(),
            skipstep: 0,
            values: Vec::with_capacity(cap),
            keys: Vec::with_capacity(cap),
            positions: Vec::with_capacity(cap),
        }
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = (Pos, &'a Row)> + 'a {
        let values = self.values.as_ptr();
        self.positions.iter().enumerate().map(move |(i, id)| {
            let val = unsafe { &*values.add(i) };
            (*id, val)
        })
    }

    pub fn from_iterator<It>(it: It) -> Result<Self, ExtendFailure<Pos>>
    where
        It: Iterator<Item = (Pos, Row)>,
    {
        let mut res = Self::new();
        res.extend(it)?;
        Ok(res)
    }

    pub fn clear(&mut self) {
        self.keys.clear();
        self.skiplist = [Default::default(); SKIP_LEN];
        self.values.clear();
        self.positions.clear();
    }

    /// Extend the map by the items provided. Panics on invalid items.
    pub fn extend<It>(&mut self, it: It) -> Result<(), ExtendFailure<Pos>>
    where
        It: Iterator<Item = (Pos, Row)>,
    {
        for (id, value) in it {
            if !self.intersects(&id) {
                return Err(ExtendFailure::InvalidPosition(id));
            }
            let [x, y] = id.as_array();
            let [x, y] = [x as u16, y as u16];
            let key = MortonKey::new(x, y);
            self.keys.push(key);
            self.positions.push(id);
            self.values.push(value);
        }
        sorting::sort(&mut self.keys, &mut self.positions, &mut self.values);
        self.rebuild_skip_list();
        Ok(())
    }

    fn rebuild_skip_list(&mut self) {
        #[cfg(debug_assertions)]
        {
            // assert that keys is sorted.
            // at the time of writing is_sorted is still unstable
            if self.keys.len() > 2 {
                let mut it = self.keys.iter();
                let mut current = it.next().unwrap();
                for item in it {
                    assert!(current <= item);
                    current = item;
                }
            }
        }

        let len = self.keys.len();
        let step = len / SKIP_LEN;
        self.skipstep = step as u32;
        if step < 1 {
            if let Some(key) = self.keys.last() {
                self.skiplist[0] = key.0;
            }
            return;
        }
        for (i, k) in (0..len).step_by(step).skip(1).take(SKIP_LEN).enumerate() {
            self.skiplist[i] = self.keys[k].0;
        }
    }

    /// May trigger reordering of items, if applicable prefer `extend` and insert many keys at once.
    pub fn insert(&mut self, id: Pos, row: Row) -> bool {
        if !self.intersects(&id) {
            return false;
        }
        let [x, y] = id.as_array();
        let [x, y] = [x as u16, y as u16];

        let ind = self
            .keys
            .binary_search(&MortonKey::new(x, y))
            .unwrap_or_else(|i| i);
        self.keys.insert(ind, MortonKey::new(x, y));
        self.positions.insert(ind, id);
        self.values.insert(ind, row);
        self.rebuild_skip_list();
        true
    }

    /// Return false if id is not in the map, otherwise override the first instance found
    pub fn update(&mut self, id: Pos, row: Row) -> bool {
        if !self.intersects(&id) {
            return false;
        }
        self.find_key(&id)
            .map(|ind| {
                self.values[ind] = row;
            })
            .is_ok()
    }

    /// Returns the first item with given id, if any
    pub fn get_by_id<'a>(&'a self, id: &Pos) -> Option<&'a Row> {
        profile!("get_by_id");

        if !self.intersects(&id) {
            return None;
        }

        self.find_key(id).map(|ind| &self.values[ind]).ok()
    }

    pub fn contains_key(&self, id: &Pos) -> bool {
        profile!("contains_key");

        if !self.intersects(&id) {
            return false;
        }
        self.find_key(id).is_ok()
    }

    /// Find the position of `id` or the position where it needs to be inserted to keep the
    /// container sorted
    fn find_key(&self, id: &Pos) -> Result<usize, usize> {
        let [x, y] = id.as_array();
        let key = MortonKey::new(x as u16, y as u16);

        self.find_key_morton(&key)
    }

    /// Find the position of `key` or the position where it needs to be inserted to keep the
    /// container sorted
    fn find_key_morton(&self, key: &MortonKey) -> Result<usize, usize> {
        use find_key_partition::find_key_partition;

        let step = self.skipstep as usize;
        if step == 0 {
            return self.keys.binary_search(&key);
        }

        let index = find_key_partition(&self.skiplist, &key);

        let (begin, end) = {
            if index < 8 {
                let begin = index * step;
                let end = self.keys.len().min(begin + step + 1);
                (begin, end)
            } else {
                debug_assert!(self.keys.len() >= step + 3);
                let end = self.keys.len();
                let begin = end - step - 3;
                (begin, end)
            }
        };
        self.keys[begin..end]
            .binary_search(&key)
            .map(|ind| ind + begin)
            .map_err(|ind| ind + begin)
    }

    /// For each id returns the first item with given id, if any
    pub fn get_by_ids<'a>(&'a self, ids: &[Pos]) -> Vec<(Pos, &'a Row)> {
        profile!("get_by_ids");

        ids.into_iter()
            .filter_map(|id| self.get_by_id(id).map(|row| (*id, row)))
            .collect()
    }

    /// Filter all in Pos'(P) in Circle (C,r) where ||C-P|| < r
    /// This is a simplfication of `query_range`, mainly here for backwards compatibility
    pub fn find_by_range<'a>(&'a self, center: &Pos, radius: u32, out: &mut Vec<(Pos, &'a Row)>) {
        self.query_range(center, radius, &mut |id, v| {
            out.push((id, v));
        });
    }

    pub fn query_range<'a, Op>(&'a self, center: &Pos, radius: u32, op: &mut Op)
    where
        Op: FnMut(Pos, &'a Row) -> (),
    {
        debug_assert!(
            radius & 0xefff == radius,
            "Radius must fit into 31 bits!; {} != {}",
            radius,
            radius & 0xefff
        );
        let r = i32::try_from(radius).expect("radius to fit into 31 bits");

        let [x, y] = center.as_array();
        let min = MortonKey::new((x - r).max(0) as u16, (y - r).max(0) as u16);
        let max = MortonKey::new(
            ((x + r).min(MORTON_POS_MAX)) as u16,
            ((y + r).min(MORTON_POS_MAX)) as u16,
        );
        self.query_range_impl(center, radius, min, max, op);
    }

    fn query_range_impl<'a>(
        &'a self,
        center: &Pos,
        radius: u32,
        min: MortonKey,
        max: MortonKey,
        op: &mut impl FnMut(Pos, &'a Row) -> (),
    ) {
        let (imin, pmin) = self
            .find_key_morton(&min)
            .map(|i| (i, self.positions[i].as_array()))
            .unwrap_or_else(|i| {
                let [x, y] = min.as_point();
                (i, [x as i32, y as i32])
            });

        let (imax, pmax) = self
            .find_key_morton(&max)
            // add 1 to include this node in the range query as otherwise an element might be
            // missed
            .map(|i| (i + 1, self.positions[i].as_array()))
            .unwrap_or_else(|i| {
                let [x, y] = max.as_point();
                (i, [x as i32, y as i32])
            });

        if imax < imin {
            return;
        }

        // The original paper counts the garbage items and splits above a threshold.
        // Instead let's speculate if we need a split or if it more beneficial to just scan the
        // range
        // The number I picked is more or less arbitrary, it is a power of two and I ran the basic
        // benchmarks to probe a few numbers.
        if imax - imin > 32 {
            let [x, y] = pmin;
            let pmin = [x as u32, y as u32];
            let [x, y] = pmax;
            let pmax = [x as u32, y as u32];
            let [litmax, bigmin] = litmax_bigmin(min.0, pmin, max.0, pmax);
            // split and recurse
            self.query_range_impl(center, radius, min, litmax, op);
            self.query_range_impl(center, radius, bigmin, max, op);
            return;
        }

        let values = self.values.as_ptr();

        for (i, id) in self.positions[imin..imax].iter().enumerate() {
            if center.dist(id) < radius {
                let val = unsafe { &*values.add(i + imin) };
                op(*id, val);
            }
        }
    }

    /// Count in AABB
    pub fn count_in_range<'a>(&'a self, center: &Pos, radius: u32) -> u32 {
        profile!("count_in_range");

        let r = i32::try_from(radius).expect("radius to fit into 31 bits");
        let min = *center + Pos::new(-r, -r);
        let max = *center + Pos::new(r, r);

        let [min, max] = self.morton_min_max(&min, &max);

        self.positions[min..max]
            .iter()
            .filter(move |id| center.dist(&id) < radius)
            .count()
            .try_into()
            .expect("count to fit into 32 bits")
    }

    /// Count in AABB
    pub fn count_in_range_if<'a, Query>(&'a self, center: &Pos, radius: u32, query: Query) -> u32
    where
        Query: Fn(&Pos, &Row) -> bool,
    {
        profile!("count_in_range");

        let r = i32::try_from(radius).expect("radius to fit into 31 bits");
        let min = *center + Pos::new(-r, -r);
        let max = *center + Pos::new(r, r);

        let [min, max] = self.morton_min_max(&min, &max);

        let values = self.values.as_ptr();
        self.positions[min..max]
            .iter()
            .enumerate()
            .filter(move |(i, id)| {
                let val = unsafe { &*values.add(min + i) };
                query(id, val)
            })
            .count()
            .try_into()
            .expect("count to fit into 32 bits")
    }

    /// Turn AABB min-max to from-to indices
    /// Clamps `min` and `max` to intersect `self`
    fn morton_min_max(&self, min: &Pos, max: &Pos) -> [usize; 2] {
        let min: usize = {
            if !self.intersects(&min) {
                0
            } else {
                self.find_key(&min).unwrap_or_else(|i| i)
            }
        };
        let max: usize = {
            let lim = (self.keys.len() as i64 - 1).max(0) as usize;
            if !self.intersects(&max) {
                lim
            } else {
                self.find_key(&max).unwrap_or_else(|i| i)
            }
        };
        [min, max]
    }

    /// Return wether point is within the bounds of this node
    pub fn intersects(&self, point: &Pos) -> bool {
        let [x, y] = point.as_array();
        (x & MORTON_POS_MAX) == x && (y & MORTON_POS_MAX) == y
    }

    /// Return [min, max) of the bounds of this table
    pub fn bounds(&self) -> (Pos, Pos) {
        (
            Pos::new(0, 0),
            Pos::new(MORTON_POS_MAX + 1, MORTON_POS_MAX + 1),
        )
    }

    /// Remove duplicate values from self, leaving one.
    /// Note that during sorting the order of values may alter from the order which they were
    /// inserted.
    pub fn dedupe(&mut self) -> &mut Self {
        for i in (1..self.keys.len()).rev() {
            if self.keys[i] == self.keys[i - 1] {
                self.keys.remove(i);
                self.values.remove(i);
                self.positions.remove(i);
            }
        }
        self.rebuild_skip_list();
        self
    }
}

impl<Pos, Row> Table for MortonTable<Pos, Row>
where
    Pos: SpatialKey2d + Send + Sync,
    Row: TableRow + Send + Sync,
{
    type Id = Pos;
    type Row = Row;

    /// delete all values at id and return the first one, if any
    fn delete(&mut self, id: &Pos) -> Option<Row> {
        profile!("delete");
        if !self.intersects(id) {
            return None;
        }

        let val = self
            .find_key(&id)
            .map(|ind| {
                self.keys.remove(ind);
                self.positions.remove(ind);
                self.values.remove(ind)
            })
            .ok()?;

        while let Ok(ind) = self.find_key(&id) {
            self.keys.remove(ind);
            self.positions.remove(ind);
            self.values.remove(ind);
        }

        Some(val)
    }
}

impl PositionTable for MortonTable<Point, EntityComponent> {
    fn get_entities_in_range(&self, vision: &Circle) -> Vec<(EntityId, PositionComponent)> {
        profile!("get_entities_in_range");

        let mut res = Vec::new();
        self.find_by_range(&vision.center, vision.radius * 3 / 2, &mut res);
        res.into_iter()
            .filter(|(pos, _)| pos.hex_distance(vision.center) <= vision.radius)
            .map(|(pos, id)| (id.0, PositionComponent(pos)))
            .collect()
    }

    fn count_entities_in_range(&self, vision: &Circle) -> usize {
        profile!("count_entities_in_range");

        self.count_in_range(&vision.center, vision.radius * 3 / 2) as usize
    }
}
