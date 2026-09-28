//! The bolt rifle (the Paladin's shoulder guns, the Raptor's nose guns): it fires a
//! bolt of blue plasma, so it is neither of the other ARC energy guns. No induction
//! rings (the electric bore) and no open pair of rails (the rail gun): a sculpted,
//! faceted body round a hidden bore. Its blue is thin seams, never a lit block.
//!
//! Every piece is built on the gun's +y side as its outboard side: the callers mirror
//! the gun, so the cell ends up outboard on both.

use glam::{Affine3A, Vec2, Vec3};

use super::parts::*;
use crate::builder::MeshBuilder;
use crate::material::*;
use crate::pattern;

/// The Paladin's bolt rifle, level from `breech` forward to `muzzle` (same y and z): a
/// sculpted housing with a plasma cell on its outboard flank, a slim core barrel
/// carrying swept radiator blades, a stepped muzzle collar. `r` is half the housing's
/// height; the rest scales from it and the length.
pub(super) fn bolt_rifle(b: &mut MeshBuilder, breech: Vec3, muzzle: Vec3, r: f32) {
    let length = muzzle.x - breech.x;
    b.at(breech, |b| {
        if b.coarse() {
            coarse(b, length, r);
            return;
        }
        if !b.fine() {
            // Mid distance: the housing, the core with its spine blade, the collar.
            b.paint(PLATING).pattern(pattern::PLAIN);
            b.beam(
                v3(-0.04 * length, 0.0, 0.0),
                v3(0.34 * length, 0.0, 0.0),
                v2(r * 2.3, r * 2.3),
                v2(r * 1.5, r * 1.4),
            );
            b.beam(
                v3(0.88 * length, 0.0, 0.0),
                Vec3::X * length,
                v2(r * 1.25, r * 1.2),
                v2(r * 0.95, r * 0.9),
            );
            b.paint(PLATING_DARK);
            b.beam(
                v3(0.34 * length, 0.0, 0.0),
                v3(0.88 * length, 0.0, 0.0),
                v2(r * 0.84, r * 0.84),
                v2(r * 0.84, r * 0.84),
            );
            b.paint(PLATING);
            b.extrude_y(
                &[
                    [0.36 * length, r * 0.3],
                    [0.84 * length, r * 0.3],
                    [0.4 * length, r * 1.05],
                ],
                -r * 0.05,
                r * 0.05,
            );
            return;
        }
        housing(b, length, r);
        finned_core(b, length, r, 0.9);
        collar(b, length, r);
    });
}

/// The Raptor's bolt rifle, the same gun cut down to a few faces: a dark core in a light
/// armoured sleeve cut on a slant, a cowl over it, the core running on past it to a
/// faceted point.
pub(super) fn fighter_bolt_rifle(b: &mut MeshBuilder, breech: Vec3, muzzle: Vec3, r: f32) {
    let length = muzzle.x - breech.x;
    b.at(breech, |b| {
        if b.coarse() {
            coarse(b, length, r);
            return;
        }
        if !b.fine() {
            // Mid distance: the sleeve and the core past it.
            b.paint(PLATING).pattern(pattern::PLAIN);
            b.beam(
                v3(-0.04 * length, 0.0, 0.0),
                v3(0.66 * length, 0.0, 0.0),
                v2(r * 2.1, r * 2.1),
                v2(r * 2.1, r * 2.1),
            );
            b.paint(PLATING_DARK);
            b.beam(
                v3(0.66 * length, 0.0, 0.0),
                Vec3::X * length,
                v2(r * 1.2, r * 1.1),
                v2(r * 0.8, r * 0.7),
            );
            return;
        }
        sleeve(b, length, r);
    });
}

/// Far away: one tapering dark bar.
fn coarse(b: &mut MeshBuilder, length: f32, r: f32) {
    b.paint(PLATING_DARK);
    b.beam(
        Vec3::ZERO,
        Vec3::X * length,
        v2(r * 2.2, r * 2.0),
        v2(r * 0.8, r * 0.8),
    );
}

/// A flattened hexagon, `w` wide and `h` tall, as an x-extrusion profile.
fn hexagon(w: f32, h: f32) -> Vec<[f32; 2]> {
    let (y, z) = (w * 0.5, h * 0.5);
    vec![
        [-y, -z * 0.45],
        [-y * 0.62, -z],
        [y * 0.62, -z],
        [y, -z * 0.45],
        [y, z * 0.45],
        [y * 0.62, z],
        [-y * 0.62, z],
        [-y, z * 0.45],
    ]
}

/// `profile` placed round the x axis at `x`, as a loft ring.
fn ring(profile: &[[f32; 2]], x: f32) -> Vec<Vec3> {
    profile.iter().map(|p| v3(x, p[0], p[1])).collect()
}

