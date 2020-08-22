use super::System;
use crate::components::LogEntry;
use crate::indices::EntityTime;
use crate::intents::LogIntent;
use crate::profile;
use crate::storage::views::UnsafeView;
use crate::tables::Table;
use log::trace;
use std::mem;

pub struct LogIntentSystem {
    pub intents: Vec<LogIntent>,
}

impl<'a> System<'a> for LogIntentSystem {
    type Mut = (UnsafeView<EntityTime, LogEntry>,);
    type Const = ();

    fn update(&mut self, (mut log_table,): Self::Mut, (): Self::Const) {
        profile!("LogIntentSystem update");

        let intents = mem::replace(&mut self.intents, vec![]);

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
