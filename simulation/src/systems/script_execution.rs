use crate::components::{EntityScript, ScriptComponent};
use crate::model::{EntityId, ScriptId, UserId};
use crate::{intents::Intents, profile, World};
use cao_lang::prelude::*;
use rayon::prelude::*;
use slog::o;
use slog::{trace, warn};
use std::fmt::{self, Display, Formatter};
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

    let n_scripts = storage.view::<EntityId, EntityScript>().len();
    let intents = Mutex::new(Intents::with_capacity(n_scripts));
    execute_scripts_parallel(&intents, storage);

    intents.into_inner().expect("Mutex unwrap")
}

fn execute_scripts_parallel(intents: &Mutex<Intents>, storage: &World) {
    let logger = storage.logger.new(o!("tick" => storage.time));

    let table = storage.view::<EntityId, EntityScript>().reborrow();
    table.par_iter().for_each(|(entity_id, script)| {
        match execute_single_script(&logger, *entity_id, script.script_id, storage) {
            Ok(ints) => {
                let mut intents = intents.lock().unwrap();
                intents.merge(&ints);
            }
            Err(err) => {
                warn!(
                    logger,
                    "Execution failure in {:?} of {:?}:\n{}", script.script_id, entity_id, err
                );
            }
        }
    });
}

pub fn execute_single_script(
    logger: &slog::Logger,
    entity_id: EntityId,
    script_id: ScriptId,
    storage: &World,
) -> ExecutionResult {
    let program = storage
        .view::<ScriptId, ScriptComponent>()
        .reborrow()
        .get_by_id(&script_id)
        .ok_or_else(|| {
            warn!(logger, "Script by ID {:?} does not exist", script_id);
            ExecutionError::ScriptNotFound(script_id)
        })?;

    let data = ScriptExecutionData::new(
        logger,
        storage,
        Intents::with_capacity(4),
        entity_id,
        Some(Default::default()), // TODO
    );
    let mut vm = VM::new(data);
    crate::api::make_import().execute_imports(&mut vm);

    vm.run(&program.0).map_err(|err| {
        warn!(
            logger,
            "Error while executing script {:?} of entity {:?}\n{:?}", script_id, entity_id, err
        );
        ExecutionError::RuntimeError {
            script_id,
            entity_id,
            error: err,
        }
    })?;

    let aux = vm.unwrap_aux();
    trace!(logger, "Script execution completed\n{:?}", aux);

    Ok(aux.intents)
}

#[derive(Debug)]
pub struct ScriptExecutionData {
    pub entity_id: EntityId,
    pub user_id: Option<UserId>,
    pub intents: Intents,
    storage: *const World,
    pub logger: slog::Logger,
}

impl Display for ScriptExecutionData {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:?}", self.entity_id)?;
        if let Some(ref user_id) = self.user_id {
            write!(f, " UserId: {}", user_id.0)?
        }
        Ok(())
    }
}

impl ScriptExecutionData {
    pub fn new(
        logger: &slog::Logger,
        storage: &World,
        intents: Intents,
        entity_id: EntityId,
        user_id: Option<UserId>,
    ) -> Self {
        let logger = logger.new(o!( "entity_id" => entity_id.0 ));

        Self {
            storage: storage as *const _,
            intents,
            entity_id,
            user_id,
            logger,
        }
    }

    pub fn storage(&self) -> &World {
        unsafe { &*self.storage }
    }
}
