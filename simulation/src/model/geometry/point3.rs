use super::point::Point;
use cao_lang::traits::AutoByteEncodeProperties;
use serde_derive::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

/// Represents a 3D point
///
#[derive(Debug, Clone, Default, Copy, Eq, PartialEq, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub struct Point3 {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Point3 {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Return the distance between two points in a hexagonal coordinate space
    /// Interprets points as axial coordiante vectors
    /// See https://www.redblobgames.com/grids/hexagons/#distances for more information
    pub fn hex_distance(self, other: Point3) -> u64 {
        let diff = self - other;
        ((diff.x.abs() + diff.y.abs() + diff.z.abs()) / 2) as u64
    }

    /// Convert a point from a hexagonal axial vector to a hexagonal cube vector
    pub fn hex_axial_to_cube(p: Point) -> Self {
        let x = p.q;
        let z = p.r;
        let y = -x - z;
        Self { x, y, z }
    }

    /// Convert self to an axial vector
    pub fn into_axial(self) -> Point {
        let q = self.x;
        let r = self.z;
        Point { q, r }
    }
}

impl AddAssign for Point3 {
    fn add_assign(&mut self, rhs: Point3) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
    }
}

impl Add for Point3 {
    type Output = Self;

    fn add(mut self, rhs: Point3) -> Point3 {
        self += rhs;
        self
    }
}

impl SubAssign for Point3 {
    fn sub_assign(&mut self, rhs: Point3) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
    }
}

impl Sub for Point3 {
    type Output = Self;

    fn sub(mut self, rhs: Point3) -> Point3 {
        self -= rhs;
        self
    }
}

impl MulAssign<i32> for Point3 {
    fn mul_assign(&mut self, rhs: i32) {
        self.x *= rhs;
        self.y *= rhs;
        self.z *= rhs;
    }
}

impl Mul<i32> for Point3 {
    type Output = Point3;

    fn mul(mut self, rhs: i32) -> Self {
        self *= rhs;
        self
    }
}

impl DivAssign<i32> for Point3 {
    fn div_assign(&mut self, rhs: i32) {
        self.x /= rhs;
        self.y /= rhs;
        self.z /= rhs;
    }
}

impl Div<i32> for Point3 {
    type Output = Point3;

    fn div(mut self, rhs: i32) -> Self {
        self /= rhs;
        self
    }
}

#[derive(Debug, Clone, Default, Copy, Eq, PartialEq, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub struct Sphere {
    pub center: Point3,
    pub radius: u32,
}

impl Sphere {
    pub fn is_inside(&self, point: Point3) -> bool {
        point.hex_distance(self.center) < u64::from(self.radius)
    }
}

impl AutoByteEncodeProperties for Point3 {}
impl AutoByteEncodeProperties for Sphere {}
