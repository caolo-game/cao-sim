//! Actions, world updates the clients _intend_ to execute.
//!
mod dropoff_intent;
mod log_intent;
mod mine_intent;
mod move_intent;
mod pathcache_intent;
mod spawn_intent;

pub use self::dropoff_intent::*;
pub use self::log_intent::*;
pub use self::mine_intent::*;
pub use self::move_intent::*;
pub use self::pathcache_intent::*;
pub use self::spawn_intent::*;

use crate::indices::{EmptyKey, EntityId};
use crate::tables::{unique::UniqueTable, Component};
use serde::{Deserialize, Serialize};

impl Intents {
    pub fn new() -> Self {
        Self::default()
    }
}

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

/// Implements the SOA style intents container
macro_rules! intents {
    ($($name: ident: $type: ty),+,) =>{
        #[derive(Debug, Clone, Default, Serialize, Deserialize)]
        pub struct Intents {
            $(pub $name: Vec<$type>),*
        }

        impl Intents {
            pub fn with_capacity(cap: usize) -> Self {
                Self{
                    $($name: Vec::<$type>::with_capacity(cap)),*
                }
            }

            pub fn merge(&mut self, other: &Intents) -> &mut Self {
                $(self.$name.extend_from_slice(&other.$name));* ;
                self
            }

            pub fn clear(&mut self) {
                $(self.$name.clear());* ;
            }

            pub fn append(&mut self, intents: BotIntents) -> &mut Self {
                $(
                    if let Some(intent) = intents.$name {
                        self.$name.push(intent);
                    }
                )*
                self
            }
        }

        impl Component<EmptyKey> for Intents {
            type Table = UniqueTable<Self>;
        }

        /// Possible intents of a single bot
        #[derive(Debug, Clone, Default)]
        pub struct BotIntents {
            $(pub $name: Option<$type>),*
        }

        $(
            impl<'a> Into<&'a [$type]> for &'a Intents {
                fn into(self) -> &'a [$type] {
                    self.$name.as_slice()
                }
            }
        )*
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
);
