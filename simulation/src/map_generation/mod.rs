mod diamond_square;

use diamond_square::create_noise;

use crate::model::components::TerrainComponent;
use crate::model::geometry::Axial;
use crate::model::terrain::TileTerrainType;
use crate::storage::views::{UnsafeView, View};
use crate::tables::morton::ExtendFailure;
use crate::tables::msb_de_bruijn;
use crate::tables::{MortonTable, SpatialKey2d};
use rand::{rngs::SmallRng, thread_rng, RngCore, SeedableRng};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum MapGenerationError {
    #[error("Can not generate room with the given parameters: {center:?} {radius}")]
    BadArguments { center: Axial, radius: u32 },
    #[error("Failed to generate the initial layout: {0}")]
    TerrainExtendFailure(ExtendFailure<Axial>),
    #[error("A room may only have up to 6 neihgbours, got: {0}")]
    TooManyNeighbours(usize),
    #[error("Got an invlid neighbour {0:?}")]
    InvalidNeighbour(Axial),
}

type MapTables = (UnsafeView<Axial, TerrainComponent>,);

type GradientMap = MortonTable<Axial, f32>;

/// find the smallest power of two that can hold `size`
fn pot(size: u32) -> u32 {
    if size & (size - 1) == 0 {
        size
    } else {
        let msb = msb_de_bruijn(size);
        1 << (msb + 1)
    }
}

#[derive(Debug, Clone)]
pub struct HeightMapProperties {
    /// standard deviation of the height map
    pub std: f32,
    /// mean height of the map
    pub mean: f32,
    /// standard deviation of the normalized height map
    pub normal_std: f32,
    /// mean of normalized heights
    pub normal_mean: f32,
    pub min: f32,
    pub max: f32,
    /// max - min
    pub depth: f32,
    pub width: i32,
    pub height: i32,

    pub plain_mass: u32,
    pub wall_mass: u32,
}

