//! The lance's energy core: an armoured drum through the hull at the ship's middle,
//! clamped where it passes through the plating. Its upper end is a round aperture (an
//! armoured rim, a bright ring, overlapping iris plates stepped in to a collar and the
//! white-hot pinch-fusion lens), what is seen from above. Its lower end is at the keel,
//! where the emitter the Edict Lance is laid from (`emitter`) is set into the belly: a
//! ribbed lens dome inside rings of machinery that turn with the charge, in a well whose
//! seat is faired up into the hull all round.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::{GLOW_LASER, GLOW_PRISM};

use super::super::super::kit::{dark_plate, metal, seam, slab, v3};
use super::super::super::machine::{hoop, hoop_on};
use super::super::body::Body;

/// The drum's radius, its upper aperture's rim's, and how far that lens stands out of its
/// face.
const DRUM: f32 = 34.0;
const RIM: f32 = 31.0;
const LENS_OUT: f32 = 4.8;
/// The emitter's seat in the belly: the flat face round its well, and how far out the
/// fairing runs up into the hull; the crown's ring inside the well; the lens dome's radius
/// at its foot and at its face, and how far the face hangs under the drum.
const SEAT: f32 = 54.0;
const FAIRING: f32 = 78.0;
const CROWN: f32 = 47.0;
const DOME: f32 = 22.0;
const FACE: f32 = 7.0;
const DROP: f32 = 12.0;
/// The emitter's rings turn about the drum's axis through a point this far under the
/// drum (the spinner pivot): the toothed crown, the vaned race the other way, and two
/// tilted gimbal hoops crossing each other round the dome. Rates in rad/s with the lance
/// lit; each a whole number of `gpu_consts::orbit::RATE_STEP`s.
const PIVOT_DROP: f32 = 8.0;
const CROWN_RATE: f32 = 0.75;
const RACE_RATE: f32 = -1.25;
const GIMBAL_RATE: f32 = 2.0;

/// Where the drum stands: its middle along the hull (the ship's middle, so the beam
/// leaves the same point whichever way it is laid), and its lower and upper faces.
#[derive(Clone, Copy)]
pub(super) struct Core {
    pub(super) x: f32,
    pub(super) low: f32,
    pub(super) high: f32,
}

