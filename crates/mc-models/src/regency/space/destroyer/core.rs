//! The lance's energy core: an armoured drum through the hull at the ship's middle,
//! clamped where it passes through the plating, a round aperture in each end: an
//! armoured rim, a bright ring, overlapping iris plates stepped in to a collar and the
//! white-hot pinch-fusion lens. The upper face is what is seen from above; the Heavy
//! Pinch-fusion Lance is laid from the lower lens. Nothing turns.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::{GLOW_LASER, GLOW_PRISM};

use super::super::super::kit::{dark_plate, metal, seam, slab, v3};
use super::super::super::machine::hoop;
use super::super::body::Body;

/// The drum's radius, its apertures' rims', and how far a lens stands out of its face.
const DRUM: f32 = 34.0;
const RIM: f32 = 31.0;
const LENS_OUT: f32 = 4.8;

/// Where the drum stands: its middle along the hull (the ship's middle, so the beam
/// leaves the same point whichever way it is laid), and its lower and upper faces.
#[derive(Clone, Copy)]
pub(super) struct Core {
    pub(super) x: f32,
    pub(super) low: f32,
    pub(super) high: f32,
}

impl Core {
    /// The lower lens's face: where the beam leaves.
    #[cfg(test)]
    pub(super) fn lens(&self) -> Vec3 {
        v3(self.x, 0.0, self.low - LENS_OUT)
    }
}

pub(super) fn core(b: &mut MeshBuilder, body: &Body, at: Core) {
    let sides = b.sides(28);
    dark_plate(b);
    b.prism(v3(at.x, 0.0, at.low), sides, DRUM, DRUM, at.high - at.low);
    if b.fine() {
        // Clamp bands where it passes through the hull.
        let s = body.at(at.x);
        seam(b);
        for z in [s.keel + 2.5, s.spine - 2.5] {
            if z > at.low + 3.0 && z < at.high - 3.0 {
                hoop(b, v3(at.x, 0.0, z), DRUM + 0.8, 3.0, 4.0, sides);
            }
        }
    }
    aperture(b, v3(at.x, 0.0, at.high), 1.0);
    let lens = aperture(b, v3(at.x, 0.0, at.low), -1.0);
    b.set_beam_core(lens, RIM * 0.39);
}

/// An aperture on the face at `c` facing `dir` (+1 up, -1 down). Returns its lens's face.
fn aperture(b: &mut MeshBuilder, c: Vec3, dir: f32) -> Vec3 {
    let r = RIM;
    let sides = b.sides(28);
    let out = Vec3::Z * dir;
    dark_plate(b);
    hoop(b, c + out * 1.0, r, r * 0.12, 2.0, sides);
    // The recess behind the plates, dark.
    seam(b);
    b.prism(c - Vec3::Z * 0.1, sides, r * 0.94, r * 0.94, 0.2);
    if !b.coarse() {
        // The bright ring inside the rim.
        b.paint(GLOW_LASER);
        hoop(b, c + out * 0.75, r * 0.86, r * 0.045, 0.75, sides);
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
                    at(0.0, 0.8, 1.2),
                    at(0.42, 0.8, 1.2),
                    at(0.62, 0.48, 2.8),
                    at(0.3, 0.48, 2.8),
                ],
                out * 0.9,
            );
        }
    }
    metal(b);
    hoop(b, c + out * 3.5, r * 0.45, r * 0.08, 2.5, sides);
    b.paint(GLOW_PRISM);
    let face = c + out * LENS_OUT;
    b.cylinder_between(c, face, r * 0.41, r * 0.39, sides);
    face
}
