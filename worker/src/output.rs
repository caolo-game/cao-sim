use cao_messages::Bot as BotMsg;
use cao_messages::LogEntry as LogMsg;
use cao_messages::{AxialPoint, TerrainType, Tile as TileMsg, WorldPosition as WorldPositionMsg};
use cao_messages::{Resource as ResourceMsg, ResourceType};
use cao_messages::{
    Structure as StructureMsg, StructurePayload as StructurePayloadMsg,
    StructureSpawn as StructureSpawnMsg,
};
use caolo_sim::prelude::*;
use caolo_sim::tables::JoinIterator;
use caolo_sim::terrain::TileTerrainType;

type BotInput<'a> = (
    View<'a, EntityId, Bot>,
    View<'a, EntityId, PositionComponent>,
    View<'a, EntityId, OwnedEntity>,
    View<'a, EmptyKey, RoomProperties>,
);

pub fn build_bots<'a>(
    (bots, positions, owned_entities, room_props): BotInput<'a>,
) -> impl Iterator<Item = BotMsg> + 'a {
    let bots = bots.reborrow().iter();
    let positions = positions.reborrow().iter();
    let position_tranform = init_world_pos(room_props);
    JoinIterator::new(bots, positions).map(move |(id, (_bot, pos))| BotMsg {
        id: id.0,
        position: position_tranform(pos.0),
        owner: owned_entities
            .get_by_id(&id)
            .map(|OwnedEntity { owner_id }| owner_id.0),
    })
}

pub fn build_logs<'a>(v: View<'a, EntityTime, LogEntry>) -> impl Iterator<Item = LogMsg> + 'a {
    v.reborrow()
        .iter()
        .map(|(EntityTime(EntityId(eid), time), entries)| LogMsg {
            entity_id: eid,
            time,
            payload: entries.payload.to_vec(),
        })
}

pub fn build_terrain<'a>(
    (v, room_props): (
        View<'a, WorldPosition, TerrainComponent>,
        View<'a, EmptyKey, RoomProperties>,
    ),
) -> impl Iterator<Item = (AxialPoint, Vec<TileMsg>)> + 'a {
    let room_props = room_props;
    let position_tranform = init_world_pos(room_props);
    v.reborrow().table.iter().map(move |(room, table)| {
        (
            AxialPoint {
                q: room.q,
                r: room.r,
            },
            table
                .iter()
                .map(|(pos, TerrainComponent(tile))| {
                    let pos = WorldPosition { room, pos };
                    TileMsg {
                        position: position_tranform(pos),
                        ty: match tile {
                            TileTerrainType::Plain => TerrainType::Plain,
                            TileTerrainType::Wall => TerrainType::Wall,
                            TileTerrainType::Bridge => TerrainType::Bridge,
                        },
                    }
                })
                .collect(),
        )
    })
}

type ResourceInput<'a> = (
    View<'a, EntityId, ResourceComponent>,
    View<'a, EntityId, PositionComponent>,
    View<'a, EntityId, EnergyComponent>,
    View<'a, EmptyKey, RoomProperties>,
);

pub fn build_resources<'a>(
    (resource_table, position_table, energy_table, room_props): ResourceInput<'a>,
) -> impl Iterator<Item = ResourceMsg> + 'a {
    let join = JoinIterator::new(
        resource_table.reborrow().iter(),
        position_table.reborrow().iter(),
    );

    let position_tranform = init_world_pos(room_props);
    JoinIterator::new(join, energy_table.reborrow().iter()).filter_map(
        move |(id, ((resource, pos), energy))| match resource.0 {
            Resource::Energy => {
                let msg = ResourceMsg {
                    id: id.0,
                    position: position_tranform(pos.0),
                    ty: ResourceType::Energy {
                        energy: energy.energy as u32,
                        energy_max: energy.energy_max as u32,
                    },
                };
                Some(msg)
            }
            _ => None,
        },
    )
}

type StructuresInput<'a> = (
    View<'a, EntityId, Structure>,
    View<'a, EntityId, SpawnComponent>,
    View<'a, EntityId, PositionComponent>,
    View<'a, EntityId, OwnedEntity>,
    View<'a, EmptyKey, RoomProperties>,
);

pub fn build_structures<'a>(
    (structure_table, spawn_table, position_table, owner_table, room_props): StructuresInput<'a>,
) -> impl Iterator<Item = StructureMsg> + 'a {
    let spawns = JoinIterator::new(
        spawn_table.reborrow().iter(),
        structure_table.reborrow().iter(),
    );
    let position_tranform = init_world_pos(room_props);
    JoinIterator::new(spawns, position_table.reborrow().iter()).map(
        move |(id, ((spawn, _structure), pos))| StructureMsg {
            id: id.0,
            position: position_tranform(pos.0),
            owner: owner_table
                .get_by_id(&id)
                .map(|OwnedEntity { owner_id }| owner_id.0),
            payload: StructurePayloadMsg::Spawn(StructureSpawnMsg {
                spawning: spawn.spawning.map(|EntityId(id)| id),
                time_to_spawn: spawn.time_to_spawn as i32,
            }),
        },
    )
}

fn init_world_pos(
    _conf: View<EmptyKey, RoomProperties>,
) -> impl Fn(WorldPosition) -> WorldPositionMsg {
    move |world_pos| WorldPositionMsg {
        room: AxialPoint {
            q: world_pos.room.q,
            r: world_pos.room.r,
        },
        room_pos: AxialPoint {
            q: world_pos.pos.q,
            r: world_pos.pos.r,
        },
    }
}
