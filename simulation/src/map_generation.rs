use crate::model::components::TerrainComponent;
use crate::model::geometry::Point;
use crate::model::terrain::TileTerrainType;
use crate::storage::views::{UnsafeView, View};
use crate::tables::msb_de_bruijn;
use crate::tables::{ExtendFailure, MortonTable, SpatialKey2d, Table};
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

/// find the smallest power of two that can hold `size`
fn pot(size: u32) -> u32 {
    let msb = msb_de_bruijn(size);
    1 << msb
}

fn fheight(
    _gradient: &GradientMap,
    _p: Point,
    radius: i32,
    mean_heights: f32,
    rng: &mut impl Rng,
) -> f32 {
    mean_heights + (rng.gen_range(0.0, 1.0) - 0.5) * radius as f32
}

/// returns the new gradient
fn square(gradient: &mut GradientMap, p: Point, radius: i32, rng: &mut impl Rng) -> f32 {
    let mut sum = 0.0;
    let mut num = 0;

    let [x, y] = p.as_array();
    for point in [
        Point::new(x - radius, y - radius),
        Point::new(x - radius, y + radius),
        Point::new(x + radius, y - radius),
        Point::new(x + radius, y + radius),
    ]
    .iter()
    {
        if let Some(grad) = gradient.get_by_id(point) {
            sum += grad;
            num += 1;
        }
    }

    let grad = fheight(&gradient, p, radius, sum / num as f32, rng);
    gradient.update(p, grad);
    grad
}

/// returns the new gradient at point p
fn diamond(gradient: &mut GradientMap, p: Point, radius: i32, rng: &mut impl Rng) -> f32 {
    let mut sum = 0.0;
    let mut num = 0;

    let [x, y] = p.as_array();

    for point in [
        Point::new(x - radius, y),
        Point::new(x + radius, y),
        Point::new(x, y - radius),
        Point::new(x, y + radius),
    ]
    .iter()
    {
        if let Some(grad) = gradient.get_by_id(point) {
            sum += grad;
            num += 1;
        }
    }

    let grad = fheight(&gradient, p, radius, sum / num as f32, rng);
    gradient.update(p, grad);
    grad
}

/// Generate a random terrain in the AABB (from,to)
/// TODO: clamp the map to from,to (currently will expand the map)
/// Usese the [Diamond-square algorithm](https://en.wikipedia.org/wiki/Diamond-square_algorithm)
pub fn generate_terrain(
    from: Point,
    to: Point,
    (mut terrain,): MapTables,
    seed: Option<[u8; 16]>,
) -> Result<(), MapGenerationError> {
    if from.x >= to.x || from.y >= to.y {
        return Err(MapGenerationError::BadBox { from, to });
    }

    let dx = to.x - from.x;
    let dy = to.y - from.y;

    let dsides = pot(dx.max(dy) as u32) as i32;
    let to = Point::new(from.x + dsides, from.y + dsides);

    let seed = seed.unwrap_or_else(|| {
        let mut bytes = [0; 16];
        thread_rng().fill_bytes(&mut bytes);
        bytes
    });
    let mut rng = SmallRng::from_seed(seed);
    let mut gradient = GradientMap::from_iterator(
        (from.x..=to.x).flat_map(|x| (from.y..=to.y).map(move |y| (Point::new(x, y), 0.0))),
    )
    .map_err(|e| MapGenerationError::TerrainExtendFailure(e))?;

    // init corners
    let corners = [from, Point::new(to.x, from.y), Point::new(from.x, to.y), to];
    for edge in corners.iter() {
        gradient.delete(&edge);
        gradient.insert(*edge, fheight(&gradient, from, 3, 0.0, &mut rng));
    }

    let mut d = dsides / 2;
    let mut max_grad = 0.0f32;
    let mut min_grad = 0.0f32;

    while 1 <= d {
        for x in (d..dsides).step_by(2 * d as usize) {
            for y in (d..dsides).step_by(2 * d as usize) {
                let g = square(&mut gradient, Point::new(x, y), d, &mut rng);
                max_grad = max_grad.max(g);
                min_grad = min_grad.min(g);
            }
        }
        for x in (d..dsides).step_by(2 * d as usize) {
            for y in (from.y..=dsides).step_by(2 * d as usize) {
                let g = diamond(&mut gradient, Point::new(x, y), d, &mut rng);
                max_grad = max_grad.max(g);
                min_grad = min_grad.min(g);
            }
        }
        for x in (from.x..=dsides).step_by(2 * d as usize) {
            for y in (d..dsides).step_by(2 * d as usize) {
                let g = diamond(&mut gradient, Point::new(x, y), d, &mut rng);
                max_grad = max_grad.max(g);
                min_grad = min_grad.min(g);
            }
        }
        d /= 2;
    }

    let terrain = unsafe { terrain.as_mut() };
    terrain.clear();
    let points = (from.x..=to.x)
        .flat_map(move |x| (from.y..=to.y).map(move |y| Point::new(x, y)))
        .collect::<Vec<_>>();
    terrain
        .extend(points.into_iter().filter_map(move |p| {
            let mut grad = *gradient.get_by_id(&p)?;

            // normalize grad
            grad -= min_grad;
            grad /= max_grad - min_grad;

            if grad <= 0.43 {
                return None;
            }
            let terrain = if grad < 0.8 {
                TileTerrainType::Plain
            } else if grad <= 1.5 {
                // accounting for numerical errors
                TileTerrainType::Wall
            } else {
                // Should we add more terrain
                unreachable!("grad {}", grad);
            };
            Some((p, TerrainComponent(terrain)))
        }))
        .map_err(|e| MapGenerationError::TerrainExtendFailure(e))?;

    Ok(())
}

