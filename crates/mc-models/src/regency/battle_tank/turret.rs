//! The battle tank's turret and its Pinched-plasmeric Cannon, a three-pronged claw.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::*;

use super::super::kit::{dark_plate, metal, v3};
use super::super::machine::{collar, hoop_on, red_slot, Course, Frame};
use super::hull::plan;
use super::{DECK, MUZZLE, PIVOT};

/// The roof of the turret.
const ROOF: f32 = 4.3;

/// The turret: a bronze race on the deck, a faceted block drawn in toward its roof, two swept plates lapped back down each flank, red optics
/// either side of the gun, the team colour on the roof and a lit vent across its bustle.
pub(super) fn turret(b: &mut MeshBuilder) {
    metal(b);
    let sides = b.sides(14);
    b.prism(v3(0.0, 0.0, DECK - 0.05), sides, 2.0, 1.9, 0.2);
    let half = [
        [2.3, 1.1],
        [1.2, 2.0],
        [-1.7, 2.1],
        [-3.0, 1.35],
        [-3.3, 0.0],
    ];
    let levels = [
        Section::new(DECK + 0.1, 1.0),
        Section::new(3.5, 1.0),
        Section::new(4.05, 0.86).shifted(-0.15, 0.0),
        Section::new(ROOF, 0.62).shifted(-0.4, 0.0),
    ];
    dark_plate(b);
    let outline = plan(&half);
    b.with_facets(|b| b.loft_z(&outline, &levels));
    b.mirror_y(|b| {
        dark_plate(b);
        let f = Frame::new(
            v3(1.7, 1.95, 3.85),
            v3(-1.0, 0.08, -0.12),
            v3(0.0, 1.0, 0.55),
        );
        let plates = Course {
            count: 2,
            step: 2.3,
            len: 2.6,
            half: 0.5,
            tip: 1.0,
            thick: 0.18,
            tail: 0.9,
        }
        .lay(b, &f);
        if b.fine() {
            red_slot(
                b,
                plates[0].0.at(plates[0].1 - 0.2, 0.0, -0.02),
                plates[0].0.n,
                plates[0].0.v,
                0.4,
                0.04,
            );
        }
        red_slot(
            b,
            v3(2.3, 1.2, 3.85),
            v3(0.8, 0.6, 0.0),
            Vec3::Z,
            0.32,
            0.12,
        );
    });
    if b.fine() {
        red_slot(b, v3(-3.3, 0.0, 3.4), -Vec3::X, Vec3::Y, 1.0, 0.05);
        collar(b, v3(-2.9, 0.0, ROOF - 0.05), Vec3::Y, 0.25, 1.6);
    }
    b.paint(TEAM);
    b.face(&[
        v3(-1.0, 0.0, ROOF + 0.01),
        v3(-2.2, 0.75, ROOF + 0.01),
        v3(-2.2, -0.75, ROOF + 0.01),
    ]);
}

/// The gun, in the unit's frame at rest (it pitches about `PIVOT`): its trunnion and a
/// plated mantlet in the turret's face, a faceted barrel (`with_recoil`) banded in bronze
/// with a red ring in its mouth, and its projectors reaching past the mouth to hold the
/// charge at `MUZZLE`.
pub(super) fn gun(b: &mut MeshBuilder) {
    let p = PIVOT;
    let fine = b.fine();
    collar(b, p, Vec3::Y, 0.5, 2.2);
    dark_plate(b);
    b.with_facets(|b| {
        b.chamfered_box(v3(p.x + 1.0, 0.0, p.z), v3(1.1, 2.4, 1.15), 0.3);
    });
    let mouth = 6.5;
    b.with_recoil(|b| {
        dark_plate(b);
        let sides = b.sides(8);
        b.with_facets(|b| {
            b.cylinder_between(
                v3(p.x + 1.5, 0.0, p.z),
                v3(mouth, 0.0, p.z),
                0.42,
                0.36,
                sides,
            );
        });
        if fine {
            for x in [p.x + 2.4, p.x + 3.4] {
                collar(b, v3(x, 0.0, p.z), Vec3::X, 0.47, 0.25);
            }
            red_slot(
                b,
                v3(p.x + 3.0, 0.0, p.z + 0.4),
                Vec3::Z,
                Vec3::X,
                1.4,
                0.04,
            );
        }
        metal(b);
        b.cylinder_between(
            v3(mouth - 0.3, 0.0, p.z),
            v3(mouth + 0.1, 0.0, p.z),
            0.44,
            0.4,
            sides,
        );
        b.paint(GLOW_LASER);
        hoop_on(
            b,
            v3(mouth + 0.08, 0.0, p.z),
            Vec3::X,
            0.3,
            0.12,
            0.1,
            sides,
        );
    });
    tri(b);
}

/// A projector's emitter: a bronze boss at `at` with a red lens turned to the charge.
fn emitter(b: &mut MeshBuilder, at: Vec3, r: f32) {
    let d = (MUZZLE - at).normalize();
    let sides = b.sides(8);
    metal(b);
    b.cylinder_between(at - d * r * 0.6, at, r, r * 0.85, sides);
    b.paint(GLOW_LASER);
    b.cylinder_between(at, at + d * r * 0.25, r * 0.7, r * 0.5, sides);
}

/// A plated bar through `points` (each its width across `across` and its depth), square
/// in section: a projector prong.
fn prong(b: &mut MeshBuilder, points: &[(Vec3, Vec2)], across: Vec3) {
    let rings: Vec<Vec<Vec3>> = points
        .iter()
        .enumerate()
        .map(|(i, &(c, size))| {
            let d = if i + 1 < points.len() {
                points[i + 1].0 - c
            } else {
                c - points[i - 1].0
            }
            .normalize();
            let side = (across - d * across.dot(d)).normalize();
            let up = d.cross(side);
            [[1.0, -1.0], [-1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]]
                .iter()
                .map(|&[s, u]| c + side * (s * size.x * 0.5) + up * (u * size.y * 0.5))
                .collect()
        })
        .collect();
    dark_plate(b);
    b.with_facets(|b| b.loft(&rings, true, true));
}

/// The claw: three prongs round the barrel, one over it and two below either side, each
/// with a red emitter turned in on the charge, bound behind by a bronze ring.
fn tri(b: &mut MeshBuilder) {
    let z = PIVOT.z;
    let tip = MUZZLE.x + 0.35;
    for k in 0..3 {
        let a = std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * k as f32 / 3.0;
        let d = v3(0.0, a.cos(), a.sin());
        let across = v3(0.0, -a.sin(), a.cos());
        let at = |x: f32, r: f32| v3(x, 0.0, z) + d * r;
        prong(
            b,
            &[
                (at(2.5, 0.85), Vec2::new(0.36, 0.7)),
                (at(5.8, 1.02), Vec2::new(0.3, 0.6)),
                (at(tip, 0.88), Vec2::new(0.24, 0.42)),
            ],
            across,
        );
        emitter(b, at(MUZZLE.x - 0.25, 0.72), 0.18);
    }
    metal(b);
    hoop_on(b, v3(4.6, 0.0, z), Vec3::X, 0.98, 0.3, 0.3, b.sides(12));
}
