use super::WorldPosition;
use crate::model::geometry::Axial;
use crate::model::terrain::TileTerrainType;
use crate::tables::{BTreeTable, Component, MortonTable, RoomMortonTable, SpatialKey2d, TableId};
use arrayvec::ArrayVec;
use serde_derive::{Deserialize, Serialize};

/// Represents a connection of a room to another.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoomConnection {
    pub direction: Axial,
    /// Where the connection points start on the edge
    pub offset_start: u32,
    /// Where the connection ends
    pub offset_end: u32,
}

/// Represents connections a room has to their neighbours. At most 6.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoomConnections(pub ArrayVec<[RoomConnection; 6]>);
impl<Id: TableId> Component<Id> for RoomConnections {
    type Table = BTreeTable<Id, Self>;
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct TerrainComponent(pub TileTerrainType);
impl Component<WorldPosition> for TerrainComponent {
    type Table = RoomMortonTable<Self>;
}
impl<Id: SpatialKey2d + Send + Sync> Component<Id> for TerrainComponent {
    type Table = MortonTable<Id, Self>;
}
