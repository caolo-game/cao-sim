pub mod point;
pub mod point3;

pub use point::*;
pub use point3::*;

pub fn aabb_over_circle(center: Axial, radius: u32) -> (Axial, Axial) {
    use crate::tables::SpatialKey2d;

    let [x, y] = center.as_array();
    let radius = radius as i32;
    let from = Axial::new(x - radius, y - radius);
    let to = Axial::new(x + radius, y + radius);

    (from, to)
}

