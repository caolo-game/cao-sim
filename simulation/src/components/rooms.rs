use crate::geometry::Axial;
use crate::model::terrain::TileTerrainType;
use crate::model::EmptyKey;
use crate::model::WorldPosition;
use crate::tables::{
    morton::MortonTable, unique::UniqueTable, Component, RoomMortonTable, SpatialKey2d,
};
use serde_derive::{Deserialize, Serialize};

/// Represents a connection of a room to another.
/// Length of the Bridge is defined by `radius - offset_end - offset_start`.
/// I choose to represent connections this way because it is much easier to invert them.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoomConnection {
    pub direction: Axial,
    /// Where the bridge points start on the edge
    pub offset_start: u32,
    /// Where the bridge points end on the edge
    pub offset_end: u32,
}

/// Represents connections a room has to their neighbours. At most 6.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoomConnections(pub [Option<RoomConnection>; 6]);
impl<Id: SpatialKey2d + Send + Sync> Component<Id> for RoomConnections {
    type Table = MortonTable<Id, Self>;
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct TerrainComponent(pub TileTerrainType);
impl Component<WorldPosition> for TerrainComponent {
    type Table = RoomMortonTable<Self>;
}
impl<Id: SpatialKey2d + Send + Sync> Component<Id> for TerrainComponent {
    type Table = MortonTable<Id, Self>;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoomProperties {
    pub radius: u32,
}
impl Component<EmptyKey> for RoomProperties {
    type Table = UniqueTable<Self>;
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct RoomComponent;

impl<Id: SpatialKey2d + Send + Sync> Component<Id> for RoomComponent {
    type Table = MortonTable<Id, Self>;
}
