//! The Heavy Pinch-fusion Lance: one emitter slung under the keel at the middle of the
//! ship. A barbette in the belly, a turret drum turning under it, and the gun hung
//! between two cheeks so it can lay down onto anything below and round it.
//!
//! The gun from the breech forward: a plated pinch chamber with the fusion seen only
//! through slits, plated sleeves along a tapering accelerator, and three focusing prongs
//! round the lens at the mouth. No caged core, nothing exposed.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::{GLOW_LASER, GLOW_PRISM};
use crate::{part, rig};

use super::super::super::kit::{dark_plate, metal, seam, v3};
use super::super::super::machine::{hoop, hoop_on};
use super::body::Body;
use super::{LANCE_MUZZLE, LANCE_PIVOT};

/// The barbette's ring radius, and the drum's.
const BARBETTE: f32 = 15.0;
const DRUM: f32 = 12.5;
/// The drum's underside and top.
const DRUM_LOW: f32 = 10.6;
const DRUM_TOP: f32 = 12.4;

pub(super) fn lance(b: &mut MeshBuilder, body: &Body) {
    barbette(b, body);
    let pivot = Vec3::from(LANCE_PIVOT);
    b.set_turret_pivot(pivot);
    b.set_arm_pivot(pivot);
    b.with_part(part::TURRET, |b| {
        drum(b);
        b.with_limb(rig::ARM_GUN, gun);
    });
}

/// The fixed ring in the belly the turret turns in: a plated skirt from the hull down to
/// a bronze race.
fn barbette(b: &mut MeshBuilder, body: &Body) {
    let keel = body.at(0.0).keel;
    let sides = b.sides(24);
    dark_plate(b);
    b.prism(
        v3(0.0, 0.0, DRUM_TOP),
        sides,
        BARBETTE,
        BARBETTE + 3.0,
        keel + 1.5 - DRUM_TOP,
    );
    if b.fine() {
        metal(b);
        hoop(b, v3(0.0, 0.0, DRUM_TOP + 0.3), DRUM + 1.0, 1.6, 0.6, 16);
    }
}

/// The drum under the barbette, and the cheeks the gun hangs between.
fn drum(b: &mut MeshBuilder) {
    let sides = b.sides(24);
    let p = Vec3::from(LANCE_PIVOT);
    dark_plate(b);
    b.prism(
        v3(0.0, 0.0, DRUM_LOW),
        sides,
        DRUM * 0.9,
        DRUM,
        DRUM_TOP - DRUM_LOW,
    );
    for y in [-1.0f32, 1.0] {
        // A cheek: a plate swept back from the drum's front to a point astern.
        let face = |o: f32| -> Vec<Vec3> {
            let y = y * (5.2 + o);
            vec![
                v3(9.5, y, DRUM_LOW + 0.2),
                v3(4.5, y, p.z - 3.4),
                v3(-4.0, y, p.z - 3.0),
                v3(-11.5, y, DRUM_LOW - 0.5),
                v3(-9.0, y, DRUM_LOW + 0.2),
            ]
        };
        b.loft(&[face(0.0), face(2.2)], true, true);
        if b.fine() {
            // The trunnion's bronze boss through the cheek.
            metal(b);
            b.cylinder_between(
                p + v3(0.0, y * 4.6, 0.0),
                p + v3(0.0, y * 7.9, 0.0),
                2.0,
                2.0,
                12,
            );
            dark_plate(b);
            b.cylinder_between(
                p + v3(0.0, y * 7.9, 0.0),
                p + v3(0.0, y * 8.3, 0.0),
                1.5,
                1.2,
                12,
            );
        }
    }
    if b.fine() {
        // Its rim: a ring of short swept plates round the drum's edge.
        dark_plate(b);
        let count = 12;
        for i in 0..count {
            let a = std::f32::consts::TAU * (i as f32 + 0.5) / count as f32;
            let d = Vec2::from_angle(a).extend(0.0);
            let s = Vec2::from_angle(a + 0.5).extend(0.0);
            b.loft(
                &[
                    vec![
                        d * (DRUM - 0.3) + v3(0.0, 0.0, DRUM_LOW + 0.2),
                        d * (DRUM + 0.5) + v3(0.0, 0.0, DRUM_LOW + 0.9),
                        d * (DRUM - 0.3) + v3(0.0, 0.0, DRUM_TOP - 0.2),
                    ],
                    vec![s * (DRUM + 0.9) + v3(0.0, 0.0, DRUM_LOW + 0.9); 3],
                ],
                true,
                false,
            );
        }
        b.paint(GLOW_LASER);
        hoop(b, v3(0.0, 0.0, DRUM_LOW - 0.02), DRUM * 0.55, 0.3, 0.06, 16);
    }
}