/// The muzzle's face: a dark bore with a small blue point deep in it.
fn bore_face(b: &mut MeshBuilder, x: f32, size: Vec2) {
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(
        v3(x - size.y * 0.3, -size.x * 0.5, -size.y * 0.5),
        v3(x + 0.01, size.x * 0.5, size.y * 0.5),
    );
    b.paint(GLOW);
    b.block(
        v3(x + 0.01, -size.x * 0.12, -size.y * 0.12),
        v3(x + 0.012, size.x * 0.12, size.y * 0.12),
    );
}

/// A thin blue seam along x at height `z` on a +y face at `y`, in a dark channel.
fn seam(b: &mut MeshBuilder, x0: f32, x1: f32, y: f32, z: f32, r: f32) {
    b.paint(ACCENT);
    b.block(
        v3(x0, y - r * 0.02, z - r * 0.07),
        v3(x1, y + r * 0.02, z + r * 0.07),
    );
    b.paint(GLOW);
    b.block(
        v3(x0 + r * 0.1, y + r * 0.02, z - r * 0.025),
        v3(x1 - r * 0.1, y + r * 0.035, z + r * 0.025),
    );
}

/// A row of `n` dark vent slots on a +y face at `y`, from x0 at `pitch`.
fn vents(b: &mut MeshBuilder, x0: f32, pitch: f32, n: usize, y: f32, z: (f32, f32), r: f32) {
    b.paint(ACCENT);
    for k in 0..n {
        let x = x0 + pitch * k as f32;
        b.block(
            v3(x, y - r * 0.02, z.0),
            v3(x + pitch * 0.45, y + r * 0.03, z.1),
        );
    }
}

/// The Vanes housing: a faceted block with a layered top plate, vents down its flank
/// and a plasma cell carried outboard, a conduit from the cell into the core.
fn housing(b: &mut MeshBuilder, length: f32, r: f32) {
    let fine = b.fine();
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.loft(
        &[
            ring(&hexagon(r * 2.2, r * 2.2), -0.04 * length),
            ring(&hexagon(r * 2.3, r * 2.3), 0.2 * length),
            ring(&hexagon(r * 1.5, r * 1.4), 0.34 * length),
        ],
        true,
        true,
    );
    // The top plate, raised and stepped.
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.loft(
        &[
            ring(
                &[
                    [-r * 0.6, r * 1.1],
                    [r * 0.6, r * 1.1],
                    [r * 0.45, r * 1.3],
                    [-r * 0.45, r * 1.3],
                ],
                0.0,
            ),
            ring(
                &[
                    [-r * 0.5, r * 1.02],
                    [r * 0.5, r * 1.02],
                    [r * 0.35, r * 1.2],
                    [-r * 0.35, r * 1.2],
                ],
                0.24 * length,
            ),
        ],
        true,
        true,
    );
    // The cell: a capped canister along the outboard flank, a blue line down its top.
    let (c0, c1, cr) = (0.01 * length, 0.22 * length, r * 0.36);
    let (cy, cz) = (r * 1.3, -r * 0.3);
    b.paint(PLATING_DARK);
    b.cylinder_between(v3(c0, cy, cz), v3(c1, cy, cz), cr, cr, b.sides(10));
    b.paint(METAL);
    for x in [c0 - 0.02 * length, c1] {
        b.cylinder_between(
            v3(x, cy, cz),
            v3(x + 0.02 * length, cy, cz),
            cr * 0.82,
            cr * 0.82,
            b.sides(10),
        );
    }
    b.paint(GLOW);
    b.block(
        v3(c0 + 0.03 * length, cy - r * 0.04, cz + cr * 0.96),
        v3(c1 - 0.03 * length, cy + r * 0.04, cz + cr * 1.02),
    );
    b.mirror_y(|b| seam(b, 0.02 * length, 0.18 * length, r * 1.15, r * 0.3, r));
    if fine {
        vents(
            b,
            0.03 * length,
            0.035 * length,
            5,
            -r * 1.15,
            (-r * 0.4, r * 0.05),
            r,
        );
        // The cell's feed, bent round into the core.
        b.paint(METAL);
        b.cylinder_between(
            v3(c1 + 0.02 * length, cy, cz),
            v3(0.3 * length, cy * 0.8, cz),
            r * 0.08,
            r * 0.08,
            6,
        );
        b.cylinder_between(
            v3(0.3 * length, cy * 0.8, cz),
            v3(0.38 * length, r * 0.35, cz * 0.6),
            r * 0.08,
            r * 0.08,
            6,
        );
    }
}