/// Generate a random terrain in hexagon
/// `connecting_neighbours` is a list of neighbours to connect to, meaning these edges are
/// reachable via land.
///
/// Returns property description of the generated height map.
pub fn generate_room(
    center: Axial,
    radius: u32,
    connecting_neighbours: &[Axial],
    (mut terrain,): MapTables,
    seed: Option<[u8; 16]>,
) -> Result<HeightMapProperties, MapGenerationError> {
    debug!(
        "Generating Room center: {:?} radius: {} seed: {:?} connecting_neighbours: {:?}",
        center, radius, seed, connecting_neighbours
    );
    if radius == 0 {
        return Err(MapGenerationError::BadArguments { center, radius });
    }
    if connecting_neighbours.len() > 6 {
        return Err(MapGenerationError::TooManyNeighbours(
            connecting_neighbours.len(),
        ));
    }

    let [x, y] = center.as_array();
    let radius = radius as i32;
    let offset = Axial::new(x - radius, y - radius);

    let from = Axial::new(0, 0);
    let dsides = pot(radius as u32 * 2) as i32;
    let to = Axial::new(from.q + dsides, from.r + dsides);

    let seed = seed.unwrap_or_else(|| {
        let mut bytes = [0; 16];
        thread_rng().fill_bytes(&mut bytes);
        bytes
    });
    let mut rng = SmallRng::from_seed(seed);

    debug!("Initializing GradientMap");
    let mut gradient = GradientMap::from_iterator(
        (from.q..=to.q).flat_map(|x| (from.r..=to.r).map(move |y| (Axial::new(x, y), 0.0))),
    )
    .map_err(|e| {
        error!("Initializing GradientMap failed {:?}", e);
        MapGenerationError::TerrainExtendFailure(e)
    })?;

    let mut gradient2 = GradientMap::with_capacity(((to.q - from.q) * (to.r - from.r)) as usize);
    debug!("Initializing GradientMap done");

    let mut min_grad = 1e15f32;
    let mut max_grad = -1e15f32;

    debug!("Layering maps");
    // generate gradient by repeatedly generating noise and layering them on top of each other
    for _ in 0..4 {
        gradient2.clear();
        gradient2
            .extend(
                (from.q..=to.q).flat_map(|x| (from.r..=to.r).map(move |y| (Axial::new(x, y), 0.0))),
            )
            .map_err(|e| {
                error!("Initializing GradientMap failed {:?}", e);
                MapGenerationError::TerrainExtendFailure(e)
            })?;
        create_noise(from, to, dsides, &mut rng, &mut gradient2);

        let min_grad = &mut min_grad;
        let max_grad = &mut max_grad;

        gradient
            .merge(&gradient2, |_, lhs, rhs| {
                let merged = lhs + rhs;
                *min_grad = min_grad.min(merged);
                *max_grad = max_grad.max(merged);
                merged
            })
            .map_err(|e| {
                error!("Failed to merge GradientMaps {:?}", e);
                MapGenerationError::TerrainExtendFailure(e)
            })?;
    }
    debug!("Layering maps done");

    let mut mean = 0.0;
    let mut std = 0.0;
    let mut normal_mean = 0.0;
    let mut normal_std = 0.0;
    let mut i = 1.0;
    let mut plain_mass = 0;
    let mut wall_mass = 0;
    let depth = max_grad - min_grad;

    let points = {
        // the process so far produced a sheared rectangle
        // we'll choose points that cut the result into a hexagonal shape
        let center = Axial::new(dsides / 2, dsides / 2);
        debug!(
            "Calculating points of a hexagon in the height map around center: {:?}",
            center
        );
        let radius = radius - 1; // skip the edge of the map
        (-radius..=radius).flat_map(move |x| {
            let fromy = (-radius).max(-x - radius);
            let toy = radius.min(-x + radius);
            (fromy..=toy).map(move |y| {
                let p = Axial::new(x, -x - y);
                p + center
            })
        })
    };

    debug!("Building terrain from height-map, offset: {:?}", offset);

    unsafe { terrain.as_mut() }
        .extend(points.filter_map(|p| {
            trace!("Computing terrain of gradient point: {:?}", p);
            let mut grad = *gradient.get_by_id(&p).or_else(|| {
                error!("{:?} has no gradient", p);
                debug_assert!(false);
                None
            })?;
            trace!("p: {:?} grad: {}", p, grad);

            let p = p + offset;

            {
                // let's do some stats
                let tmp = grad - mean;
                mean += tmp / i;
                std += tmp * (grad - mean);
            }

            // normalize grad
            grad -= min_grad;
            grad /= depth;

            {
                // let's do some stats on the normal
                let tmp = grad - normal_mean;
                normal_mean += tmp / i;
                normal_std += tmp * (grad - normal_mean);
                i += 1.0;
            }

            trace!("Normalized grad: {}", grad);

            if grad <= 0.3 || !grad.is_finite() {
                return None;
            }
            let terrain = if grad < 0.7 {
                plain_mass += 1;
                TileTerrainType::Plain
            } else if grad <= 1.1 {
                wall_mass += 1;
                // accounting for numerical errors
                TileTerrainType::Wall
            } else {
                warn!(
                    "Logic error in map generation: unreachable code executed p: {:?} grad: {:?}",
                    p, grad
                );
                return None;
            };
            Some((p, TerrainComponent(terrain)))
        }))
        .map_err(|e| {
            error!("Terrain building failed {:?}", e);
            MapGenerationError::TerrainExtendFailure(e)
        })?;

    debug!("Building terrain from height-map done");
    let chunks = calculate_plain_meshes(View::from_table(&*terrain));
    debug!("Filling edges");
    for edge in connecting_neighbours.iter().cloned() {
        fill_edge(terrain, offset, radius, edge)?;
    }
    debug!("Filling edges done");

    debug!("Deduping");
    unsafe {
        terrain.as_mut().dedupe();
    }
    debug!("Deduping done");

    std = (std / i).sqrt();
    normal_std = (normal_std / i).sqrt();

    let props = HeightMapProperties {
        normal_mean,
        normal_std,
        std,
        mean,
        min: min_grad,
        max: max_grad,
        depth,
        width: dsides,
        height: dsides,
        wall_mass,
        plain_mass,
    };

    debug!("Map generation done {:#?}", props);
    Ok(props)
}

