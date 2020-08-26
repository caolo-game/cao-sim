use cao_lang::prelude::*;
use caolo_sim::map_generation::generate_full_map;
use caolo_sim::map_generation::overworld::OverworldGenerationParams;
use caolo_sim::map_generation::room::RoomGenerationParams;
use caolo_sim::prelude::*;
use rand::Rng;
use slog::{debug, trace, Logger};
use std::pin::Pin;
use uuid::Uuid;

pub fn init_storage(logger: Logger, n_fake_users: usize) -> Pin<Box<World>> {
    debug!(logger, "initializing world");
    assert!(n_fake_users >= 1);

    let mut rng = rand::thread_rng();

    let mut storage = caolo_sim::init_inmemory_storage(logger.clone());

    let mining_script_id = ScriptId(Uuid::new_v4());
    let script: CompilationUnit =
        serde_json::from_str(include_str!("./programs/mining_program.json"))
            .expect("deserialize example program");
    debug!(logger, "compiling default program");
    let compiled = compile(None, script).expect("failed to compile example program");
    debug!(logger, "compilation done");

    caolo_sim::query!(
        mutate
        storage
        {
            ScriptId, ScriptComponent,
                .insert_or_update(mining_script_id, ScriptComponent(compiled));
        }
    );

    let center_walking_script_id = ScriptId(Uuid::new_v4());
    let script: CompilationUnit =
        serde_json::from_str(include_str!("./programs/center_walking_program.json"))
            .expect("deserialize example program");
    debug!(logger, "compiling default program");
    let compiled = compile(None, script).expect("failed to compile example program");
    debug!(logger, "compilation done");

    caolo_sim::query!(
        mutate
        storage
        {
            ScriptId, ScriptComponent,
                .insert_or_update(center_walking_script_id, ScriptComponent(compiled));
        }
    );

    let world_radius = std::env::var("CAO_MAP_OVERWORLD_RADIUS")
        .map(|w| {
            w.parse()
                .expect("expected map overworld radius to be an integer")
        })
        .unwrap_or_else(|_| {
            let a = n_fake_users as f32;
            ((a * 1.0 / (3.0 * 3.0f32.sqrt())).powf(0.33)).ceil() as usize
        });
    let width = std::env::var("CAO_MAP_WIDTH")
        .map(|w| w.parse().expect("expected map width to be an integer"))
        .unwrap_or(32);

    let radius = width as u32 / 2;
    assert!(radius > 6);
    let params = OverworldGenerationParams::builder()
        .with_radius(world_radius as u32)
        .with_room_radius(radius)
        .with_min_bridge_len(3)
        .with_max_bridge_len(radius - 3)
        .build()
        .unwrap();
    let room_params = RoomGenerationParams::builder()
        .with_radius(radius)
        .with_chance_plain(0.33)
        .with_chance_wall(0.33)
        .with_plain_dilation(2)
        .build()
        .unwrap();
    debug!(logger, "generating map {:#?} {:#?}", params, room_params);

    generate_full_map(
        storage.logger.clone(),
        &params,
        &room_params,
        None,
        FromWorldMut::new(&mut *storage),
    )
    .unwrap();
    debug!(logger, "world generation done");

    debug!(logger, "Reset position storage");
    let mut entities_by_pos = storage.unsafe_view::<WorldPosition, EntityComponent>();
    entities_by_pos.clear();
    entities_by_pos
        .table
        .extend(
            storage
                .view::<Room, RoomComponent>()
                .iter()
                .map(|(Room(roomid), _)| ((roomid, Default::default()))),
        )
        .expect("entities_by_pos init");
    let bounds = Hexagon {
        center: Axial::new(radius as i32, radius as i32),
        radius: radius as i32,
    };
    let rooms = storage
        .view::<Room, RoomComponent>()
        .iter()
        .map(|a| a.0)
        .collect::<Vec<_>>();

    let mut taken_rooms = Vec::with_capacity(n_fake_users);
    for i in 0..n_fake_users {
        trace!(logger, "initializing room #{}", i);
        let storage = &mut storage;
        let spawnid = storage.insert_entity();

        let room = rng.gen_range(0, rooms.len());
        let room = rooms[room];
        taken_rooms.push(room);

        trace!(logger, "initializing room #{} in room {:?}", i, room);
        init_spawn(
            &logger,
            &bounds,
            spawnid,
            room,
            &mut rng,
            FromWorldMut::new(storage),
            FromWorld::new(storage),
        );
        trace!(logger, "spawning entities");
        let spawn_pos = storage
            .view::<EntityId, PositionComponent>()
            .get_by_id(&spawnid)
            .expect("spawn should have position")
            .0;
        for _ in 0..3 {
            let botid = storage.insert_entity();
            init_bot(
                botid,
                mining_script_id,
                spawn_pos,
                FromWorldMut::new(storage),
            );
        }
        for _ in 0..3 {
            let botid = storage.insert_entity();
            init_bot(
                botid,
                center_walking_script_id,
                spawn_pos,
                FromWorldMut::new(storage),
            );
        }
        let id = storage.insert_entity();
        init_resource(
            &logger,
            &bounds,
            id,
            room,
            &mut rng,
            FromWorldMut::new(storage),
            FromWorld::new(storage),
        );
        trace!(logger, "initializing room #{} done", i);
    }

    debug!(logger, "init done");
    storage
}

type InitBotMuts = (
    UnsafeView<EntityId, EntityScript>,
    UnsafeView<EntityId, Bot>,
    UnsafeView<EntityId, CarryComponent>,
    UnsafeView<EntityId, OwnedEntity>,
    UnsafeView<EntityId, PositionComponent>,
    UnsafeView<WorldPosition, EntityComponent>,
);

