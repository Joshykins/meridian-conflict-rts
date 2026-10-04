//! The Breacher's thermobaric rocket launchers: sixteen cells in each, the torso's own
//! weapon. Each design sets its pair where it wants them (`launcher_at`).

use std::f32::consts::TAU;

use glam::Vec3;

use super::super::limbs::*;
use super::super::parts::*;
use crate::builder::MeshBuilder;
use crate::material::*;
use crate::pattern;

const X: Vec3 = Vec3::X;

/// The cells about the face's middle before the rake: across (y) and up the face.
const CELLS: [f32; 4] = [-2.4, -0.8, 0.8, 2.4];
/// The launcher's half width and half height round its face.
pub(super) const HALF: f32 = 3.6;
/// How deep the launcher runs back from its face.
pub(super) const DEPTH: f32 = 10.0;

/// A launcher with its face's middle at `face`, raked `pitch` nose-up, a cheek plate down
/// its outer side if `cheek` (the side away from the centreline).
pub(super) fn launcher_at(b: &mut MeshBuilder, face: Vec3, pitch: f32, cheek: bool) {
    b.pitched(face, pitch, |b| launcher(b, cheek && face.y >= 0.0));
}

/// The launcher in its own frame: the origin is its face's middle, +x out of the face.
fn launcher(b: &mut MeshBuilder, cheek: bool) {
    let fine = b.fine();
    let (x, y, z) = (0.0f32, 0.0f32, 0.0f32);
    b.paint(PLATING);
    b.loft(
        &[
            x_ring(x - 0.2, y, z, HALF, HALF, 1.2),
            x_ring(x - 1.8, y, z + 0.2, HALF + 0.3, HALF + 0.3, 1.4),
            x_ring(x - DEPTH + 2.0, y, z, HALF, HALF - 0.2, 1.4),
            x_ring(x - DEPTH, y, z - 0.4, HALF - 0.8, HALF - 1.0, 1.0),
        ],
        true,
        true,
    );
    team_panel(b, v3(x - 4.0, y, z + HALF + 0.3), v2(3.0, 2.0 * HALF - 2.6));
    // The face: a dark plate, a rim round each cell, the mouth dark in it.
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.loft(
        &[
            x_ring(x - 0.3, y, z, HALF - 0.3, HALF - 0.3, 1.0),
            x_ring(x, y, z, HALF - 0.3, HALF - 0.3, 1.0),
        ],
        true,
        true,
    );
    for dz in CELLS {
        for dy in CELLS {
            let m = v3(x, y + dy, z + dz);
            if fine {
                b.paint(METAL).pattern(pattern::PLAIN);
                b.cylinder_between(m - X * 0.2, m + X * 0.15, 0.72, 0.68, 8);
            }
            b.paint(TREAD);
            let mouth: Vec<Vec3> = (0..8)
                .map(|k| {
                    let a = (k as f32 + 0.5) * TAU / 8.0;
                    m + v3(0.18, a.cos() * 0.55, a.sin() * 0.55)
                })
                .collect();
            b.face(&mouth);
        }
    }
    if cheek {
        // A cheek plate down the outside, standing off it.
        b.paint(PLATING);
        b.extrude_y(
            &[
                [x + 0.4, z - HALF + 0.4],
                [x + 0.4, z + HALF - 0.2],
                [x - 3.0, z + HALF + 0.6],
                [x - DEPTH + 1.0, z + HALF - 1.0],
                [x - DEPTH + 2.0, z - HALF - 0.2],
            ],
            y + HALF + 0.3,
            y + HALF + 1.1,
        );
    }
    if fine {
        // Reload hatches on the lid, a hazard band round the face's rim, a blast grille
        // in the tail.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for k in 0..2 {
            let hx = x - 4.0 - 2.6 * k as f32;
            b.block(
                v3(hx - 1.0, y - HALF + 0.6, z + HALF + 0.25),
                v3(hx + 1.0, y - 0.6, z + HALF + 0.45),
            );
        }
        b.paint(PLATING).pattern(pattern::HAZARD);
        b.loft(
            &[
                x_ring(x - 1.4, y, z, HALF + 0.35, HALF + 0.35, 1.4),
                x_ring(x - 0.8, y, z, HALF + 0.35, HALF + 0.35, 1.4),
            ],
            false,
            false,
        );
        b.paint(METAL);
        for k in 0..4 {
            let gz = z - 1.8 + 1.2 * k as f32;
            b.block(
                v3(x - DEPTH - 0.1, y - 2.2, gz),
                v3(x - DEPTH + 0.2, y + 2.2, gz + 0.4),
            );
        }
    }
}

/// The sixteen cell mouths of a launcher with its face at `face`, raked `pitch`: the unit
/// file's muzzles.
#[cfg(test)]
pub(crate) fn mouths(face: Vec3, pitch: f32) -> Vec<Vec3> {
    let (s, c) = pitch.sin_cos();
    let mut out = Vec::new();
    for dz in CELLS {
        for dy in CELLS {
            out.push(face + v3(-s * dz, dy, c * dz));
        }
    }
    out
}
