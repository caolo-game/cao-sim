use cao_messages::point_capnp::{
    axial_point,
    world_position::{self, Reader as WorldPositionMsg},
};
use cao_messages::world_capnp::bot::{self, Reader as BotMsg};
use cao_messages::world_capnp::log_entry::{self, Reader as LogMsg};
use cao_messages::world_capnp::world_state;
use caolo_sim::join;
use caolo_sim::prelude::*;
use caolo_sim::tables::JoinIterator;
use caolo_sim::terrain::TileTerrainType;
use std::collections::HashMap;

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
    world: &mut world_state::Builder,
) {
    use serde_json::json;

    let len = bots.count_set();
    let bots = bots.reborrow().iter();
    let positions = positions.reborrow().iter();
    let position_tranform = init_world_pos(room_props);

    let mut list = world.init_bots(len as u32);

    join!([bots, positions])
        .enumerate()
        .for_each(move |(i, (id, (_bot, pos)))| {
            assert!(i < len);
            let mut msg = list.get(i as u32);
            let owner = owned_entities
                .get_by_id(&id)
                .map(|OwnedEntity { owner_id }| owner_id.0);

            let body = json! ({
                "hp": hp_table.get_by_id(&id)
                , "carry": carry_table.get_by_id(&id)
                , "decay": decay_table.get_by_id(&id)
                , "energy": energy_table.get_by_id(&id)
                , "energyRegen": energy_regen_table.get_by_id(&id)
                , "script": script_table.get_by_id(&id)
            });
            let body = serde_json::to_string(&body).unwrap();

            msg.set_id(id.0);
            let mut position = msg.init_position();
            position_tranform(pos.0, &mut position);
            let mut js = msg.init_body();
            js.set_value(body.as_str());
        });
}

pub fn build_script_history<'a>(
    script_history: UnwrapView<'a, ScriptHistory>,
    world: &mut world_state::Builder,
) {
    let len = script_history.0.len();
    let mut list = world.init_script_history(len as u32);

    script_history.0.iter().enumerate().for_each(|(i, entry)| {
        let mut item = list.get(i as u32);

        item.set_entity_id(entry.entity.0);
        let mut list = item.init_payload(entry.payload.len() as u32);

        entry.payload.iter().enumerate().for_each(|(i, entry)| {
            list.set(i as u32, entry.id as i64);
        });
    });
}

pub fn build_logs<'a>(v: View<'a, EntityTime, LogEntry>, world: &mut world_state::Builder) {
    let len = v.len();
    let mut list = world.init_logs(len as u32);
    v.reborrow()
        .iter()
        .enumerate()
        .for_each(|(i, (EntityTime(EntityId(eid), time), entries))| {
            // entity_id: eid,
            // time,
            // payload: entries.payload.to_vec(),
        });
}

// pub fn build_terrain<'a>(
//     (v, room_props): (
//         View<'a, WorldPosition, TerrainComponent>,
//         View<'a, EmptyKey, RoomProperties>,
//     ),
// ) -> impl Iterator<Item = (AxialPoint, Vec<TileMsg>)> + 'a {
//     let room_props = room_props;
//     let position_tranform = init_world_pos(room_props);
//     v.reborrow().table.iter().map(move |(room, table)| {
//         (
//             AxialPoint {
//                 q: room.q,
//                 r: room.r,
//             },
//             table
//                 .iter()
//                 .map(|(pos, TerrainComponent(tile))| {
//                     let pos = WorldPosition { room, pos };
//                     TileMsg {
//                         position: position_tranform(pos),
//                         ty: match tile {
//                             TileTerrainType::Plain => TerrainType::Plain,
//                             TileTerrainType::Wall => TerrainType::Wall,
//                             TileTerrainType::Bridge => TerrainType::Bridge,
//                         },
//                     }
//                 })
//                 .collect(),
//         )
//     })
// }

type ResourceInput<'a> = (
    View<'a, EntityId, ResourceComponent>,
    View<'a, EntityId, PositionComponent>,
    View<'a, EntityId, EnergyComponent>,
    View<'a, EmptyKey, RoomProperties>,
);

pub fn build_resources<'a>(
    (resource_table, position_table, energy_table, room_props): ResourceInput<'a>,
    world: &mut world_state::Builder,
) {
    let resources = resource_table.reborrow().iter();
    let positions = position_table.reborrow().iter();
    let energy = energy_table.reborrow().iter();

    let len = resource_table.len();

    let mut list = world.init_resources(len as u32);

    let position_tranform = init_world_pos(room_props);
    join!([resources, positions, energy]).enumerate().for_each(
        move |(i, (id, (resource, pos, energy)))| match resource.0 {
            Resource::Energy => {
                let mut msg = list.get(i as u32);
                msg.set_id(id.0);
                let mut position = msg.init_position();
                position_tranform(pos.0, &mut position);
                let mut e = msg.init_energy();
                e.set_energy(energy.energy as u32);
                e.set_energy_max(energy.energy_max as u32);
            }
            Resource::Empty => {}
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
    world: &mut world_state::Builder,
) {
    let len = spawn_table.len();
    let mut list = world.init_structures(len as u32);

    let spawns = spawn_table.reborrow().iter();
    let structures = structure_table.reborrow().iter();
    let positions = position_table.reborrow().iter();
    let energy = energy_table.reborrow().iter();

    let position_tranform = init_world_pos(room_props);
    join!([spawns, structures, positions, energy])
        .enumerate()
        .for_each(move |(i, (id, (spawn, _structure, pos, energy)))| {
            let mut msg = list.get(i as u32);
            msg.set_id(id.0);
            position_tranform(pos.0, &mut msg.init_position());
            if let Some(owner_id) = owner_table
                .get_by_id(&id)
                .map(|OwnedEntity { owner_id }| owner_id.0)
            {
                let mut owner = msg.init_owner();
                owner.set_data(&owner_id.as_bytes()[..]);
            }

            let mut body = msg.init_spawn();
            if let Some(spawning) = spawn.spawning.map(|EntityId(id)| id) {
                body.set_spawning(spawning);
            }
            body.set_energy(energy.energy as u32);
            body.set_energy_max(energy.energy_max as u32);
            if let Some(regen) = energy_regen_table
                .get_by_id(&id)
                .map(|EnergyRegenComponent { amount }| *amount as u32)
            {
                body.set_energy_regen(regen);
            }
        });
}

fn init_world_pos(
    _conf: View<EmptyKey, RoomProperties>,
) -> impl Fn(WorldPosition, &mut world_position::Builder) {
    move |world_pos, builder| {
        let mut room = builder.reborrow().init_room();
        room.set_q(world_pos.room.q);
        room.set_r(world_pos.room.r);

        let mut room_pos = builder.reborrow().init_room_pos();
        room_pos.set_q(world_pos.pos.q);
        room_pos.set_r(world_pos.pos.r);
    }
}