impl Core {
    /// The emitter's lens face: where the beam leaves.
    #[cfg(test)]
    pub(super) fn lens(&self) -> Vec3 {
        v3(self.x, 0.0, self.low - DROP)
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
    aperture(b, v3(at.x, 0.0, at.high));
    let lens = emitter(b, body, v3(at.x, 0.0, at.low));
    b.set_beam_core(lens, RIM * 0.39);
}

/// The upper aperture on the face at `c`, looking up.
fn aperture(b: &mut MeshBuilder, c: Vec3) {
    let r = RIM;
    let sides = b.sides(28);
    dark_plate(b);
    hoop(b, c + Vec3::Z, r, r * 0.12, 2.0, sides);
    // The recess behind the plates, dark.
    seam(b);
    b.prism(c - Vec3::Z * 0.1, sides, r * 0.94, r * 0.94, 0.2);
    if !b.coarse() {
        // The bright ring inside the rim.
        b.paint(GLOW_LASER);
        hoop(b, c + Vec3::Z * 0.75, r * 0.86, r * 0.045, 0.75, sides);
        // Iris plates, overlapping, each a step further out toward the lens.
        dark_plate(b);
        let n = if b.fine() { 16 } else { 8 };
        for i in 0..n {
            let a = TAU * i as f32 / n as f32;
            let at = |turn: f32, k: f32, lift: f32| {
                (Vec2::from_angle(a + turn) * r * k).extend(0.0) + c + Vec3::Z * lift
            };
            slab(
                b,
                [
                    at(0.0, 0.8, 1.2),
                    at(0.42, 0.8, 1.2),
                    at(0.62, 0.48, 2.8),
                    at(0.3, 0.48, 2.8),
                ],
                Vec3::Z * 0.9,
            );
        }
    }
    metal(b);
    hoop(b, c + Vec3::Z * 3.5, r * 0.45, r * 0.08, 2.5, sides);
    b.paint(GLOW_PRISM);
    b.cylinder_between(c, c + Vec3::Z * LENS_OUT, r * 0.41, r * 0.39, sides);
}

/// The emitter set into the belly at the drum's lower face `c`: its seat faired into the
/// hull (`fairing`), the lens dome hanging from it, ribbed, and round the dome the rings that turn as the lance
/// charges (orbit pieces, driven by the charge: `Model::beam_core`). Returns the lens's
/// face.
fn emitter(b: &mut MeshBuilder, body: &Body, c: Vec3) -> Vec3 {
    let sides = b.sides(36);
    let down = -Vec3::Z;
    fairing(b, body, c);
    let pivot = c + down * PIVOT_DROP;
    b.set_spinner_pivot(pivot);
    b.with_orbit(Vec3::Z, CROWN_RATE, |b| crown(b, c));
    b.with_orbit(Vec3::Z, RACE_RATE, |b| race(b, c));
    if !b.coarse() {
        // Two hoops tilted across each other round the dome, turning opposite ways, so
        // they cross and recross as it charges.
        for (tilt, rate) in [(0.42f32, GIMBAL_RATE), (-0.42, -GIMBAL_RATE)] {
            b.with_orbit(Vec3::Z, rate, |b| gimbal(b, pivot, tilt));
        }
    }
    // The lens dome, from its foot under the flange to its flat face: the face is the lens.
    metal(b);
    hoop(b, c + down * 2.6, DOME + 1.4, 3.0, 2.4, sides);
    let profile = [
        (DOME, 2.0),
        (DOME * 0.92, DROP * 0.45),
        (DOME * 0.7, DROP * 0.75),
        (FACE, DROP),
    ];
    b.paint(GLOW_PRISM);
    let rings: Vec<Vec<Vec3>> = profile
        .iter()
        .map(|&(r, z)| {
            (0..sides)
                .map(|i| {
                    let a = TAU * (i as f32 + 0.5) / sides as f32;
                    c + (Vec2::from_angle(a) * r).extend(-z)
                })
                .collect()
        })
        .collect();
    b.loft(&rings, false, true);
    if !b.coarse() {
        // Ribs down the dome, the lens's light between them.
        metal(b);
        let ribs = if b.fine() { 12 } else { 6 };
        for i in 0..ribs {
            let a = TAU * i as f32 / ribs as f32;
            let d = Vec2::from_angle(a).extend(0.0);
            let side = v3(-a.sin(), a.cos(), 0.0) * 0.7;
            for pair in profile.windows(2) {
                let (r0, z0) = pair[0];
                let (r1, z1) = pair[1];
                let p0 = c + d * (r0 + 0.2) + down * z0;
                let p1 = c + d * (r1 + 0.2) + down * z1;
                slab(b, [p0 - side, p0 + side, p1 + side, p1 - side], d * 0.9);
            }
        }
    }
    c + down * DROP
}

/// The emitter's seat in the belly, its face at `c` a little under the keel: a flat ring
/// of plate round the dark well the rings turn in, faired out and up into the hull's
/// underside all round, its outer edge laid along the keel flat and up the bilge, so the
/// emitter is set into the hull, not hung under it.
fn fairing(b: &mut MeshBuilder, body: &Body, c: Vec3) {
    let sides = b.sides(36);
    // The hull's underside over a point, and a little into it, so the edge is buried.
    let under = |p: Vec2| {
        let u = body.u_down(p.x, p.y.abs());
        body.point(p.x, u).z + 1.0
    };
    let ring = |r: f32, z: Option<f32>| -> Vec<Vec3> {
        (0..sides)
            .map(|i| {
                let a = TAU * (i as f32 + 0.5) / sides as f32;
                let p = c.truncate() + Vec2::from_angle(a) * r;
                p.extend(z.unwrap_or_else(|| under(p)))
            })
            .collect()
    };
    let face = c.z - 1.0;
    // The face, then the fairing rising out and up, steepest at first, into the hull.
    let rise = |k: f32| {
        let r = SEAT + (FAIRING - SEAT) * k;
        let lift = 1.0 - (1.0 - k) * (1.0 - k);
        ring(r, None)
            .into_iter()
            .map(|p| p.truncate().extend(face + (p.z - face) * lift))
            .collect::<Vec<_>>()
    };
    dark_plate(b);
    let steps = if b.coarse() { 1 } else { 4 };
    let mut rings = vec![ring(SEAT, Some(face))];
    rings.extend((1..=steps).map(|k| rise(k as f32 / steps as f32)));
    b.with_facets(|b| b.loft(&rings, false, false));
    // The flat face of the seat round the well, and the dark well the rings turn in.
    b.prism(c - Vec3::Z * 1.4, sides, SEAT, SEAT, 0.4);
    seam(b);
    b.prism(c - Vec3::Z * 1.6, sides, SEAT - 4.0, SEAT - 4.0, 0.4);
    if !b.coarse() {
        // A lip round the well, and a seam where the fairing leaves the face.
        hoop(b, c - Vec3::Z * 1.9, SEAT - 4.0, 1.6, 1.2, sides);
        hoop(b, c - Vec3::Z * 1.2, SEAT + 0.6, 1.2, 0.8, sides);
    }
}

/// The outer crown: a ring with teeth hanging from it, long and short by turns.
fn crown(b: &mut MeshBuilder, c: Vec3) {
    let sides = b.sides(36);
    let r = CROWN;
    let top = c - Vec3::Z * 3.0;
    dark_plate(b);
    hoop(b, top - Vec3::Z * 1.5, r, 6.0, 3.0, sides);
    if b.coarse() {
        return;
    }
    let teeth = if b.fine() { 32 } else { 16 };
    let half = TAU / teeth as f32 * 0.3;
    for i in 0..teeth {
        let a = TAU * (i as f32 + 0.5) / teeth as f32;
        let long = if i % 2 == 0 { 9.0 } else { 5.5 };
        let at = |turn: f32, rr: f32, z: f32| {
            (Vec2::from_angle(a + turn) * rr).extend(0.0) + top - Vec3::Z * z
        };
        if i % 2 == 0 {
            seam(b);
        } else {
            dark_plate(b);
        }
        slab(
            b,
            [
                at(-half, r + 2.0, 3.0),
                at(half, r + 2.0, 3.0),
                at(half * 0.25, r - 1.0, 3.0 + long),
                at(-half * 0.25, r - 1.0, 3.0 + long),
            ],
            -Vec2::from_angle(a).extend(0.0) * 1.4,
        );
    }
}

/// The race inside the crown, turning the other way: a deep ring with vanes standing
/// across it.
fn race(b: &mut MeshBuilder, c: Vec3) {
    let sides = b.sides(36);
    let r = DOME + 12.0;
    let mid = c - Vec3::Z * 5.0;
    metal(b);
    hoop(b, mid, r, 4.0, 4.0, sides);
    if b.coarse() {
        return;
    }
    dark_plate(b);
    let vanes = if b.fine() { 16 } else { 8 };
    for i in 0..vanes {
        let a = TAU * i as f32 / vanes as f32;
        let d = Vec2::from_angle(a).extend(0.0);
        let along = v3(-a.sin(), a.cos(), 0.0);
        let p = mid + d * r;
        slab(
            b,
            [
                p - d * 5.0 + Vec3::Z * 1.5,
                p + d * 5.0 + Vec3::Z * 1.5,
                p + d * 4.0 - Vec3::Z * 4.5,
                p - d * 4.0 - Vec3::Z * 4.5,
            ],
            along,
        );
    }
}

/// A gimbal hoop round the dome through `pivot`, tilted `tilt` radians off the level
/// about the y axis, with knuckles where it would bear on a frame.
fn gimbal(b: &mut MeshBuilder, pivot: Vec3, tilt: f32) {
    let sides = b.sides(32);
    let axis = v3(tilt.sin(), 0.0, tilt.cos());
    let r = DOME + 4.0;
    metal(b);
    hoop_on(b, pivot, axis, r, 1.6, 1.8, sides);
    let e1 = axis.cross(Vec3::Y).normalize();
    let e2 = axis.cross(e1);
    dark_plate(b);
    for k in 0..4 {
        let a = TAU * k as f32 / 4.0 + 0.4;
        let p = pivot + (e1 * a.cos() + e2 * a.sin()) * r;
        b.cylinder_between(p - axis * 1.6, p + axis * 1.6, 1.8, 1.8, 6);
    }
}
