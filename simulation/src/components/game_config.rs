use crate::indices::EmptyKey;
use crate::tables::{unique::UniqueTable, Component};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub execution_limit: u32,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            execution_limit: 128,
        }
    }
}

impl Component<EmptyKey> for GameConfig {
    type Table = UniqueTable<Self>;
}
