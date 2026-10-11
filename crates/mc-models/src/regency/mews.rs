//! The Mews, the Regency air staging base (`regency_t1_airbase` and its upgrades), on a
//! 6 x 6 lot (72 m square). Aircraft come down on it to refuel and mend
//! (`mc_sim::staging`): nothing opens, they settle into the cradle in the middle and are
//! taken in.
//!
//! Built in the Ark's language (`loft`): three scaled arms reach out low across the lot
//! from the cradle like an open claw laid flat, one to the front (+x), two behind, each
//! with an armour ridge down its crest in the owner's colour and low spines along it.
//! The cradle is a plated drum with a glazed band where the nanites work on what sets
//! down in it, ringed by talon plates that lean in over the deck. Aircraft leave off
//! rails at the arms' tips (`staging.launch` in the unit file holds the same numbers as
//! `LAUNCH`).
//!
//! Each tier adds to it, never height: tech 2 (Mews II) a scaled collar round the cradle
//! binding the arms' roots, and glazed courses down the arms; tech 3 (Mews III) a crown
//! of plated rings hung in the air over the cradle.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;

use super::kit::{dark_plate, metal, nanite_plate, seam, v3};
use super::loft::*;
use super::machine::*;

/// The cradle's deck: where aircraft come down (`staging.land`).
const DECK: f32 = 3.0;
const CRADLE_R: f32 = 9.5;
/// The arms' bearings: one to the front, two behind.
const ARMS: [f32; 3] = [0.0, 120.0, -120.0];
/// The launch rails' distance out along each arm, and their height.
const RAIL_R: f32 = 30.0;
const RAIL_Z: f32 = 6.0;

/// Where aircraft leave from, and their heading: the rails at the arms' tips (the unit
/// file's `staging.launch`).
#[cfg(test)]
const LAUNCH: [(f32, f32, f32, f32); 3] = [
    (30.0, 0.0, 6.0, 0.0),
    (-15.0, 25.98, 6.0, 120.0),
    (-15.0, -25.98, 6.0, -120.0),
];

fn polar(deg: f32, r: f32, z: f32) -> Vec3 {
    let a = deg.to_radians();
    v3(a.cos() * r, a.sin() * r, z)
}

/// An arm along bearing `deg`: out from under the cradle's rim, low over the lot all the
/// way, thinning to its tip.
fn arm(deg: f32) -> [St; 5] {
    let p = |r: f32, z: f32| polar(deg, r, z);
    [
        St {
            c: p(CRADLE_R + 1.0, 3.6),
            w: 5.0,
            h: 2.8,
        },
        St {
            c: p(17.0, 4.2),
            w: 4.2,
            h: 2.6,
        },
        St {
            c: p(24.0, 4.4),
            w: 3.2,
            h: 2.2,
        },
        St {
            c: p(30.0, 4.0),
            w: 2.0,
            h: 1.6,
        },
        St {
            c: p(32.5, 3.4),
            w: 0.4,
            h: 0.4,
        },
    ]
}

/// The cradle: a plated drum on the lot with a glazed band round it, its deck at `DECK`,
/// and talon plates standing round it leaning in over the deck.
fn cradle(b: &mut MeshBuilder) {
    let sides = b.sides(12);
    dark_plate(b);
    b.cylinder_between(
        Vec3::ZERO,
        Vec3::Z * 1.2,
        CRADLE_R + 1.6,
        CRADLE_R + 1.2,
        sides,
    );
    nanite_plate(b);
    b.cylinder_between(
        Vec3::Z * 1.2,
        Vec3::Z * (DECK - 0.5),
        CRADLE_R + 0.4,
        CRADLE_R + 0.2,
        sides,
    );
    dark_plate(b);
    b.cylinder_between(
        Vec3::Z * (DECK - 0.5),
        Vec3::Z * DECK,
        CRADLE_R + 0.6,
        CRADLE_R,
        sides,
    );
    if b.mid() {
        seam(b);
        hoop(
            b,
            Vec3::Z * (DECK + 0.03),
            CRADLE_R * 0.55,
            0.5,
            0.06,
            sides,
        );
        metal(b);
        b.cylinder_between(
            Vec3::Z * DECK,
            Vec3::Z * (DECK + 0.2),
            CRADLE_R * 0.3,
            CRADLE_R * 0.28,
            sides,
        );
    }
    // The talons: one plate a sixth of the way round, its spike leaning in over the deck.
    let talons = 6;
    for k in 0..talons {
        let a = TAU * (k as f32 + 0.5) / talons as f32;
        let out = v3(a.cos(), a.sin(), 0.0);
        let foot = out * (CRADLE_R + 0.8) + Vec3::Z * 0.6;
        let f = Frame::new(foot, Vec3::Z - out * 0.75, out);
        dark_plate(b);
        let len = 5.0;
        armour(b, &f, &swept(len, 1.6, 0.0, 0.35), 0.7);
        if b.fine() {
            collar(
                b,
                foot + out * 0.4 + Vec3::Z * 0.2,
                out.cross(Vec3::Z),
                0.7,
                2.2,
            );
        }
    }
}

