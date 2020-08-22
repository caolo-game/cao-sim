use crate::components;
use crate::indices::EntityTime;
use serde::Serialize;
use std::ops::Add;

pub trait SpatialKey2d: TableId + Add<Output = Self> {
    fn as_array(&self) -> [i32; 2];

    /// Get axis 0 or 1
    fn get_axis(&self, axis: u8) -> i32;

    /// Construct a new key with given coordinates
    fn new(x: i32, y: i32) -> Self;

    /// Distance between two keys
    fn dist(&self, other: &Self) -> u32;

    /// Distance among given axis. Used for separating axis tests to reduce query times when only
    /// one axis is considered..
    fn axis_dist(&self, other: &Self, axis: u8) -> u32 {
        (self.get_axis(axis) - other.get_axis(axis)).abs() as u32
    }
}

/// TableIds may be used as indices of tables
pub trait TableId:
    'static + Ord + PartialOrd + Eq + PartialEq + Copy + Default + Send + std::fmt::Debug + Serialize
{
}

/// Implement for Ids that are incremented in a serial fashion.
/// Intended to be used in tables that reserve memory for N items where N is the largest Id
/// inserted.
/// e.g. inserting ids 0, 3, 4, 10 will reserve memory for 11 ([0..10]) items.
pub trait SerialId: TableId {
    /// Return the next Id in the domain after `self`.
    fn next(&self) -> Self;
    fn as_usize(&self) -> usize;
}

impl<T> TableId for T where
    T: 'static
        + Ord
        + PartialOrd
        + Eq
        + PartialEq
        + Copy
        + Default
        + Send
        + std::fmt::Debug
        + Serialize
{
}

/// TableRows may be used as the row type of a table
pub trait TableRow: 'static + Clone + Send + std::fmt::Debug + Serialize {}
impl<T: 'static + Clone + Send + std::fmt::Debug + Serialize> TableRow for T {}

/// Components define both their shape (via their type) and the storage backend that shall be used to
/// store them.
pub trait Component<Id: TableId>: TableRow {
    type Table: Table<Row = Self> + Send + std::fmt::Debug + Default + Serialize;
}

pub trait Table {
    type Id: TableId;
    type Row: TableRow;

    fn delete(&mut self, id: &Self::Id) -> Option<Self::Row>;

    fn name() -> &'static str {
        use std::any::type_name;

        type_name::<Self>()
    }
}

pub trait LogTable {
    fn get_logs_by_time(&self, time: u64) -> Vec<(EntityTime, components::LogEntry)>;
}
