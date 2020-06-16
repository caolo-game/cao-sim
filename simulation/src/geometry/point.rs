use crate::tables::SpatialKey2d;
use cao_lang::traits::AutoByteEncodeProperties;
use serde_derive::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

/// Represents a hex point in axial coordinate space
#[derive(
    Debug, Clone, Default, Copy, Eq, PartialEq, Serialize, Deserialize, Ord, PartialOrd, Hash,
)]
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

    /// Return the index in `hex_neighbours` of the neighbour if applicable. None otherwise.
    /// `q` and `r` must be in the set {-1, 0, 1}.
    /// To get the index of the neighbour of a point
    /// ```rust
    /// use caolo_sim::model::geometry::Axial;
    /// let point = Axial::new(42, 69);
    /// let neighbour = Axial::new(42, 68);
    /// // `neighbour - point` will result in the vector pointing from `point` to `neighbour`
    /// let i = Axial::neighbour_index(neighbour - point);
    /// assert_eq!(i, Some(2));
    /// ```
    pub fn neighbour_index(Axial { q, r }: Axial) -> Option<usize> {
        let i = match (q, r) {
            (1, 0) => 0,
            (1, -1) => 1,
            (0, -1) => 2,
            (-1, 0) => 3,
            (-1, 1) => 4,
            (0, 1) => 5,
            _ => return None,
        };
        Some(i)
    }

    pub fn rotate_right(self) -> Axial {
        let [x, y, z] = self.hex_axial_to_cube();
        Self::hex_cube_to_axial([-z, -x, -y])
    }

    pub fn rotate_left(self) -> Axial {
        let [x, y, z] = self.hex_axial_to_cube();
        Self::hex_cube_to_axial([-y, -z, -x])
    }
}

impl SpatialKey2d for Axial {
    fn as_array(&self) -> [i32; 2] {
        [self.q, self.r]
    }

    fn new(q: i32, r: i32) -> Self {
        Self { q, r }
    }

    fn get_axis(&self, axis: u8) -> i32 {
        match axis & 1 {
            0 => self.q,
            _ => self.r,
        }
    }

    fn dist(&self, other: &Self) -> u32 {
        u32::try_from(self.hex_distance(*other)).expect("Distance to fit in 32 bits")
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
pub struct Hexagon {
    pub center: Axial,
    pub radius: i32,
}

impl Hexagon {
    pub fn contains(&self, point: &Axial) -> bool {
        let point = *point - self.center;
        let [x, y, z] = point.hex_axial_to_cube();
        let r = self.radius;
        -r <= x && x <= r && -r <= y && y <= r && -r <= z && z <= r
    }

    pub fn iter_points(&self) -> impl Iterator<Item = Axial> {
        let radius = self.radius;
        let center = self.center;
        (-radius..=radius).flat_map(move |x| {
            let fromy = (-radius).max(-x - radius);
            let toy = radius.min(-x + radius);
            (fromy..=toy).map(move |y| {
                let p = Axial::new(x, -x - y);
                p + center
            })
        })
    }
}

impl AutoByteEncodeProperties for Axial {}
impl AutoByteEncodeProperties for Hexagon {}

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

    #[test]
    fn neighbour_indices() {
        let p = Axial::new(13, 42);
        let neighbours = p.hex_neighbours();

        for (i, n) in neighbours.iter().cloned().enumerate() {
            let j = Axial::neighbour_index(n - p);
            assert_eq!(j, Some(i));
        }
    }
}
