use crate::components::LogEntry;
use crate::indices::EntityTime;
use crate::intents::Intents;
use crate::profile;
use crate::storage::views::{UnsafeView, UnwrapViewMut};
use crate::tables::Table;
use log::trace;
use std::mem::replace;

type Mut = (UnsafeView<EntityTime, LogEntry>, UnwrapViewMut<Intents>);

pub fn update((mut log_table, mut intents): Mut, _: ()) {
    profile!("LogIntentSystem update");

    let intents = replace(&mut intents.log_intent, vec![]);

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
