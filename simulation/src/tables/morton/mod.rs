//! Linear Quadtree.
//! # Contracts:
//! - Key axis must be in the interval [0, 2^16)
//! This is a severe restriction on the keys that can be used, however dense queries and
//! constructing from iterators is much faster than quadtrees.
//!
//! When compiling for x86 we assume that the machine is capable of executing SSE2 instructions.
//!

mod find_key_partition;
mod litmax_bigmin;
mod morton_key;
mod serde;
mod skiplist;
mod sorting;
#[cfg(test)]
mod tests;

pub use self::litmax_bigmin::msb_de_bruijn;
use self::litmax_bigmin::round_down_to_one_less_than_pow_two;
pub use self::morton_key::*;
pub use self::serde::*;
use super::*;
use litmax_bigmin::litmax_bigmin;
use skiplist::*;
use std::convert::{TryFrom, TryInto};
use thiserror::Error;

// at most 15 bits long non-negative integers
// having the 16th bit set might create problems in find_key
pub const MORTON_POS_MAX: i32 = 0b0111_1111_1111_1111;

// The original paper counts the garbage items and splits above a threshold.
// Instead let's speculate if we need a split or if it more beneficial to just scan the
// range
// The number I picked is more or less arbitrary, it is a power of two and I ran the basic
// benchmarks to probe a few numbers.
const MAX_BRUTE_ITERS: usize = 16;

#[derive(Debug, Clone, Error)]
pub enum ExtendFailure<Id: SpatialKey2d> {
    #[error("Position {0:?} is out of bounds!")]
    OutOfBounds(Id),
}

#[derive(Clone)]
pub struct MortonTable<Pos, Row>
where
    Pos: SpatialKey2d,
    Row: TableRow,
{
    skiplist: SkipList,
    skipstep: u32,
    //
    keys: Vec<MortonKey>,
    values: Vec<(Pos, Row)>,
}

impl<Pos, Row> std::fmt::Debug for MortonTable<Pos, Row>
where
    Pos: SpatialKey2d,
    Row: TableRow,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MortonTable\n{:?}", self.values)
    }
}

