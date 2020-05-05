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
    #[error("Can not generate room with the given parameters: {center:?} {radius}")]
    BadArguments { center: Point, radius: u32 },
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

/// returns the new gradient
fn square(
    gradient: &mut GradientMap,
    p: Point,
    radius: i32,
    fheight: &mut impl FnMut(&GradientMap, Point, i32, f32) -> f32,
) -> f32 {
    let mut sum = 0.0;
    let mut num = 0;

    let [x, y] = p.as_array();
    for grad in [
        Point::new(x - radius, y - radius),
        Point::new(x - radius, y + radius),
        Point::new(x + radius, y - radius),
        Point::new(x + radius, y + radius),
    ]
    .iter()
    .filter_map(|point| gradient.get_by_id(point))
    {
        sum += grad;
        num += 1;
    }

    let grad = fheight(&gradient, p, radius, sum / num as f32);
    gradient.update(p, grad);
    grad
}

/// returns the new gradient at point p
fn diamond(
    gradient: &mut GradientMap,
    p: Point,
    radius: i32,
    fheight: &mut impl FnMut(&GradientMap, Point, i32, f32) -> f32,
) -> f32 {
    let mut sum = 0.0;
    let mut num = 0;

    let [x, y] = p.as_array();

    for grad in [
        Point::new(x - radius, y),
        Point::new(x + radius, y),
        Point::new(x, y - radius),
        Point::new(x, y + radius),
    ]
    .iter()
    .filter_map(|point| gradient.get_by_id(point))
    {
        sum += grad;
        num += 1;
    }

    let grad = fheight(&gradient, p, radius, sum / num as f32);
    gradient.update(p, grad);
    grad
}

#[derive(Debug, Clone)]
pub struct HeightMapProperties {
    /// standard deviation of the height map
    pub std: f32,
    /// mean height of the map
    pub mean: f32,
    pub min: f32,
    pub max: f32,
    /// max - min
    pub depth: f32,
    pub width: i32,
    pub height: i32,

    pub plain_mass: u32,
    pub wall_mass: u32,
}

