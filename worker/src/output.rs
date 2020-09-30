use cao_messages::Bot as BotMsg;
use cao_messages::LogEntry as LogMsg;
use cao_messages::{AxialPoint, TerrainType, Tile as TileMsg, WorldPosition as WorldPositionMsg};
use cao_messages::{Resource as ResourceMsg, ResourceType};
use cao_messages::{
    ScriptHistoryEntry as ScriptHistoryEntryMsg, ScriptHistoryEntryPayload,
    Structure as StructureMsg, StructurePayload as StructurePayloadMsg,
    StructureSpawn as StructureSpawnMsg,
};
use caolo_sim::join;
use caolo_sim::prelude::*;
use caolo_sim::tables::JoinIterator;
use caolo_sim::terrain::TileTerrainType;

type BotInput<'a> = (
    View<'a, EntityId, Bot>,
    View<'a, EntityId, PositionComponent>,
    View<'a, EntityId, OwnedEntity>,
    // body
    (
        View<'a, EntityId, CarryComponent>,
        View<'a, EntityId, HpComponent>,
        View<'a, EntityId, DecayComponent>,
        View<'a, EntityId, EnergyComponent>,
        View<'a, EntityId, EnergyRegenComponent>,
        View<'a, EntityId, EntityScript>,
    ),
    View<'a, EmptyKey, RoomProperties>,
);

pub fn build_bots<'a>(
    (
        bots,
        positions,
        owned_entities,
        (carry_table, hp_table, decay_table, energy_table, energy_regen_table, script_table),
        room_props,
    ): BotInput<'a>,
) -> impl Iterator<Item = BotMsg> + 'a {
    use serde_json::json;

    let bots = bots.reborrow().iter();
    let positions = positions.reborrow().iter();
    let position_tranform = init_world_pos(room_props);

    join!([bots, positions]).map(move |(id, (_bot, pos))| BotMsg {
        id: id.0,
        position: position_tranform(pos.0),
        owner: owned_entities
            .get_by_id(&id)
            .map(|OwnedEntity { owner_id }| owner_id.0),
        body: json! ({
            "hp": hp_table.get_by_id(&id)
            , "carry": carry_table.get_by_id(&id)
            , "decay": decay_table.get_by_id(&id)
            , "energy": energy_table.get_by_id(&id)
            , "energyRegen": energy_regen_table.get_by_id(&id)
            , "script": script_table.get_by_id(&id)
        }),
    })
}

pub fn build_script_history<'a>(
    script_history: UnwrapView<'a, ScriptHistory>,
) -> Vec<ScriptHistoryEntryMsg> {
    use serde_json::to_value;

    script_history
        .0
        .iter()
        .map(|entry| {
            let entity_id = entry.entity;
            let payload = entry
                .payload
                .iter()
                .map(|entry| ScriptHistoryEntryPayload {
                    id: entry.id as i64,
                    instruction: to_value(&entry.instr).unwrap(),
                })
                .collect::<Vec<_>>();
            ScriptHistoryEntryMsg {
                entity_id: entity_id.0,
                payload,
            }
        })
        .collect()
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
    View<'a, EntityId, PositionComponent>,
    View<'a, EntityId, OwnedEntity>,
    View<'a, EmptyKey, RoomProperties>,
    (
        View<'a, EntityId, SpawnComponent>,
        View<'a, EntityId, EnergyComponent>,
        View<'a, EntityId, EnergyRegenComponent>,
    ),
);

pub fn build_structures<'a>(
    (
        structure_table,
        position_table,
        owner_table,
        room_props,
        (spawn_table, energy_table, energy_regen_table),
    ): StructuresInput<'a>,
) -> impl Iterator<Item = StructureMsg> + 'a {
    let spawns = spawn_table.reborrow().iter();
    let structures = structure_table.reborrow().iter();
    let positions = position_table.reborrow().iter();
    let energy = energy_table.reborrow().iter();

    let position_tranform = init_world_pos(room_props);
    join!([spawns, structures, positions, energy]).map(
        move |(id, (spawn, _structure, pos, energy))| StructureMsg {
            id: id.0,
            position: position_tranform(pos.0),
            owner: owner_table
                .get_by_id(&id)
                .map(|OwnedEntity { owner_id }| owner_id.0),
            payload: StructurePayloadMsg::Spawn(StructureSpawnMsg {
                spawning: spawn.spawning.map(|EntityId(id)| id),
                time_to_spawn: spawn.time_to_spawn as i32,
                energy: energy.energy as u32,
                energy_max: energy.energy_max as u32,
                energy_regen: energy_regen_table
                    .get_by_id(&id)
                    .map(|EnergyRegenComponent { amount }| *amount as u32),
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
