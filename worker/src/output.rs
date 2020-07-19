use cao_math::vec::vec2f32::Point as Vec2;
use cao_math::vec::vec3f32::Point as Vec3;
use caolo_messages::Bot as BotMsg;
use caolo_messages::LogEntry as LogMsg;
use caolo_messages::{
    AxialPoint, Point as PointMsg, TerrainType, Tile as TileMsg, WorldPosition as WorldPositionMsg,
};
use caolo_messages::{Resource as ResourceMsg, ResourceType};
use caolo_messages::{
    Structure as StructureMsg, StructurePayload as StructurePayloadMsg,
    StructureSpawn as StructureSpawnMsg,
};
use caolo_sim::components::{
    Bot, EnergyComponent, LogEntry, OwnedEntity, PositionComponent, Resource, ResourceComponent,
    RoomProperties, SpawnComponent, Structure, TerrainComponent,
};
use caolo_sim::model::{
    indices::EntityTime, terrain::TileTerrainType, EmptyKey, EntityId, WorldPosition,
};
use caolo_sim::storage::views::View;
use caolo_sim::tables::traits::SpatialKey2d;
use caolo_sim::tables::JoinIterator;

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
    JoinIterator::new(bots, positions).map(move |(id, (_bot, pos))| {
        let msg = BotMsg {
            id: id.0,
            position: position_tranform(pos.0),
            owner: owned_entities
                .get_by_id(&id)
                .map(|OwnedEntity { owner_id }| owner_id.0),
        };
        msg
    })
}

pub fn build_logs<'a>(v: View<'a, EntityTime, LogEntry>) -> impl Iterator<Item = LogMsg> + 'a {
    v.reborrow()
        .iter()
        .map(|(EntityTime(EntityId(eid), time), entries)| LogMsg {
            entity_id: eid,
            time,
            payload: entries.payload.iter().cloned().collect(),
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
    JoinIterator::new(join, energy_table.reborrow().iter()).map(
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
                msg
            }
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
        move |(id, ((spawn, _structure), pos))| {
            let msg = StructureMsg {
                id: id.0,
                position: position_tranform(pos.0),
                owner: owner_table
                    .get_by_id(&id)
                    .map(|OwnedEntity { owner_id }| owner_id.0),
                payload: StructurePayloadMsg::Spawn(StructureSpawnMsg {
                    spawning: spawn.spawning.map(|EntityId(id)| id),
                    time_to_spawn: spawn.time_to_spawn as i32,
                }),
            };
            msg
        },
    )
}

fn init_world_pos(
    conf: View<EmptyKey, RoomProperties>,
) -> impl Fn(WorldPosition) -> WorldPositionMsg {
    let radius = conf.unwrap_value().radius;
    let trans = cao_math::hex::axial_to_pixel_mat_flat().as_mat3f().val
        * ((radius as f32 + 0.5) * 3.0f32.sqrt());
    let transform = cao_math::hex::axial_to_pixel_mat_pointy().as_mat3f();

    move |world_pos| {
        let [x, y] = world_pos.room.as_array();
        let [x, y, z] = trans.right_prod([x as f32, y as f32, 1.0]);
        let offset = Vec3::new(x, y, z);

        let [x, y] = world_pos.pos.as_array();
        let p = Vec2::new(x as f32, y as f32).to_3d_vector();
        let p = transform.right_prod(&p);
        let p = p + offset;
        let [x, y] = [p.x, p.y];

        WorldPositionMsg {
            room: AxialPoint {
                q: world_pos.room.q,
                r: world_pos.room.r,
            },
            room_pos: AxialPoint {
                q: world_pos.pos.q,
                r: world_pos.pos.r,
            },
            absolute_pos: PointMsg { x, y },
        }
    }
}
