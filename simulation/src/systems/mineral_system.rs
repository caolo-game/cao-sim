use super::System;
use crate::model::{components, geometry::Point, EntityId};
use crate::storage::views::{DeferredDeleteEntityView, UnsafeView, View};
use crate::tables::JoinIterator;
use rand::Rng;

pub struct MineralSystem;

impl<'a> System<'a> for MineralSystem {
    type Mut = (
        UnsafeView<EntityId, components::PositionComponent>,
        UnsafeView<EntityId, components::EnergyComponent>,
        DeferredDeleteEntityView,
    );
    type Const = (
        View<'a, Point, components::EntityComponent>,
        View<'a, Point, components::TerrainComponent>,
        View<'a, EntityId, components::ResourceComponent>,
    );

    fn update(
        &mut self,
        (mut entity_positions, mut energy, mut delete_entity_deferred): Self::Mut,
        (position_entities, terrain_table, resources): Self::Const,
    ) {
        debug!("update minerals system called");

        let mut rng = rand::thread_rng();

        let minerals_it = resources.iter().filter(|(_, r)| match r.0 {
            components::Resource::Energy => true,
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
            if energy.energy > 0 {
                return;
            }
            // respawning
            let pos = random_uncontested_pos_in_range(
                position_entities.clone(),
                terrain_table.clone(),
                &mut rng,
                position.0,
                15,
                100,
            );
            debug!(
                "Mineral [{:?}] has been depleted, respawning at {:?}",
                id, pos
            );
            match pos {
                Some(pos) => {
                    energy.energy = energy.energy_max;
                    position.0 = pos;
                }
                None => {
                    error!("Failed to find adequate position for resource {:?}", id);
                    unsafe {
                        delete_entity_deferred.delete_entity(id);
                    }
                }
            }
        });

        debug!("update minerals system done");
    }
}

fn random_uncontested_pos_in_range<'a>(
    position_entities_table: View<'a, Point, components::EntityComponent>,
    terrain_table: View<'a, Point, components::TerrainComponent>,
    rng: &mut rand::rngs::ThreadRng,
    around: Point,
    range: u16,
    max_tries: u16,
) -> Option<Point> {
    let range = range as i32;
    let x = around.x as i32;
    let y = around.y as i32;

    let (bfrom, bto) = position_entities_table.bounds();

    let mut result = None;
    for _ in 0..max_tries {
        let dx = rng.gen_range(-range, range);
        let dy = rng.gen_range(-range, range);

        let x = (x + dx).max(bfrom.x).min(bto.x);
        let y = (y + dy).max(bfrom.y).min(bto.y);

        let pos = Point::new(x, y);

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
    result
}
