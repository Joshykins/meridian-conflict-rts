//! Design A, the spire: a plated spire on the apron, its faces clad in courses of plate
//! whose points sweep up it; three great rings turning round it on spoked races, each
//! the other way from the one under it; four plated buttresses from the lot's corners to
//! its waist; coil drums at their feet; the emitter's crown at its point.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::PLATING_DARK;
use crate::pattern;

use super::super::gravitic_engine::rotors::{self, Rotor};
use super::super::gravitic_engine::works::coil;
use super::super::kit::{dark_plate, metal};
use super::super::machine::{armour, collar, swept, Course, Frame};
use super::ground::{apron, dir, emitter_crown, intakes, limb, spokes};
use super::EMITTER;

/// The spire's section (radius to its corners) at heights up it; its top.
const SPIRE: [(f32, f32); 5] = [
    (3.0, 30.0),
    (100.0, 25.0),
    (190.0, 19.0),
    (262.0, 13.0),
    (272.0, 10.0),
];

/// The spire's radius to its corners at `z`.
fn spire_r(z: f32) -> f32 {
    for w in SPIRE.windows(2) {
        let ((z0, r0), (z1, r1)) = (w[0], w[1]);
        if z <= z1 {
            return r0 + (r1 - r0) * ((z - z0) / (z1 - z0)).clamp(0.0, 1.0);
        }
    }
    SPIRE[SPIRE.len() - 1].1
}

/// The rings turning round the spire.
fn rings() -> [Rotor; 3] {
    [
        Rotor {
            z0: 58.0,
            z1: 70.0,
            outer: 54.0,
            inner: 41.0,
            vanes: 16,
            rate: 0.1,
            lit: false,
        },
        Rotor {
            z0: 138.0,
            z1: 148.0,
            outer: 45.0,
            inner: 34.0,
            vanes: 14,
            rate: -0.16,
            lit: true,
        },
        Rotor {
            z0: 212.0,
            z1: 220.0,
            outer: 35.0,
            inner: 26.0,
            vanes: 12,
            rate: 0.22,
            lit: false,
        },
    ]
}

pub(in crate::regency) fn spire(b: &mut MeshBuilder, _tech: u8) {
    apron(b);
    if b.coarse() {
        dark_plate(b);
        b.prism(Vec3::Z * 2.0, 4, 28.0, 6.0, EMITTER.z - 2.0);
        return;
    }
    intakes(b, 36.0, 30.0);
    column(b);
    let rings = rings();
    rotors::build(b, &rings);
    for r in &rings {
        let z = (r.z0 + r.z1) * 0.5;
        b.with_orbit(Vec3::Z, r.rate, |b| spokes(b, r, spire_r(z)));
        // The race it turns on, round the spire.
        collar(b, Vec3::Z * z, Vec3::Z, spire_r(z) + 2.5, r.z1 - r.z0 + 4.0);
    }
    for k in 0..4 {
        let a = 45.0 + 90.0 * k as f32;
        let d = dir(a);
        limb(
            b,
            &[
                (d * 122.0 + Vec3::Z * 0.5, 9.0),
                (d * 92.0 + Vec3::Z * 48.0, 8.0),
                (d * 58.0 + Vec3::Z * 88.0, 6.5),
                (d * (spire_r(112.0) + 3.0) + Vec3::Z * 112.0, 5.0),
            ],
            d + Vec3::Z,
        );
        // A plated boot round its foot, an optic looking out of it.
        dark_plate(b);
        b.with_facets(|b| b.prism(d * 120.0, 6, 15.0, 11.0, 9.0));
        if b.fine() {
            b.paint(PLATING_DARK).pattern(pattern::EMBER_SLOT);
            let f = Frame::new(d * 133.0 + Vec3::Z * 1.0, Vec3::Z + d * 0.3, d);
            armour(b, &f, &swept(9.0, 4.0, 0.0, 0.7), 0.5);
        }
        coil(b, d * 74.0 + Vec3::Z * 6.0, dir(a + 90.0), 5.0, 22.0);
    }
    // The neck the crown stands on, out of the spire's point.
    metal(b);
    let sides = b.sides(12);
    b.cylinder_between(Vec3::Z * 268.0, EMITTER - Vec3::Z * 4.0, 7.0, 6.0, sides);
    emitter_crown(b, 6, 15.0);
}

/// The spire: eight faces drawn in as it rises, a course of plates laid up each face
/// below its middle and another over it, their points sweeping up.
fn column(b: &mut MeshBuilder) {
    let ring = |z: f32, r: f32| -> Vec<Vec3> {
        (0..8)
            .map(|i| dir(22.5 + 45.0 * i as f32) * r + Vec3::Z * z)
            .collect()
    };
    let rings: Vec<Vec<Vec3>> = SPIRE.iter().map(|&(z, r)| ring(z, r)).collect();
    dark_plate(b);
    b.with_facets(|b| b.loft(&rings, false, true));
    let flat = (22.5f32).to_radians().cos();
    for i in 0..8 {
        let d = dir(45.0 * i as f32);
        // Each face from its foot to its point, as a frame running up it.
        let face = |z: f32| d * (spire_r(z) * flat) + Vec3::Z * z;
        let courses: &[(f32, f32, Course)] = &[
            (
                6.0,
                140.0,
                Course {
                    count: 3,
                    step: 44.0,
                    len: 56.0,
                    half: 9.0,
                    tip: 0.0,
                    thick: 1.5,
                    tail: 16.0,
                },
            ),
            (
                150.0,
                258.0,
                Course {
                    count: 3,
                    step: 34.0,
                    len: 44.0,
                    half: 6.0,
                    tip: 0.0,
                    thick: 1.2,
                    tail: 14.0,
                },
            ),
        ];
        for (k, &(z0, z1, course)) in courses.iter().enumerate() {
            if !b.fine() && k == 1 {
                continue;
            }
            let (p, q) = (face(z0), face(z1));
            let f = Frame::new(p + d * 0.3, q - p, d);
            let course = if b.fine() {
                course
            } else {
                Course {
                    count: 2,
                    step: course.step * 1.5,
                    ..course
                }
            };
            dark_plate(b);
            b.with_facets(|b| {
                course.lay(b, &f);
            });
        }
    }
}
