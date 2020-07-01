//! Structs intended to be used as table indices.
//!
use crate::geometry::Axial;
use crate::tables::{SerialId, SpatialKey2d};
use cao_lang::{prelude::Scalar, traits::AutoByteEncodeProperties};
use serde_derive::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::ops::Add;

#[derive(
    Debug, Clone, Default, Ord, PartialOrd, Eq, PartialEq, Copy, Hash, Serialize, Deserialize,
)]
pub struct EntityTime(pub EntityId, pub u64);

#[derive(
    Debug, Clone, Default, Ord, PartialOrd, Eq, PartialEq, Copy, Hash, Serialize, Deserialize,
)]
pub struct EntityId(pub u32);

#[derive(Debug, Clone, Default, Ord, PartialOrd, Eq, PartialEq, Copy, Serialize, Deserialize)]
pub struct ScriptId(pub uuid::Uuid);

#[derive(
    Debug, Clone, Default, Ord, PartialOrd, Eq, PartialEq, Copy, Hash, Serialize, Deserialize,
)]
pub struct UserId(pub uuid::Uuid);

impl SerialId for EntityId {
    fn next(&self) -> Self {
        Self(self.0 + 1)
    }

    fn as_usize(&self) -> usize {
        self.0 as usize
    }
}

impl AutoByteEncodeProperties for EntityId {}
impl TryFrom<Scalar> for EntityId {
    type Error = Scalar;
    fn try_from(s: Scalar) -> Result<EntityId, Scalar> {
        match s {
            Scalar::Integer(i) => {
                if i < 0 {
                    return Err(s);
                }
                Ok(EntityId(i as u32))
            }
            _ => Err(s),
        }
    }
}

#[derive(
    Debug, Clone, Default, Ord, PartialOrd, Eq, PartialEq, Copy, Hash, Serialize, Deserialize,
)]
pub struct WorldPosition {
    pub room: Axial,
    pub pos: Axial,
}
impl AutoByteEncodeProperties for WorldPosition {}

impl WorldPosition {
    pub fn absolute(&self, room_radius: i32) -> Axial {
        let offset = self.room * room_radius * 2;
        offset + self.pos
    }
}

/// Newtype wrapper around Axial point for positions that are inside a room.
#[derive(
    Debug, Clone, Default, Ord, PartialOrd, Eq, PartialEq, Copy, Hash, Serialize, Deserialize,
)]
pub struct RoomPosition(pub Axial);
impl AutoByteEncodeProperties for RoomPosition {}

/// Newtype wrapper around Axial point for room ids.
#[derive(
    Debug, Clone, Default, Ord, PartialOrd, Eq, PartialEq, Copy, Hash, Serialize, Deserialize,
)]
pub struct Room(pub Axial);
impl AutoByteEncodeProperties for Room {}

impl SpatialKey2d for Room {
    fn as_array(&self) -> [i32; 2] {
        self.0.as_array()
    }

    fn get_axis(&self, axis: u8) -> i32 {
        self.0.get_axis(axis)
    }

    fn new(x: i32, y: i32) -> Self {
        Self(Axial::new(x, y))
    }

    fn dist(&self, Room(ref other): &Self) -> u32 {
        self.0.dist(other)
    }
}

impl Add for Room {
    type Output = Self;

    fn add(self, Room(b): Self) -> Self {
        Self(self.0.add(b))
    }
}

#[derive(
    Debug, Clone, Default, Ord, PartialOrd, Eq, PartialEq, Copy, Hash, Serialize, Deserialize,
)]
pub struct EmptyKey;

unsafe impl Send for EmptyKey {}
unsafe impl Send for Room {}
unsafe impl Send for UserId {}
unsafe impl Send for EntityId {}
unsafe impl Send for ScriptId {}
