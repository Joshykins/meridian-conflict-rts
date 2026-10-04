//! The Strider's Pinch-fusion Cannons, each drawn in its own frame: the origin on the
//! trunnion level with the bore, +x down the bore, the middle of its charge at
//! `(LEN, 0, 0)`. The fusion is housed in plated machinery; its light is inlaid lines of
//! the prism (`GLOW_PRISM` under a `pattern::COIL` stage), lighting from the breech
//! through the charge, as the Kiln's and the Sunspear's do. No exposed core.
//!
//! The gun: an armoured containment drum at the breech, slits of light over and under it,
//! and a deep plated rail either side of a short bore, reaching past it to hold the charge
//! between their lit faces. It holds still through the charge: charge gear parts along the
//! model's y from its middle (`charge_gear_pose`), and these stand off it.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::pattern;

use super::super::commander::form::{ring, sleeve, OCT};
use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::hoop_on;

/// The trunnion to the middle of the charge.
pub(super) const LEN: f32 = 8.8;

/// A `GLOW_PRISM` brush lit with the charge at `stage` (0 at the breech, 7 at the mouth).
fn coil_light(b: &mut MeshBuilder, stage: u32) {
    b.paint(GLOW_PRISM).pattern(pattern::COIL + stage);
}

/// A lit line let into a face: `len` along `along`, `h` across, just proud of the face
/// whose outward normal is `out`, lit at `stage`.
fn fusion_line(
    b: &mut MeshBuilder,
    at: Vec3,
    out: Vec3,
    along: Vec3,
    len: f32,
    h: f32,
    stage: u32,
) {
    let (out, along) = (out.normalize(), along.normalize());
    let across = out.cross(along) * (h * 0.5);
    let half = along * (len * 0.5);
    let quad = [
        at - half - across,
        at + half - across,
        at + half + across,
        at - half + across,
    ];
    coil_light(b, stage);
    b.loft(
        &[
            quad.to_vec(),
            quad.iter().map(|&p| p + out * 0.04).collect(),
        ],
        false,
        true,
    );
}

/// The breech: a plated block round the trunnion, its back cut
/// away, a seam-dark band round its middle.
fn breech(b: &mut MeshBuilder, half: f32, high: f32, front: f32) {
    dark_plate(b);
    b.with_facets(|b| {
        b.beam(
            v3(-1.2, 0.0, 0.0),
            v3(front, 0.0, 0.0),
            Vec2::new(half * 1.6, high * 1.7),
            Vec2::new(half * 2.0, high * 2.0),
        );
    });
    if b.fine() {
        seam(b);
        b.beam(
            v3(0.2, 0.0, 0.0),
            v3(0.45, 0.0, 0.0),
            Vec2::new(half * 2.06, high * 2.06),
            Vec2::new(half * 2.06, high * 2.06),
        );
    }
}

/// An armoured containment drum at the breech, slits of fusion light over and under it, a
/// short bore, and a deep plated rail either side of it reaching past its mouth to hold
/// the charge between their lit faces.
pub(super) fn cannon(b: &mut MeshBuilder) {
    breech(b, 0.8, 0.6, 1.4);
    let fine = b.fine();
    // The drum: a plated octagon across the gun behind the bore.
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(1.2, 0.0, 0.0), Vec3::Z, 0.8, 0.7),
            ring(v3(1.6, 0.0, 0.0), Vec3::Z, 1.0, 0.85),
            ring(v3(3.4, 0.0, 0.0), Vec3::Z, 1.0, 0.85),
            ring(v3(3.8, 0.0, 0.0), Vec3::Z, 0.7, 0.6),
        ],
        &OCT,
    );
    if fine {
        for (k, z) in [0.3f32, -0.3].into_iter().enumerate() {
            fusion_line(
                b,
                v3(2.5, 0.0, z + 0.56 * z.signum()),
                Vec3::Z * z.signum(),
                Vec3::X,
                1.4,
                0.12,
                k as u32 * 2,
            );
        }
    }
    b.with_recoil(|b| {
        metal(b);
        b.cylinder_between(v3(3.6, 0.0, 0.0), v3(6.4, 0.0, 0.0), 0.36, 0.32, b.sides(8));
        coil_light(b, 4);
        hoop_on(b, v3(6.4, 0.0, 0.0), Vec3::X, 0.26, 0.1, 0.06, b.sides(8));
    });
    // The rails either side of the bore, their lit faces toward it.
    b.mirror_y(|b| {
        let y = 1.1;
        dark_plate(b);
        let ring = |x: f32, depth: f32, h: f32| {
            vec![
                v3(x, y, -h),
                v3(x, y + depth, -h * 0.7),
                v3(x, y + depth, h * 0.7),
                v3(x, y, h),
            ]
        };
        let rings = [
            ring(3.0, 0.8, 0.7),
            ring(6.0, 0.6, 0.6),
            ring(9.2, 0.3, 0.36),
        ];
        b.with_facets(|b| b.loft(&rings, true, true));
        for k in 0..3u32 {
            let x = 4.2 + k as f32 * 1.6;
            fusion_line(
                b,
                v3(x, y - 0.01, 0.0),
                -Vec3::Y,
                Vec3::X,
                1.3,
                0.3,
                3 + k * 2,
            );
        }
    });
}
