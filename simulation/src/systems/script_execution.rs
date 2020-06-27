use crate::components::{EntityScript, ScriptComponent};
use crate::model::{EntityId, ScriptId, UserId};
use crate::{intents::Intents, profile, World};
use cao_lang::prelude::*;
use std::sync::Mutex;

pub type ExecutionResult = Result<Intents, String>;

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
    rayon::scope(move |s| {
        for (entityid, script) in storage.view::<EntityId, EntityScript>().reborrow().iter() {
            s.spawn(
                move |_| match execute_single_script(entityid, script.script_id, storage) {
                    Ok(ints) => {
                        let mut intents = intents.lock().unwrap();
                        intents.merge(&ints);
                    }
                    Err(e) => {
                        error!(
                            "Execution failure of script {:?} of entity {:?}: {:?}",
                            script.script_id, entityid, e
                        );
                    }
                },
            );
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
            error!("Script by ID {:?} does not exist", script_id);
            "not found"
        })?;

    let data = ScriptExecutionData {
        intents: Intents::new(),
        storage: storage as *const _,
        entity_id,
        user_id: Some(Default::default()), // None, // TODO
    };
    let mut vm = VM::new(data);
    crate::api::make_import().execute_imports(&mut vm);

    vm.run(&program.0).map_err(|e| {
        warn!(
            "Error while executing script {:?} of entity {:?}\n{:?}",
            script_id, entity_id, e
        );
        "runtime error"
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
