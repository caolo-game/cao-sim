use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Copy, Serialize, Deserialize)]
#[repr(u8)]
pub enum TileTerrainType {
    Plain = 0,
    Wall,
}

impl Default for TileTerrainType {
    fn default() -> Self {
        TileTerrainType::Plain
    }
}

impl TileTerrainType {
    pub fn is_walkable(&self) -> bool {
        is_walkable(*self)
    }
}

pub fn is_walkable(tile: TileTerrainType) -> bool {
    match tile {
        TileTerrainType::Plain => true,
        _ => false,
    }
}
