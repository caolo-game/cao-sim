use crate::components;
use crate::geometry::Axial;
use crate::indices::{EntityId, WorldPosition};
use crate::profile;
use crate::storage::views::{DeferredDeleteEntityView, UnsafeView, View, WorldLogger};
use crate::tables::JoinIterator;
use rand::Rng;
use slog::{debug, error, trace, Logger};

type Mut = (
    UnsafeView<EntityId, components::PositionComponent>,
    UnsafeView<EntityId, components::EnergyComponent>,
    DeferredDeleteEntityView,
);
type Const<'a> = (
    View<'a, WorldPosition, components::EntityComponent>,
    View<'a, WorldPosition, components::TerrainComponent>,
    View<'a, EntityId, components::ResourceComponent>,
    WorldLogger,
);

pub fn update(
    (mut entity_positions, mut energy, mut delete_entity_deferred): Mut,
    (position_entities, terrain_table, resources, WorldLogger(logger)): Const,
) {
    profile!("Mineral System update");
    debug!(logger, "update minerals system called");

    let mut rng = rand::thread_rng();

    let minerals_it = resources.iter().filter(|(_, r)| match r.0 {
        components::Resource::Energy => true,
        _ => false,
    });
    let entity_positions_it = unsafe { entity_positions.as_mut().iter_mut() };
    let energy_iter = unsafe { energy.as_mut().iter_mut() };

    // in case of an error we need to clean up the mineral
    // however best not to clean it inside the iterator, hmmm???
    JoinIterator::new(
        JoinIterator::new(minerals_it, entity_positions_it),
        energy_iter,
    )
    .for_each(|(id, ((_resource, position), energy))| {
        trace!(
            logger,
            "updating {:?} {:?} {:?} {:?}",
            id,
            _resource,
            position,
            energy
        );

        if energy.energy > 0 {
            return;
        }
        trace!(logger, "Respawning {:?}", id);

        let position_entities = position_entities
            .table
            .get_by_id(&position.0.room)
            .expect("get room entities table");
        let terrain_table = terrain_table
            .table
            .get_by_id(&position.0.room)
            .expect("get room terrain table");

        let position_entities = View::from_table(position_entities);
        let terrain_table = View::from_table(terrain_table);

        // respawning
        let pos = random_uncontested_pos_in_range(
            &logger,
            position_entities,
            terrain_table,
            &mut rng,
            position.0.pos,
            15,
            100,
        );
        trace!(
            logger,
            "Mineral [{:?}] has been depleted, respawning at {:?}",
            id,
            pos
        );
        match pos {
            Some(pos) => {
                energy.energy = energy.energy_max;
                position.0.pos = pos;
            }
            None => {
                error!(
                    logger,
                    "Failed to find adequate position for resource {:?}", id
                );
                unsafe {
                    delete_entity_deferred.delete_entity(id);
                }
            }
        }
    });

    debug!(logger, "update minerals system done");
}

fn random_uncontested_pos_in_range<'a>(
    logger: &Logger,
    position_entities_table: View<'a, Axial, components::EntityComponent>,
    terrain_table: View<'a, Axial, components::TerrainComponent>,
    rng: &mut rand::rngs::ThreadRng,
    around: Axial,
    range: u16,
    max_tries: u16,
) -> Option<Axial> {
    trace!(
        logger,
        "random_uncontested_pos_in_range {:?} range: {}, max_tries: {}",
        around,
        range,
        max_tries
    );

    let range = range as i32;
    let x = around.q as i32;
    let y = around.r as i32;

    let (bfrom, bto) = position_entities_table.bounds();

    let mut result = None;
    for _ in 0..max_tries {
        let dx = rng.gen_range(-range, range);
        let dy = rng.gen_range(-range, range);

        let x = (x + dx).max(bfrom.q).min(bto.q);
        let y = (y + dy).max(bfrom.r).min(bto.r);

        let pos = Axial::new(x, y);

        if position_entities_table.intersects(&pos)
            && position_entities_table.count_in_range(&pos, 1) == 0
            && terrain_table
                .get_by_id(&pos)
                .map(|components::TerrainComponent(t)| t.is_walkable())
                .unwrap_or(false)
        {
            result = Some(pos);
            break;
        }
    }
    trace!(
        logger,
        "random_uncontested_pos_in_range returns {:?}",
        result
    );
    result
}
