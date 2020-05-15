use cao_lang::traits::AutoByteEncodeProperties;
use serde_derive::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

/// Represents a 2D point
/// x and y coordinates are interpreted as column and row when Point is used as a hex tile.
#[derive(
    Debug, Clone, Default, Copy, Eq, PartialEq, Serialize, Deserialize, Ord, PartialOrd, Hash,
)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Return the distance between two points in a hexagonal coordinate space
    /// Interprets points as axial coordiante vectors
    /// See https://www.redblobgames.com/grids/hexagons/#distances for more information
    pub fn hex_distance(self, other: Point) -> u32 {
        let [ax, ay, az] = self.hex_axial_to_cube();
        let [bx, by, bz] = other.hex_axial_to_cube();
        let x = ax - bx;
        let y = ay - by;
        let z = az - bz;
        x.abs().max(y.abs()).max(z.abs()) as u32
    }

    /// Convert self from a hexagonal axial vector to a hexagonal cube vector
    fn hex_axial_to_cube(self) -> [i32; 3] {
        let x = self.x;
        let z = self.y;
        let y = -x - z;
        [x, y, z]
    }

    /// Get the neighbours of this point starting at top left and going counter-clockwise
    pub fn hex_neighbours(self) -> [Point; 6] {
        [
            Point::new(self.x + 1, self.y),
            Point::new(self.x + 1, self.y - 1),
            Point::new(self.x, self.y - 1),
            Point::new(self.x - 1, self.y),
            Point::new(self.x - 1, self.y + 1),
            Point::new(self.x, self.y + 1),
        ]
    }
}

impl AddAssign for Point {
    fn add_assign(&mut self, rhs: Point) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Add for Point {
    type Output = Self;

    fn add(mut self, rhs: Point) -> Point {
        self += rhs;
        self
    }
}

impl SubAssign for Point {
    fn sub_assign(&mut self, rhs: Point) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl Sub for Point {
    type Output = Self;

    fn sub(mut self, rhs: Point) -> Point {
        self -= rhs;
        self
    }
}

impl MulAssign<i32> for Point {
    fn mul_assign(&mut self, rhs: i32) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

impl Mul<i32> for Point {
    type Output = Point;

    fn mul(mut self, rhs: i32) -> Self {
        self *= rhs;
        self
    }
}

impl DivAssign<i32> for Point {
    fn div_assign(&mut self, rhs: i32) {
        self.x /= rhs;
        self.y /= rhs;
    }
}

impl Div<i32> for Point {
    type Output = Point;

    fn div(mut self, rhs: i32) -> Self {
        self /= rhs;
        self
    }
}

#[derive(Debug, Clone, Default, Copy, Eq, PartialEq, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub struct Circle {
    pub center: Point,
    pub radius: u32,
}

impl Circle {
    pub fn is_inside(&self, point: Point) -> bool {
        point.hex_distance(self.center) < self.radius
    }
}

impl AutoByteEncodeProperties for Point {}
impl AutoByteEncodeProperties for Circle {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_arithmetic() {
        let p1 = Point::new(0, 0);
        let p2 = Point::new(-1, 2);

        let sum = p1 + p2;
        assert_eq!(sum, p2);
        assert_eq!(sum - p2, p1);
    }

    #[test]
    fn distance_simple() {
        let a = Point::new(0, 0);
        let b = Point::new(1, 3);

        assert_eq!(a.hex_distance(b), 4);

        for p in a.hex_neighbours().iter() {
            assert_eq!(p.hex_distance(a), 1);
        }
    }
}
