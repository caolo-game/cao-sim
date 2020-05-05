pub mod point;
pub mod point3;

pub use point::*;
pub use point3::*;

pub fn aabb_over_circle(center: Point, radius: u32) -> (Point, Point) {
    use crate::tables::SpatialKey2d;
    let [x, y] = center.as_array();
    let radius = radius as i32;
    let from = Point::new(x - radius, y - radius);
    let to = Point::new(x + radius, y + radius);

    (from, to)
}

