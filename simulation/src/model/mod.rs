pub mod components;
pub mod geometry;
pub mod indices;
pub mod pathfinding;
pub mod terrain;

pub use self::indices::*;
pub use cao_lang::prelude::*;

use self::geometry::point::Point;
use crate::tables::SpatialKey2d;
use serde_derive::{Deserialize, Serialize};
use std::convert::TryFrom;

impl SpatialKey2d for Point {
    fn as_array(&self) -> [i32; 2] {
        [self.x, self.y]
    }

    fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    fn get_axis(&self, axis: u8) -> i32 {
        match axis & 1 {
            0 => self.x,
            _ => self.y,
        }
    }

    fn dist(&self, other: &Self) -> u32 {
        u32::try_from(self.hex_distance(*other)).expect("Distance to fit in 32 bits")
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Copy)]
#[repr(i32)]
pub enum OperationResult {
    Ok = 0,
    NotOwner = -1,
    InvalidInput = -2,
    OperationFailed = -3,
    NotInRange = -4,
    InvalidTarget = -5,
    Empty = -6,
    Full = -7,
}

impl TryFrom<i32> for OperationResult {
    type Error = i32;

    fn try_from(i: i32) -> Result<OperationResult, i32> {
        let op = match i {
            0 => OperationResult::Ok,
            -1 => OperationResult::NotOwner,
            -2 => OperationResult::InvalidInput,
            -3 => OperationResult::OperationFailed,
            -4 => OperationResult::NotInRange,
            -5 => OperationResult::InvalidTarget,
            -6 => OperationResult::Empty,
            -7 => OperationResult::Full,
            _ => {
                return Err(i);
            }
        };
        Ok(op)
    }
}

impl cao_lang::traits::AutoByteEncodeProperties for OperationResult {}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Script {
    pub compiled: Option<CompiledProgram>,
    pub script: CompilationUnit,
}

#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
pub struct Time(pub u64);
