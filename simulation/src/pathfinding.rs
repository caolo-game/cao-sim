use crate::components::{EntityComponent, RoomConnections, RoomProperties, TerrainComponent};
use crate::geometry::Axial;
use crate::map_generation::room::iter_edge;
use crate::model::terrain::TileTerrainType;
use crate::model::{indices::Room, terrain, EmptyKey, RoomPosition, WorldPosition};
use crate::profile;
use crate::storage::views::View;
use arrayvec::ArrayVec;
use slog::{debug, error, trace, warn, Logger};
use std::cmp::{Ord, Ordering, PartialOrd};
use std::collections::BinaryHeap;
use std::collections::HashMap;
use thiserror::Error;

const MAX_BRIDGE_LEN: usize = 64;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct Node {
    pub pos: Axial,
    pub parent: Axial,
    pub h_cost: i32,
    pub g_cost: i32,
}

// std::BinaryHeap puts the max value at the top, so the ordering of Node is reversed!!!!
impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let fa = self.f_cost();
        let fb = other.f_cost();
        fb.partial_cmp(&fa)
    }
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        let fa = self.f_cost();
        let fb = other.f_cost();
        fb.cmp(&fa)
    }
}

impl Node {
    pub fn new(pos: Axial, parent: Axial, h_cost: i32, g_cost: i32) -> Self {
        Self {
            parent,
            h_cost,
            g_cost,
            pos,
        }
    }

    pub fn f_cost(&self) -> i32 {
        self.h_cost + self.g_cost
    }
}

#[derive(Debug, Clone, Copy, Error)]
pub enum PathFindingError {
    #[error("No path was found")]
    NotFound { remaining_steps: u32 },
    #[error("Target is unreachable")]
    Unreachable,
    #[error("Room {0:?} does not exist")]
    RoomDoesNotExists(Axial),

    #[error("Proposed edge {0:?} does not exist")]
    EdgeNotExists(Axial),
}

type FindPathTables<'a> = (
    View<'a, WorldPosition, EntityComponent>,
    View<'a, WorldPosition, TerrainComponent>,
    View<'a, Room, RoomConnections>,
    View<'a, EmptyKey, RoomProperties>,
);

/// Find path from `from` to `to`. Will append the resulting path to the `path` output vector.
/// The output' path is in reverse order. Pop the elements to walk the path.
/// This is a performance consideration, as most callers should not need to reverse the order of
/// elements.
/// Returns the remaining steps
pub fn find_path(
    logger: &Logger,
    from: WorldPosition,
    to: WorldPosition,
    (positions, terrain, connections, room_properties): FindPathTables,
    max_steps: u32,
    path: &mut Vec<RoomPosition>,
    rooms_to_visit: &mut Vec<Room>,
) -> Result<u32, PathFindingError> {
    profile!("find_path");
    trace!(logger, "find_path from {:?} to {:?}", from, to);
    let positions = View::from_table(positions.table.get_by_id(&from.room).ok_or_else(|| {
        trace!(logger, "Room of EntityComponents not found");
        PathFindingError::RoomDoesNotExists(from.room)
    })?);
    let terrain = View::from_table(terrain.table.get_by_id(&from.room).ok_or_else(|| {
        trace!(logger, "Room of TerrainComponents not found");
        PathFindingError::RoomDoesNotExists(from.room)
    })?);
    if from.room == to.room {
        find_path_in_room(
            logger,
            from.pos,
            to.pos,
            (positions, terrain),
            max_steps,
            path,
        )
    } else {
        find_path_multiroom(
            logger,
            from,
            to,
            (positions, terrain, connections, room_properties),
            max_steps,
            path,
            rooms_to_visit,
        )
    }
}

type FindPathMultiRoomTables<'a> = (
    View<'a, Axial, EntityComponent>,
    View<'a, Axial, TerrainComponent>,
    View<'a, Room, RoomConnections>,
    View<'a, EmptyKey, RoomProperties>,
);

