use crate::indices::{EntityId, RoomPosition, ScriptId, UserId, WorldPosition};
use crate::tables::{btree::BTreeTable, vector::DenseVecTable, Component, TableId};
use arrayvec::ArrayVec;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Copy, Default)]
#[serde(rename_all = "camelCase")]
pub struct MeleeAttackComponent {
    pub strength: u16,
}
impl<Id: TableId> Component<Id> for MeleeAttackComponent {
    type Table = BTreeTable<Id, Self>;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Bot;

impl Component<EntityId> for Bot {
    type Table = DenseVecTable<EntityId, Self>;
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
pub struct EntityScript(pub ScriptId);

unsafe impl Send for EntityScript {}
impl Component<EntityId> for EntityScript {
    type Table = DenseVecTable<EntityId, Self>;
}
impl Component<UserId> for EntityScript {
    type Table = BTreeTable<UserId, Self>;
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
