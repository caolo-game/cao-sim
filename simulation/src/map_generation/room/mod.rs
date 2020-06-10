//! Generate rooms
//!
mod diamond_square;
mod params;

pub use params::*;

use diamond_square::create_noise;

use crate::model::components::{RoomConnection, TerrainComponent};
use crate::model::geometry::{Axial, Hexagon};
use crate::model::terrain::TileTerrainType;
use crate::storage::views::{UnsafeView, View};
use crate::tables::morton::ExtendFailure;
use crate::tables::{msb_de_bruijn, MortonTable, Table};
use rand::{rngs::SmallRng, thread_rng, Rng, RngCore, SeedableRng};
use std::cmp::Ordering;
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum RoomGenerationError {
    #[error("Can not generate room with the given parameters: {radius}")]
    BadArguments { radius: u32 },
    #[error("Failed to generate the initial layout: {0}")]
    TerrainExtendFailure(ExtendFailure<Axial>),
    #[error("A room may only have up to 6 neihgbours, got: {0}")]
    TooManyNeighbours(usize),
    #[error("Got an invlid neighbour {0:?}")]
    InvalidNeighbour(Axial),
    #[error("Internal error: Failed to connect chunks, remaining: {0:?}")]
    ExpectedSingleChunk(usize),
    #[error("Bad edge offsets at edge {edge:?} with a radius of {radius}. Start is {offset_start} and End is {offset_end}")]
    BadEdgeOffset {
        edge: Axial,
        offset_start: i32,
        offset_end: i32,
        radius: i32,
    },
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
    pub center: Axial,
    pub radius: i32,
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
}

/// Generate a random terrain in hexagon
/// `edges` is a list of neighbours to connect to, meaning these edges are
/// reachable via land.
///
/// Returns property description of the generated height map.
pub fn generate_room(
    params: &RoomGenerationParams,
    edges: &[RoomConnection],
    (mut terrain,): MapTables,
) -> Result<HeightMapProperties, RoomGenerationError> {
    debug!("Generating Room {:#?}\nedges:\n{:?}", params, edges);
    let RoomGenerationParams { radius, seed, .. } = params;
    if edges.len() > 6 {
        return Err(RoomGenerationError::TooManyNeighbours(edges.len()));
    }

    let radius = *radius as i32;
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
        RoomGenerationError::TerrainExtendFailure(e)
    })?;

    let mut gradient2 = GradientMap::with_capacity(((to.q - from.q) * (to.r - from.r)) as usize);
    debug!("Initializing GradientMap done");

    let center = Axial::new(radius, radius);

    let mut min_grad = 1e15f32;
    let mut max_grad = -1e15f32;

    debug!("Layering maps");
    // generate gradient by repeatedly generating noise and layering them on top of each other
    for i in 0..3 {
        gradient2.clear();
        gradient2
            .extend(
                (from.q..=to.q).flat_map(|x| (from.r..=to.r).map(move |y| (Axial::new(x, y), 0.0))),
            )
            .map_err(|e| {
                error!("Initializing GradientMap failed {:?}", e);
                RoomGenerationError::TerrainExtendFailure(e)
            })?;
        create_noise(from, to, dsides, &mut rng, &mut gradient2);

        let min_grad = &mut min_grad;
        let max_grad = &mut max_grad;

        gradient
            .merge(&gradient2, |_, lhs, rhs| {
                let merged = lhs + rhs * i as f32;
                *min_grad = min_grad.min(merged);
                *max_grad = max_grad.max(merged);
                merged
            })
            .map_err(|e| {
                error!("Failed to merge GradientMaps {:?}", e);
                RoomGenerationError::TerrainExtendFailure(e)
            })?;
    }
    debug!("Layering maps done");

    let heightmap_props = transform_heightmap_into_terrain(
        center,
        max_grad,
        min_grad,
        dsides,
        radius - 1,
        params.chance_plain,
        params.chance_wall,
        &gradient,
        terrain,
    )?;

    {
        // ensure at least 1 plain at this point
        let minq = center.q - (radius - 1);
        let minr = center.r - (radius - 1);
        let maxq = center.q + (radius - 1);
        let maxr = center.r + (radius - 1);

        let q = rng.gen_range(minq, maxq);
        let r = rng.gen_range(minr, maxr);
        unsafe { terrain.as_mut() }
            .insert_or_update(Axial::new(q, r), TerrainComponent(TileTerrainType::Plain))
            .map_err(|e| {
                error!("Failed to update the center point {:?}", e);
                RoomGenerationError::TerrainExtendFailure(e)
            })?;
    }

    coastline(center, radius - 1, terrain);

    let chunk_metadata = calculate_plain_chunks(View::from_table(&*terrain));
    if chunk_metadata.chunks.len() > 1 {
        connect_chunks(
            center,
            radius - 1,
            &mut rng,
            &chunk_metadata.chunks,
            terrain,
        );
    }

    fill_edges(center, radius, edges, terrain, &mut rng)?;

    {
        // to make dilation unbiased we clone the terrain and inject that as separate input
        let terrain_in = (*terrain).clone();
        dilate(
            center,
            radius,
            params.plain_dilation,
            View::from_table(&terrain_in),
            terrain,
        );
    }

    // cleanup potential post-condition violations
    // this step is designed to make experimental changes to generation algorithms easier at the
    // cost of performance.
    // it is the author's opinion that this is a good trade-off
    debug!("Deduping");
    unsafe {
        terrain.as_mut().dedupe();
    }
    debug!("Deduping done");

    debug!("Cutting outliers");
    // cut the edges, because generation might insert invalid Plains on the edge
    let bounds = Hexagon { center, radius };
    let delegates: Vec<Axial> = terrain
        .iter()
        .filter(|(p, _)| !bounds.contains(p))
        .map(|(p, _)| p)
        .collect();
    debug!("Deleting {} items from the room", delegates.len());
    for p in delegates.iter() {
        unsafe {
            terrain.as_mut().delete(p);
        }
    }
    debug!("Cutting outliers done");

    debug!("Map generation done {:#?}", heightmap_props);
    Ok(heightmap_props)
}

