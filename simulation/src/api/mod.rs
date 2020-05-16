//! Methods that are exported to the Cao-lang clients
//!
//! Methods that may fail return an OperationResult
//!
pub mod bots;
pub mod resources;
pub mod structures;
use crate::model::geometry::point::Axial;
use crate::model::{components, EntityId, OperationResult};
use crate::systems::script_execution::ScriptExecutionData;
use cao_lang::prelude::*;
use cao_lang::scalar::Scalar;
use cao_lang::traits::ByteEncodeProperties;
use std::convert::TryFrom;

/// Write an OperationResult to the program
pub fn make_operation_result(
    vm: &mut VM<ScriptExecutionData>,
    op: i32,
) -> Result<(), ExecutionError> {
    let op = OperationResult::try_from(op).map_err(|_| ExecutionError::InvalidArgument)?;
    vm.set_value(op)?;
    Ok(())
}

pub fn make_point(
    vm: &mut VM<ScriptExecutionData>,
    (x, y): (i32, i32),
) -> Result<(), ExecutionError> {
    let point = Axial::new(x, y);
    vm.set_value(point)?;
    Ok(())
}

pub fn console_log(
    vm: &mut VM<ScriptExecutionData>,
    message: TPointer,
) -> Result<(), ExecutionError> {
    let message: String = vm.get_value(message).ok_or_else(|| {
        debug!("console_log called with invalid message");
        ExecutionError::InvalidArgument
    })?;
    let entity_id = vm.get_aux().entity_id;
    let time = vm.get_aux().storage().time();

    let payload = format!("{:?} says {}", entity_id, message);
    debug!("{}", payload);
    vm.get_aux_mut()
        .intents
        .log_intents
        .push(crate::intents::LogIntent {
            entity: entity_id,
            payload,
            time,
        });

    Ok(())
}

pub fn log_scalar(vm: &mut VM<ScriptExecutionData>, value: Scalar) -> Result<(), ExecutionError> {
    let entity_id = vm.get_aux().entity_id;
    let time = vm.get_aux().storage().time();
    let payload = format!("{:?} says {:?}", entity_id, value);
    debug!("{}", payload);
    vm.get_aux_mut()
        .intents
        .log_intents
        .push(crate::intents::LogIntent {
            entity: entity_id,
            payload,
            time,
        });
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
                    find_closest_resource_by_range,
                    "Find the resource closest to the current entity",
                    [],
                    [OperationResult, EntityId],
                    []
                ),
                fo: Procedure::new(FunctionWrapper::new(
                    resources::find_closest_resource_by_range,
                )),
            },
            FunctionRow {
                desc: subprogram_description!(
                    make_operation_result,
                    "Produces an OperationResult",
                    [i32],
                    [OperationResult],
                    []
                ),
                fo: Procedure::new(FunctionWrapper::new(make_operation_result)),
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
        ],
    }
}