fn find_path_multiroom(
    logger: &Logger,
    from: WorldPosition,
    to: WorldPosition,
    (positions, terrain, connections, room_properties): FindPathMultiRoomTables,
    mut max_steps: u32,
    path: &mut Vec<RoomPosition>,
    rooms: &mut Vec<Room>,
) -> Result<u32, PathFindingError> {
    trace!(logger, "find_path_multiroom from {:?} to {:?}", from, to);

    let from_room = from.room;
    max_steps = find_path_overworld(
        logger,
        Room(from_room),
        Room(to.room),
        connections,
        max_steps,
        rooms,
    )
    .map_err(|err| {
        trace!(logger, "find_path_overworld failed {:?}", err);
        err
    })?;
    let Room(next_room) = rooms
        .last()
        .expect("find_path_overworld returned OK, but the room list is empty");

    let edge = *next_room - from_room;
    let bridge = connections.get_by_id(&Room(from_room)).ok_or_else(|| {
        trace!(logger, "Room of bridge not found");
        PathFindingError::RoomDoesNotExists(from_room)
    })?;

    let bridge_ind =
        Axial::neighbour_index(edge).expect("expected the calculated edge to be a valid neighbour");
    let bridge = bridge.0[bridge_ind]
        .as_ref()
        .expect("expected a connection to the next room!");

    let RoomProperties { radius, center } = room_properties
        .value
        .as_ref()
        .expect("expected RoomProperties to be set");

    let bridge = iter_edge(*center, *radius, bridge).map_err(|e| {
        error!(logger, "Failed to obtain edge iterator {:?}", e);
        PathFindingError::EdgeNotExists(edge)
    })?;
    let mut is_bot_on_bridge = false;
    let mut bridge = {
        bridge
            .map(|pos| {
                is_bot_on_bridge = is_bot_on_bridge || pos == from.pos;
                pos
            })
            .filter(|p| !positions.contains_key(p)) // consider only empty spots
            .take(MAX_BRIDGE_LEN)
            .collect::<ArrayVec<[_; MAX_BRIDGE_LEN]>>()
    };
    if is_bot_on_bridge {
        // bot is standing on the bridge
        return Ok(max_steps);
    }

    bridge.sort_unstable_by_key(|p| p.hex_distance(from.pos));

    'a: for p in bridge {
        match find_path_in_room(
            logger,
            from.pos,
            p,
            (positions.clone(), terrain.clone()),
            max_steps,
            path,
        ) {
            Ok(_) => {
                break 'a;
            }
            Err(PathFindingError::NotFound { remaining_steps: m }) => {
                max_steps = m;
            }
            Err(e) => return Err(e),
        }
    }
    trace!(
        logger,
        "find_path_in_room succeeded with {} steps remaining",
        max_steps
    );
    Ok(max_steps)
}

/// find the rooms one has to visit to go from room `from` to room `to`
/// uses the A* algorithm
/// return the remaning iterations
pub fn find_path_overworld(
    logger: &Logger,
    Room(from): Room,
    Room(to): Room,
    connections: View<Room, RoomConnections>,
    mut max_steps: u32,
    path: &mut Vec<Room>,
) -> Result<u32, PathFindingError> {
    profile!("find_path_overworld");
    trace!(logger, "find_path_overworld from {:?} to {:?}", from, to);

    let end = to;

    let mut closed_set = HashMap::<Axial, Node>::with_capacity(max_steps as usize);
    let mut open_set = BinaryHeap::with_capacity(max_steps as usize);
    let mut current = Node::new(from, from, from.hex_distance(end) as i32, 0);
    closed_set.insert(current.pos, current.clone());
    open_set.push(current.clone());
    while current.pos != end && !open_set.is_empty() && max_steps > 0 {
        max_steps -= 1;
        current = open_set.pop().unwrap();
        closed_set.insert(current.pos, current.clone());
        let current_pos = current.pos;
        // [0, 6] items
        for neighbour in connections
            .get_by_id(&Room(current_pos))
            .ok_or_else(|| {
                trace!(
                    logger,
                    "Room {:?} not found in RoomConnections table",
                    current_pos
                );
                PathFindingError::RoomDoesNotExists(current_pos)
            })?
            .0
            .iter()
            .filter_map(|edge| edge.as_ref().map(|edge| edge.direction + current_pos))
            .filter(|pos| !closed_set.contains_key(pos))
        {
            let node = Node::new(
                neighbour,
                current.pos,
                neighbour.hex_distance(end) as i32,
                current.g_cost + 1,
            );
            open_set.push(node);
        }
    }
    if current.pos != end {
        if max_steps > 0 {
            trace!(
                logger,
                "{:?} is unreachable from {:?}, remaining steps: {}, closed_set contains: {}",
                to,
                from,
                max_steps,
                closed_set.len()
            );
            // we ran out of possible paths
            return Err(PathFindingError::Unreachable);
        }
        return Err(PathFindingError::NotFound {
            remaining_steps: max_steps,
        });
    }

    // reconstruct path
    let mut current = end;
    let end = from;
    while current != end {
        path.push(Room(current));
        current = closed_set[&current].parent;
    }
    trace!(
        logger,
        "find_path_overworld returning with {} steps remaining\n{:?}",
        max_steps,
        path
    );
    Ok(max_steps)
}

