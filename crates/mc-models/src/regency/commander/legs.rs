//! The legs: a heavy thigh tapering to the knee, a cuisse over its outside swept up past
//! the hip; an armoured knee guard thrust up and forward; a shin that flares into a great
//! greave over the ankle, a blade swept back off the calf; an armoured boot. Bronze rams
//! work in the gaps behind the armour. Every piece rides its bone (`rig::THIGH`, `SHIN`,
//! `FOOT`) and nothing reaches across a joint.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::{part, rig};

use super::super::kit::{dark_plate, metal, seam, v3};
use super::form::ram;
use super::form::{ball, blade, ring, sleeve, KEEL, OCT};
use super::{ANKLE, HIP, KNEE};

/// The left leg (the right is its mirror).
pub(super) fn leg(b: &mut MeshBuilder) {
    b.with_part(part::LOCOMOTION, |b| {
        b.with_limb(rig::THIGH, thigh);
        b.with_limb(rig::SHIN, shin);
        b.with_limb(rig::FOOT, foot);
    });
}

/// A bone from `a` to `c` in the leg's plane: its direction, and the normal out of its
/// front (forward).
fn frame(a: Vec3, c: Vec3) -> (Vec3, Vec3) {
    let u = (c - a).normalize();
    let n = Vec3::new(-u.z, 0.0, u.x);
    (u, if n.x < 0.0 { -n } else { n })
}

fn thigh(b: &mut MeshBuilder) {
    let (u, n) = frame(HIP, KNEE);
    let at = |t: f32, fwd: f32| HIP.lerp(KNEE, t) + n * fwd;
    // Rams behind, in the gap between the armour and the hip.
    for dy in [-0.45f32, 0.45] {
        ram(
            b,
            at(0.12, -0.85) + Vec3::Y * dy,
            at(0.88, -0.6) + Vec3::Y * dy,
            0.2,
        );
    }
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(at(0.0, 0.1), n, 1.05, 1.05),
            ring(at(0.2, 0.25), n, 1.32, 1.42),
            ring(at(0.5, 0.3), n, 1.22, 1.36),
            ring(at(0.9, 0.2), n, 0.85, 0.98),
        ],
        &KEEL,
    );
    // The cuisse down the outside, its point swept up and back past the hip.
    blade(
        b,
        at(0.72, -0.1) + Vec3::Y * 1.22,
        -u - n * 0.3,
        Vec3::Y,
        4.1,
        0.95,
        -0.6,
        0.3,
    );
    // The front plate, lapped over the cuisse's edge, its point up at the hip.
    blade(b, at(0.8, 1.42), -u + n * 0.08, n, 3.6, 0.95, 0.0, 0.34);
}

fn shin(b: &mut MeshBuilder) {
    let (u, n) = frame(KNEE, ANKLE);
    let at = |t: f32, fwd: f32| KNEE.lerp(ANKLE, t) + n * fwd;
    // The knee: a bronze hub, mostly behind the guard.
    metal(b);
    b.cylinder_between(
        KNEE - Vec3::Y * 0.75,
        KNEE + Vec3::Y * 0.75,
        0.62,
        0.62,
        b.sides(10),
    );
    ram(b, at(0.1, -0.75), at(0.62, -1.05), 0.22);
    dark_plate(b);
    // The shin, flaring into the greave: widest just above the ankle, the calf behind.
    sleeve(
        b,
        &[
            ring(at(0.1, 0.1), n, 0.95, 0.95),
            ring(at(0.4, 0.05), n, 1.08, 1.12),
            ring(at(0.72, -0.15), n, 1.38, 1.52),
            ring(at(0.93, -0.2), n, 1.48, 1.65),
            ring(at(1.02, -0.1), n, 1.28, 1.35),
        ],
        &OCT,
    );
    // The knee cap: a keeled wedge round the front of the knee, thrust up and forward
    // into a point, its sides wrapped back over the drum.
    sleeve(
        b,
        &[
            ring(at(0.28, 0.75), n, 1.12, 0.62),
            ring(at(0.08, 1.05), n, 1.18, 0.8),
            ring(at(-0.12, 1.28), n, 0.88, 0.56),
            ring(at(-0.22, 1.8), n, 0.08, 0.08),
        ],
        &KEEL,
    );
    for side in [1.0f32, -1.0] {
        blade(
            b,
            at(0.2, 0.7) + Vec3::Y * (side * 1.15),
            -u - n * 0.9,
            Vec3::Y * side,
            1.3,
            0.55,
            0.3,
            0.2,
        );
    }
    // The greave, down the front of the flare, its point up the shin under the guard.
    blade(b, at(0.98, 1.5), -u + n * 0.12, n, 3.3, 1.25, 0.0, 0.38);
    // Down the outside, swept up and back.
    blade(
        b,
        at(0.95, -0.2) + Vec3::Y * 1.45,
        -u - n * 0.1,
        Vec3::Y,
        2.8,
        1.0,
        -0.5,
        0.26,
    );
    if !b.fine() {
        return;
    }
    // A blade swept back and up off the calf.
    blade(
        b,
        at(0.78, -1.62),
        -n * 0.75 - u * 0.65,
        Vec3::Y,
        2.6,
        0.7,
        0.5,
        0.28,
    );
}

/// The foot: an armoured boot, no toes. A heavy faceted body from the heel to a blunt
/// wedged toe, a toe cap laid back up the instep, a plate down each side swept back, a
/// heel block.
fn foot(b: &mut MeshBuilder) {
    let (x, y) = (ANKLE.x, ANKLE.y);
    metal(b);
    ball(b, ANKLE, 0.85);
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(x - 1.5, y, 0.8), Vec3::Z, 1.0, 0.75),
            ring(v3(x + 0.2, y, 1.0), Vec3::Z, 1.3, 0.98),
            ring(v3(x + 1.9, y, 0.72), Vec3::Z, 1.25, 0.66),
            ring(v3(x + 3.1, y, 0.42), Vec3::Z, 0.85, 0.38),
        ],
        &OCT,
    );
    // The toe cap, from the toe's point laid back up the instep toward the greave.
    blade(
        b,
        v3(x + 3.15, y, 0.62),
        Vec3::new(-1.0, 0.0, 0.42),
        Vec3::new(0.38, 0.0, 1.0),
        3.0,
        1.0,
        0.0,
        0.34,
    );
    for side in [1.0f32, -1.0] {
        blade(
            b,
            v3(x + 2.7, y + side * 1.2, 0.5),
            Vec3::new(-1.0, 0.0, 0.22),
            Vec3::new(0.0, side, 0.2),
            3.8,
            0.42,
            0.6,
            0.24,
        );
    }
    seam(b);
    b.chamfered_box(v3(x - 1.55, y, 0.55), v3(1.0, 1.9, 1.1), 0.25);
}
