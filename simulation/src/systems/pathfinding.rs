use crate::model::{
    components::{EntityComponent, TerrainComponent},
    geometry::Point,
    terrain::TileTerrainType,
};
use crate::profile;
use crate::storage::views::View;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct Node {
    pub pos: Point,
    pub parent: Point,
    pub h: i32,
    pub g: i32,
}

impl Node {
    pub fn new(pos: Point, parent: Point, h: i32, g: i32) -> Self {
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
}

/// Find path from `from` to `to`. Will append the resulting path to the `path` output vector.
/// The output' path is in reverse order. Pop the elements to walk the path.
/// This is a performance consideration, as most callers should not need to reverse the order of
/// elements.
pub fn find_path(
    from: Point,
    to: Point,
    (positions, terrain): (View<Point, EntityComponent>, View<Point, TerrainComponent>),
    mut max_iterations: u32,
    path: &mut Vec<Point>,
) -> Result<(), PathFindingError> {
    profile!("find_path");

    let current = from;
    let end = to;

    let mut closed_set = HashMap::<Point, Node>::with_capacity(max_iterations as usize);
    let mut open_set = HashSet::with_capacity(max_iterations as usize);

    let mut current = Node::new(current, current, current.hex_distance(end) as i32, 0);
    closed_set.insert(current.pos, current.clone());
    open_set.insert(current.clone());

    while current.pos != end && !open_set.is_empty() && max_iterations > 0 {
        current = open_set.iter().min_by_key(|node| node.f()).unwrap().clone();
        open_set.remove(&current);
        closed_set.insert(current.pos, current.clone());
        current
            .pos
            .hex_neighbours()
            .iter()
            .cloned()
            .filter(|p| {
                let res = positions.intersects(&p);
                debug_assert!(
                    terrain.intersects(&p) == res,
                    "if p intersects positions it must also intersect terrain!"
                );
                res && (
                    // Filter only the free neighbours
                    // End may be in the either tables!
                    *p == end || (!positions.contains_key(p) && !is_wall(p, terrain.clone()))
                )
            })
            .for_each(|point| {
                if !closed_set.contains_key(&point) {
                    let node = Node::new(
                        point,
                        current.pos,
                        point.hex_distance(end) as i32,
                        current.g + 1,
                    );
                    open_set.insert(node);
                }
                if let Some(node) = closed_set.get_mut(&point) {
                    if current.g + 1 < node.g {
                        node.g = current.g + 1;
                        node.parent = current.pos;
                    }
                }
            });
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
        path.push(current);
        current = closed_set[&current].parent;
    }
    Ok(())
}

fn is_wall(p: &Point, terrain: View<Point, TerrainComponent>) -> bool {
    terrain
        .get_by_id(p)
        .map(|tile| match tile.0 {
            TileTerrainType::Wall => true,
            _ => false,
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::MortonTable;

    #[test]
    fn test_simple_wall() {
        let from = Point::new(0, 2);
        let to = Point::new(5, 2);

        let positions = MortonTable::new();
        let terrain = MortonTable::from_iterator(
            (0..=5).map(|i| (Point::new(2, i), TerrainComponent(TileTerrainType::Wall))),
        )
        .unwrap();

        let mut path = vec![];
        find_path(
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
            assert_eq!(point.hex_distance(current), 1);
            if point.x == 2 {
                assert!(point.y > 5, "{:?}", point);
            }
            current = *point;
        }
        assert_eq!(current, to);
    }

    #[test]
    fn test_path_is_continous() {
        let from = Point::new(17, 6);
        let to = Point::new(7, 16);

        let positions = MortonTable::new();
        let terrain = MortonTable::new();

        let mut path = vec![];
        find_path(
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
            assert_eq!(point.hex_distance(current), 1);
            if point.x == 2 {
                assert!(point.y.abs() > 5, "{:?}", point);
            }
            current = *point;
        }
        assert_eq!(current, to);
    }
}
