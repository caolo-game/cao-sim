//! Structs intended to be used as table indices.
//!
use crate::model::Axial;
use crate::tables::SerialId;
use cao_lang::{prelude::Scalar, traits::AutoByteEncodeProperties};
use serde_derive::{Deserialize, Serialize};
use std::convert::TryFrom;

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

/// Newtype wrapper around Axial point for positions that are inside a room.
#[derive(
    Debug, Clone, Default, Ord, PartialOrd, Eq, PartialEq, Copy, Hash, Serialize, Deserialize,
)]
pub struct RoomPosition(pub Axial);
impl AutoByteEncodeProperties for RoomPosition {}
