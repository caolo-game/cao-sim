use crate::components::{EntityComponent, RoomConnections, RoomProperties, TerrainComponent};
use crate::geometry::Axial;
use crate::map_generation::room::iter_edge;
use crate::model::terrain::TileTerrainType;
use crate::model::{indices::Room, terrain, EmptyKey, RoomPosition, WorldPosition};
use crate::profile;
use crate::storage::views::View;
use arrayvec::ArrayVec;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

const MAX_BRIDGE_LEN: usize = 64;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct Node {
    pub pos: Axial,
    pub parent: Axial,
    pub h_cost: i32,
    pub g_cost: i32,
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
    from: WorldPosition,
    to: WorldPosition,
    (positions, terrain, connections, room_properties): FindPathTables,
    max_steps: u32,
    path: &mut Vec<RoomPosition>,
) -> Result<u32, PathFindingError> {
    profile!("find_path");
    trace!("find_path from {:?} to {:?}", from, to);
    let positions = View::from_table(positions.table.get_by_id(&from.room).ok_or_else(|| {
        trace!("Room of EntityComponents not found");
        PathFindingError::RoomDoesNotExists(from.room)
    })?);
    let terrain = View::from_table(terrain.table.get_by_id(&from.room).ok_or_else(|| {
        trace!("Room of TerrainComponents not found");
        PathFindingError::RoomDoesNotExists(from.room)
    })?);
    if from.room == to.room {
        find_path_in_room(from.pos, to.pos, (positions, terrain), max_steps, path)
    } else {
        find_path_multiroom(
            from,
            to,
            (positions, terrain, connections, room_properties),
            max_steps,
            path,
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
    from: WorldPosition,
    to: WorldPosition,
    (positions, terrain, connections, room_properties): FindPathMultiRoomTables,
    mut max_steps: u32,
    path: &mut Vec<RoomPosition>,
) -> Result<u32, PathFindingError> {
    trace!("find_path_multiroom from {:?} to {:?}", from, to);

    let mut rooms = Vec::with_capacity(4);
    let from_room = from.room;
    max_steps = find_path_overworld(
        Room(from_room),
        Room(to.room),
        connections,
        max_steps,
        &mut rooms,
    )
    .map_err(|err| {
        trace!("find_path_overworld failed {:?}", err);
        err
    })?;
    let Room(next_room) = rooms
        .pop()
        .expect("find_path_overworld returned OK, but the room list is empty");

    let edge = next_room - from_room;
    let bridge = connections.get_by_id(&Room(from_room)).ok_or_else(|| {
        trace!("Room of bridge not found");
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
        error!("Failed to obtain edge iterator {:?}", e);
        PathFindingError::EdgeNotExists(edge)
    })?;
    // If running in debug mode just use `collect` which panics if the length of the bridge is
    // larger than MAX_BRIDGE_LEN
    //
    // in release mode take only MAX_BRIDGE_LEN candidates and avoid panic
    #[cfg(debug_assertions)]
    let mut bridge = { bridge.collect::<ArrayVec<[_; MAX_BRIDGE_LEN]>>() };
    #[cfg(not(debug_assertions))]
    let mut bridge = {
        bridge
            .take(MAX_BRIDGE_LEN)
            .collect::<ArrayVec<[_; MAX_BRIDGE_LEN]>>()
    };

    bridge.sort_unstable_by_key(|p| p.hex_distance(from.pos));

    'a: for p in bridge {
        match find_path_in_room(
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
        "find_path_in_room succeeded with {} steps remaining",
        max_steps
    );
    Ok(max_steps)
}

/// find the rooms one has to visit to go from room `from` to room `to`
/// uses the A* algorithm
/// return the remaning iterations
pub fn find_path_overworld(
    Room(from): Room,
    Room(to): Room,
    connections: View<Room, RoomConnections>,
    mut max_steps: u32,
    path: &mut Vec<Room>,
) -> Result<u32, PathFindingError> {
    profile!("find_path_overworld");
    trace!("find_path_overworld from {:?} to {:?}", from, to);

    let end = to;

    let mut closed_set = HashMap::<Axial, Node>::with_capacity(max_steps as usize);
    let mut open_set = HashSet::with_capacity(max_steps as usize);
    let mut current = Node::new(from, from, from.hex_distance(end) as i32, 0);
    closed_set.insert(current.pos, current.clone());
    open_set.insert(current.clone());
    while current.pos != end && !open_set.is_empty() && max_steps > 0 {
        max_steps -= 1;
        current = open_set
            .iter()
            .min_by_key(|node| node.f_cost())
            .unwrap()
            .clone();
        open_set.remove(&current);
        closed_set.insert(current.pos, current.clone());
        let current_pos = current.pos;
        // [0, 6] items
        for neighbour in connections
            .get_by_id(&Room(current_pos))
            .ok_or_else(|| {
                trace!("Room {:?} not found in RoomConnections table", current_pos);
                PathFindingError::RoomDoesNotExists(current_pos)
            })?
            .0
            .iter()
            .filter_map(|edge| edge.as_ref().map(|edge| edge.direction + current_pos))
        {
            if !closed_set.contains_key(&neighbour) {
                let node = Node::new(
                    neighbour,
                    current.pos,
                    neighbour.hex_distance(end) as i32,
                    current.g_cost + 1,
                );
                open_set.insert(node);
            }
            if let Some(node) = closed_set.get_mut(&neighbour) {
                let g_cost = current.g_cost + 1;
                if g_cost < node.g_cost {
                    node.g_cost = g_cost;
                    node.parent = current.pos;
                }
            }
        }
    }
    if current.pos != end {
        if max_steps > 0 {
            trace!(
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
    from: Axial,
    to: Axial,
    (positions, terrain): (View<Axial, EntityComponent>, View<Axial, TerrainComponent>),
    mut max_steps: u32,
    path: &mut Vec<RoomPosition>,
) -> Result<u32, PathFindingError> {
    profile!("find_path_in_room");
    trace!("find_path_in_room from {:?} to {:?}", from, to);

    let current = from;
    let end = to;

    let mut closed_set = HashMap::<Axial, Node>::with_capacity(max_steps as usize);
    let mut open_set = HashSet::with_capacity(max_steps as usize);

    let mut current = Node::new(current, current, current.hex_distance(end) as i32, 0);
    closed_set.insert(current.pos, current.clone());
    open_set.insert(current.clone());

    while current.pos != end && !open_set.is_empty() && max_steps > 0 {
        current = open_set
            .iter()
            .min_by_key(|node| node.f_cost())
            .unwrap()
            .clone();
        open_set.remove(&current);
        closed_set.insert(current.pos, current.clone());
        for point in current
            .pos
            .hex_neighbours()
            .iter()
            .cloned()
            .filter(|neighbour_pos| {
                let res = positions.intersects(&neighbour_pos);
                debug_assert!(
                    terrain.clone().intersects(&neighbour_pos) == res,
                    "if neighbour_pos intersects positions it must also intersect terrain!"
                );
                res && (
                    // Filter only the free neighbours
                    // End may be in the either tables!
                    *neighbour_pos == end
                        || (!positions.contains_key(neighbour_pos)
                            && is_walkable(*neighbour_pos, terrain.clone()))
                )
            })
        {
            if !closed_set.contains_key(&point) {
                let node = Node::new(
                    point,
                    current.pos,
                    point.hex_distance(end) as i32,
                    current.g_cost + 1,
                );
                open_set.insert(node);
            }
            if let Some(node) = closed_set.get_mut(&point) {
                let g_cost = current.g_cost + 1;
                if g_cost < node.g_cost {
                    node.g_cost = g_cost;
                    node.parent = current.pos;
                }
            }
        }
        max_steps -= 1;
    }

    if current.pos != end {
        debug!("find_path_in_room failed, remaining_steps: {}", max_steps);
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
        "find_path_in_room succeeded, remaining_steps: {}",
        max_steps
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
    current_pos: WorldPosition,
    target_room: Room,
    (terrain, entities, room_connections, room_properties): (
        View<WorldPosition, TerrainComponent>,
        View<WorldPosition, EntityComponent>,
        View<Room, RoomConnections>,
        View<EmptyKey, RoomProperties>,
    ),
) -> Result<ArrayVec<[WorldPosition; 3]>, TransitError> {
    trace!("get_valid_transits {:?} {:?}", current_pos, target_room);
    // from a bridge the bot can reach at least 1 and at most 3 tiles
    // try to find an empty one and move the bot there, otherwise the move fails

    // to obtain the edge we require the bot's current pos (room)
    // the room_connection

    // the bridge on the other side
    let bridge = match room_connections.get_by_id(&target_room).and_then(|c| {
        let direction = current_pos.room - target_room.0;
        let ind = Axial::neighbour_index(direction)?;
        c.0[ind].as_ref()
    }) {
        Some(conn) => conn,
        None => {
            let msg = format!("Room {:?} has no (valid) connections", target_room);
            trace!("{}", msg);
            return Err(TransitError::InternalError(anyhow::Error::msg(msg)));
        }
    };
    // to obtain the pos we need an edge point that's absolute position is 1 away from
    // current pos and is uncontested.
    let props = room_properties.unwrap_value();

    let current_abs = current_pos.absolute(props.radius as i32);
    trace!("current_abs {:?}", current_abs);

    // if this fails once it will fail always, so we'll just panic
    let candidates: ArrayVec<[_; 3]> = iter_edge(props.center, props.radius, bridge)
        .expect("Failed to iter the edge")
        .filter(|pos| {
            let pos = WorldPosition {
                room: target_room.0,
                pos: *pos,
            };
            let abs_pos = pos.absolute(props.radius as i32);
            trace!("pos {:?} abs_pos {:?}", pos, abs_pos);
            // the candidate terrain must be a Bridge and must be 1 tile away
            current_abs.hex_distance(abs_pos) <= 1
                && terrain
                    .get_by_id(&pos)
                    .map(|t| t.0 == TileTerrainType::Bridge)
                    .unwrap_or(false)
        })
        .map(|pos| WorldPosition {
            room: target_room.0,
            pos,
        })
        .collect();

    if candidates.is_empty() {
        let msg = "Could not find an acceptable bridge candidate";
        trace!("{}", msg);
        return Err(TransitError::InternalError(anyhow::Error::msg(msg)));
    }

    let candidates: ArrayVec<[_; 3]> = candidates
        .into_iter()
        .filter(|p| !entities.contains_key(p))
        .collect();

    if candidates.is_empty() {
        return Err(TransitError::NotFound);
    }

    debug_assert!(candidates.len() >= 1);
    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::terrain::TileTerrainType;
    use crate::tables::morton::MortonTable;

    #[test]
    fn test_simple_wall() {
        let from = Axial::new(0, 2);
        let to = Axial::new(5, 2);

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
        find_path_in_room(
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
