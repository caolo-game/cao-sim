use super::System;
use crate::components::LogEntry;
use crate::indices::EntityTime;
use crate::profile;
use crate::storage::views::UnsafeView;
use crate::tables::Table;
use crate::Time;

pub struct LogSystem;

impl<'a> System<'a> for LogSystem {
    type Mut = UnsafeView<EntityTime, LogEntry>;
    type Const = Time;

    fn update(&mut self, mut logs: Self::Mut, time: Self::Const) {
        profile!("LogSystem update");
        // clear the old logs
        let changeset = logs
            .iter()
            .filter_map(|(id, _)| {
                if id.1 < time.0.max(5) - 5 {
                    Some(id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        unsafe {
            let logs = logs.as_mut();
            for id in changeset.into_iter() {
                logs.delete(&id);
            }
        }
    }
}
