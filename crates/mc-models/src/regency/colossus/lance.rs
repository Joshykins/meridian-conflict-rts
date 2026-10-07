//! The Heavy Pinch-fusion Lance: the cannon head on the hull's front, in its own frame (the
//! origin on the trunnion, +x down the bore, the middle of its charge at `(MOUTH, 0, 0)`). It
//! pitches about the trunnion with the weapon (`rig::ARM_GUN`).
//!
//! A breech block sunk in the hull, then a long octagonal barrel tapering to a point, split
//! down its length into two armoured jaw halves that shut on a seam along its middle, bands of
//! plate lapped over them. Between them lies the bore: a tube wound in pinch coils narrowing
//! toward the mouth, a hex prism in it. Through the charge the jaws part (`SPREAD`), the
//! prism slides out toward the mouth (`EXTEND`) and the coils light one after another from
//! the breech, a strip of fusion light down each jaw's inner face with them (entity.wgsl
//! `charge_gear_pose`, the Sunspear's gear). Shut, the jaws hide it all. The charge forms
//! past the jaws' tips and is held between them.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::gpu_consts::charge_gear::{EXTEND, SPREAD};
use crate::rig;

use super::super::commander::form::OCT;
use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::{collar, hoop_on, red_slot};
use super::super::turrets::sunspear::coil_light;
use super::LANCE_PIVOT;

/// From the trunnion to the middle of the charge.
#[cfg(test)]
pub(super) const MOUTH: f32 = 29.6;
/// The jaws' tips, and where they start.
const TIP: f32 = 27.2;
const ROOT: f32 = 8.0;
/// The gap each jaw leaves at the seam when shut (so the two never share a face).
const GAP: f32 = 0.12;
/// How far the gear's travels run over their `charge_gear` metres: the jaws part 2.45 m each
/// way, the prism runs out 3.85 m.
pub(super) const GEAR: f32 = 3.5;

/// The barrel's radius at `x`: tapering from its root to its tip.
fn radius(x: f32) -> f32 {
    5.0 - (x - ROOT) * (2.6 / (26.0 - ROOT))
}

/// A jaw's section at `x`, the +y half of an octagon of radius `r`: its flat inner face at the
/// seam, then round the flank. Listed from the top inner corner.
fn jaw_ring(x: f32, r: f32) -> Vec<Vec3> {
    let k = std::f32::consts::FRAC_1_SQRT_2;
    vec![
        v3(x, GAP, r),
        v3(x, r * k, r * k),
        v3(x, r, 0.0),
        v3(x, r * k, -r * k),
        v3(x, GAP, -r),
    ]
}

pub(super) fn lance(b: &mut MeshBuilder) {
    b.set_charge_gear(LANCE_PIVOT, GEAR);
    b.with_limb(rig::ARM_GUN, |b| {
        b.at(LANCE_PIVOT, |b| {
            breech(b);
            core(b);
            b.mirror_y(|b| b.with_charge_gear(SPREAD, jaw));
        });
    });
}

/// The breech: a plated octagonal block from behind the trunnion to the jaws' roots, a seam
/// band, a red slot down each flank, a plated collar at its front.
fn breech(b: &mut MeshBuilder) {
    let fine = b.fine();
    let ring =
        |x: f32, r: f32| -> Vec<Vec3> { OCT.iter().map(|&[s, t]| v3(x, s * r, t * r)).collect() };
    dark_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[
                ring(-6.0, 4.4),
                ring(-2.5, 5.5),
                ring(3.0, 5.8),
                ring(ROOT + 0.4, 5.3),
            ],
            true,
            true,
        );
    });
    if fine {
        seam(b);
        b.with_facets(|b| b.loft(&[ring(2.4, 5.86), ring(2.9, 5.86)], true, true));
        dark_plate(b);
        b.with_facets(|b| b.loft(&[ring(ROOT - 1.8, 5.9), ring(ROOT - 0.2, 5.6)], true, true));
        b.mirror_y(|b| red_slot(b, v3(-1.2, 5.55, 0.9), Vec3::Y, Vec3::X, 4.4, 0.34));
    }
}

