use crate::components::LogEntry;
use crate::indices::EntityTime;
use crate::profile;
use crate::storage::views::UnsafeView;
use crate::tables::Table;
use crate::Time;

type Mut = UnsafeView<EntityTime, LogEntry>;
type Const = Time;

pub fn update(mut logs: Mut, time: Const) {
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

    // we delete in the same table we iterated on above
    // so we can't actually call delete before collecting
    for id in changeset {
        logs.delete(&id);
    }
}
