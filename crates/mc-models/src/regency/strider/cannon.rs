//! The Strider's Pinch-fusion Cannons, each drawn in its own frame: the origin on the
//! trunnion level with the bore, +x down the bore, the middle of its charge at
//! `(LEN, 0, 0)`. The fusion is housed in plated machinery; its light is inlaid lines of
//! the prism (`GLOW_PRISM` under a `pattern::COIL` stage), lighting from the breech
//! through the charge, as the Kiln's and the Sunspear's do. No exposed core.
//!
//! Three designs to pick from (the model's `~` keys):
//!
//! - [`Design::Split`]: the bore split down its length into two plated halves, fusion
//!   light down each inner face, a gravity lens at each tip either side of the charge.
//! - [`Design::Collars`]: a heavy keeled barrel through three plated pinch collars, each
//!   ringed with a line of fusion light, and four short claws out past the mouth round
//!   the charge.
//! - [`Design::Rails`]: an armoured containment drum at the breech, slits of light over
//!   and under it, and a deep plated rail either side of a short bore, reaching past it to
//!   hold the charge between their lit faces.
//!
//! The guns hold still through the charge: charge gear parts along the model's y from its
//! middle (`charge_gear_pose`), and these stand off it.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::pattern;

use super::super::commander::form::{ring, sleeve, KEEL, OCT};
use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::{collar, hoop_on};

/// The trunnion to the middle of the charge.
pub(super) const LEN: f32 = 8.8;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Design {
    Split,
    Collars,
    Rails,
}

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

/// The breech every design shares: a plated block round the trunnion, its back cut
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

pub(super) fn cannon(b: &mut MeshBuilder, design: Design) {
    match design {
        Design::Split => split(b),
        Design::Collars => collars(b),
        Design::Rails => rails(b),
    }
}

/// The bore split into two plated halves (+-y), a fusion
/// strip down each inner face lighting from the breech, a gravity lens at each tip.
fn split(b: &mut MeshBuilder) {
    breech(b, 0.85, 0.75, 2.4);
    // The bore's core between the halves, kicking back on the shot.
    b.with_recoil(|b| {
        metal(b);
        b.cylinder_between(v3(2.2, 0.0, 0.0), v3(6.6, 0.0, 0.0), 0.24, 0.22, b.sides(8));
    });
    let fine = b.fine();
    // A half's section out from its flat inner face (y out, z up).
    let shape: [[f32; 2]; 6] = [
        [0.0, 0.62],
        [0.36, 0.56],
        [0.62, 0.24],
        [0.62, -0.24],
        [0.36, -0.56],
        [0.0, -0.62],
    ];
    let stations = [(2.2, 1.0), (4.0, 1.05), (6.6, 0.9), (7.6, 0.7)];
    b.mirror_y(|b| {
        {
            let rings: Vec<Vec<Vec3>> = stations
                .iter()
                .map(|&(x, s)| {
                    shape
                        .iter()
                        .map(|&[y, z]| v3(x, 0.3 + y * s, z * s))
                        .collect()
                })
                .collect();
            dark_plate(b);
            b.with_facets(|b| b.loft(&rings, true, true));
            if fine {
                // A plate course lapped along the half's top.
                dark_plate(b);
                b.beam(
                    v3(2.6, 0.55, 0.58),
                    v3(6.8, 0.5, 0.5),
                    Vec2::new(0.5, 0.14),
                    Vec2::new(0.36, 0.12),
                );
            }
            // The inner face's fusion strip, lit in four stages from the breech.
            for k in 0..4u32 {
                let x = 2.7 + k as f32 * 1.2;
                fusion_line(
                    b,
                    v3(x, 0.29, 0.0),
                    -Vec3::Y,
                    Vec3::X,
                    1.05,
                    0.22,
                    1 + k * 2,
                );
            }
            // The gravity lens on the tip: a plated cap swept in toward the charge, a
            // lens of light on its inner face.
            dark_plate(b);
            b.beam(
                v3(7.4, 0.95, 0.0),
                v3(8.2, 1.25, 0.0),
                Vec2::new(0.9, 1.0),
                Vec2::new(0.5, 0.7),
            );
            fusion_line(b, v3(7.8, 0.78, 0.0), -Vec3::Y, Vec3::X, 0.6, 0.5, 7);
        }
    });
}

/// A heavy keeled barrel through three plated pinch collars, each ringed with a line of
/// fusion light, and four short claws out past the mouth, round the charge.
fn collars(b: &mut MeshBuilder) {
    breech(b, 0.9, 0.8, 2.2);
    let fine = b.fine();
    b.with_recoil(|b| {
        dark_plate(b);
        sleeve(
            b,
            &[
                ring(v3(2.0, 0.0, 0.0), Vec3::Z, 0.6, 0.58),
                ring(v3(7.0, 0.0, 0.0), Vec3::Z, 0.48, 0.46),
            ],
            &KEEL,
        );
        for (k, x) in [3.0f32, 4.5, 6.0].into_iter().enumerate() {
            dark_plate(b);
            sleeve(
                b,
                &[
                    ring(v3(x - 0.4, 0.0, 0.0), Vec3::Z, 0.76, 0.76),
                    ring(v3(x - 0.25, 0.0, 0.0), Vec3::Z, 0.9, 0.9),
                    ring(v3(x + 0.25, 0.0, 0.0), Vec3::Z, 0.9, 0.9),
                    ring(v3(x + 0.4, 0.0, 0.0), Vec3::Z, 0.76, 0.76),
                ],
                &OCT,
            );
            coil_light(b, 1 + k as u32 * 2);
            hoop_on(b, v3(x, 0.0, 0.0), Vec3::X, 0.91, 0.08, 0.16, b.sides(8));
        }
        // The muzzle crown the claws stand on.
        collar(b, v3(7.2, 0.0, 0.0), Vec3::X, 0.62, 0.5);
    });
    // Four claws round the charge, square to the bore, each lit along its inner face.
    for a in [0.0f32, 90.0, 180.0, 270.0] {
        let (s, c) = (a + 45.0).to_radians().sin_cos();
        let out = v3(0.0, c, s);
        dark_plate(b);
        b.beam(
            v3(6.9, 0.0, 0.0) + out * 0.55,
            v3(9.2, 0.0, 0.0) + out * 1.15,
            Vec2::new(0.42, 0.36),
            Vec2::new(0.22, 0.26),
        );
        if fine {
            fusion_line(
                b,
                v3(8.2, 0.0, 0.0) + out * 0.76,
                -out,
                (v3(9.2, 0.0, 0.0) + out * 1.15 - v3(6.9, 0.0, 0.0) - out * 0.55).normalize(),
                1.4,
                0.12,
                7,
            );
        }
    }
}

/// An armoured containment drum at the breech, slits of fusion light over and under it, a
/// short bore, and a deep plated rail either side of it reaching past its mouth to hold
/// the charge between their lit faces.
fn rails(b: &mut MeshBuilder) {
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
