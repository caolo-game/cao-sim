use crate::intents::Intents;
use crate::prelude::*;
use crate::profile;
use crate::storage::views::{UnsafeView, UnwrapViewMut};
use std::mem;

type Mut = (
    UnwrapViewMut<EmptyKey, Intents<ScriptHistoryEntry>>,
    UnsafeView<EntityId, ScriptHistory>,
);
type Const<'a> = ();

pub fn update((mut history_intents, mut history_table): Mut, _: Const) {
    profile!("ScriptHistorySystem update");

    let Intents(intents) = mem::take(&mut *history_intents);
    for intent in intents {
        history_table.insert_or_update(intent.entity_id, ScriptHistory(intent.payload));
    }
}
