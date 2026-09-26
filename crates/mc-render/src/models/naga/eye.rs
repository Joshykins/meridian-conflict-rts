//! The Eye, the Naga radar, on its 2 x 2 lot (24 m square, cut to an octagon): a sensor
//! mast.
//!
//! - Four plated legs on the diagonals brace a ribbed bronze mast, each leg's plates
//!   lapped down into a spike, a foot block where it meets the lot.
//! - The mast climbs in three armoured sleeves, each a ring of plates lapped down over
//!   the bronze, the bronze showing in the gaps between them, to a bearing ring.
//! - The sensor head on the bearing looks about (`part::SPINNER` with
//!   `set_spinner_scan`: it swings one way, dwells and looks back, never round and
//!   round): a swept armoured wedge pointed the way it looks, a bank of red optics across
//!   its face, a flat radar vane on a bronze frame over it, plates lapped back off its
//!   flanks into spikes, the owner's colour on its roof.

use glam::{Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::part;

use super::kit::{dark_plate, metal, seam, v3};
use super::machine::*;

/// Where the head turns (the top of the bearing).
const SPIN: Vec3 = Vec3::new(0.0, 0.0, 18.4);
/// The mast: its foot and top, and its radius.
const MAST: (f32, f32, f32) = (1.0, 18.0, 0.8);
/// The legs' bearings and how far out their feet stand.
const LEGS: [f32; 4] = [45.0, 135.0, 225.0, 315.0];
const FOOT: f32 = 8.4;
const THICK: f32 = 0.3;

pub(super) fn eye(b: &mut MeshBuilder, _tech: u8) {
    b.set_spinner_pivot(SPIN);
    // It looks about, slowly, one way and back, not round and round.
    b.set_spinner_scan();
    if b.coarse() {
        coarse(b);
        return;
    }
    mast(b);
    for deg in LEGS {
        b.yawed(Vec3::ZERO, deg.to_radians(), leg);
    }
    b.with_part(part::SPINNER, head);
}

/// Far off: the legs as plates, the mast, the head as a wedge, its optics, the owner's
/// colour.
fn coarse(b: &mut MeshBuilder) {
    for deg in LEGS {
        b.yawed(Vec3::ZERO, deg.to_radians(), |b| {
            dark_plate(b);
            b.face(&[
                v3(0.8, -1.0, 7.0),
                v3(FOOT + 1.0, 0.0, 0.3),
                v3(0.8, 1.0, 7.0),
            ]);
            b.face(&[
                v3(0.8, 1.0, 7.0),
                v3(FOOT + 1.0, 0.0, 0.3),
                v3(0.8, -1.0, 7.0),
            ]);
        });
    }
    dark_plate(b);
    b.prism(Vec3::ZERO, 4, 2.0, 1.1, SPIN.z);
    b.with_part(part::SPINNER, |b| {
        dark_plate(b);
        b.beam(
            v3(-3.4, 0.0, SPIN.z + 1.6),
            v3(3.2, 0.0, SPIN.z + 1.6),
            Vec2::new(3.8, 2.6),
            Vec2::new(1.4, 1.8),
        );
        b.paint(GLOW_LASER);
        b.face(&[
            v3(3.25, -0.5, SPIN.z + 1.2),
            v3(3.25, 0.5, SPIN.z + 1.2),
            v3(3.25, 0.5, SPIN.z + 2.0),
            v3(3.25, -0.5, SPIN.z + 2.0),
        ]);
        b.paint(TEAM);
        b.face(&[
            v3(-2.6, -0.8, SPIN.z + 2.92),
            v3(-0.8, -0.8, SPIN.z + 2.92),
            v3(-0.8, 0.8, SPIN.z + 2.92),
            v3(-2.6, 0.8, SPIN.z + 2.92),
        ]);
    });
}

/// The mast: its footing, the ribbed bronze column, three sleeves of plates lapped down
/// over it, the bearing ring at the top.
fn mast(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (z0, z1, r) = MAST;
    seam(b);
    b.prism(Vec3::ZERO, 8, 2.6, 2.2, z0 + 1.0);
    ribbed(b, Vec3::Z * z0, Vec3::Z * z1, r, if fine { 8 } else { 0 });
    // The sleeves: four plates round the mast at each, lapped down.
    for (k, &z) in [7.0f32, 11.6, 16.0].iter().enumerate() {
        let spread = 1.25 - 0.12 * k as f32;
        for q in 0..4 {
            let a = (45.0 + 90.0 * q as f32).to_radians();
            let d = v3(a.cos(), a.sin(), 0.0);
            let f = Frame::new(
                d * (r * spread) + Vec3::Z * z,
                d * 0.28 - Vec3::Z,
                d + Vec3::Z * 0.2,
            );
            dark_plate(b);
            Course {
                count: if fine { 2 } else { 1 },
                step: 1.2,
                len: 2.2,
                half: 0.75,
                tip: 0.0,
                thick: THICK,
                tail: 0.8,
            }
            .lay(b, &f);
        }
    }
    metal(b);
    hoop(
        b,
        Vec3::Z * (z1 + 0.1),
        r + 0.5,
        1.0,
        0.7,
        if fine { 16 } else { 8 },
    );
}

/// One leg, standing out along +x (turned into place by the caller): a ribbed bronze
/// strut from the mast down to a foot block, plates lapped down it into a spike.
fn leg(b: &mut MeshBuilder) {
    let fine = b.fine();
    let root = v3(MAST.2 + 0.1, 0.0, 7.6);
    let foot = v3(FOOT - 0.8, 0.0, 1.0);
    ribbed(b, root, foot, 0.42, if fine { 3 } else { 0 });
    let down = (foot - root).normalize();
    let out = v3(-down.z, 0.0, down.x);
    let f = Frame::new(root + out * 0.55, down, out);
    dark_plate(b);
    Course {
        count: 3,
        step: 2.4,
        len: 3.6,
        half: 0.95,
        tip: 0.0,
        thick: THICK,
        tail: 1.0,
    }
    .lay(b, &f);
    dark_plate(b);
    b.block(v3(FOOT - 2.0, -1.1, 0.0), v3(FOOT, 1.1, 1.3));
    if fine {
        red_slot(b, v3(FOOT + 0.02, 0.0, 0.8), Vec3::X, Vec3::Y, 1.2, 0.16);
    }
}

/// The sensor head (it turns about the bearing): a swept armoured wedge pointed the way
/// it looks, a bank of red optics across its face, the radar vane over it on a bronze
/// frame, plates lapped back off its flanks, the owner's colour on its roof.
fn head(b: &mut MeshBuilder) {
    let fine = b.fine();
    let z = SPIN.z;
    dark_plate(b);
    let ring = |h: f32, k: f32| -> Vec<Vec3> {
        vec![
            v3(3.4 * k, -0.9 * k, h),
            v3(3.4 * k, 0.9 * k, h),
            v3(-0.4, 2.0 * k, h),
            v3(-3.4, 1.2 * k, h),
            v3(-3.4, -1.2 * k, h),
            v3(-0.4, -2.0 * k, h),
        ]
    };
    b.loft(
        &[ring(z + 0.2, 0.8), ring(z + 1.1, 1.0), ring(z + 2.6, 0.82)],
        true,
        true,
    );
    // The optics: a bank of red slots across its face.
    let rows: &[f32] = if fine { &[1.0, 1.6] } else { &[1.3] };
    for &h in rows {
        red_slot(b, v3(3.25, 0.0, z + h), Vec3::X, Vec3::Y, 1.3, 0.2);
    }
    // Plates lapped back off its flanks into spikes.
    b.mirror_y(|b| {
        let f = Frame::new(
            v3(1.4, 1.6, z + 1.9),
            v3(-1.0, 0.35, -0.1),
            v3(0.0, 1.0, 0.5),
        );
        dark_plate(b);
        Course {
            count: 2,
            step: 1.8,
            len: 2.8,
            half: 0.8,
            tip: -1.0,
            thick: THICK,
            tail: 1.4,
        }
        .lay(b, &f);
    });
    b.paint(TEAM);
    b.face(&[
        v3(-2.6, -0.8, z + 2.62),
        v3(-0.8, -0.8, z + 2.62),
        v3(-0.8, 0.8, z + 2.62),
        v3(-2.6, 0.8, z + 2.62),
    ]);
    // The radar vane: a flat plated panel on a bronze frame over the head.
    metal(b);
    for x in [-1.8f32, 1.0] {
        b.cylinder_between(v3(x, 0.0, z + 2.5), v3(x, 0.0, z + 3.6), 0.18, 0.18, 6);
    }
    b.beam(
        v3(-2.4, 0.0, z + 3.6),
        v3(1.6, 0.0, z + 3.6),
        Vec2::new(0.3, 0.3),
        Vec2::new(0.3, 0.3),
    );
    dark_plate(b);
    b.beam(
        v3(-0.4, -4.2, z + 3.95),
        v3(-0.4, 4.2, z + 3.95),
        Vec2::new(1.6, 0.35),
        Vec2::new(1.6, 0.35),
    );
}

#[cfg(test)]
mod tests {
    use crate::models::{build_model_scaled, part};

    #[test]
    fn eye_fits() {
        super::super::check("naga_eye", 7.0, 24.0, Some(2), &[]);
        let model = build_model_scaled("naga_eye", 7.0, 24.0, 1).unwrap();
        assert!(model.spinner_scans, "the head should look about");
        assert!(model.lods[0]
            .vertices
            .iter()
            .any(|v| v.part == part::SPINNER));
    }
}
