//! The Gorget's gun: an over-under pair in a tall breech, a gorget of lapped bands round
//! the breech's front, a spine blade between the barrels, heat-sink fins along each.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::gpu_consts::surface_mark as mark;

use super::super::super::kit::{dark_plate, metal, v3};
use super::super::super::machine::*;
use super::super::*;
use super::BARRELS;

/// One barrel at height `z` in the gun's frame from `x0` to its mouth at `len`: a jacket
/// stepping down, bronze collars round it, a brake at the mouth slotted either side.
fn barrel(b: &mut MeshBuilder, z: f32, x0: f32, len: f32, r: f32, collars: &[f32]) {
    let fine = b.fine();
    dark_plate(b);
    hull_x(
        b,
        &[
            [x0, r * 2.0, r * 1.9, z],
            [x0 + (len - x0) * 0.35, r * 1.7, r * 1.6, z],
            [len - 1.0, r * 1.25, r * 1.25, z],
        ],
        &CHAMFERED,
    );
    metal(b);
    for &x in collars {
        let s = b.sides(8);
        b.cylinder_between(
            v3(x - 0.12, 0.0, z),
            v3(x + 0.12, 0.0, z),
            r * 1.1,
            r * 1.1,
            s,
        );
    }
    // The brake.
    dark_plate(b);
    hull_x(
        b,
        &[
            [len - 1.05, r * 1.7, r * 1.45, z],
            [len, r * 1.55, r * 1.3, z],
        ],
        &CHAMFERED,
    );
    if fine {
        b.mark(
            v3(len - 0.9, -r * 0.9, z - r * 0.5),
            v3(len - 0.15, r * 0.9, z + r * 0.5),
            mark::GRILLE,
            0.0,
        );
    }
}

/// The breech the pair shares: a trunnion collar across and a tall keeled block, the
/// mantlet's face lapped over its front.
fn breech(b: &mut MeshBuilder) {
    let fine = b.fine();
    metal(b);
    collar(b, Vec3::ZERO, Vec3::Y, 0.8, 4.4);
    dark_plate(b);
    hull_x(
        b,
        &[
            [-2.7, 2.4, 3.0, 0.1],
            [-1.0, 2.9, 3.6, 0.0],
            [1.8, 2.8, 3.4, 0.0],
        ],
        &KEELED,
    );
    hull_x(b, &[[1.7, 3.2, 3.8, 0.0], [2.4, 2.7, 3.2, 0.0]], &CHAMFERED);
    if fine {
        b.mark(v3(-2.3, -1.25, 1.0), v3(1.5, 1.25, 1.9), mark::OUTLINE, 0.0);
    }
}

/// `~collar`: a gorget of three lapped bands round the breech's front, a spine blade
/// between the barrels to past their mouths, heat-sink fins along each barrel.
pub(super) fn gun(b: &mut MeshBuilder, len: f32) {
    let fine = b.fine();
    breech(b);
    // The gorget: three bands, each a little wider and further back, open underneath.
    dark_plate(b);
    let segs = if fine { 10 } else { 5 };
    for (x, r) in [(2.7, 1.9), (2.2, 2.2), (1.7, 2.5)] {
        // Each band lofted round its arc through its section: its front edge at `x`,
        // flaring back and out 0.7 m, 0.2 m thick.
        let rings: Vec<Vec<Vec3>> = (0..=segs)
            .map(|i| {
                let a = (-130.0 + 260.0 * i as f32 / segs as f32).to_radians();
                let d = v3(0.0, a.sin(), a.cos() * 0.95);
                vec![
                    v3(x, 0.0, 0.0) + d * r,
                    v3(x, 0.0, 0.0) + d * (r + 0.2),
                    v3(x - 0.7, 0.0, 0.0) + d * (r + 0.55),
                    v3(x - 0.7, 0.0, 0.0) + d * (r + 0.35),
                ]
            })
            .collect();
        b.loft(&rings, true, true);
    }
    // The spine blade between the barrels, its tip swept down past the mouths.
    armour(
        b,
        &Frame::new(v3(2.2, 0.12, 0.0), Vec3::X, -Vec3::Y),
        &[
            [0.0, -0.22],
            [0.0, 0.22],
            [len - 3.0, 0.18],
            [len - 2.0, 0.0],
            [len - 3.0, -0.18],
        ],
        0.24,
    );
    for z in BARRELS {
        b.with_recoil(|b| {
            barrel(b, z, 2.0, len, 0.36, &[3.2]);
            if fine {
                dark_plate(b);
                let fin = Fin {
                    len: 0.35,
                    w0: 0.5,
                    w1: 0.3,
                    thick: 0.08,
                };
                let out = if z > 0.0 { Vec3::Z } else { -Vec3::Z };
                for x in [3.8, 4.4, 5.0, 5.6] {
                    for y in [-0.3, 0.3] {
                        fin.at(b, v3(x, y, z + out.z * 0.3), out, Vec3::Y);
                    }
                }
            }
        });
    }
    if fine {
        b.mark(
            v3(2.6, -0.14, -0.15),
            v3(len - 1.8, 0.14, 0.15),
            mark::LAMP,
            0.0,
        );
    }
}