impl<Pos, Row> Default for MortonTable<Pos, Row>
where
    Pos: SpatialKey2d + Send,
    Row: TableRow + Send,
{
    fn default() -> Self {
        Self {
            skipstep: 0,
            skiplist: Default::default(),
            keys: Default::default(),
            values: Default::default(),
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
    Pos: SpatialKey2d,
    Row: TableRow + Send,
{
    pub fn new() -> Self {
        Self {
            skiplist: Default::default(),
            skipstep: 0,
            keys: vec![],
            values: vec![],
        }
    }

    pub fn from_vec(values: Vec<(Pos, Row)>) -> Result<Self, ExtendFailure<Pos>> {
        let mut keys = Vec::with_capacity(values.len());
        for (pos, _) in values.iter() {
            if !Self::is_valid_pos(pos) {
                return Err(ExtendFailure::OutOfBounds(*pos));
            }
            let [x, y] = pos.as_array();
            // the above check ensured that x and y are safely convertible
            keys.push(MortonKey::new(x as u16, y as u16))
        }
        let mut res = Self {
            keys,
            values,
            ..Default::default()
        };
        sorting::sort(&mut res.keys, &mut res.values);
        res.rebuild_skip_list();
        Ok(res)
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            skiplist: Default::default(),
            skipstep: 0,
            values: Vec::with_capacity(cap),
            keys: Vec::with_capacity(cap),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn iter_mut<'a>(&'a mut self) -> impl Iterator<Item = (Pos, &'a mut Row)> + 'a {
        self.values.iter_mut().map(|(p, v)| (*p, v))
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = (Pos, &'a Row)> + 'a {
        self.values.iter().map(|(p, v)| (*p, v))
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
        self.values.clear();
        self.rebuild_skip_list();
    }

    /// Extend the map by the items provided.
    pub fn extend<It>(&mut self, it: It) -> Result<(), ExtendFailure<Pos>>
    where
        It: Iterator<Item = (Pos, Row)>,
    {
        for (id, value) in it {
            if !self.intersects(&id) {
                return Err(ExtendFailure::OutOfBounds(id));
            }
            let [x, y] = id.as_array();
            let [x, y] = [x as u16, y as u16];
            let key = MortonKey::new(x, y);
            self.keys.push(key);
            self.values.push((id, value));
        }
        sorting::sort(&mut self.keys, &mut self.values);
        self.rebuild_skip_list();
        Ok(())
    }

    /// Extend the map by the items provided.
    /// Note that `Row`s are cloned!
    pub fn extend_from_slice(&mut self, items: &[(Pos, Row)]) -> Result<(), ExtendFailure<Pos>> {
        self.extend(items.iter().map(|(pos, row)| (*pos, row.clone())))
    }

    fn rebuild_skip_list(&mut self) {
        // assert that keys is sorted.
        // at the time of writing is_sorted is still unstable
        #[cfg(debug_assertions)]
        if self.keys.len() > 2 {
            let mut it = self.keys.iter();
            let mut current = it.next().unwrap();
            for next in it {
                assert!(
                    current <= next,
                    "`keys` was not sorted when calling `rebuild_skip_list` current: {:?} next: {:?}",
                    current,
                    next
                );
                current = next;
            }
        }

        let len = self.keys.len();
        let step = len / SKIP_LEN + 1;
        self.skipstep = step as u32;
        // leaving items 0 will cause errors in find_key_morton
        self.skiplist = SkipList::default();
        if step == 1 {
            if let Some(key) = self.keys.last() {
                self.skiplist.set(0, key.0 as i32)
            }
            return;
        }
        for (i, k) in (0..len).step_by(step).skip(1).take(SKIP_LEN).enumerate() {
            self.skiplist.set(i, self.keys[k].0 as i32);
        }
    }

    /// If applicable prefer `extend` and insert many keys at once.
    pub fn insert(&mut self, id: Pos, row: Row) -> Result<(), ExtendFailure<Pos>> {
        if !self.intersects(&id) {
            return Err(ExtendFailure::OutOfBounds(id));
        }
        let [x, y] = id.as_array();
        let [x, y] = [x as u16, y as u16];

        let ind = self
            .keys
            .binary_search(&MortonKey::new(x, y))
            .unwrap_or_else(|i| i);
        self.keys.insert(ind, MortonKey::new(x, y));
        self.values.insert(ind, (id, row));
        self.rebuild_skip_list();
        Ok(())
    }

    /// Return false if id is not in the map, otherwise override the first instance found
    pub fn update<'a>(&'a mut self, id: &Pos, row: Row) -> Option<&'a Row> {
        self.find_key(id)
            .map(move |ind| {
                self.values[ind].1 = row;
                &self.values[ind].1
            })
            .ok()
    }

    /// Return a reference to the new Row if it's in the map or None otherwise
    pub fn update_with<'a, F>(&'a mut self, id: &Pos, f: F) -> Option<&'a Row>
    where
        F: FnOnce(&mut Row),
    {
        self.find_key(id)
            .map(move |ind| {
                f(&mut self.values[ind].1);
                &self.values[ind].1
            })
            .ok()
    }

    /// Return a reference to the new Row if it's in the map or None otherwise
    pub fn insert_or_update(&mut self, id: Pos, row: Row) -> Result<(), ExtendFailure<Pos>> {
        if !self.intersects(&id) {
            return Err(ExtendFailure::OutOfBounds(id));
        }
        match self.find_key(&id) {
            Ok(ind) => {
                self.values[ind].1 = row;
            }
            Err(ind) => {
                let [x, y] = id.as_array();
                let [x, y] = [x as u16, y as u16];
                self.keys.insert(ind, MortonKey::new(x, y));
                self.values.insert(ind, (id, row));
                self.rebuild_skip_list();
            }
        }
        Ok(())
    }

    /// Returns the first item with given id, if any
    pub fn get_by_id<'a>(&'a self, id: &Pos) -> Option<&'a Row> {
        if !self.intersects(&id) {
            return None;
        }

        self.find_key(id).map(|ind| &self.values[ind].1).ok()
    }

    /// Returns the first item with given id, if any
    pub fn get_by_id_mut<'a>(&'a mut self, id: &Pos) -> Option<&'a mut Row> {
        if !self.intersects(&id) {
            return None;
        }

        self.find_key(id)
            .map(move |ind| &mut self.values[ind].1)
            .ok()
    }

    pub fn contains_key(&self, id: &Pos) -> bool {
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

        self.find_key_morton(key)
    }

    /// Find the position of `key` or the position where it needs to be inserted to keep the
    /// container sorted
    fn find_key_morton(&self, key: MortonKey) -> Result<usize, usize> {
        use find_key_partition::find_key_partition;

        let step = self.skipstep as usize;
        if step <= 1 {
            return self.keys.binary_search(&key);
        }

        let index = find_key_partition(&self.skiplist, key);

        let (begin, end) = if index < SKIP_LEN {
            let begin = index * step;
            let end = self.keys.len().min(begin + step + 1);
            (begin, end)
        } else {
            let end = self.keys.len();
            let begin = end - 1 - step;
            (begin, end)
        };
        self.keys[begin..end]
            .binary_search(&key)
            .map(|ind| ind + begin)
            .map_err(|ind| ind + begin)
    }

    /// For each id returns the first item with given id, if any
    pub fn get_by_ids<'a>(&'a self, ids: &[Pos]) -> Vec<(Pos, &'a Row)> {
        ids.iter()
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
        Op: FnMut(Pos, &'a Row),
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
        op: &mut impl FnMut(Pos, &'a Row),
    ) {
        let (imin, pmin) = self
            .find_key_morton(min)
            .map(|mut i| {
                // find_key_morton might not return the first index of a 'duplicate group'
                // we need to find the first index, so none gets missed
                while 0 < i && self.keys[i - 1] == min {
                    i -= 1;
                }
                (i, self.values[i].0.as_array())
            })
            .unwrap_or_else(|i| {
                let [x, y] = min.as_point();
                (i, [x as i32, y as i32])
            });

        let (imax, pmax) = self
            .find_key_morton(max)
            .map(|i| {
                let mut j = i;
                // add 1 to include this node in the range query as otherwise an element might be
                // missed
                //
                // also it seems like we missed duplicate values.
                while j < self.keys.len() && self.keys[j] == max {
                    j += 1;
                }
                (j, self.values[i].0.as_array())
            })
            .unwrap_or_else(|i| {
                let [x, y] = max.as_point();
                (i, [x as i32, y as i32])
            });

        debug_assert!(
            imin <= imax,
            "find_key_morton returned bad indices: (min,max): ({}, {})",
            imin,
            imax
        );

        if imax - imin > MAX_BRUTE_ITERS {
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

        for (id, val) in self.values[imin..imax].iter() {
            if center.dist(id) <= radius {
                op(*id, val);
            }
        }
    }

    /// If any found return the closest one to `center` and the distance to it.
    // TODO: try spiraling out from center to find a match faster
    pub fn find_closest_by_filter<F>(&self, center: &Pos, filter: F) -> Option<(u32, Pos, &Row)>
    where
        F: Fn(&Pos, &Row) -> bool,
    {
        self.values
            .iter()
            .filter(|(id, row)| filter(id, row))
            .map(|(id, row)| (id.dist(center), *id, row))
            .min_by_key(|t| t.0)
    }

    /// Count in AABB
    pub fn count_in_range<'a>(&'a self, center: &Pos, radius: u32) -> u32 {
        let r = i32::try_from(radius).expect("radius to fit into 31 bits");
        let min = *center + Pos::new(-r, -r);
        let max = *center + Pos::new(r, r);

        let [min, max] = self.morton_min_max(&min, &max);

        self.values[min..max]
            .iter()
            .filter(move |(id, _)| center.dist(&id) < radius)
            .count()
            .try_into()
            .expect("count to fit into 32 bits")
    }

    /// Count in AABB
    pub fn count_in_range_if<'a, Query>(&'a self, center: &Pos, radius: u32, query: Query) -> u32
    where
        Query: Fn(&Pos, &Row) -> bool,
    {
        let r = i32::try_from(radius).expect("radius to fit into 31 bits");
        let min = *center + Pos::new(-r, -r);
        let max = *center + Pos::new(r, r);

        let [min, max] = self.morton_min_max(&min, &max);

        self.values[min..max]
            .iter()
            .filter(move |(id, val)| query(id, val))
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
            if !self.intersects(&max) {
                (self.keys.len() as i64 - 1).max(0) as usize
            } else {
                self.find_key(&max).unwrap_or_else(|i| i)
            }
        };
        [min, max]
    }

    pub fn is_valid_pos(point: &Pos) -> bool {
        let [x, y] = point.as_array();
        (x & MORTON_POS_MAX) == x && (y & MORTON_POS_MAX) == y
    }

    /// Return wether point is within the bounds of this node
    pub fn intersects(&self, point: &Pos) -> bool {
        Self::is_valid_pos(point)
    }

    /// Return [min, max) of the bounds of this table
    pub fn bounds(&self) -> (Pos, Pos) {
        (
            Pos::new(0, 0),
            Pos::new(MORTON_POS_MAX + 1, MORTON_POS_MAX + 1),
        )
    }

    /// Compute the minimum and maximum positions for this table's AABB.
    /// Note that this might be (a lot) larger than the minimum bounding box that might hold this table!
    pub fn aabb(&self) -> Option<[Pos; 2]> {
        let min = self.keys.get(0)?;
        let [minx, miny] = self.values[0].0.as_array();
        let min_loc = round_down_to_one_less_than_pow_two(min.0) + 1;
        let [ax, ay] = MortonKey(min_loc).as_point();
        let [minx, miny] = [minx.min(ax as i32), miny.min(ay as i32)];

        let max = *self.keys.last().unwrap_or(min);
        let max = round_down_to_one_less_than_pow_two(max.0) + 1;
        let max = MortonKey(max);
        let [maxx, maxy] = self.values[self.values.len() - 1].0.as_array();
        let [bx, by] = max.as_point();
        let [maxx, maxy] = [maxx.max(bx as i32), maxy.max(by as i32)];

        let res = [Pos::new(minx, miny), Pos::new(maxx, maxy)];
        Some(res)
    }

    /// Remove duplicate values from self, leaving one.
    /// Note that during sorting the order of values may alter from the order which they were
    /// inserted.
    pub fn dedupe(&mut self) -> &mut Self {
        for i in (1..self.keys.len()).rev() {
            if self.keys[i] == self.keys[i - 1] {
                self.keys.remove(i);
                self.values.remove(i);
            }
        }
        self.rebuild_skip_list();
        self
    }

    /// Merge two `MortonTable`s by inserting all points that are in `other` but not in `self` and
    /// calling `update` with all points that are present in both tables.
    pub fn merge<F>(&mut self, other: &Self, mut update: F) -> Result<(), ExtendFailure<Pos>>
    where
        F: FnMut(&Pos, &Row, &Row) -> Row,
    {
        let inserts = {
            let mut lhs = self.iter_mut();
            let mut rhs = other.iter();

            let mut current_left = lhs.next();
            let mut current_right = rhs.next();

            let mut inserts = Vec::with_capacity(other.keys.len());

            while let Some(((p1, v1), (p2, v2))) = current_left
                .as_mut()
                .and_then(|lhs| current_right.map(|rhs| (lhs, rhs)))
            {
                if p1 != &p2 {
                    if &p2 < p1 {
                        // `self` can not have any more common items between these two
                        inserts.push((p2, v2));
                        current_right = rhs.next();
                    } else {
                        current_left = lhs.next();
                    }
                } else {
                    **v1 = update(&p1, v1, v2);
                    current_left = lhs.next();
                    current_right = rhs.next();
                }
            }
            while let Some(r) = current_right {
                inserts.push(r);
                current_right = rhs.next();
            }
            inserts
        };

        self.extend(inserts.into_iter().map(|(pos, v)| (pos, v.clone())))
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
        if !self.intersects(id) {
            return None;
        }

        let val = self
            .find_key(&id)
            .map(|ind| {
                self.keys.remove(ind);
                self.values.remove(ind)
            })
            .ok()?
            .1;

        while let Ok(ind) = self.find_key(&id) {
            self.keys.remove(ind);
            self.values.remove(ind);
        }

        self.rebuild_skip_list();

        Some(val)
    }

    fn get_by_id(&self, id: &Pos) -> Option<&Row> {
        MortonTable::get_by_id(self, id)
    }
}
