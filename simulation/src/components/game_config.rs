use crate::indices::ConfigKey;
use crate::tables::{unique::UniqueTable, Component};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub execution_limit: u32,
    pub target_tick_ms: u64,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            execution_limit: 128,
            target_tick_ms: 100,
        }
    }
}

impl Component<ConfigKey> for GameConfig {
    type Table = UniqueTable<ConfigKey, Self>;
}
