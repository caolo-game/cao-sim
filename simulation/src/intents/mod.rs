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

impl Intents {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Implements the SOA style intents container
macro_rules! intents {
    ($($name: ident: $type: ty),+,) =>{
        #[derive(Debug, Clone, Default)]
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
    move_intents: MoveIntent,
    spawn_intents: SpawnIntent,
    mine_intents: MineIntent,
    dropoff_intents: DropoffIntent,
    log_intents: LogIntent,
    update_path_cache_intents: CachePathIntent,
    pop_path_cache_intents: PopPathCacheIntent,
);