/// The gun, built along +x about the pivot.
fn gun(b: &mut MeshBuilder) {
    let p = Vec3::from(LANCE_PIVOT);
    let m = Vec3::from(LANCE_MUZZLE);
    let at = |x: f32| v3(x, 0.0, p.z);
    let sides = b.sides(16);
    let fine = b.fine();
    // Breech and pinch chamber: an octagonal plated body.
    dark_plate(b);
    b.with_facets(|b| {
        b.cylinder_between(at(-3.0), at(-1.6), 2.6, 3.4, 8);
        b.cylinder_between(at(-1.6), at(9.0), 3.5, 3.5, 8);
        b.cylinder_between(at(9.0), at(11.0), 3.5, 2.9, 8);
    });
    // The fusion seen through the chamber's slits: a lit core inside, plated over but
    // for four long slots.
    if fine {
        b.paint(GLOW_PRISM);
        for k in 0..4 {
            let a = std::f32::consts::FRAC_PI_4 + std::f32::consts::FRAC_PI_2 * k as f32;
            let d = v3(0.0, a.cos(), a.sin());
            let side = v3(0.0, -a.sin(), a.cos());
            let r = 3.5 * std::f32::consts::FRAC_PI_8.cos() + 0.03;
            let c = at(3.7) + d * r;
            b.loft(
                &[
                    vec![
                        c - Vec3::X * 4.2 - side * 0.35,
                        c + Vec3::X * 4.2 - side * 0.35,
                        c + Vec3::X * 4.2 + side * 0.35,
                        c - Vec3::X * 4.2 + side * 0.35,
                    ],
                    vec![
                        c - Vec3::X * 4.2 - side * 0.35 + d * 0.06,
                        c + Vec3::X * 4.2 - side * 0.35 + d * 0.06,
                        c + Vec3::X * 4.2 + side * 0.35 + d * 0.06,
                        c - Vec3::X * 4.2 + side * 0.35 + d * 0.06,
                    ],
                ],
                false,
                true,
            );
        }
        // Bronze collars at either end of the chamber.
        metal(b);
        for x in [-0.6, 8.0] {
            b.cylinder_between(at(x), at(x + 1.2), 3.8, 3.8, sides);
        }
    }
    // The accelerator: tapering, in plated sleeves with dark gaps.
    let sleeves: &[(f32, f32)] = &[(11.0, 2.7), (15.0, 2.45), (19.0, 2.2)];
    seam(b);
    b.cylinder_between(at(11.0), at(m.x - 4.0), 2.2, 1.7, sides);
    dark_plate(b);
    for &(x, r) in if fine { sleeves } else { &sleeves[..1] } {
        b.with_facets(|b| b.cylinder_between(at(x), at(x + 3.2), r, r * 0.95, 8));
    }
    // The mouth: a collar, the lens, and three prongs swept forward round it.
    metal(b);
    b.cylinder_between(at(m.x - 4.0), at(m.x - 2.6), 2.3, 2.3, sides);
    b.paint(GLOW_PRISM);
    b.cylinder_between(at(m.x - 2.6), m, 1.25, 1.05, sides);
    if fine {
        hoop_on(b, at(m.x - 2.4), Vec3::X, 1.75, 0.35, 0.1, sides);
    }
    dark_plate(b);
    for k in 0..3 {
        let a = std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * k as f32 / 3.0;
        let d = v3(0.0, a.cos(), a.sin());
        let side = v3(0.0, -a.sin(), a.cos());
        let root = at(m.x - 5.0) + d * 2.2;
        let knee = at(m.x - 1.0) + d * 2.9;
        let tip = at(m.x + 1.6) + d * 1.6;
        let w = 0.75;
        b.loft(
            &[
                vec![
                    root - side * w,
                    root + d * 0.9,
                    root + side * w,
                    root - d * 0.2,
                ],
                vec![
                    knee - side * w,
                    knee + d * 0.8,
                    knee + side * w,
                    knee - d * 0.2,
                ],
                vec![tip; 4],
            ],
            true,
            false,
        );
    }
}