/// A launch rail at a horn's tip or an arm's: a bronze cradle the aircraft sits on,
/// pointing along `heading`, with a red slot down its throat.
fn rail(b: &mut MeshBuilder, at: Vec3, heading: f32) {
    let d = polar(heading, 1.0, 0.0);
    let across = Vec3::Z.cross(d);
    metal(b);
    for s in [-1.0, 1.0] {
        b.beam(
            at + across * (1.1 * s) - d * 4.0 - Vec3::Z * 0.6,
            at + across * (1.1 * s) + d * 1.5 - Vec3::Z * 0.6,
            Vec2::new(0.5, 0.5),
            Vec2::new(0.4, 0.4),
        );
    }
    if b.fine() {
        red_slot(b, at - Vec3::Z * 0.75 - d * 1.2, Vec3::Z, d, 4.0, 0.5);
    }
}

pub(super) fn mews(b: &mut MeshBuilder, tech: u8) {
    if b.coarse() {
        for deg in ARMS {
            coarse_body(b, &arm(deg), 3);
        }
        dark_plate(b);
        b.cylinder_between(Vec3::ZERO, Vec3::Z * DECK, CRADLE_R + 1.2, CRADLE_R, 4);
        team(
            b,
            polar(180.0, 3.0, DECK + 0.1),
            polar(180.0, 7.0, DECK + 0.1),
            1.2,
        );
        if tech >= 3 {
            // The crown, far off: a plate seen from above.
            dark_plate(b);
            let corner = |deg: f32| polar(deg, CRADLE_R * 0.8, 13.5);
            b.face(&[corner(45.0), corner(135.0), corner(225.0), corner(315.0)]);
        }
        return;
    }
    cradle(b);
    for deg in ARMS {
        let path = arm(deg);
        body(b, &path);
        let l = lengths(&path)[path.len() - 1];
        scales(b, &path, &[1, 2, 3, 4], 0.0, l - 1.0, 3.6, &[]);
        ridge(b, &path, l * 0.15, l * 0.7);
        spines(b, &path, 2.0, l * 0.75, 2, 3.0);
        rail(b, polar(deg, RAIL_R, RAIL_Z), deg);
    }
    tier(b, tech, 2, 0.2, collar_ring);
    tier(b, tech, 3, 0.25, crown);
}

/// Tech 2: a scaled collar round the cradle binding the arms' roots, and a glazed course
/// laid down each arm's flanks.
fn collar_ring(b: &mut MeshBuilder) {
    let r = CRADLE_R + 3.4;
    let ring: Vec<St> = (0..=18)
        .map(|k| St {
            c: polar(360.0 * k as f32 / 18.0, r, 2.4),
            w: 2.2,
            h: 1.8,
        })
        .collect();
    body(b, &ring);
    let l = lengths(&ring)[ring.len() - 1];
    scales(b, &ring, &[2, 3, 4], 0.0, l, 3.4, &[]);
    for deg in ARMS {
        let path = arm(deg);
        let l = lengths(&path)[path.len() - 1];
        scales(b, &path, &[1, 4], l * 0.2, l * 0.85, 4.4, &[1, 4]);
    }
}

/// Tech 3: plated rings hung in the air over the cradle, one over the other.
fn crown(b: &mut MeshBuilder) {
    let sides = b.sides(16);
    for (z, r) in [(10.5, CRADLE_R * 0.95), (13.5, CRADLE_R * 0.7)] {
        dark_plate(b);
        hoop(b, Vec3::Z * z, r, 1.6, 0.9, sides);
        nanite_plate(b);
        hoop(b, Vec3::Z * (z - 0.55), r, 0.9, 0.2, sides);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mews_fits_its_lot_at_every_tier() {
        for (tech, height) in [(1, 8.0), (2, 8.0), (3, 15.0)] {
            super::super::check_at("regency_mews", tech, 34.0, height, Some(6), &[]);
        }
    }

    #[test]
    fn the_unit_files_cradle_and_rails_are_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        for key in [
            "regency_t1_airbase",
            "regency_t2_airbase",
            "regency_t3_airbase",
        ] {
            let bp = blueprints.unit(blueprints.id_of(key).unwrap());
            assert_eq!(bp.visual.mesh, "regency_mews");
            let s = bp.staging.as_ref().expect("it stages aircraft");
            assert_eq!(
                Vec3::from(s.land.to_f32()),
                v3(0.0, 0.0, DECK),
                "{key}: land"
            );
            assert_eq!(s.launches as usize, LAUNCH.len(), "{key}: launches");
            for ((at, heading), (x, y, z, deg)) in s.launch.iter().zip(LAUNCH) {
                assert!(
                    Vec3::from(at.to_f32()).distance(v3(x, y, z)) < 0.05,
                    "{key}: {at:?}"
                );
                let steps = (deg.rem_euclid(360.0) / 360.0 * 65536.0).round() as i32;
                assert!(
                    (heading.0 as i32 - steps).abs() <= 1,
                    "{key}: heading {deg}"
                );
            }
            for &(x, y, z, deg) in &LAUNCH {
                let rail = polar(deg, RAIL_R, RAIL_Z);
                assert!(rail.distance(v3(x, y, z)) < 0.05, "rail at {deg}");
            }
        }
    }
}