fn init_bot(
    id: EntityId,
    script_id: ScriptId,
    pos: WorldPosition,
    (
        mut entity_scripts,
        mut bots,
        mut carry_component,
        mut owners,
        mut positions,
        mut entities_by_pos,
    ): InitBotMuts,
) {
    entity_scripts.insert_or_update(id, EntityScript { script_id });
    bots.insert_or_update(id, Bot {});
    carry_component.insert_or_update(
        id,
        CarryComponent {
            carry: 0,
            carry_max: 50,
        },
    );
    owners.insert_or_update(
        id,
        OwnedEntity {
            owner_id: Default::default(),
        },
    );

    positions.insert_or_update(id, PositionComponent(pos));
    entities_by_pos
        .table
        .get_by_id_mut(&pos.room)
        .expect("expected bot pos to be in the table")
        .insert(pos.pos, EntityComponent(id))
        .expect("entities_by_pos insert");
}

type InitSpawnMuts = (
    UnsafeView<EntityId, OwnedEntity>,
    UnsafeView<EntityId, SpawnComponent>,
    UnsafeView<EntityId, Structure>,
    UnsafeView<EntityId, PositionComponent>,
    UnsafeView<WorldPosition, EntityComponent>,
);
type InitSpawnConst<'a> = (View<'a, WorldPosition, TerrainComponent>,);

fn init_spawn(
    logger: &Logger,
    bounds: &Hexagon,
    id: EntityId,
    room: Room,
    rng: &mut impl Rng,
    (mut owners, mut spawns, mut structures, mut positions, mut entities_by_pos): InitSpawnMuts,
    (terrain,): InitSpawnConst,
) {
    debug!(logger, "init_spawn");
    structures.insert_or_update(id, Structure {});
    spawns.insert_or_update(id, SpawnComponent::default());
    owners.insert_or_update(
        id,
        OwnedEntity {
            owner_id: Default::default(),
        },
    );

    let pos = uncontested_pos(logger, room, bounds, &*entities_by_pos, &*terrain, rng);

    positions.insert_or_update(id, PositionComponent(pos));
    entities_by_pos
        .table
        .get_by_id_mut(&room.0)
        .expect("expected room to be in entities_by_pos table")
        .insert(pos.pos, EntityComponent(id))
        .expect("entities_by_pos insert");
    debug!(logger, "init_spawn done");
}

type InitResourceMuts = (
    UnsafeView<EntityId, PositionComponent>,
    UnsafeView<EntityId, ResourceComponent>,
    UnsafeView<EntityId, EnergyComponent>,
    UnsafeView<WorldPosition, EntityComponent>,
);

type InitResourceConst<'a> = (View<'a, WorldPosition, TerrainComponent>,);

fn init_resource(
    logger: &Logger,
    bounds: &Hexagon,
    id: EntityId,
    room: Room,
    rng: &mut impl Rng,
    (mut positions_table, mut resources_table, mut energy_table, mut entities_by_pos, ): InitResourceMuts,
    (terrain,): InitResourceConst,
) {
    resources_table.insert_or_update(id, ResourceComponent(Resource::Energy));
    energy_table.insert_or_update(
        id,
        EnergyComponent {
            energy: 250,
            energy_max: 250,
        },
    );

    let pos = uncontested_pos(logger, room, bounds, &*entities_by_pos, &*terrain, rng);

    positions_table.insert_or_update(id, PositionComponent(pos));
    entities_by_pos
        .table
        .get_by_id_mut(&room.0)
        .expect("expected room to be in entities_by_pos table")
        .insert(pos.pos, EntityComponent(id))
        .expect("entities_by_pos insert");
}

fn uncontested_pos<T: caolo_sim::tables::TableRow + Send + Sync>(
    logger: &Logger,
    room: Room,
    bounds: &Hexagon,
    positions_table: &caolo_sim::tables::morton_hierarchy::RoomMortonTable<T>,
    terrain_table: &caolo_sim::tables::morton_hierarchy::RoomMortonTable<TerrainComponent>,
    rng: &mut impl Rng,
) -> WorldPosition {
    const TRIES: usize = 10_000;
    let from = bounds.center - Axial::new(bounds.radius, bounds.radius);
    let to = bounds.center + Axial::new(bounds.radius, bounds.radius);
    for _ in 0..TRIES {
        let x = rng.gen_range(from.q, to.q);
        let y = rng.gen_range(from.r, to.r);

        let pos = Axial::new(x, y);

        trace!(logger, "checking pos {:?}", pos);

        if !bounds.contains(pos) {
            trace!(logger, "point {:?} is out of bounds {:?}", pos, bounds);
            continue;
        }

        let pos = WorldPosition { room: room.0, pos };

        if let Some(TerrainComponent(terrain)) = terrain_table.get_by_id(&pos) {
            if terrain.is_walkable() && !positions_table.contains_key(&pos) {
                return pos;
            }
        }
    }
    panic!(
        "Failed to find an uncontested_pos in {:?} {:?} in {} iterations",
        from, to, TRIES
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use slog::*;

    #[test]
    fn can_init_the_game() {
        let decorator = slog_term::TermDecorator::new().build();
        let drain = slog_term::FullFormat::new(decorator).build().fuse();
        let drain = slog_envlogger::new(drain).fuse();
        let drain = slog_async::Async::new(drain)
            .overflow_strategy(slog_async::OverflowStrategy::DropAndReport)
            .chan_size(16000)
            .build()
            .fuse();
        let logger = slog::Logger::root(drain, o!());

        // smoke test: can the game be even initialized?
        init_storage(logger, 5);
    }
}
