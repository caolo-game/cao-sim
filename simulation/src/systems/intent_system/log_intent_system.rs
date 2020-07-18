use super::IntentExecutionSystem;
use crate::components::LogEntry;
use crate::intents::LogIntent;
use crate::model::EntityTime;
use crate::profile;
use crate::storage::views::UnsafeView;

pub struct LogSystem;

impl<'a> IntentExecutionSystem<'a> for LogSystem {
    type Mut = (UnsafeView<EntityTime, LogEntry>,);
    type Const = ();
    type Intent = LogIntent;

    fn execute(&mut self, (mut log_table,): Self::Mut, _: Self::Const, intents: &[Self::Intent]) {
        profile!(" LogSystem update");
        for intent in intents {
            trace!("inserting log entry {:?}", intent);
            let id = EntityTime(intent.entity, intent.time);
            let entry = match log_table.get_by_id(&id).cloned() {
                Some(mut entry) => {
                    entry.payload.push(intent.payload.clone());
                    entry
                }
                None => LogEntry {
                    payload: vec![intent.payload.clone()],
                },
            };
            unsafe {
                log_table.as_mut().insert_or_update(id, entry);
            }
        }
    }
}