fn fill_edges(
    center: Axial,
    radius: i32,
    edges: &[RoomConnection],
    terrain: UnsafeView<Axial, TerrainComponent>,
    rng: &mut impl Rng,
) -> Result<(), RoomGenerationError> {
    debug!("Filling edges");
    let mut chunk_metadata = calculate_plain_chunks(View::from_table(&*terrain));
    if chunk_metadata.chunks.len() != 1 {
        error!(
            "Expected 1 single chunk when applying edges, intead got {}",
            chunk_metadata.chunks.len()
        );
        return Err(RoomGenerationError::ExpectedSingleChunk(
            chunk_metadata.chunks.len(),
        ));
    }
    for mut edge in edges.iter().cloned() {
        // offset - 1 but at least 0
        edge.offset_start = 1.max(edge.offset_start) - 1;
        edge.offset_end = 1.max(edge.offset_end) - 1;
        chunk_metadata
            .chunks
            .push(HashSet::with_capacity(radius as usize));
        fill_edge(
            center,
            radius - 1,
            TileTerrainType::Plain,
            edge,
            terrain,
            chunk_metadata.chunks.last_mut().unwrap(),
        )?;
    }
    debug!("Connecting edges to the mainland");
    connect_chunks(center, radius - 2, rng, &chunk_metadata.chunks, terrain);
    debug!("Filling edges done");
    for edge in edges.iter().cloned() {
        chunk_metadata
            .chunks
            .push(HashSet::with_capacity(radius as usize));
        fill_edge(
            center,
            radius,
            TileTerrainType::Bridge,
            edge,
            terrain,
            chunk_metadata.chunks.last_mut().unwrap(),
        )?;
    }
    Ok(())
}

