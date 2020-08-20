use crate::components::{EntityScript, ScriptComponent};
use crate::model::EmptyKey;
use crate::model::{EntityId, ScriptId, UserId};
use crate::{
    intents::{BotIntents, Intents},
    profile, World,
};
use cao_lang::prelude::*;
use rayon::prelude::*;
use slog::o;
use slog::{trace, warn};
use std::fmt::{self, Display, Formatter};
use std::mem;
use std::sync::Mutex;
use thiserror::Error;

pub type ExecutionResult = Result<BotIntents, ExecutionError>;

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
pub fn execute_scripts(storage: &mut World) {
    profile!("execute_scripts");

    let n_scripts = storage.view::<EntityId, EntityScript>().len();

    // Reuse the existing intent memory
    // Replace the one in the storage, so there is no way to mutate it while the scripts are
    // running
    let intents = unsafe {
        mem::replace(
            &mut storage.unsafe_view::<EmptyKey, Intents>().as_mut().value,
            None,
        )
    };
    let mut intents = intents.unwrap_or_else(|| Intents::with_capacity(n_scripts));
    intents.clear();
    let intents = Mutex::new(intents);

    execute_scripts_parallel(&intents, storage);

    let intents = intents.into_inner().expect("Mutex unwrap");

    // place the final intents into the storage
    unsafe {
        mem::replace(
            &mut storage.unsafe_view::<EmptyKey, Intents>().as_mut().value,
            Some(intents),
        )
    };
}

fn execute_scripts_parallel(intents: &Mutex<Intents>, storage: &World) {
    let logger = storage.logger.new(o!("tick" => storage.time));

    let table = storage.view::<EntityId, EntityScript>().reborrow();
    table.par_iter().for_each(|(entity_id, script)| {
        match execute_single_script(&logger, *entity_id, script.script_id, storage) {
            Ok(ints) => {
                let mut intents = intents.lock().unwrap();
                intents.append(ints);
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

    let logger = logger.new(o!( "entity_id" => entity_id.0 ));
    let data = ScriptExecutionData::new(
        logger.clone(),
        storage,
        BotIntents::default(),
        entity_id,
        Some(Default::default()), // TODO
    );
    let mut vm = VM::new(logger.clone(), data);
    crate::api::make_import().execute_imports(&mut vm);

    trace!(logger, "Starting script execution");

    vm.run(&program.0).map_err(|err| {
        warn!(
            logger,
            "Error while executing script {:?} {:?}", script_id, err
        );
        ExecutionError::RuntimeError {
            script_id,
            entity_id,
            error: err,
        }
    })?;

    let aux = vm.unwrap_aux();
    trace!(
        logger,
        "Script execution completed, intents:{:?}",
        aux.intents
    );

    Ok(aux.intents)
}

#[derive(Debug)]
pub struct ScriptExecutionData {
    pub entity_id: EntityId,
    pub user_id: Option<UserId>,
    pub intents: BotIntents,
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
        logger: slog::Logger,
        storage: &World,
        intents: BotIntents,
        entity_id: EntityId,
        user_id: Option<UserId>,
    ) -> Self {
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