fn fill_edge(
    mut terrain: UnsafeView<Axial, TerrainComponent>,
    offset: Axial,
    radius: i32,
    edge: Axial,
) -> Result<(), MapGenerationError> {
    if edge.q.abs() > 1 || edge.r.abs() > 1 || edge.r == edge.q {
        return Err(MapGenerationError::InvalidNeighbour(edge));
    }
    let [x, y, z] = edge.hex_axial_to_cube();
    let end = [-z, -x, -y];
    let end = Axial::hex_cube_to_axial(end);
    let vel = end - edge;

    let mut vertex = (edge * radius) + offset + Axial::new(radius, radius);

    debug!(
        "Filling edge {:?}, vertex: {:?} end {:?} vel {:?} radius {} offset {:?}",
        edge, vertex, end, vel, radius, offset
    );
    unsafe { terrain.as_mut() }
        .extend((1..radius).map(move |_| {
            vertex += vel;
            (vertex, TerrainComponent(TileTerrainType::Plain))
        }))
        .map_err(|e| {
            error!("Failed to expand terrain with edge {:?} {:?}", edge, e);
            MapGenerationError::TerrainExtendFailure(e)
        })?;

    Ok(())
}

/// Find the connecting `Plain` chunks
fn calculate_plain_meshes(terrain: View<Axial, TerrainComponent>) -> Vec<HashSet<Axial>> {
    debug!("calculate_plain_meshes");
    let mut res = Vec::new();
    let mut chunk = HashSet::with_capacity(terrain.len());
    let mut visited = HashSet::with_capacity(terrain.len());
    let mut todo = HashSet::with_capacity(terrain.len());
    let mut startind = 0;
    'a: loop {
        let current = terrain
            .iter()
            .enumerate()
            .skip(startind)
            .find_map(|(i, (p, t))| {
                let TerrainComponent(t) = t;
                if t.is_walkable() && !visited.contains(&p) {
                    Some((i, p))
                } else {
                    None
                }
            });
        if current.is_none() {
            break 'a;
        }
        let (i, current) = current.unwrap();
        startind = i;
        todo.insert(current);
        while !todo.is_empty() {
            let current = todo.iter().next().cloned().unwrap();
            todo.remove(&current);
            visited.insert(current);
            chunk.insert(current);
            terrain.query_range(&current, 2, &mut |p, t| {
                let TerrainComponent(t) = t;
                if t.is_walkable() && !visited.contains(&p) {
                    todo.insert(p);
                }
            });
        }
        res.push(HashSet::with_capacity(chunk.len()));
        std::mem::swap(res.last_mut().unwrap(), &mut chunk);
    }
    debug!("calculate_plain_meshes done, found {} meshes", res.len());
    res
}

/// Print a 2D TerrainComponent map to the console, intended for debugging small maps.
#[allow(unused)]
fn print_terrain(from: &Axial, to: &Axial, terrain: View<Axial, TerrainComponent>) {
    assert!(from.q < to.q);
    assert!(from.r < to.r);

    for y in (from.r..=to.r) {
        for x in (from.q..=to.q) {
            match terrain.get_by_id(&Axial::new(x, y)) {
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
    use crate::pathfinding::find_path_in_room;
    use crate::storage::views::View;

    #[test]
    fn basic_generation() {
        let mut terrain = MortonTable::with_capacity(512);

        let center = Axial::new(5, 5);
        let props = generate_room(
            center,
            5,
            &[],
            (UnsafeView::from_table(&mut terrain),),
            Some(*b"deadbeefstewbisc"),
        )
        .unwrap();

        dbg!(props);

        let from = Axial::new(0, 0);
        let to = Axial::new(16, 16);
        print_terrain(&from, &to, View::from_table(&terrain));

        let mut seen_empty = false;
        let mut seen_wall = false;
        let mut seen_plain = false;

        // assert that the terrain is not homogeneous
        for x in 0..=16 {
            for y in 0..=16 {
                match terrain.get_by_id(&Axial::new(x, y)) {
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

        let center = Axial::new(8, 8);

        let props = generate_room(
            center,
            8,
            &[],
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

        let from = Axial::new(0, 0);
        let to = Axial::new(16, 16);

        print_terrain(&from, &to, View::from_table(&terrain));

        let positions = MortonTable::<Axial, EntityComponent>::new();
        let mut path = Vec::with_capacity(1024);

        let first = plains.iter().next().expect("at least 1 plain");
        for b in plains.iter().skip(1) {
            path.clear();
            if let Err(e) = find_path_in_room(
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

    #[test]
    fn produces_the_expected_number_of_chunks() {
        let terrain: MortonTable<Axial, TerrainComponent> =
            serde_json::from_str(std::include_str!("./chunk_test_map.json")).unwrap();

        let chunks = calculate_plain_meshes(View::from_table(&terrain));

        assert_eq!(chunks.len(), 5);
    }
}
