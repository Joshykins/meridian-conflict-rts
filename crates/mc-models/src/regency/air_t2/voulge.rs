//! The Voulge, the Regency's tech 2 long-range strike drone (`regency_t2_strike_drone` in
//! `data/factions/regency/units/air_t2.ron`): a slow, thin-skinned airframe that circles
//! its mark far out and lets Gravitic Seekers go from four open cages along its back.
//!
//! Not the shape of a long-winged spy drone (no straight wing, no bulb nose, no V tail):
//! a blade of a nose, wings swept and lapped with plates, tips turned down, one drive
//! burning red aft. Each cage is a seam-dark socket in the back with three bronze tines
//! rising round the charge it holds; the charge's middle is the unit file's muzzle.
//! Authored at blueprint scale (radius 7.5, height 2.2).
//!
//! From above, a sawtooth: a flying wing whose trailing edge is cut in a W, fins turned
//! down at its tips.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::library::ModelDef;
use crate::material::*;

use super::super::kit::{seam, v3};
use super::super::machine::{shaft, Course};
use super::jet::{
    body, down_fin, drive, nose_blade, optics, plates, st, team_mark, tip, wing, workings, Station,
};

pub(super) const RADIUS: f32 = 7.5;
pub(super) const HEIGHT: f32 = 2.2;

pub(crate) const MODELS: &[ModelDef] = &[ModelDef::new(
    "regency_strike_drone",
    RADIUS,
    HEIGHT,
    voulge,
)];

/// The port cages' charges, fore then aft: the unit file's muzzles (with their mirrors).
pub(super) const CAGES: [Vec3; 2] = [Vec3::new(-0.9, 0.55, 1.95), Vec3::new(-2.0, 0.55, 1.85)];

const BODY: [Station; 5] = [
    st(-3.6, 0.5, 1.35, 1.0, 0.75),
    st(-2.2, 0.95, 1.66, 1.05, 0.55),
    st(0.0, 1.0, 1.74, 1.05, 0.45),
    st(2.2, 0.66, 1.6, 1.05, 0.55),
    tip(4.0, 1.05),
];
/// One cage at `c`: a socket let into the back under it, three bronze tines rising round
/// the charge, and the charge, a red ball.
fn cage(b: &mut MeshBuilder, c: Vec3, r: f32) {
    {
        seam(b);
        let sides = b.sides(8);
        b.prism(c - Vec3::Z * (r + 0.32), sides, r * 1.3, r * 1.15, 0.24);
        if b.fine() {
            for k in 0..3 {
                let a = std::f32::consts::TAU * (k as f32 + 0.25) / 3.0;
                let out = v3(a.cos(), a.sin(), 0.0);
                shaft(
                    b,
                    c - Vec3::Z * (r + 0.1) + out * (r * 1.05),
                    c + Vec3::Z * (r * 0.4) + out * (r * 1.15),
                    0.035,
                );
            }
        }
    }
    b.paint(GLOW_LASER);
    let sides = if b.fine() { 8 } else { 5 };
    b.spheroid(
        c,
        Vec3::splat(r * 0.72),
        sides,
        if b.fine() { 4 } else { 3 },
    );
}

/// The cages, the spine's plates and workings, the drive, the optics, the team's mark.
fn common(b: &mut MeshBuilder, aft: f32) {
    b.mirror_y(|b| {
        if b.coarse() {
            // Far off, one red strip over a side's two cages.
            let (fore, aft) = (CAGES[0], CAGES[1]);
            b.paint(GLOW_LASER);
            b.face(&[
                aft + v3(-0.2, -0.2, 0.0),
                fore + v3(0.2, -0.2, 0.0),
                fore + v3(0.2, 0.2, 0.0),
                aft + v3(-0.2, 0.2, 0.0),
            ]);
        } else {
            for c in CAGES {
                cage(b, c, 0.22);
            }
        }
        workings(b, v3(1.4, 0.72, 1.48), v3(aft + 0.9, 0.62, 1.38), 0.1, 3);
    });
    plates(
        b,
        v3(0.2, 0.0, 1.75),
        v3(-1.0, 0.0, -0.05),
        Vec3::Z,
        Course {
            count: 3,
            step: 1.1,
            len: 1.4,
            half: 0.3,
            tip: 0.0,
            thick: 0.07,
            tail: 0.8,
        },
    );
    drive(b, v3(aft - 0.05, 0.0, 1.05), 0.42, 0.5);
    optics(b, v3(2.9, 0.42, 1.34), Vec3::Y, v3(1.0, 0.0, -0.15), 0.5);
    team_mark(b, v3(1.0, 0.0, 1.72), Vec3::X, Vec3::Z, 1.2, 0.35);
}

/// The Voulge.
fn voulge(b: &mut MeshBuilder, _tech: u8) {
    body(b, &BODY);
    nose_blade(
        b,
        &[
            [6.2, 0.95],
            [4.0, 1.3],
            [2.6, 1.15],
            [2.8, 0.6],
            [4.2, 0.62],
        ],
        0.07,
    );
    b.mirror_y(|b| {
        let droop = 0.06;
        wing(
            b,
            &[
                [2.4, 0.6],
                [-2.2, 6.2],
                [-3.4, 6.4],
                [-2.5, 4.2],
                [-3.7, 2.6],
                [-2.7, 0.6],
            ],
            1.1,
            droop,
            0.09,
        );
        down_fin(
            b,
            &[[-2.3, 0.0], [-3.3, 0.0], [-3.7, -0.6], [-2.9, -0.6]],
            6.15,
            1.1 - 6.15 * droop,
            -0.3,
        );
        plates(
            b,
            v3(1.4, 1.0, 1.16),
            v3(-0.8, 1.0, -0.06),
            Vec3::Z,
            Course {
                count: 4,
                step: 1.05,
                len: 1.4,
                half: 0.38,
                tip: 0.35,
                thick: 0.08,
                tail: 0.6,
            },
        );
    });
    common(b, -3.6);
}

#[cfg(test)]
mod tests {
    use super::super::jet::check;
    use super::*;

    fn all() -> Vec<Vec3> {
        CAGES
            .iter()
            .flat_map(|&c| [c, c * v3(1.0, -1.0, 1.0)])
            .collect()
    }

    #[test]
    fn fits_the_airframe_checks() {
        check::airframe("regency_strike_drone", RADIUS, HEIGHT, &all(), GLOW_LASER);
    }

    #[test]
    fn the_unit_files_cages_are_the_models() {
        let bp = check::blueprint(
            "regency_t2_strike_drone",
            "regency_strike_drone",
            RADIUS,
            HEIGHT,
        );
        let drawn = check::muzzles(&bp);
        assert_eq!(drawn.len(), 4);
        for m in all() {
            assert!(
                drawn.iter().any(|d| d.distance(m) < 1e-3),
                "no muzzle at the cage {m}"
            );
        }
    }
}
