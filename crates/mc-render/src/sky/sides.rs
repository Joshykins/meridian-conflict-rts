//! A climate divide's line as the shaders hold it.

use crate::gpu_consts::divide as consts;
use mc_data::weather::ClimateDivide;

const _: () = assert!(consts::POINTS as usize == ClimateDivide::MAX_POINTS);

/// A divide's line as the shaders hold it (`Globals::divide`): its points, and how
/// many (0 with no divide).
pub(crate) fn divide_points(
    divide: Option<&ClimateDivide>,
) -> ([[f32; 4]; consts::POINTS as usize], f32) {
    let mut points = [[0.0; 4]; consts::POINTS as usize];
    let Some(divide) = divide else {
        return (points, 0.0);
    };
    for (slot, p) in points.iter_mut().zip(&divide.line) {
        *slot = [p.0, p.1, 0.0, 0.0];
    }
    (points, divide.line.len().min(points.len()) as f32)
}
