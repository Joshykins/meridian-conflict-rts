//! The plasma guns the Regency's tech 1 line carries (docs/STYLE.md "The Regency suite"),
//! each drawn along +x from its breech, so a mount can lay it where it likes:
//!
//! - [`repeater`]: a Plasmeric Repeater pod, chunky and hunched to shed heat, a wide red
//!   emitter mouth rather than a rifle's bore (the Picket's gun, small).
//! - [`flak_organ`]: a Plasmeric Flak Cannon, short flak tubes side by side in one clamped
//!   block (the Canopy's organ, small).

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::*;

use super::commander::form::{blade, ring, sleeve, OCT};
use super::kit::{dark_plate, metal, seam, v3};
use super::machine::{hoop_on, red_slot};

/// A Plasmeric Repeater pod from `breech` forward along +x to `muzzle` (level, at the
/// same y and z), `r` its housing's half width: a faceted plated housing, open heat
/// louvres raked back over a red slit on its back, a seam-dark collar, a bronze nozzle,
/// and a flared shroud whose lip is the muzzle, a red lens set in its face.
pub(super) fn repeater(b: &mut MeshBuilder, breech: Vec3, muzzle: Vec3, r: f32) {
    let at = |t: f32| breech.lerp(muzzle, t);
    let len = muzzle.x - breech.x;
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(at(0.0), Vec3::Z, r * 0.7, r * 0.75),
            ring(at(0.1), Vec3::Z, r, r * 1.05),
            ring(at(0.5), Vec3::Z, r * 0.95, r),
            ring(at(0.64), Vec3::Z, r * 0.7, r * 0.72),
        ],
        &OCT,
    );
    let sides = b.sides(8);
    if b.fine() {
        seam(b);
        b.cylinder_between(at(0.62), at(0.7), r * 0.6, r * 0.6, sides);
    }
    metal(b);
    b.cylinder_between(at(0.7), at(0.9), r * 0.4, r * 0.36, sides);
    // The emitter: a dark flared shroud, its lip the muzzle, a red lens set in its face.
    dark_plate(b);
    b.cylinder_between(at(0.88), muzzle, r * 0.46, r * 0.62, sides);
    b.paint(GLOW_LASER);
    b.cylinder_between(at(0.9), muzzle + Vec3::X * 0.02, r * 0.26, r * 0.3, sides);
    if b.fine() {
        // Heat louvres raked back over the housing's back, a red slit under them.
        red_slot(
            b,
            at(0.3) + Vec3::Z * (r * 1.0),
            Vec3::Z,
            Vec3::X,
            len * 0.3,
            r * 0.06,
        );
        for t in [0.18f32, 0.3, 0.42] {
            blade(
                b,
                at(t) + Vec3::Z * (r * 1.02),
                v3(-1.0, 0.0, 0.55),
                v3(0.5, 0.0, 1.0),
                len * 0.12,
                r * 0.7,
                0.0,
                r * 0.08,
            );
        }
    }
}

/// A Plasmeric Flak Cannon drawn in its own frame (the origin at the trunnion, +x up the
/// bore): a bronze trunnion `trunnion` across, a plated breech block and a clamp, and a
/// flak tube at each of `tubes` (y) running out to `len`, each `r` round, a red lip on its
/// mouth. The tubes' mouths are in a row through `(len, 0, 0)`.
pub(super) fn flak_organ(b: &mut MeshBuilder, len: f32, tubes: &[f32], r: f32, trunnion: f32) {
    let fine = b.fine();
    let half = tubes.iter().fold(0.0f32, |m, y| m.max(y.abs())) + r * 1.4;
    let sides = b.sides(8);
    metal(b);
    b.cylinder_between(
        v3(0.0, -trunnion, 0.0),
        v3(0.0, trunnion, 0.0),
        r * 1.1,
        r * 1.1,
        sides,
    );
    dark_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[
                block_ring(-len * 0.3, half * 0.9, r * 1.5),
                block_ring(len * 0.05, half, r * 1.7),
                block_ring(len * 0.3, half, r * 1.6),
            ],
            true,
            true,
        )
    });
    dark_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[
                block_ring(len * 0.62, half * 0.98, r * 1.4),
                block_ring(len * 0.72, half * 0.98, r * 1.4),
            ],
            true,
            true,
        )
    });
    if fine {
        red_slot(
            b,
            v3(-len * 0.05, 0.0, r * 1.7),
            Vec3::Z,
            Vec3::Y,
            half * 0.8,
            r * 0.12,
        );
    }
    for &y in tubes {
        dark_plate(b);
        b.cylinder_between(v3(len * 0.25, y, 0.0), v3(len, y, 0.0), r, r * 1.08, sides);
        seam(b);
        b.cylinder_between(
            v3(len * 0.36, y, 0.0),
            v3(len * 0.42, y, 0.0),
            r * 1.12,
            r * 1.12,
            sides,
        );
        if fine {
            b.paint(GLOW_LASER);
            hoop_on(
                b,
                v3(len - 0.03, y, 0.0),
                Vec3::X,
                r * 0.82,
                r * 0.3,
                0.06,
                sides,
            );
        }
    }
}

/// A block's section at `x`: `half` across, `h` up from the bore, corners cut.
fn block_ring(x: f32, half: f32, h: f32) -> Vec<Vec3> {
    [
        [1.0, -0.55],
        [0.75, -0.8],
        [-0.75, -0.8],
        [-1.0, -0.55],
        [-1.0, 0.6],
        [-0.7, 1.0],
        [0.7, 1.0],
        [1.0, 0.6],
    ]
    .iter()
    .map(|[s, t]| v3(x, s * half, t * h))
    .collect()
}
