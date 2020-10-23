use crate::components::{
    game_config::GameConfig, EntityScript, OwnedEntity, ScriptComponent, ScriptHistoryEntry,
};
use crate::indices::{EntityId, ScriptId, UserId};
use crate::{intents::*, profile, World};
use cao_lang::prelude::*;
use rayon::prelude::*;
use slog::{debug, info, o, trace, warn};
use std::convert::TryFrom;
use std::fmt::{self, Display, Formatter};
use std::mem::{replace, take};
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

pub fn execute_scripts(
    workload: &[(EntityId, EntityScript)],
    storage: &mut World,
) -> Vec<BotIntents> {
    profile!("execute_scripts");

    let logger = storage.logger.new(o!("tick" => storage.time));
    let owners_table = storage.view::<EntityId, OwnedEntity>().reborrow();

    let n_scripts = workload.len();
    let n_threads = rayon::current_num_threads();
    // the +1 handles edge cases, where n_script < n_threads
    // this way in practice 1 thread will have a bit less work to perform than the others,
    // but it should be fine.
    // Also if the programs call engine functions that have internal parallelisation, then
    // load balancing should be even less of a problem...
    let chunk_size = (n_scripts / n_threads) + 1;

    info!(
        logger,
        "Executing {} scripts on {} threads in chunks of {}", n_scripts, n_threads, chunk_size
    );

    let intents: Option<Vec<BotIntents>> = workload
        .par_chunks(chunk_size)
        .fold(
            || Vec::with_capacity(chunk_size),
            |mut intents, entity_scripts| {
                let data = ScriptExecutionData::unsafe_default(logger.clone());

                let conf = storage.resource::<GameConfig>();
                let mut vm = VM::new(None, data);
                vm.history.reserve(conf.execution_limit as usize);
                vm.max_iter = i32::try_from(conf.execution_limit)
                    .expect("Expected execution_limit to fit into 31 bits");
                crate::scripting_api::make_import().execute_imports(&mut vm);

                for (entity_id, script) in entity_scripts {
                    let owner_id = owners_table
                        .get_by_id(&entity_id)
                        .map(|OwnedEntity { owner_id }| *owner_id);

                    vm.clear();
                    match execute_single_script(
                        &logger, *entity_id, script.0, owner_id, storage, &mut vm,
                    ) {
                        Ok(ints) => intents.push(ints),
                        Err(err) => {
                            warn!(
                                logger,
                                "Execution failure in {:?} of {:?}:\n{:?}", script, entity_id, err
                            );
                        }
                    }
                }
                intents
            },
        )
        .reduce_with(|mut res, intermediate| {
            res.extend(intermediate);
            res
        });

    debug!(
        logger,
        "Executing scripts done. Returning {:?} intents",
        intents.as_ref().map(|i| i.len())
    );
    intents.unwrap_or_else(Vec::default)
}

fn prepare_script_data(
    logger: &slog::Logger,
    entity_id: EntityId,
    user_id: Option<UserId>,
    storage: &World,
) -> ScriptExecutionData {
    let logger = logger.new(o!( "entity_id" => entity_id.0 ));
    let intents = BotIntents {
        entity_id,
        ..Default::default()
    };
    ScriptExecutionData::new(logger.clone(), storage, intents, entity_id, user_id)
}

pub fn execute_single_script<'a>(
    logger: &slog::Logger,
    entity_id: EntityId,
    script_id: ScriptId,
    user_id: Option<UserId>,
    storage: &'a World,
    vm: &mut VM<'a, ScriptExecutionData>,
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
    vm.logger = logger.clone();
    let data = prepare_script_data(&logger, entity_id, user_id, storage);
    vm.auxiliary_data = data;

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

    let history = take(&mut vm.history);
    let aux = replace(
        &mut vm.auxiliary_data,
        ScriptExecutionData::unsafe_default(logger.clone()),
    );
    trace!(
        logger,
        "Script execution completed, intents:{:?}",
        aux.intents
    );

    let mut intents = aux.intents;
    intents.script_history_intent = Some(ScriptHistoryEntry {
        entity: entity_id,
        payload: history,
        time: storage.time,
    });

    Ok(intents)
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
    /// To be used as a placeholder, do not consume
    pub fn unsafe_default(logger: slog::Logger) -> Self {
        Self {
            entity_id: Default::default(),
            user_id: None,
            intents: Default::default(),
            storage: std::ptr::null(),
            logger,
        }
    }

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
