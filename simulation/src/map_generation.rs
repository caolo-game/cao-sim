use crate::model::components::TerrainComponent;
use crate::model::geometry::Point;
use crate::model::terrain::TileTerrainType;
use crate::storage::views::UnsafeView;
use crate::tables::{ExtendFailure, MortonTable};
use rand::{rngs::SmallRng, thread_rng, Rng, RngCore, SeedableRng};
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum MapGenerationError {
    #[error("Got invalid boxes `{from:?}` `{to:?}`")]
    BadBox { from: Point, to: Point },
    #[error("Failed to generate the initial layout: {0}")]
    TerrainExtendFailure(ExtendFailure<Point>),
}

type MapTables = (UnsafeView<Point, TerrainComponent>,);

type GradientMap = MortonTable<Point, f32>;

/// Generate a random terrain in the AABB (from,to)
pub fn generate_terrain(
    from: Point,
    to: Point,
    (mut terrain,): MapTables,
    seed: Option<[u8; 16]>,
) -> Result<(), MapGenerationError> {
    if from.x >= to.x || from.y >= to.y {
        return Err(MapGenerationError::BadBox { from, to });
    }
    assert!(from.x < to.x);
    assert!(from.y < to.y);

    let seed = seed.unwrap_or_else(|| {
        let mut bytes = [0; 16];
        thread_rng().fill_bytes(&mut bytes);
        bytes
    });
    let mut rng = SmallRng::from_seed(seed);

    let mut gradient = GradientMap::from_iterator((from.x..=to.x).flat_map(|x| {
        let rng = &mut rng as *mut SmallRng;
        (from.y..=to.y).map(move |y| {
            let rng = unsafe { &mut *rng };
            let a: f32 = rng.gen_range(-1.0f32, 1.0f32);
            (Point::new(x, y), a)
        })
    }))
    .map_err(|e| MapGenerationError::TerrainExtendFailure(e))?;

    for x in 1..to.x {
        for y in 1..to.y {
            let point = Point::new(x, y);
            let mut points = Vec::with_capacity(6); // FIXME: can not reuse because of `update`
            gradient.find_by_range(&point, 1, &mut points);
            let mut x = *gradient
                .get_by_id(&point)
                .expect("Grid point to be in the map");
            for y in points.iter().filter(|(p, _)| p != &point).map(|(_, v)| *v) {
                let y = *y;
                x = lerp(x, y, 1.0);
            }
            let _b = gradient.update(point, x);
            debug_assert!(_b, "Failed to update the gradient at point {:?}", point);
        }
    }

    let terrain = unsafe { terrain.as_mut() };
    terrain.clear();
    let points = (from.x..=to.x)
        .flat_map(move |x| (from.y..=to.y).map(move |y| Point::new(x, y)))
        .collect::<Vec<_>>();
    terrain
        .extend(points.into_iter().filter_map(move |p| {
            let grad = *gradient.get_by_id(&p)?;
            debug_assert!(-1.0 <= grad && grad <= 1.0);

            let grad = grad + 1.0;

            if grad < 1.0 / 3.0 {
                return None;
            }
            let terrain = if grad < 1.0 {
                TileTerrainType::Wall
            } else if grad < 2.0 {
                TileTerrainType::Plain
            } else {
                // Should we add more terrain
                unreachable!();
            };
            Some((p, TerrainComponent(terrain)))
        }))
        .map_err(|e| MapGenerationError::TerrainExtendFailure(e))?;

    Ok(())
}

fn lerp(a0: f32, a1: f32, w: f32) -> f32 {
    (1.0 - w) * a0 + w * a1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_generation() {
        let mut table = MortonTable::with_capacity(512);

        generate_terrain(
            Point::new(0, 0),
            Point::new(10, 10),
            (UnsafeView::from_table(&mut table),),
            Some(*b"deadbeefstewbisc"),
        )
        .unwrap();

        let mut seen_empty = false;
        let mut seen_wall = false;
        let mut seen_plain = false;

        // assert that the terrain is not homogeneous
        for x in 0..=10 {
            for y in 0..=10 {
                match table.get_by_id(&Point::new(x, y)) {
                    None => seen_empty = true,
                    Some(TerrainComponent(TileTerrainType::Plain)) => seen_plain = true,
                    Some(TerrainComponent(TileTerrainType::Wall)) => seen_wall = true,
                }
            }
        }

        assert!(seen_plain);
        assert!(seen_wall);
        assert!(seen_empty);
    }
    
    // TODO: any two Plain must be reachable
}
