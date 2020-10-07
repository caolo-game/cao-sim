use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Copy, Serialize, Deserialize)]
#[repr(u8)]
#[serde(rename_all = "camelCase")]
pub enum TileTerrainType {
    Plain,
    /// allows teleporting to new rooms
    Bridge,
    Wall,
}

impl Default for TileTerrainType {
    fn default() -> Self {
        TileTerrainType::Plain
    }
}

impl TileTerrainType {
    pub fn is_walkable(self) -> bool {
        is_walkable(self)
    }
}

pub fn is_walkable(tile: TileTerrainType) -> bool {
    match tile {
        TileTerrainType::Plain | TileTerrainType::Bridge => true,
        _ => false,
    }
}
