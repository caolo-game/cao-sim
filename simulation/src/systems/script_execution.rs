use crate::components::{EntityScript, ScriptComponent};
use crate::model::{EntityId, ScriptId, UserId};
use crate::{intents::Intents, profile, World};
use cao_lang::prelude::*;
use rayon::prelude::*;
use std::sync::Mutex;
use thiserror::Error;

pub type ExecutionResult = Result<Intents, ExecutionError>;

#[derive(Debug, Error, Clone)]
pub enum ExecutionError {
    #[error("{0:?} was not found")]
    ScriptNotFound(ScriptId),
    #[error(" {script_id:?} of {entity_id:?} failed {error:?}")]
    RuntimeError {
        script_id: ScriptId,
        entity_id: EntityId,
        error: cao_lang::prelude::ExecutionError,
    },
}

/// Must be called from a tokio runtime!
/// Returns the intents that are expected to be executed
pub fn execute_scripts(storage: &World) -> Intents {
    profile!("execute_scripts");

    let n_stricts = storage.view::<EntityId, EntityScript>().len();
    let intents = Mutex::new(Intents::with_capacity(n_stricts));
    execute_scripts_parallel(&intents, storage);

    intents.into_inner().expect("Mutex unwrap")
}

fn execute_scripts_parallel(intents: &Mutex<Intents>, storage: &World) {
    storage
        .view::<EntityId, EntityScript>()
        .reborrow()
        .par_iter()
        .for_each(|(entity_id, script)| {
            match execute_single_script(*entity_id, script.script_id, storage) {
                Ok(ints) => {
                    let mut intents = intents.lock().unwrap();
                    intents.merge(&ints);
                }
                Err(err) => {
                    warn!(
                        "Execution failure in {:?} of {:?}:\n{}",
                        script.script_id, entity_id, err
                    );
                }
            }
        });
}

pub fn execute_single_script(
    entity_id: EntityId,
    script_id: ScriptId,
    storage: &World,
) -> ExecutionResult {
    profile!("execute_single_script");

    let program = storage
        .view::<ScriptId, ScriptComponent>()
        .reborrow()
        .get_by_id(&script_id)
        .ok_or_else(|| {
            warn!("Script by ID {:?} does not exist", script_id);
            ExecutionError::ScriptNotFound(script_id)
        })?;

    let data = ScriptExecutionData {
        intents: Intents::with_capacity(4),
        storage: storage as *const _,
        entity_id,
        user_id: Some(Default::default()), // None, // TODO
    };
    let mut vm = VM::new(data);
    crate::api::make_import().execute_imports(&mut vm);

    vm.run(&program.0).map_err(|err| {
        warn!(
            "Error while executing script {:?} of entity {:?}\n{:?}",
            script_id, entity_id, err
        );
        ExecutionError::RuntimeError {
            script_id,
            entity_id,
            error: err,
        }
    })?;

    Ok(vm.unwrap_aux().intents)
}

pub struct ScriptExecutionData {
    storage: *const World,

    pub intents: Intents,
    pub entity_id: EntityId,
    pub user_id: Option<UserId>,
}

impl ScriptExecutionData {
    pub fn storage(&self) -> &World {
        unsafe { &*self.storage }
    }
}
