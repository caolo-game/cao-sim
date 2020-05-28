use cao_lang::traits::AutoByteEncodeProperties;
use serde_derive::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

/// Represents a hex point in axial coordinate space
#[derive(
    Debug, Clone, Default, Copy, Eq, PartialEq, Serialize, Deserialize, Ord, PartialOrd, Hash,
)]
#[serde(rename_all = "camelCase")]
pub struct Axial {
    pub q: i32,
    pub r: i32,
}

impl Axial {
    pub fn new(q: i32, r: i32) -> Self {
        Self { q, r }
    }

    /// Return the "Manhattan" distance between two points in a hexagonal coordinate space
    /// Interprets points as axial coordiantes
    /// See https://www.redblobgames.com/grids/hexagons/#distances for more information
    pub fn hex_distance(self, other: Axial) -> u32 {
        let [ax, ay, az] = self.hex_axial_to_cube();
        let [bx, by, bz] = other.hex_axial_to_cube();
        let x = (ax - bx).abs() as u32;
        let y = (ay - by).abs() as u32;
        let z = (az - bz).abs() as u32;
        x.max(y).max(z)
    }

    /// Convert self from a hexagonal axial vector to a hexagonal cube vector
    pub fn hex_axial_to_cube(self) -> [i32; 3] {
        let x = self.q;
        let z = self.r;
        let y = -x - z;
        [x, y, z]
    }

    pub fn hex_cube_to_axial([q, _, r]: [i32; 3]) -> Self {
        Self { q, r }
    }

    /// Get the neighbours of this point starting at top left and going counter-clockwise
    pub fn hex_neighbours(self) -> [Axial; 6] {
        [
            Axial::new(self.q + 1, self.r),
            Axial::new(self.q + 1, self.r - 1),
            Axial::new(self.q, self.r - 1),
            Axial::new(self.q - 1, self.r),
            Axial::new(self.q - 1, self.r + 1),
            Axial::new(self.q, self.r + 1),
        ]
    }
}

impl AddAssign for Axial {
    fn add_assign(&mut self, rhs: Self) {
        self.q += rhs.q;
        self.r += rhs.r;
    }
}

impl Add for Axial {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self {
        self += rhs;
        self
    }
}

impl SubAssign for Axial {
    fn sub_assign(&mut self, rhs: Self) {
        self.q -= rhs.q;
        self.r -= rhs.r;
    }
}

impl Sub for Axial {
    type Output = Self;

    fn sub(mut self, rhs: Self) -> Self {
        self -= rhs;
        self
    }
}

impl MulAssign<i32> for Axial {
    fn mul_assign(&mut self, rhs: i32) {
        self.q *= rhs;
        self.r *= rhs;
    }
}

impl Mul<i32> for Axial {
    type Output = Self;

    fn mul(mut self, rhs: i32) -> Self {
        self *= rhs;
        self
    }
}

impl DivAssign<i32> for Axial {
    fn div_assign(&mut self, rhs: i32) {
        self.q /= rhs;
        self.r /= rhs;
    }
}

impl Div<i32> for Axial {
    type Output = Self;

    fn div(mut self, rhs: i32) -> Self {
        self /= rhs;
        self
    }
}

#[derive(Debug, Clone, Default, Copy, Eq, PartialEq, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub struct Circle {
    pub center: Axial,
    pub radius: u32,
}

impl Circle {
    pub fn is_inside(&self, point: Axial) -> bool {
        point.hex_distance(self.center) < self.radius
    }
}

impl AutoByteEncodeProperties for Axial {}
impl AutoByteEncodeProperties for Circle {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_arithmetic() {
        let p1 = Axial::new(0, 0);
        let p2 = Axial::new(-1, 2);

        let sum = p1 + p2;
        assert_eq!(sum, p2);
        assert_eq!(sum - p2, p1);
    }

    #[test]
    fn distance_simple() {
        let a = Axial::new(0, 0);
        let b = Axial::new(1, 3);

        assert_eq!(a.hex_distance(b), 4);

        for p in a.hex_neighbours().iter() {
            assert_eq!(p.hex_distance(a), 1);
        }
    }
}