/// The core barrel from the housing to `to` (a fraction of the length), and radiator
/// blades on it, spine and both lower flanks, each in two swept pieces with a blue
/// line at its root.
fn finned_core(b: &mut MeshBuilder, length: f32, r: f32, to: f32) {
    let fine = b.fine();
    let end = to * length;
    let core: Vec<[f32; 2]> = (0..8)
        .map(|k| {
            let a = (k as f32 + 0.5) * std::f32::consts::TAU / 8.0;
            [a.cos() * r * 0.42, a.sin() * r * 0.42]
        })
        .collect();
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.extrude_x(&core, 0.3 * length, end);
    let (x0, x1) = (0.36 * length, end - 0.05 * length);
    let at = |f: f32| x0 + (x1 - x0) * f;
    let rear = [
        [at(0.0), r * 0.3],
        [at(0.5), r * 0.3],
        [at(0.46), r * 0.85],
        [at(0.12), r * 1.08],
    ];
    let front = [
        [at(0.54), r * 0.3],
        [at(1.0), r * 0.3],
        [at(0.94), r * 0.55],
        [at(0.58), r * 0.8],
    ];
    let t = (r * 0.05).max(0.012);
    for angle in [0.0, 2.1, -2.1] {
        b.with(Affine3A::from_rotation_x(angle), |b| {
            b.paint(PLATING).pattern(pattern::PLAIN);
            b.extrude_y(&rear, -t, t);
            b.extrude_y(&front, -t, t);
            if fine {
                // A dark cap along each blade's outer edge.
                b.paint(ACCENT);
                b.block(
                    v3(at(0.14), -t * 1.4, r * 0.98),
                    v3(at(0.4), t * 1.4, r * 1.03),
                );
                b.paint(GLOW);
                b.block(
                    v3(at(0.06), -t * 1.3, r * 0.38),
                    v3(at(0.94), t * 1.3, r * 0.42),
                );
            }
        });
    }
}

/// The Vanes muzzle collar: a dark stage and a light faceted stage, ports down its
/// flanks, the bore in it.
fn collar(b: &mut MeshBuilder, length: f32, r: f32) {
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft(
        &[
            ring(&hexagon(r * 0.9, r * 0.9), 0.86 * length),
            ring(&hexagon(r * 1.1, r * 1.05), 0.89 * length),
        ],
        true,
        true,
    );
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.loft(
        &[
            ring(&hexagon(r * 1.2, r * 1.15), 0.89 * length),
            ring(&hexagon(r * 1.25, r * 1.2), 0.94 * length),
            ring(&hexagon(r * 0.95, r * 0.9), length),
        ],
        true,
        true,
    );
    b.mirror_y(|b| {
        vents(
            b,
            0.9 * length,
            0.022 * length,
            3,
            r * 0.62,
            (-r * 0.18, r * 0.18),
            r,
        )
    });
    bore_face(b, length, v2(r * 0.6, r * 0.55));
}

/// The Sleeve: a dark core inside a light sleeve whose front is cut on a slant, a dark
/// cowl layered over the sleeve's back, a swept strake under it, the core running on to
/// a faceted point. A blue seam down each flank.
fn sleeve(b: &mut MeshBuilder, length: f32, r: f32) {
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.loft(
        &[
            ring(&hexagon(r * 1.2, r * 1.1), 0.1 * length),
            ring(&hexagon(r * 1.2, r * 1.1), 0.86 * length),
            ring(&hexagon(r * 0.8, r * 0.7), length),
        ],
        false,
        true,
    );
    bore_face(b, length, v2(r * 0.5, r * 0.45));
    // The sleeve: its front edge slants back from the keel to the spine.
    let outer = hexagon(r * 2.1, r * 2.1);
    let lean = 0.16 * length;
    let slant = |x: f32| -> Vec<Vec3> {
        outer
            .iter()
            .map(|p| v3(x - lean * (p[1] / r + 1.0) * 0.5, p[0], p[1]))
            .collect()
    };
    let mouth = 0.72 * length;
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.loft(
        &[
            ring(&outer, -0.04 * length),
            ring(&hexagon(r * 2.2, r * 2.2), 0.3 * length),
            slant(mouth),
        ],
        true,
        false,
    );
    // Its mouth, closed dark round the core, facing forward.
    b.paint(ACCENT);
    b.loft(&[slant(mouth - 0.01 * length), slant(mouth)], false, true);
    // The cowl: a dark armour shell over the sleeve's back half, cut on the same slant.
    let cowl = [
        [-r * 1.12, r * 0.35],
        [r * 1.12, r * 0.35],
        [r * 0.7, r * 1.16],
        [-r * 0.7, r * 1.16],
    ];
    let cowl_front: Vec<Vec3> = cowl
        .iter()
        .map(|p| v3(0.5 * length - lean * 0.6 * (p[1] / r), p[0], p[1]))
        .collect();
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.loft(&[ring(&cowl, -0.05 * length), cowl_front], true, true);
    // A swept strake under the sleeve.
    b.paint(PLATING_DARK);
    b.extrude_y(
        &[
            [0.04 * length, -r * 1.02],
            [0.1 * length, -r * 1.3],
            [0.5 * length, -r * 1.02],
        ],
        -r * 0.06,
        r * 0.06,
    );
    b.mirror_y(|b| seam(b, 0.36 * length, 0.6 * length, r * 1.05, 0.0, r));
}
