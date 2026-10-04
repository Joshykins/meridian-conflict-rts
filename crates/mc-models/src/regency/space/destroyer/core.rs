//! The lance's energy core: an armoured drum through the hull at the ship's middle, clamped
//! where it passes through the plating, a round aperture in each end: an armoured rim, a
//! bright ring, overlapping iris plates stepped in to a bronze collar and the white-hot
//! pinch-fusion lens. The upper face is what is seen from above; the Heavy Pinch-fusion
//! Lance is laid from the lower lens (`LENS`). Nothing turns.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::{GLOW_LASER, GLOW_PRISM};

use super::super::super::kit::{dark_plate, metal, seam, slab, v3};
use super::super::super::machine::hoop;
use super::body::Body;

/// The core's middle along the hull (authoring frame): the ship's middle as built, so the
/// beam leaves the same point whichever way it is laid.
pub(super) const CORE_X: f32 = -25.0;
/// The drum's lower and upper faces: clear of the keel below and the spine above there.
pub(super) const LOW: f32 = 11.0;
pub(super) const HIGH: f32 = 43.5;
/// The drum's radius, its apertures' rims', and how far a lens stands out of its face.
const DRUM: f32 = 13.5;
const RIM: f32 = 12.5;
const LENS_OUT: f32 = 1.9;
/// The lower lens's face: where the beam leaves (authoring frame).
#[cfg(test)]
pub(super) const LENS: [f32; 3] = [CORE_X, 0.0, LOW - LENS_OUT];

pub(super) fn core(b: &mut MeshBuilder, body: &Body) {
    let sides = b.sides(28);
    dark_plate(b);
    b.prism(v3(CORE_X, 0.0, LOW), sides, DRUM, DRUM, HIGH - LOW);
    if b.fine() {
        // Clamp bands where it passes through the hull.
        let s = body.at(CORE_X);
        seam(b);
        for z in [s.keel + 1.0, s.spine - 1.0] {
            hoop(b, v3(CORE_X, 0.0, z), DRUM + 0.3, 1.2, 1.6, sides);
        }
    }
    aperture(b, v3(CORE_X, 0.0, HIGH), 1.0);
    let lens = aperture(b, v3(CORE_X, 0.0, LOW), -1.0);
    b.set_beam_core(lens, RIM * 0.39);
}

/// An aperture on the face at `c` facing `dir` (+1 up, -1 down). Returns its lens's face.
fn aperture(b: &mut MeshBuilder, c: Vec3, dir: f32) -> Vec3 {
    let r = RIM;
    let sides = b.sides(28);
    let out = Vec3::Z * dir;
    dark_plate(b);
    hoop(b, c + out * 0.4, r, r * 0.12, 0.8, sides);
    // The recess behind the plates, dark.
    seam(b);
    b.prism(c - Vec3::Z * 0.05, sides, r * 0.94, r * 0.94, 0.1);
    if !b.coarse() {
        // The bright ring inside the rim.
        b.paint(GLOW_LASER);
        hoop(b, c + out * 0.3, r * 0.86, r * 0.045, 0.3, sides);
        // Iris plates, overlapping, each a step further out toward the lens.
        dark_plate(b);
        let n = if b.fine() { 16 } else { 8 };
        for i in 0..n {
            let a = std::f32::consts::TAU * i as f32 / n as f32;
            let at = |turn: f32, k: f32, lift: f32| {
                (Vec2::from_angle(a + turn) * r * k).extend(0.0) + c + out * lift
            };
            slab(
                b,
                [
                    at(0.0, 0.8, 0.5),
                    at(0.42, 0.8, 0.5),
                    at(0.62, 0.48, 1.1),
                    at(0.3, 0.48, 1.1),
                ],
                out * 0.35,
            );
        }
    }
    metal(b);
    hoop(b, c + out * 1.4, r * 0.45, r * 0.08, 1.0, sides);
    b.paint(GLOW_PRISM);
    let face = c + out * LENS_OUT;
    b.cylinder_between(c, face, r * 0.41, r * 0.39, sides);
    face
}
