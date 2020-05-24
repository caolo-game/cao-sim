use super::GradientMap;
use crate::model::geometry::Axial;
use crate::tables::SpatialKey2d;

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