fn dilate(
    center: Axial,
    radius: i32,
    kernel_width: u32,
    terrain_in: View<Axial, TerrainComponent>,
    mut terrain_out: UnsafeView<Axial, TerrainComponent>,
) {
    debug!(
        "Dilating terrain center: {:?} radius: {} kernel: {}",
        center, radius, kernel_width
    );
    if radius < kernel_width as i32 + 1 || kernel_width == 0 {
        debug!("Skipping dilating");
        return;
    }

    let threshold = (kernel_width * kernel_width / 3).max(1);

    let points = room_points(center, radius);
    for p in points.filter(|p| {
        !terrain_in
            .get_by_id(p)
            .map(|TerrainComponent(t)| t.is_walkable())
            .unwrap_or(false)
    }) {
        let mut neighbours_on = -1; // account for p
        terrain_in.query_range(&p, kernel_width, &mut |_, TerrainComponent(t)| {
            neighbours_on += t.is_walkable() as i32;
        });

        if neighbours_on > threshold as i32 {
            unsafe { terrain_out.as_mut() }
                .insert_or_update(p, TerrainComponent(TileTerrainType::Plain))
                .expect("dilate plain update");
        }
    }
    debug!("Dilate done");
}

fn connect_chunks(
    center: Axial,
    radius: i32,
    rng: &mut impl Rng,
    chunks: &[HashSet<Axial>],
    mut terrain: UnsafeView<Axial, TerrainComponent>,
) {
    debug!("Connecting {} chunks", chunks.len());
    debug_assert!(radius > 0);
    let bounds = Hexagon { center, radius };
    'chunks: for chunk in chunks[1..].iter() {
        let avg: Axial =
            chunk.iter().cloned().fold(Axial::default(), |a, b| a + b) / chunk.len() as i32;
        let closest = *chunks[0]
            .iter()
            .min_by_key(|p| p.hex_distance(avg))
            .unwrap();
        let mut current = *chunk
            .iter()
            .min_by_key(|p| p.hex_distance(closest))
            .unwrap();

        let get_next_step = |current| {
            let vel = closest - current;
            debug_assert!(vel.q != 0 || vel.r != 0);
            match vel.q.abs().cmp(&vel.r.abs()) {
                Ordering::Equal => {
                    if (vel.q + vel.r) % 2 == 0 {
                        Axial::new(vel.q / vel.q.abs(), 0)
                    } else {
                        Axial::new(0, vel.r / vel.r.abs())
                    }
                }
                Ordering::Less => Axial::new(0, vel.r / vel.r.abs()),
                Ordering::Greater => Axial::new(vel.q / vel.q.abs(), 0),
            }
        };

        if current.hex_distance(closest) <= 1 {
            continue 'chunks;
        }
        let terrain = unsafe { terrain.as_mut() };
        'connecting: loop {
            let vel = get_next_step(current);
            current += vel;
            terrain
                .insert_or_update(current, TerrainComponent(TileTerrainType::Plain))
                .unwrap();
            if current.hex_distance(closest) == 0 {
                break 'connecting;
            }
            for _ in 0..2 {
                let vel = if rng.gen_bool(0.5) {
                    vel.rotate_left()
                } else {
                    vel.rotate_right()
                };
                let c = current + vel;
                if !bounds.contains(&c) {
                    continue;
                }
                current = c;
                terrain
                    .insert_or_update(current, TerrainComponent(TileTerrainType::Plain))
                    .unwrap();
                if current.hex_distance(closest) < 1 {
                    break 'connecting;
                }
            }
        }
    }
    debug!("Connecting chunks done");
}

/// Turn every `Wall` into `Plain` if it has empty neighbour(s).
/// This should result in a nice coastline where the `Walls` were neighbours with the ocean.
fn coastline(center: Axial, radius: i32,mut terrain: UnsafeView<Axial, TerrainComponent>) {
    debug!("Building coastline");
    let bounds = Hexagon{center, radius};
    let mut changeset = vec![];
    for (p, _) in terrain
        .iter()
        .filter(|(p, TerrainComponent(t))| bounds.contains(p) && *t == TileTerrainType::Wall)
    {
        let count = terrain.count_in_range(&p, 2);
        // 7 == 1 + 6 neighbours
        if count < 7 {
            // at least 1 neighbour tile is empty
            changeset.push(p);
        }
    }
    trace!("Changing walls to plains {:#?}", changeset);
    for p in changeset {
        unsafe { terrain.as_mut() }.update(p, TerrainComponent(TileTerrainType::Plain));
    }
    debug!("Building coastline done");
}

