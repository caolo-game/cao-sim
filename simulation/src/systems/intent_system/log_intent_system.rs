use super::IntentExecutionSystem;
use crate::components::LogEntry;
use crate::intents::LogIntent;
use crate::model::EntityTime;
use crate::profile;
use crate::storage::views::UnsafeView;
use crate::tables::Table;
use log::trace;

pub struct LogSystem;

impl<'a> IntentExecutionSystem<'a> for LogSystem {
    type Mut = (UnsafeView<EntityTime, LogEntry>,);
    type Const = ();
    type Intents = Vec<LogIntent>;

    fn execute(&mut self, (mut log_table,): Self::Mut, _: Self::Const, intents: Self::Intents) {
        profile!(" LogSystem update");
        for intent in intents {
            trace!("inserting log entry {:?}", intent);
            let id = EntityTime(intent.entity, intent.time);
            let log_table = unsafe { log_table.as_mut() };
            // use delete to move out of the data structure, then we'll move it back in
            // this should be cheaper than cloning all the time, because of the inner vectors
            match log_table.delete(&id) {
                Some(mut entry) => {
                    entry.payload.extend_from_slice(intent.payload.as_slice());
                    log_table.insert_or_update(id, entry);
                }
                None => {
                    let entry = LogEntry {
                        payload: intent.payload,
                    };
                    log_table.insert_or_update(id, entry);
                }
            };
        }
    }
}
