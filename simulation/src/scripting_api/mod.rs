//! Methods that are exported to the Cao-lang clients
//!
//! Methods that may fail return an OperationResult
//!
pub mod bots;
pub mod find_api;
use crate::components;
use crate::geometry::point::Axial;
use crate::indices::{EntityId, WorldPosition};
use crate::profile;
use crate::systems::script_execution::ScriptExecutionData;
use cao_lang::prelude::*;
use cao_lang::scalar::Scalar;
use find_api::FindConstant;
use serde_derive::{Deserialize, Serialize};
use slog::trace;
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

pub fn make_point(
    vm: &mut VM<ScriptExecutionData>,
    (x, y): (i32, i32),
) -> Result<(), ExecutionError> {
    let point = Axial::new(x, y);
    vm.set_value(point)?;
    Ok(())
}

pub fn world_position(
    vm: &mut VM<ScriptExecutionData>,
    [rx, ry, x, y]: [i32; 4],
) -> Result<(), ExecutionError> {
    let room = Axial::new(rx, ry);
    let pos = Axial::new(x, y);
    let wp = WorldPosition { room, pos };

    vm.set_value(wp)?;
    Ok(())
}

pub fn console_log(
    vm: &mut VM<ScriptExecutionData>,
    message: TPointer,
) -> Result<(), ExecutionError> {
    profile!("console_log");
    let logger = &vm.get_aux().logger;
    trace!(logger, "console_log");
    let message = vm.get_value_in_place::<&str>(message).ok_or_else(|| {
        trace!(logger, "console_log called with invalid message");
        ExecutionError::InvalidArgument { context: None }
    })?;
    let entity_id = vm.get_aux().entity_id;
    let time = vm.get_aux().storage().time();

    let payload = format!("{:?} says {}", entity_id, message);
    trace!(logger, "{}", payload);
    vm.get_aux_mut().intents.with_log(entity_id, payload, time);

    Ok(())
}

pub fn log_scalar(vm: &mut VM<ScriptExecutionData>, value: Scalar) -> Result<(), ExecutionError> {
    profile!("log_scalar");
    let logger = &vm.get_aux().logger;
    trace!(logger, "log_scalar");
    let entity_id = vm.get_aux().entity_id;
    let time = vm.get_aux().storage().time();
    let payload = format!("{:?} says {:?}", entity_id, value);
    trace!(logger, "{}", payload);
    vm.get_aux_mut().intents.with_log(entity_id, payload, time);
    Ok(())
}

/// Holds data about a function
pub struct FunctionRow {
    pub desc: SubProgram<'static>,
    pub fo: Procedure<ScriptExecutionData>,
}

impl std::fmt::Debug for FunctionRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FunctionRow of {:?}", self.desc,)
    }
}

#[derive(Debug)]
pub struct Schema {
    imports: Vec<FunctionRow>,
}

impl Schema {
    pub fn imports(&self) -> &[FunctionRow] {
        &self.imports
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.imports.iter().map(|fr| fr.desc.name)
    }

    pub fn execute_imports(self, vm: &mut VM<ScriptExecutionData>) {
        for fr in self.imports {
            vm.register_function_obj(fr.desc.name, fr.fo);
        }
    }
}

/// Bootstrap the game API in the VM
pub fn make_import() -> Schema {
    Schema {
        imports: vec![
            FunctionRow {
                desc: subprogram_description!(console_log, "Log a string", [String], [], []),
                fo: Procedure::new(FunctionWrapper::new(console_log)),
            },
            FunctionRow {
                desc: subprogram_description!(log_scalar, "Log a scalar value", [Scalar], [], []),
                fo: Procedure::new(FunctionWrapper::new(log_scalar)),
            },
            FunctionRow {
                desc: subprogram_description!(
                    mine_resource,
                    "Move the bot to the given Axial",
                    [EntityId],
                    [OperationResult],
                    []
                ),
                fo: Procedure::new(FunctionWrapper::new(bots::mine_resource)),
            },
            FunctionRow {
                desc: subprogram_description!(
                    approach_entity,
                    "Move the bot to the given Entity",
                    [EntityId],
                    [OperationResult],
                    []
                ),
                fo: Procedure::new(FunctionWrapper::new(bots::approach_entity)),
            },
            FunctionRow {
                desc: subprogram_description!(
                    move_bot_to_position,
                    "Move the bot to the given Axial",
                    [Axial],
                    [OperationResult],
                    []
                ),
                fo: Procedure::new(FunctionWrapper::new(bots::move_bot_to_position)),
            },
            FunctionRow {
                desc: subprogram_description!(
                    make_point,
                    "Create a point from x and y coordinates",
                    [i32, i32],
                    [Axial],
                    []
                ),
                fo: Procedure::new(FunctionWrapper::new(make_point)),
            },
            FunctionRow {
                desc: subprogram_description!(
                    world_position,
                    "Create a WorldPosition from coordinates: [room.x, room.y, x, y]",
                    [i32, i32, i32, i32],
                    [Axial],
                    []
                ),
                fo: Procedure::new(FunctionWrapper::new(world_position)),
            },
            FunctionRow {
                desc: subprogram_description!(
                    find_closest_by_range,
                    "Find an object of type `FindConstant`, closest to the current entity",
                    [FindConstant],
                    [OperationResult, EntityId],
                    []
                ),
                fo: Procedure::new(FunctionWrapper::new(find_api::find_closest_by_range)),
            },
            FunctionRow {
                desc: subprogram_description!(
                    unload,
                    "Unload resources",
                    [u16, components::Resource, EntityId],
                    [OperationResult],
                    []
                ),
                fo: Procedure::new(FunctionWrapper::new(bots::unload)),
            },
            FunctionRow {
                desc: subprogram_description!(
                    parse_find_constant,
                    "Converts string literal to a find constant",
                    [String],
                    [FindConstant],
                    []
                ),
                fo: Procedure::new(FunctionWrapper::new(find_api::parse_find_constant)),
            },
        ],
    }
}