fn room_points(center: Axial, radius: i32) -> impl Iterator<Item = Axial> {
    // the process so far produced a sheared rectangle
    // we'll choose points that cut the result into a hexagonal shape
    let radius = radius - 1; // skip the edge of the map
    (-radius..=radius).flat_map(move |x| {
        let fromy = (-radius).max(-x - radius);
        let toy = radius.min(-x + radius);
        (fromy..=toy).map(move |y| {
            let p = Axial::new(x, -x - y);
            p + center
        })
    })
}

fn transform_heightmap_into_terrain(
    center: Axial,
    max_grad: f32,
    min_grad: f32,
    dsides: i32,
    radius: i32,
    chance_plain: f32,
    chance_wall: f32,
    gradient: &MortonTable<Axial, f32>,
    mut terrain: UnsafeView<Axial, TerrainComponent>,
) -> Result<HeightMapProperties, RoomGenerationError> {
    debug!("Building terrain from height-map");
    let mut mean = 0.0;
    let mut std = 0.0;
    let mut normal_mean = 0.0;
    let mut normal_std = 0.0;
    let mut i = 1.0;
    let depth = max_grad - min_grad;

    let points = room_points(center, radius);

    debug!(
        "Calculating points of a hexagon in the height map around center: {:?}",
        center
    );
    unsafe { terrain.as_mut() }
        .extend(points.filter_map(|p| {
            trace!("Computing terrain of gradient point: {:?}", p);
            let mut grad = *gradient.get_by_id(&p).or_else(|| {
                error!("{:?} has no gradient", p);
                debug_assert!(false);
                None
            })?;
            trace!("p: {:?} grad: {}", p, grad);

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

            if !grad.is_finite() {
                return None;
            }
            let terrain = if grad <= chance_plain {
                TileTerrainType::Plain
            } else if grad <= chance_plain + chance_wall {
                TileTerrainType::Wall
            } else {
                return None;
            };
            Some((p, TerrainComponent(terrain)))
        }))
        .map_err(|e| {
            error!("Terrain building failed {:?}", e);
            RoomGenerationError::TerrainExtendFailure(e)
        })?;

    debug!("Building terrain from height-map done");
    std = (std / i).sqrt();
    normal_std = (normal_std / i).sqrt();

    let props = HeightMapProperties {
        center,
        radius,
        normal_mean,
        normal_std,
        std,
        mean,
        min: min_grad,
        max: max_grad,
        depth,
        width: dsides,
        height: dsides,
    };

    Ok(props)
}

fn fill_edge(
    center: Axial,
    radius: i32,
    ty: TileTerrainType,
    edge: RoomConnection,
    mut terrain: UnsafeView<Axial, TerrainComponent>,
    chunk: &mut HashSet<Axial>,
) -> Result<(), RoomGenerationError> {
    let RoomConnection {
        offset_start,
        offset_end,
        direction: edge,
    } = edge;
    if edge.q.abs() > 1 || edge.r.abs() > 1 || edge.r == edge.q {
        return Err(RoomGenerationError::InvalidNeighbour(edge));
    }
    let [x, y, z] = edge.hex_axial_to_cube();
    let end = [-z, -x, -y];
    let end = Axial::hex_cube_to_axial(end);
    let vel = end - edge;

    let vertex = (edge * radius) + center;

    debug!(
        "Filling edge {:?}, vertex: {:?} end {:?} vel {:?} radius {} offset_start {} offset_end {}",
        edge, vertex, end, vel, radius, offset_start, offset_end
    );
    let offset_start = offset_start as i32;
    let offset_end = offset_end as i32;
    if radius - offset_start - offset_end <= 0 {
        return Err(RoomGenerationError::BadEdgeOffset {
            radius,
            edge,
            offset_start,
            offset_end,
        });
    }
    unsafe { terrain.as_mut() }
        .extend((offset_start..=(radius - offset_end)).map(move |i| {
            let vertex = vertex + (vel * i);
            chunk.insert(vertex);
            (vertex, TerrainComponent(ty))
        }))
        .map_err(|e| {
            error!("Failed to expand terrain with edge {:?} {:?}", edge, e);
            RoomGenerationError::TerrainExtendFailure(e)
        })?;

    Ok(())
}