fn is_walkable(p: Axial, terrain: View<Axial, TerrainComponent>) -> bool {
    terrain
        .get_by_id(&p)
        .map(|tile| terrain::is_walkable(tile.0))
        .unwrap_or(false)
}

/// return the remaining steps
/// uses the A* algorithm
pub fn find_path_in_room(
    logger: &Logger,
    from: Axial,
    to: Axial,
    (positions, terrain): (View<Axial, EntityComponent>, View<Axial, TerrainComponent>),
    mut max_steps: u32,
    path: &mut Vec<RoomPosition>,
) -> Result<u32, PathFindingError> {
    profile!("find_path_in_room");
    trace!(logger, "find_path_in_room from {:?} to {:?}", from, to);

    let current = from;
    let end = to;

    let mut closed_set = HashMap::<Axial, Node>::with_capacity(max_steps as usize);
    let mut open_set = BinaryHeap::with_capacity(max_steps as usize);

    let mut current = Node::new(current, current, current.hex_distance(end) as i32, 0);
    closed_set.insert(current.pos, current.clone());
    open_set.push(current.clone());

    while current.pos != end && !open_set.is_empty() && max_steps > 0 {
        current = open_set.pop().unwrap();
        closed_set.insert(current.pos, current.clone());
        for point in current
            .pos
            .hex_neighbours()
            .iter()
            .cloned()
            .filter(|pos| !closed_set.contains_key(pos))
            .filter(|neighbour_pos| {
                // Filter only the free neighbours
                // End may be in the either tables!
                *neighbour_pos == end
                    || (!positions.contains_key(neighbour_pos)
                        && is_walkable(*neighbour_pos, terrain.clone()))
            })
        {
            let node = Node::new(
                point,
                current.pos,
                point.hex_distance(end) as i32,
                current.g_cost + 1,
            );
            open_set.push(node);
        }
        max_steps -= 1;
    }

    if current.pos != end {
        debug!(
            logger,
            "find_path_in_room failed, remaining_steps: {}", max_steps
        );
        if max_steps > 0 {
            // we ran out of possible paths
            return Err(PathFindingError::Unreachable);
        }
        return Err(PathFindingError::NotFound {
            remaining_steps: max_steps,
        });
    }

    // reconstruct path
    let mut current = end;
    let end = from;
    while current != end {
        path.push(RoomPosition(current));
        current = closed_set[&current].parent;
    }
    debug!(
        logger,
        "find_path_in_room succeeded, remaining_steps: {}", max_steps
    );
    Ok(max_steps)
}

#[derive(Debug)]
pub enum TransitError {
    InternalError(anyhow::Error),
    NotFound,
    InvalidPos,
    InvalidRoom,
}

/// If the result is `Ok` it will contain at least 1 item
pub fn get_valid_transits(
    logger: &Logger,
    current_pos: WorldPosition,
    target_room: Room,
    (terrain, entities, room_properties): (
        View<WorldPosition, TerrainComponent>,
        View<WorldPosition, EntityComponent>,
        View<EmptyKey, RoomProperties>,
    ),
) -> Result<ArrayVec<[WorldPosition; 3]>, TransitError> {
    trace!(
        logger,
        "get_valid_transits {:?} {:?}",
        current_pos,
        target_room
    );
    // from a bridge the bot can reach at least 1 and at most 3 tiles
    // try to find an empty one and move the bot there, otherwise the move fails

    if current_pos.room.hex_distance(target_room.0) != 1 {
        debug!(
            logger,
            "Trying to find valid transit from {:?} to {:?} which are not neighbours",
            current_pos,
            target_room
        );
        return Err(TransitError::InvalidRoom);
    }

    let props = room_properties.unwrap_value();

    let mirror_pos = mirrored_room_position(logger, current_pos.pos, props)?;

    debug_assert_eq!(
        mirror_pos.hex_distance(props.center),
        props.radius,
        "expected {:?} to be {} steps from center: {:?}",
        mirror_pos,
        props.radius,
        props.center
    );

    let mut candidates: ArrayVec<[_; 16]> = ArrayVec::default();
    terrain
        .table
        .get_by_id(&target_room.0)
        .ok_or_else(|| {
            let err = format!("target room {:?} does not exist in terrain", target_room);
            warn!(logger, "{}", err);
            TransitError::InternalError(anyhow::Error::msg(err))
        })?
        .query_range(&mirror_pos, 1, &mut |pos, TerrainComponent(tile)| {
            if *tile == TileTerrainType::Bridge {
                candidates
                    .try_push(WorldPosition {
                        room: target_room.0,
                        pos,
                    })
                    .unwrap_or_else(|e| warn!(logger, "Failed to push bridge candidate: {:?}", e));
            }
        });

    trace!(logger, "Bridge candidates {:?}", candidates);

    if candidates.is_empty() {
        debug!(
            logger,
            "Could not find an acceptable bridge candidate around pos {:?} in {:?}",
            mirror_pos,
            target_room
        );
        return Err(TransitError::NotFound);
    }

    let candidates: ArrayVec<[_; 3]> = candidates
        .into_iter()
        .filter(|p| !entities.contains_key(p))
        .take(3)
        .collect();

    if candidates.is_empty() {
        trace!(logger, "No empty candidate was found");
        return Err(TransitError::NotFound);
    }

    trace!(logger, "Returning bridge candidates: {:?}", candidates);

    debug_assert!(candidates.len() >= 1);
    Ok(candidates)
}