/// Print a 2D TerrainComponent map to the console, intended for debugging small maps.
#[allow(unused)]
fn print_terrain(from: &Point, to: &Point, terrain: View<Point, TerrainComponent>) {
    assert!(from.x < to.x);
    assert!(from.y < to.y);

    for y in (from.y..=to.y) {
        for x in (from.x..=to.x) {
            match terrain.get_by_id(&Point::new(x, y)) {
                Some(TerrainComponent(TileTerrainType::Wall)) => print!("#"),
                Some(TerrainComponent(TileTerrainType::Plain)) => print!("."),
                None => print!(" "),
            }
        }
        print!("\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::components::EntityComponent;
    use crate::pathfinding::find_path;
    use crate::storage::views::View;

    #[test]
    fn basic_generation() {
        let mut terrain = MortonTable::with_capacity(512);

        let [from, to] = [Point::new(0, 0), Point::new(10, 10)];
        generate_terrain(
            from,
            to,
            (UnsafeView::from_table(&mut terrain),),
            Some(*b"deadbeefstewbisc"),
        )
        .unwrap();

        let mut seen_empty = false;
        let mut seen_wall = false;
        let mut seen_plain = false;

        print_terrain(&from, &to, View::from_table(&terrain));

        // assert that the terrain is not homogeneous
        for x in 0..=10 {
            for y in 0..=10 {
                match terrain.get_by_id(&Point::new(x, y)) {
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

    #[test]
    fn all_plain_are_reachable() {
        // doesn't work all the time...
        let mut plains = Vec::with_capacity(512);
        let mut terrain = MortonTable::with_capacity(512);

        let from = Point::new(0, 0);
        let to = Point::new(8, 8);

        generate_terrain(
            from,
            to,
            (UnsafeView::from_table(&mut terrain),),
            None, // Some(*b"deadbeefstewbisc"),
        )
        .unwrap();

        for (p, t) in terrain.iter() {
            let TerrainComponent(tile) = t;
            if tile.is_walkable() {
                plains.push(p);
            }
        }

        print_terrain(&from, &to, View::from_table(&terrain));

        let positions = MortonTable::<Point, EntityComponent>::new();
        let mut path = Vec::with_capacity(1024);
        for (i, a) in plains.iter().enumerate() {
            for b in plains.iter().skip(i) {
                path.clear();
                find_path(
                    *a,
                    *b,
                    (View::from_table(&positions), View::from_table(&terrain)),
                    1024,
                    &mut path,
                )
                .expect("pathfinding");
            }
        }
    }
}
