//! The Saker's feet, level on the ground under each ankle: a faceted pad with the ankle
//! drum seated between two yoke plates on it, a spade raked up off its outer edge in two
//! lapped blades, and a plated toe either side of the spade.

use glam::Vec3;

use crate::builder::MeshBuilder;

use super::super::kit::dark_plate;
use super::super::machine::{armour, Frame};
use super::parts::ridged;

/// The foot under `ankle`: `out` the way the leg reaches, `across` the drum's axis.
pub(super) fn foot(b: &mut MeshBuilder, ankle: Vec3, out: Vec3, across: Vec3) {
    let ground = ankle.with_z(0.0);
    let f = Frame::new(ground, out, Vec3::Z);
    dark_plate(b);
    ridged(
        b,
        &f,
        &[
            [2.2, 0.0],
            [1.2, 1.8],
            [-1.4, 1.7],
            [-1.9, 0.0],
            [-1.4, -1.7],
            [1.2, -1.8],
        ],
        ankle.z - 0.25,
        [0.0, 0.0],
        0.3,
    );
    if !b.mid() {
        return;
    }
    // Yoke plates either side of the ankle drum.
    if b.fine() {
        for sign in [-1.0f32, 1.0] {
            let side = across * sign;
            dark_plate(b);
            ridged(
                b,
                &Frame::new(ankle.with_z(ankle.z - 0.4) + side * 1.6, Vec3::Z, side),
                &[
                    [-0.2, -0.9],
                    [-0.2, 0.9],
                    [0.9, 0.5],
                    [1.2, 0.0],
                    [0.9, -0.5],
                ],
                0.22,
                [0.4, 0.0],
                0.12,
            );
        }
    }
    spade(b, ground, out, 0.8);
    for turn in [-0.95f32, 0.95] {
        toe(b, ground, out * turn.cos() + across * turn.sin(), 1.3, 0.85);
    }
}

/// The spade: two blades lapped and raked up off the pad's outer edge, `s` their size.
fn spade(b: &mut MeshBuilder, ground: Vec3, out: Vec3, s: f32) {
    let rake = (out * 0.55 + Vec3::Z).normalize();
    dark_plate(b);
    armour(
        b,
        &Frame::new(ground + out * 1.4 + Vec3::Z * 0.3, rake, out),
        &[
            [0.0, -1.8 * s],
            [0.0, 1.8 * s],
            [1.6 * s, 1.5 * s],
            [2.5 * s, 0.0],
            [1.6 * s, -1.5 * s],
        ],
        0.35,
    );
    if b.fine() {
        ridged(
            b,
            &Frame::new(ground + out * 1.85 + Vec3::Z * 0.55, rake, out),
            &[
                [0.0, -1.3 * s],
                [0.0, 1.3 * s],
                [1.4 * s, 1.0 * s],
                [2.0 * s, 0.0],
                [1.4 * s, -s],
            ],
            0.22,
            [0.8 * s, 0.0],
            0.14,
        );
    }
}

/// A plated toe off the pad toward `d`, `from` out from the ankle, `s` its size.
fn toe(b: &mut MeshBuilder, ground: Vec3, d: Vec3, from: f32, s: f32) {
    dark_plate(b);
    ridged(
        b,
        &Frame::new(ground + d * from, d, Vec3::Z),
        &[
            [0.0, -0.55 * s],
            [0.0, 0.55 * s],
            [1.4 * s, 0.4 * s],
            [2.3 * s, 0.0],
            [1.4 * s, -0.4 * s],
        ],
        0.35 * s,
        [0.7 * s, 0.0],
        0.3 * s,
    );
}