/// Mirror of the current position, this should be the immediate bridge in the next room.
///
/// Example:
///
/// Transform X to Y
///
/// ```
/// //    ++
/// //  +    +
/// //  +    +
/// //    Y+
/// //    X+
/// //  +    +
/// //  +    +
/// //    ++
/// ```
///
/// Mirror is determined by:
/// - Translating the position to 0
/// - Taking the cubic representation.
/// - Fixing the largest abs value and swapping the other two.
/// - Inverting the position ( pos * -1 )
/// - Translating it back to center
pub fn mirrored_room_position(
    _logger: &Logger,
    current_pos: Axial,
    props: &RoomProperties,
) -> Result<Axial, TransitError> {
    let offset = props.center;
    let pos = current_pos - offset;

    let cube = pos.hex_axial_to_cube();

    #[cfg(debug_assertions)]
    let mut zero_ind = None;

    let (maxind, _) = cube
        .iter()
        .enumerate()
        .max_by_key(|(_i, x)| {
            let x = x.abs();
            #[cfg(debug_assertions)]
            {
                if x == 0 {
                    zero_ind = Some(*_i);
                }
            }
            x
        })
        .unwrap();

    #[cfg(debug_assertions)]
    {
        if zero_ind.is_some() {
            error!(_logger, "Room corners are not supported {:?}", current_pos);
            return Err(TransitError::InvalidPos);
        }
    }

    let [x, y, z] = cube;
    let mirror_cube = match maxind {
        0 => [-x, -z, -y],
        1 => [-z, -y, -x],
        2 => [-y, -x, -z],
        _ => unreachable!(),
    };
    let pos = Axial::hex_cube_to_axial(mirror_cube);
    Ok(pos + offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::terrain::TileTerrainType;
    use crate::tables::morton::MortonTable;
    use slog::{o, Drain};

    #[test]
    fn test_simple_wall() {
        let from = Axial::new(0, 2);
        let to = Axial::new(5, 2);

        let logger = slog::Logger::root(slog_stdlog::StdLog.fuse(), o!());
        let positions = MortonTable::new();
        let terrain = MortonTable::from_iterator((0..25).flat_map(|x| {
            (0..25).map(move |y| {
                let ty = if x == 3 && y <= 5 {
                    TileTerrainType::Wall
                } else {
                    TileTerrainType::Plain
                };

                (Axial::new(x, y), TerrainComponent(ty))
            })
        }))
        .unwrap();

        let mut path = vec![];
        find_path_in_room(
            &logger,
            from,
            to,
            (View::from_table(&positions), View::from_table(&terrain)),
            512,
            &mut path,
        )
        .expect("Path finding failed");
        path.reverse();

        let mut current = from;
        for point in path.iter() {
            let point = point.0;
            assert_eq!(point.hex_distance(current), 1);
            if point.q == 3 {
                assert!(point.r > 5, "{:?}", point);
            }
            current = point;
        }
        assert_eq!(current, to);
    }

    #[test]
    fn test_path_is_continous() {
        let from = Axial::new(17, 6);
        let to = Axial::new(7, 16);

        let positions = MortonTable::new();
        let mut terrain = MortonTable::new();

        for x in 0..25 {
            for y in 0..25 {
                terrain
                    .insert(Axial::new(x, y), TerrainComponent(TileTerrainType::Plain))
                    .unwrap();
            }
        }

        let mut path = vec![];
        let logger = slog::Logger::root(slog_stdlog::StdLog.fuse(), o!());
        find_path_in_room(
            &logger,
            from,
            to,
            (View::from_table(&positions), View::from_table(&terrain)),
            512,
            &mut path,
        )
        .expect("Path finding failed");
        path.reverse();

        let mut current = from;
        for point in path.iter() {
            let point = point.0;
            assert_eq!(point.hex_distance(current), 1);
            if point.q == 2 {
                assert!(point.r.abs() > 5, "{:?}", point);
            }
            current = point;
        }
        assert_eq!(current, to);
    }
}