/// Generate a random terrain in circle
/// TODO: clamp the map to from,to (currently will expand the map)
/// Usese the [Diamond-square algorithm](https://en.wikipedia.org/wiki/Diamond-square_algorithm)
///
/// Return property description of the generated height map
pub fn generate_room(
    center: Point,
    radius: u32,
    (mut terrain,): MapTables,
    seed: Option<[u8; 16]>,
) -> Result<HeightMapProperties, MapGenerationError> {
    if radius == 0 {
        return Err(MapGenerationError::BadArguments { center, radius });
    }

    let [x, y] = center.as_array();
    let radius = radius as i32;
    let from = Point::new(x - radius, y - radius);
    let to = Point::new(x + radius, y + radius);

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

    let mut fheight = move |gradient: &GradientMap, p: Point, radius: i32, mean_heights: f32| {
        let mut mean = 0.0;
        let mut cnt = 1.0;
        let mut std = 0.0;
        gradient.query_range(&p, radius as u32, &mut |_, g| {
            let tmp = g - mean;
            mean += tmp / cnt;
            std += tmp * (g - mean);
            cnt += 1.0;
        });
        mean_heights
            + std * rng.gen_range(1.0, 2.0) * mean
            + (rng.gen_range(0.0, 1.0) - 0.5) * radius as f32
    };
    let fheight = &mut fheight;

    // init corners
    let corners = [from, Point::new(to.x, from.y), Point::new(from.x, to.y), to];
    for edge in corners.iter() {
        gradient.delete(&edge);
        gradient.insert(*edge, fheight(&gradient, from, 8, 0.0));
    }

    let mut d = dsides / 2;
    let mut max_grad = 0.0f32;
    let mut min_grad = 0.0f32;

    while 1 <= d {
        for x in (d..dsides).step_by(2 * d as usize) {
            for y in (d..dsides).step_by(2 * d as usize) {
                let g = square(&mut gradient, Point::new(x, y), d, fheight);
                max_grad = max_grad.max(g);
                min_grad = min_grad.min(g);
            }
        }
        for x in (d..dsides).step_by(2 * d as usize) {
            for y in (from.y..=dsides).step_by(2 * d as usize) {
                let g = diamond(&mut gradient, Point::new(x, y), d, fheight);
                max_grad = max_grad.max(g);
                min_grad = min_grad.min(g);
            }
        }
        for x in (from.x..=dsides).step_by(2 * d as usize) {
            for y in (d..dsides).step_by(2 * d as usize) {
                let g = diamond(&mut gradient, Point::new(x, y), d, fheight);
                max_grad = max_grad.max(g);
                min_grad = min_grad.min(g);
            }
        }
        d /= 2;
    }

    let terrain = unsafe { terrain.as_mut() };
    terrain.clear();
    let mut mean = 0.0;
    let mut std = 0.0;
    let mut i = 1.0;
    let mut plain_mass = 0;
    let mut wall_mass = 0;
    let points = (from.x..=to.x).flat_map(move |x| (from.y..=to.y).map(move |y| Point::new(x, y)));
    terrain
        .extend(points.filter_map(|p| {
            if center.hex_distance(p) > radius as u64 {
                return None;
            }
            let mut grad = *gradient.get_by_id(&p)?;

            {
                // let's do some stats
                let tmp = grad - mean;
                mean += tmp / i;
                std += tmp * (grad - mean);
                i += 1.0;
            }
            // normalize grad
            grad -= min_grad;
            grad /= max_grad - min_grad;

            if grad <= 0.33 {
                return None;
            }
            let terrain = if grad < 0.6 {
                plain_mass += 1;
                TileTerrainType::Plain
            } else if grad <= 1.1 {
                wall_mass += 1;
                // accounting for numerical errors
                TileTerrainType::Wall
            } else {
                warn!("Logic error in map generation: unreachable code executed");
                return None;
            };
            Some((p, TerrainComponent(terrain)))
        }))
        .map_err(|e| MapGenerationError::TerrainExtendFailure(e))?;

    std = (std / i).sqrt();

    let props = HeightMapProperties {
        std,
        mean,
        min: min_grad,
        max: max_grad,
        depth: max_grad - min_grad,
        width: dsides,
        height: dsides,
        wall_mass,
        plain_mass,
    };

    Ok(props)
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

        let center = Point::new(5, 5);
        let props = generate_room(
            center,
            5,
            (UnsafeView::from_table(&mut terrain),),
            Some(*b"deadbeefstewbisc"),
        )
        .unwrap();

        dbg!(props);

        let from = Point::new(0, 0);
        let to = Point::new(16, 16);
        print_terrain(&from, &to, View::from_table(&terrain));

        let mut seen_empty = false;
        let mut seen_wall = false;
        let mut seen_plain = false;

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

        let center = Point::new(8, 8);

        let props = generate_room(
            center,
            8,
            (UnsafeView::from_table(&mut terrain),),
            None, // Some(*b"deadbeefstewbisc"),
        )
        .unwrap();

        dbg!(props);

        for (p, t) in terrain.iter() {
            let TerrainComponent(tile) = t;
            if tile.is_walkable() {
                plains.push(p);
            }
        }

        let from = Point::new(0, 0);
        let to = Point::new(16, 16);

        print_terrain(&from, &to, View::from_table(&terrain));

        let positions = MortonTable::<Point, EntityComponent>::new();
        let mut path = Vec::with_capacity(1024);

        let first = plains.iter().next().expect("at least 1 plain");
        for b in plains.iter().skip(1) {
            path.clear();
            if let Err(e) = find_path(
                *first,
                *b,
                (View::from_table(&positions), View::from_table(&terrain)),
                1024,
                &mut path,
            ) {
                panic!("Failed to find path from {:?} to {:?}: {:?}", first, b, e);
            }
        }
    }
}
