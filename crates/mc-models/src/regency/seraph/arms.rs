//! The arms: an upper arm hanging from the pauldron to the elbow, and the forearm laid
//! forward with a Rapid Pinch-fusion Repeater along its outside. The repeater's bore is
//! split between two rails whose inner faces burn red (`pattern::EMBER_RIM`); its charge
//! is pinched between them and loosed from the muzzle collar. The forearm and gun pitch
//! about the elbow together (`rig::ARM_GUN`).

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{pattern, rig};

use super::super::commander::form::{ball, blade, ring, sleeve, KEEL, OCT};
use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::{armour, collar, shaft, Frame};
use super::{ELBOW, MUZZLE, SHOULDER};

/// The left arm (the right is its mirror).
pub(super) fn arm(b: &mut MeshBuilder) {
    upper(b);
    b.with_limb(rig::ARM_GUN, |b| {
        forearm(b);
        repeater(b);
    });
}

/// The upper arm, from under the pauldron down to the elbow's drum.
fn upper(b: &mut MeshBuilder) {
    let (a, c) = (SHOULDER + v3(0.0, 0.25, -0.6), ELBOW + v3(0.0, 0.0, 0.5));
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(a, Vec3::X, 0.85, 0.88),
            ring(a.lerp(c, 0.45), Vec3::X, 0.98, 1.0),
            ring(c, Vec3::X, 0.75, 0.8),
        ],
        &OCT,
    );
    metal(b);
    b.cylinder_between(
        ELBOW - Vec3::Y * 0.62,
        ELBOW + Vec3::Y * 0.62,
        0.55,
        0.55,
        b.sides(10),
    );
}

/// The forearm: a plated vambrace from the elbow to a gauntleted fist under the gun.
fn forearm(b: &mut MeshBuilder) {
    let wrist = ELBOW + v3(2.6, 0.0, -0.1);
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(ELBOW + v3(0.2, 0.0, 0.0), Vec3::Z, 0.75, 0.75),
            ring(ELBOW + v3(1.2, 0.0, 0.0), Vec3::Z, 0.92, 0.86),
            ring(wrist, Vec3::Z, 0.68, 0.66),
        ],
        &OCT,
    );
    // The vambrace's guard down the inside, swept back to a point past the elbow.
    blade(
        b,
        wrist + v3(-0.2, -0.62, 0.05),
        v3(-1.0, 0.0, 0.05),
        -Vec3::Y,
        3.4,
        0.55,
        0.0,
        0.18,
    );
    // The fist round the gun's grip.
    metal(b);
    ball(b, wrist + v3(0.2, 0.0, 0.0), 0.42);
    dark_plate(b);
    b.chamfered_box(wrist + v3(0.75, 0.0, -0.1), v3(0.95, 0.85, 0.9), 0.2);
}

/// Where the repeater's bore runs: along x at the muzzle's height and offset.
fn bore(x: f32) -> Vec3 {
    v3(x, MUZZLE.y, MUZZLE.z)
}

/// The repeater: a deep receiver along the forearm, heat collars round its chamber, then
/// two long rails ahead of it either side of a recessed bore, closed by the muzzle collar.
fn repeater(b: &mut MeshBuilder) {
    // The receiver, keeled along its top, carried off the forearm on a trunnion.
    shaft(b, ELBOW + v3(1.0, 0.0, 0.0), bore(1.0), 0.22);
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(bore(-0.9), Vec3::Z, 0.42, 0.48),
            ring(bore(0.2), Vec3::Z, 0.55, 0.62),
            ring(bore(3.0), Vec3::Z, 0.52, 0.58),
            ring(bore(3.9), Vec3::Z, 0.36, 0.42),
        ],
        &KEEL,
    );
    // The capacitor slung under the receiver, its charge in its slotted plate.
    b.paint(PLATING_DARK).pattern(pattern::EMBER_SLOT);
    sleeve(
        b,
        &[
            ring(bore(0.3) - Vec3::Z * 0.68, -Vec3::Z, 0.34, 0.26),
            ring(bore(2.4) - Vec3::Z * 0.7, -Vec3::Z, 0.36, 0.28),
            ring(bore(3.1) - Vec3::Z * 0.55, -Vec3::Z, 0.08, 0.08),
        ],
        &OCT,
    );
    // Two lit strips down the receiver's flanks, either side of its keel: hanging at
    // its side, they run down the forearm's front.
    b.paint(PLATING_DARK).pattern(pattern::EMBER_SLOT);
    for side in [1.0f32, -1.0] {
        armour(
            b,
            &Frame::new(
                bore(0.15) + v3(0.0, 0.34 * side, 0.4),
                Vec3::X,
                v3(0.0, 0.7 * side, 1.0),
            ),
            &[
                [0.0, -0.09],
                [2.9, -0.09],
                [3.05, 0.0],
                [2.9, 0.09],
                [0.0, 0.09],
            ],
            0.08,
        );
    }
    // Heat collars round the chamber's breech, behind the strips.
    metal(b);
    for x in [-0.55f32, -0.15] {
        collar(b, bore(x), Vec3::X, 0.56, 0.16);
    }
    // The bore, recessed between the rails.
    seam(b);
    b.cylinder_between(bore(3.6), bore(6.95), 0.16, 0.14, b.sides(8));
    // The rails: a keeled blade over and under the bore, their inner faces lit.
    for up in [1.0f32, -1.0] {
        let n = Vec3::Z * up;
        b.paint(PLATING_DARK).pattern(pattern::EMBER_RIM);
        sleeve(
            b,
            &[
                ring(bore(3.5) + n * 0.42, n, 0.34, 0.16),
                ring(bore(5.2) + n * 0.36, n, 0.3, 0.16),
                ring(bore(7.0) + n * 0.3, n, 0.2, 0.12),
            ],
            &KEEL,
        );
        // A fin off each rail, swept back: the gun's silhouette from above.
        dark_plate(b);
        blade(
            b,
            bore(6.6) + n * 0.42,
            v3(-1.0, 0.0, 0.18 * up),
            n,
            2.2,
            0.18,
            1.0,
            0.08,
        );
    }
    // The muzzle collar, closing the rails round the bore's mouth.
    dark_plate(b);
    b.cylinder_between(bore(6.95), MUZZLE, 0.5, 0.42, b.sides(8));
    seam(b);
    b.cylinder_between(MUZZLE - Vec3::X * 0.02, MUZZLE, 0.3, 0.3, b.sides(8));
}
