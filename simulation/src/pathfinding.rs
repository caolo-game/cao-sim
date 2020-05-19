use crate::model::{
    components::{EntityComponent, RoomConnections, TerrainComponent},
    geometry::Axial,
    indices::Room,
    terrain, RoomPosition, WorldPosition,
};
use crate::profile;
use crate::storage::views::View;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct Node {
    pub pos: Axial,
    pub parent: Axial,
    pub h: i32,
    pub g: i32,
}

impl Node {
    pub fn new(pos: Axial, parent: Axial, h: i32, g: i32) -> Self {
        Self { parent, h, g, pos }
    }

    pub fn f(&self) -> i32 {
        self.h + self.g
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PathFindingError {
    NotFound,
    Unreachable,
    RoomDoesNotExists(Axial),
}

/// Find path from `from` to `to`. Will append the resulting path to the `path` output vector.
/// The output' path is in reverse order. Pop the elements to walk the path.
/// This is a performance consideration, as most callers should not need to reverse the order of
/// elements.
/// Returns the remaining steps
pub fn find_path(
    from: WorldPosition,
    to: WorldPosition,
    (positions, terrain, connections): (
        View<WorldPosition, EntityComponent>,
        View<WorldPosition, TerrainComponent>,
        View<Room, RoomConnections>,
    ),
    mut max_iterations: u32,
    path: &mut Vec<RoomPosition>,
) -> Result<u32, PathFindingError> {
    profile!("find_path");
    if from.room == to.room {
        let positions = View::from_table(
            positions
                .table
                .get_by_id(&from.room)
                .ok_or_else(|| PathFindingError::RoomDoesNotExists(from.room))?,
        );
        let terrain = View::from_table(
            terrain
                .table
                .get_by_id(&from.room)
                .ok_or_else(|| PathFindingError::RoomDoesNotExists(from.room))?,
        );
        find_path_in_room(from.pos, to.pos, (positions, terrain), max_iterations, path)
    } else {
        let mut rooms = Vec::with_capacity(4);
        let from_room = from.room;
        max_iterations =
            find_path_room_scale(from_room, to.room, connections, max_iterations, &mut rooms)?;
        let next_room = rooms
            .pop()
            .expect("find_path_room_scale returned OK, but the room list is empty");

        let edge = next_room.0 - from_room;

        // TODO: find the edge that connects this room to next_room
        // find the shortest path to that edge
        unimplemented!()
    }
}

/// find the rooms one has to visit to go from room `from` to room `to`
/// uses the A* algorithm
/// return the remaning iterations
pub fn find_path_room_scale(
    from: Axial,
    to: Axial,
    connections: View<Room, RoomConnections>,
    mut max_iterations: u32,
    path: &mut Vec<Room>,
) -> Result<u32, PathFindingError> {
    profile!("find_path_room_scale");
    let current = from;
    let end = to;

    let mut closed_set = HashMap::<Axial, Node>::with_capacity(max_iterations as usize);
    let mut open_set = HashSet::with_capacity(max_iterations as usize);
    let mut current = Node::new(current, current, current.hex_distance(end) as i32, 0);
    closed_set.insert(current.pos, current.clone());
    open_set.insert(current.clone());
    while current.pos != end && !open_set.is_empty() && max_iterations > 0 {
        current = open_set.iter().min_by_key(|node| node.f()).unwrap().clone();
        open_set.remove(&current);
        closed_set.insert(current.pos, current.clone());
        for point in connections
            .get_by_id(&Room(current.pos))
            .ok_or_else(|| PathFindingError::RoomDoesNotExists(current.pos))?
            .0
            .iter()
        {
            if !closed_set.contains_key(&point) {
                let node = Node::new(
                    *point,
                    current.pos,
                    point.hex_distance(end) as i32,
                    current.g + 1,
                );
                open_set.insert(node);
            }
            if let Some(node) = closed_set.get_mut(&point) {
                let g = current.g + 1;
                if g < node.g {
                    node.g = g;
                    node.parent = current.pos;
                }
            }
        }
        max_iterations -= 1;
    }
    if current.pos != end {
        if max_iterations > 0 {
            // we ran out of possible paths
            return Err(PathFindingError::Unreachable);
        }
        return Err(PathFindingError::NotFound);
    }

    // reconstruct path
    let mut current = end;
    let end = from;
    while current != end {
        path.push(Room(current));
        current = closed_set[&current].parent;
    }
    Ok(max_iterations)
}

fn is_walkable(p: &Axial, terrain: View<Axial, TerrainComponent>) -> bool {
    terrain
        .get_by_id(p)
        .map(|tile| terrain::is_walkable(tile.0))
        .unwrap_or(false)
}

/// return the remaining steps
/// uses the A* algorithm
pub fn find_path_in_room(
    from: Axial,
    to: Axial,
    (positions, terrain): (View<Axial, EntityComponent>, View<Axial, TerrainComponent>),
    mut max_iterations: u32,
    path: &mut Vec<RoomPosition>,
) -> Result<u32, PathFindingError> {
    profile!("find_path_in_room");

    let current = from;
    let end = to;

    let mut closed_set = HashMap::<Axial, Node>::with_capacity(max_iterations as usize);
    let mut open_set = HashSet::with_capacity(max_iterations as usize);

    let mut current = Node::new(current, current, current.hex_distance(end) as i32, 0);
    closed_set.insert(current.pos, current.clone());
    open_set.insert(current.clone());

    while current.pos != end && !open_set.is_empty() && max_iterations > 0 {
        current = open_set.iter().min_by_key(|node| node.f()).unwrap().clone();
        open_set.remove(&current);
        closed_set.insert(current.pos, current.clone());
        for point in current.pos.hex_neighbours().iter().filter(|p| {
            let res = positions.intersects(&p);
            debug_assert!(
                terrain.clone().intersects(&p) == res,
                "if p intersects positions it must also intersect terrain!"
            );
            res && (
                // Filter only the free neighbours
                // End may be in the either tables!
                *p == &end || (!positions.contains_key(p) && is_walkable(p, terrain.clone()))
            )
        }) {
            if !closed_set.contains_key(&point) {
                let node = Node::new(
                    *point,
                    current.pos,
                    point.hex_distance(end) as i32,
                    current.g + 1,
                );
                open_set.insert(node);
            }
            if let Some(node) = closed_set.get_mut(&point) {
                let g = current.g + 1;
                if g < node.g {
                    node.g = g;
                    node.parent = current.pos;
                }
            }
        }
        max_iterations -= 1;
    }

    if current.pos != end {
        if max_iterations > 0 {
            // we ran out of possible paths
            return Err(PathFindingError::Unreachable);
        }
        return Err(PathFindingError::NotFound);
    }

    // reconstruct path
    let mut current = end;
    let end = from;
    while current != end {
        path.push(RoomPosition(current));
        current = closed_set[&current].parent;
    }
    Ok(max_iterations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::terrain::TileTerrainType;
    use crate::tables::MortonTable;

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
                terrain.insert(Axial::new(x, y), TerrainComponent(TileTerrainType::Plain));
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