struct ChunkMeta {
    pub chungus_mass: usize,
    pub chunks: Vec<HashSet<Axial>>,
}

/// Find the connecting `Plain` chunks.
/// The first one will be the largest chunk
fn calculate_plain_chunks(terrain: View<Axial, TerrainComponent>) -> ChunkMeta {
    debug!("calculate_plain_chunks");
    let mut visited = HashSet::new();
    let mut todo = HashSet::new();
    let mut startind = 0;
    let mut chunk_id = 0;

    let mut chungus_id = 0;
    let mut chungus_mass = 0;
    let mut chunks = Vec::with_capacity(4);
    'a: loop {
        let current = terrain
            .iter()
            .enumerate()
            .skip(startind)
            .find_map(|(i, (p, t))| {
                if t.0.is_walkable() && !visited.contains(&p) {
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
        todo.clear();
        todo.insert(current);
        let mut chunk = HashSet::new();

        while !todo.is_empty() {
            let current = todo.iter().next().cloned().unwrap();
            todo.remove(&current);
            visited.insert(current);
            chunk.insert(current);
            terrain.query_range(&current, 1, &mut |p, t| {
                let TerrainComponent(t) = t;
                if t.is_walkable() && !visited.contains(&p) {
                    todo.insert(p);
                }
            });
        }
        let mass = chunk.len();
        if mass > chungus_mass {
            chungus_mass = mass;
            chungus_id = chunk_id;
        }
        chunks.push(chunk);
        chunk_id += 1;
    }
    if chunks.len() >= 2 {
        chunks.swap(0, chungus_id);
    }
    debug!("calculate_plain_chunks done, found {} chunks", chunks.len());
    debug_assert!(
        chunks
            .iter()
            .zip(chunks.iter().skip(1))
            .find(|(a, b)| !a.is_disjoint(b))
            .is_none(),
        "Internal error: chunks must be disjoint!"
    );
    let meta = ChunkMeta {
        chungus_mass,
        chunks,
    };
    meta
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
                Some(TerrainComponent(TileTerrainType::Bridge)) => print!("x"),
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
    fn maps_are_not_homogeneous() {
        let mut terrain = MortonTable::with_capacity(512);

        let params = RoomGenerationParams::builder()
            .with_radius(8)
            .with_plain_dilation(2)
            .with_seed(Some(*b"deadbeefstewbisc"))
            .build()
            .unwrap();

        let props = generate_room(&params, &[], (UnsafeView::from_table(&mut terrain),)).unwrap();

        dbg!(props);

        let from = Axial::new(0, 0);
        let to = Axial::new(16, 16);
        print_terrain(&from, &to, View::from_table(&terrain));

        let mut seen_empty = false;
        let mut seen_wall = false;
        let mut seen_plain = false;

        // assert that the terrain is not homogeneous
        // check points in radius-1 to account for the bridges
        for point in room_points(Axial::new(8, 8), 7) {
            match terrain.get_by_id(&point) {
                None => seen_empty = true,
                Some(TerrainComponent(TileTerrainType::Plain))
                | Some(TerrainComponent(TileTerrainType::Bridge)) => seen_plain = true,
                Some(TerrainComponent(TileTerrainType::Wall)) => seen_wall = true,
            }
        }

        assert!(seen_plain);
        assert!(seen_wall || seen_empty);
    }

    #[test]
    fn all_plain_are_reachable() {
        // doesn't work all the time...
        let mut plains = Vec::with_capacity(512);
        let mut terrain = MortonTable::with_capacity(512);

        let params = RoomGenerationParams::builder()
            .with_radius(8)
            .with_plain_dilation(2)
            // .with_seed(Some(*b"deadbeefstewbisc"))
            .build()
            .unwrap();
        let props = generate_room(&params, &[], (UnsafeView::from_table(&mut terrain),)).unwrap();

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

        let meta = calculate_plain_chunks(View::from_table(&terrain));

        assert_eq!(meta.chunks.len(), 2);
    }
}
