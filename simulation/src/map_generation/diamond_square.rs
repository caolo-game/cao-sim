use super::GradientMap;
use crate::model::geometry::Axial;
use crate::tables::{SpatialKey2d, Table};
use rand::Rng;

pub fn create_noise(
    from: Axial,
    to: Axial,
    dsides: i32,
    rng: &mut impl Rng,
    gradient: &mut GradientMap,
) {
    let fheight = &mut move |_gradient: &GradientMap, _p: Axial, radius: i32, mean_heights: f32| {
        mean_heights + rng.gen_range(-1.0, 1.0) * radius as f32
    };

    // init corners
    for edge in [from, Axial::new(to.q, from.r), Axial::new(from.q, to.r), to].iter() {
        gradient.delete(&edge);
        gradient.insert(*edge, fheight(&gradient, from, 16, 0.0));
    }

    let mut d = dsides / 2;
    let mut max_grad = -1e15f32;
    let mut min_grad = 1e15f32;

    debug!("Running diamond-square");

    while 1 <= d {
        for x in (d..dsides).step_by(2 * d as usize) {
            for y in (d..dsides).step_by(2 * d as usize) {
                let g = square(gradient, Axial::new(x, y), d, fheight);
                max_grad = max_grad.max(g);
                min_grad = min_grad.min(g);
            }
        }
        for x in (d..dsides).step_by(2 * d as usize) {
            for y in (from.r..=dsides).step_by(2 * d as usize) {
                let g = diamond(gradient, Axial::new(x, y), d, fheight);
                max_grad = max_grad.max(g);
                min_grad = min_grad.min(g);
            }
        }
        for x in (from.q..=dsides).step_by(2 * d as usize) {
            for y in (d..dsides).step_by(2 * d as usize) {
                let g = diamond(gradient, Axial::new(x, y), d, fheight);
                max_grad = max_grad.max(g);
                min_grad = min_grad.min(g);
            }
        }
        d /= 2;
    }

    debug!("Running diamond-square done");
}

/// returns the new gradient
pub fn square(
    gradient: &mut GradientMap,
    p: Axial,
    radius: i32,
    fheight: &mut impl FnMut(&GradientMap, Axial, i32, f32) -> f32,
) -> f32 {
    let mut sum = 0.0;
    let mut num = 0;

    let [x, y] = p.as_array();
    for grad in [
        Axial::new(x - radius, y - radius),
        Axial::new(x - radius, y + radius),
        Axial::new(x + radius, y - radius),
        Axial::new(x + radius, y + radius),
    ]
    .iter()
    .filter_map(|point| gradient.get_by_id(point))
    {
        sum += grad;
        num += 1;
    }

    let grad = fheight(&gradient, p, radius, sum / num as f32);
    gradient.update(p, grad);
    grad
}

/// returns the new gradient at point p
pub fn diamond(
    gradient: &mut GradientMap,
    p: Axial,
    radius: i32,
    fheight: &mut impl FnMut(&GradientMap, Axial, i32, f32) -> f32,
) -> f32 {
    let mut sum = 0.0;
    let mut num = 0;

    let [x, y] = p.as_array();

    for grad in [
        Axial::new(x - radius, y),
        Axial::new(x + radius, y),
        Axial::new(x, y - radius),
        Axial::new(x, y + radius),
    ]
    .iter()
    .filter_map(|point| gradient.get_by_id(point))
    {
        sum += grad;
        num += 1;
    }

    let grad = fheight(&gradient, p, radius, sum / num as f32);
    gradient.update(p, grad);
    grad
}
