mod resources;
mod rooms;
pub use resources::*;
pub use rooms::*;

use crate::indices::{EntityId, RoomPosition, ScriptId, UserId, WorldPosition};
use crate::tables::{
    btree::BTreeTable, morton::MortonTable, vector::VecTable, Component, RoomMortonTable,
    SpatialKey2d, TableId,
};
use arrayvec::ArrayVec;
use serde_derive::{Deserialize, Serialize};
use std::collections::VecDeque;

/// For tables that store entity ids as values
#[derive(Debug, Clone, Serialize, Deserialize, Copy, Default, Ord, PartialOrd, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EntityComponent(pub EntityId);
impl<Id: SpatialKey2d + Send + Sync> Component<Id> for EntityComponent {
    type Table = MortonTable<Id, Self>;
}
impl Component<WorldPosition> for EntityComponent {
    type Table = RoomMortonTable<Self>;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Bot;

impl Component<EntityId> for Bot {
    type Table = VecTable<EntityId, Self>;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Structure {}

impl<Id: TableId> Component<Id> for Structure {
    type Table = BTreeTable<Id, Self>;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OwnedEntity {
    pub owner_id: UserId,
}

impl Component<EntityId> for OwnedEntity {
    type Table = VecTable<EntityId, Self>;
}

#[derive(Default, Debug, Clone, Copy, Ord, PartialOrd, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionComponent(pub WorldPosition);
impl Component<EntityId> for PositionComponent {
    type Table = VecTable<EntityId, Self>;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EnergyComponent {
    pub energy: u16,
    pub energy_max: u16,
}
impl<Id: TableId> Component<Id> for EnergyComponent {
    type Table = BTreeTable<Id, Self>;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SpawnComponent {
    /// Time to spawn the current entity
    pub time_to_spawn: i16,
    /// Currently spawning entity
    pub spawning: Option<EntityId>,
}

impl<Id: TableId> Component<Id> for SpawnComponent {
    type Table = BTreeTable<Id, Self>;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SpawnQueueComponent {
    /// Entities waiting for spawn
    pub queue: VecDeque<EntityId>,
}

impl<Id: TableId> Component<Id> for SpawnQueueComponent {
    type Table = BTreeTable<Id, Self>;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HpComponent {
    pub hp: u16,
    pub hp_max: u16,
}
impl Component<EntityId> for HpComponent {
    type Table = VecTable<EntityId, Self>;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EnergyRegenComponent {
    pub amount: u16,
}
impl<Id: TableId> Component<Id> for EnergyRegenComponent {
    type Table = BTreeTable<Id, Self>;
}

/// Represent time to decay of bots
/// On decay the bot will loose hp
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DecayComponent {
    pub hp_amount: u16,
    pub interval: u8,
    pub time_remaining: u8,
}
impl<Id: TableId> Component<Id> for DecayComponent {
    type Table = BTreeTable<Id, Self>;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SpawnBotComponent {
    pub bot: Bot,
}

impl<Id: TableId> Component<Id> for SpawnBotComponent {
    type Table = BTreeTable<Id, Self>;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CarryComponent {
    pub carry: u16,
    pub carry_max: u16,
}
impl<Id: TableId> Component<Id> for CarryComponent {
    type Table = BTreeTable<Id, Self>;
}

/// Entity - Script join table
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EntityScript {
    pub script_id: ScriptId,
}
unsafe impl Send for EntityScript {}
impl<Id: TableId> Component<Id> for EntityScript {
    type Table = BTreeTable<Id, Self>;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub payload: Vec<String>,
}
impl<Id: TableId> Component<Id> for LogEntry {
    type Table = BTreeTable<Id, Self>;
}

/// Entities with Scripts
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScriptComponent(pub cao_lang::CompiledProgram);
impl<Id: TableId> Component<Id> for ScriptComponent {
    type Table = BTreeTable<Id, Self>;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserComponent;
impl<Id: TableId> Component<Id> for UserComponent {
    type Table = BTreeTable<Id, Self>;
}

pub const PATH_CACHE_LEN: usize = 64;
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PathCacheComponent {
    pub target: WorldPosition,
    pub path: ArrayVec<[RoomPosition; PATH_CACHE_LEN]>,
}
impl<Id: TableId> Component<Id> for PathCacheComponent {
    type Table = BTreeTable<Id, Self>;
}
