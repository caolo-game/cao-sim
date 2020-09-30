use crate::intents::Intents;
use crate::prelude::*;
use crate::profile;
use crate::storage::views::UnwrapViewMut;
use std::mem;
use std::ops::DerefMut;

type Mut = (
    UnwrapViewMut<Intents<ScriptHistoryEntry>>,
    UnwrapViewMut<ScriptHistory>,
);
type Const<'a> = ();

pub fn update((mut history_intents, mut history_table): Mut, _: Const) {
    profile!("ScriptHistorySystem update");

    mem::swap(
        &mut history_intents.deref_mut().0,
        &mut history_table.deref_mut().0,
    );

    history_table
        .0
        .sort_unstable_by(|a, b| a.entity.cmp(&b.entity));
}
