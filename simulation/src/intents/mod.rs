//! Actions, world updates the clients _intend_ to execute.
//!
mod attack_intent;
mod dropoff_intent;
mod log_intent;
mod mine_intent;
mod move_intent;
mod pathcache_intent;
mod spawn_intent;

pub use self::attack_intent::*;
pub use self::dropoff_intent::*;
pub use self::log_intent::*;
pub use self::mine_intent::*;
pub use self::move_intent::*;
pub use self::pathcache_intent::*;
pub use self::spawn_intent::*;

use crate::components::ScriptHistoryEntry;
use crate::indices::{EmptyKey, EntityId};
use crate::prelude::World;
use crate::tables::{unique::UniqueTable, Component};
use serde::{Deserialize, Serialize};

impl BotIntents {
    pub fn with_log<S: Into<String>>(
        &mut self,
        entity: EntityId,
        payload: S,
        time: u64,
    ) -> &mut Self {
        if self.log_intent.is_none() {
            self.log_intent = Some(LogIntent {
                entity,
                payload: Vec::with_capacity(64),
                time,
            })
        }
        if let Some(ref mut log_intent) = self.log_intent {
            log_intent.payload.push(payload.into());
        }
        self
    }
}

/// Implements the SOA style intents container.
///
/// Assumes that the Intents were registered in World. (see the data_store module)
macro_rules! intents {
    ($($name: ident : $type: ty),+,) => {
        /// Move the botintents into the world, overriding the existing ones
        pub fn move_into_storage(s: &mut World, intents: Vec<BotIntents>)  {
            use crate::storage::views::{UnwrapViewMut, FromWorldMut};
            // reset the intent tables
            $(
                let mut ints = s.unsafe_view::<EmptyKey, Intents<$type>>();
                match ints.value.as_mut() {
                    Some(ints) => ints.0.clear(),
                    None => {
                        ints.value = Some(Intents(Vec::with_capacity(intents.len())));
                    }
                }
            )*
            for intent in intents {
            $(
                if let Some(intent) = intent.$name {
                    let mut ints = UnwrapViewMut::<Intents<$type>>::new(s);
                    ints.0.push(intent);
                }
            )*
            }
        }

        /// Newtype wrapper on intents to implement Component
        #[derive(Debug, Clone, Default, Serialize, Deserialize)]
        pub struct Intents<T> (pub Vec<T>);
        $(
            impl Component<EmptyKey> for Intents<$type> {
                type Table = UniqueTable<Self>;
            }
        )*

        impl<T> std::ops::DerefMut for Intents<T> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                self.0.as_mut_slice()
            }
        }

        impl<T> std::ops::Deref for Intents<T> {
            type Target=[T];
            fn deref(&self) -> &Self::Target {
                self.0.as_slice()
            }
        }

        /// Possible intents of a single bot
        #[derive(Debug, Clone, Default)]
        pub struct BotIntents {
            pub entity_id: EntityId,
            $(pub $name: Option<$type>),*
        }
    };
}

intents!(
    move_intent: MoveIntent,
    spawn_intent: SpawnIntent,
    mine_intent: MineIntent,
    dropoff_intent: DropoffIntent,
    log_intent: LogIntent,
    update_path_cache_intent: CachePathIntent,
    mut_path_cache_intent: MutPathCacheIntent,
    script_history_intent: ScriptHistoryEntry,
    melee_attack_intent: MeleeIntent,
);
