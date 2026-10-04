//! The lance's energy core: a plated pod under the keel at the ship's middle, a belt of
//! lit slots round its widest, and at its lowest point a round aperture: an armoured
//! rim, a bright ring, overlapping iris plates stepped in to a bronze collar and the
//! white-hot pinch-fusion lens. Nothing turns: the Heavy Pinch-fusion Lance is laid from
//! the lens itself (`LENS`).

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::{GLOW_LASER, GLOW_PRISM};

use super::super::super::kit::{dark_plate, metal, seam, slab, v3};
use super::super::super::machine::hoop;
use super::body::Body;

/// The core's middle along the hull (authoring frame): the ship's middle as built, so the
/// beam leaves the same point whichever way it is laid.
const CORE_X: f32 = -25.0;
/// The aperture's face, its rim's radius, and how far the lens stands out of it.
const FACE: f32 = 6.0;
const RIM: f32 = 11.0;
const LENS_OUT: f32 = 1.9;
/// The lens's face: where the beam leaves (authoring frame).
#[cfg(test)]
pub(super) const LENS: [f32; 3] = [CORE_X, 0.0, FACE - LENS_OUT];

pub(super) fn core(b: &mut MeshBuilder, body: &Body) {
    let keel = body.at(CORE_X).keel;
    let middle = keel + 1.0;
    let (rings, bands) = if b.fine() { (24, 10) } else { (12, 5) };
    dark_plate(b);
    b.spheroid(
        v3(CORE_X, 0.0, middle),
        v3(26.0, 17.0, middle - 7.0),
        rings,
        bands,
    );
    // The aperture's collar, out of the pod's underside.
    let sides = b.sides(28);
    b.prism(v3(CORE_X, 0.0, FACE), sides, RIM + 0.6, RIM + 1.4, 3.5);
    if !b.coarse() {
        belt(b, middle);
    }
    aperture(b, v3(CORE_X, 0.0, FACE));
}

/// The lit belt: slots round the pod's widest.
fn belt(b: &mut MeshBuilder, middle: f32) {
    b.paint(GLOW_LASER);
    let n = if b.fine() { 14 } else { 6 };
    for i in 0..n {
        let a = std::f32::consts::TAU * (i as f32 + 0.5) / n as f32;
        let at = |s: f32| {
            let e = Vec2::from_angle(a + s);
            v3(CORE_X + e.x * 25.9, e.y * 16.9, middle)
        };
        let d = Vec2::from_angle(a).extend(0.0);
        slab(
            b,
            [
                at(-0.08) - Vec3::Z * 0.35,
                at(0.08) - Vec3::Z * 0.35,
                at(0.08) + Vec3::Z * 0.35,
                at(-0.08) + Vec3::Z * 0.35,
            ],
            d * 0.25,
        );
    }
}

/// The aperture on the face at `c`, facing down.
fn aperture(b: &mut MeshBuilder, c: Vec3) {
    let r = RIM;
    let sides = b.sides(28);
    let out = -Vec3::Z;
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
    b.cylinder_between(c, c + out * LENS_OUT, r * 0.41, r * 0.39, sides);
    b.set_beam_core(c + out * LENS_OUT, r * 0.39);
}
