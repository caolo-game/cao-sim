pub mod indices;
pub mod pathfinding;
pub mod terrain;

pub use self::indices::*;
pub use cao_lang::prelude::*;

use serde_derive::{Deserialize, Serialize};
use std::convert::TryFrom;

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
    PathNotFound = -8,
}

impl TryFrom<Scalar> for OperationResult {
    type Error = Scalar;

    fn try_from(i: Scalar) -> Result<OperationResult, Scalar> {
        let op = match i {
            Scalar::Integer(0) => OperationResult::Ok,
            Scalar::Integer(-1) => OperationResult::NotOwner,
            Scalar::Integer(-2) => OperationResult::InvalidInput,
            Scalar::Integer(-3) => OperationResult::OperationFailed,
            Scalar::Integer(-4) => OperationResult::NotInRange,
            Scalar::Integer(-5) => OperationResult::InvalidTarget,
            Scalar::Integer(-6) => OperationResult::Empty,
            Scalar::Integer(-7) => OperationResult::Full,
            Scalar::Integer(-8) => OperationResult::PathNotFound,
            _ => {
                return Err(i);
            }
        };
        Ok(op)
    }
}

impl cao_lang::traits::AutoByteEncodeProperties for OperationResult {}

impl Into<Scalar> for OperationResult {
    fn into(self) -> Scalar {
        Scalar::Integer(self as i32)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Script {
    pub compiled: Option<CompiledProgram>,
    pub script: CompilationUnit,
}

#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
pub struct Time(pub u64);