/// The bore's core in the gap between the jaws: a bronze tube through six pinch coils
/// narrowing toward the mouth, and the prism that slides out through them.
fn core(b: &mut MeshBuilder) {
    let fine = b.fine();
    let sides = b.sides(14);
    metal(b);
    b.cylinder_between(v3(ROOT, 0.0, 0.0), v3(TIP - 0.4, 0.0, 0.0), 1.2, 1.0, sides);
    let coils: &[(f32, f32, u32)] = if fine {
        &[
            (9.5, 2.0, 1),
            (12.5, 1.95, 2),
            (15.5, 1.9, 3),
            (18.5, 1.8, 4),
            (21.5, 1.65, 5),
            (24.5, 1.5, 5),
        ]
    } else {
        &[(12.5, 1.95, 2), (21.5, 1.65, 5)]
    };
    for &(x, r, stage) in coils {
        dark_plate(b);
        hoop_on(b, v3(x, 0.0, 0.0), Vec3::X, r, 0.8, 0.9, sides);
        metal(b);
        hoop_on(b, v3(x, 0.0, 0.0), Vec3::X, r + 0.4, 0.25, 0.65, sides);
        coil_light(b, stage);
        hoop_on(b, v3(x, 0.0, 0.0), Vec3::X, r - 0.45, 0.14, 0.6, sides);
    }
    // The prism: a hex rod of fusion light, tapered to its point, riding out toward the mouth.
    b.with_charge_gear(EXTEND, |b| {
        collar(b, v3(12.0, 0.0, 0.0), Vec3::X, 1.0, 0.6);
        coil_light(b, 6);
        b.cylinder_between(v3(12.0, 0.0, 0.0), v3(21.0, 0.0, 0.0), 0.8, 0.8, 6);
        b.cylinder_between(v3(21.0, 0.0, 0.0), v3(22.2, 0.0, 0.0), 0.8, 0.12, 6);
    });
}

/// One jaw, the +y half (`mirror_y` makes the other): an octagonal half barrel from root to a
/// pointed tip, bands of plate lapped over it, red optics down its flank, a strip of fusion
/// light down its inner face for each coil.
fn jaw(b: &mut MeshBuilder) {
    let fine = b.fine();
    dark_plate(b);
    let stations: &[f32] = if fine {
        &[ROOT, 13.0, 19.0, 26.0]
    } else {
        &[ROOT, 26.0]
    };
    let mut rings: Vec<Vec<Vec3>> = stations.iter().map(|&x| jaw_ring(x, radius(x))).collect();
    // The tip: drawn in to a point past the last station.
    rings.push(jaw_ring(TIP, radius(26.0) * 0.4));
    b.with_facets(|b| b.loft(&rings, true, true));
    if !fine {
        return;
    }
    // Bands of plate lapped over the barrel, each a little proud of it.
    dark_plate(b);
    for (x0, x1) in [(9.0f32, 11.2f32), (14.6, 16.4), (20.0, 21.6)] {
        let (r0, r1) = (radius(x0) * 1.1, radius(x1) * 1.1);
        b.with_facets(|b| {
            b.loft(
                &[jaw_ring(x0, r0), jaw_ring(x0 + 0.5, r0), jaw_ring(x1, r1)],
                true,
                true,
            )
        });
    }
    red_slot(
        b,
        v3(16.0, radius(16.0) + 0.02, 0.0),
        Vec3::Y,
        Vec3::X,
        6.0,
        0.34,
    );
    // The inner face lit stage by stage from the breech.
    for i in 0..7u32 {
        let x = ROOT + 0.6 + 2.6 * i as f32;
        coil_light(b, i);
        let h = radius(x) * 0.55;
        b.block(v3(x, GAP - 0.05, -h), v3(x + 1.6, GAP, h));
    }
}
