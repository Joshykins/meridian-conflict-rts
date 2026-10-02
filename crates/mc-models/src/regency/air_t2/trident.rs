//! The Trident, the Regency's tech 2 torpedo bomber (`regency_t2_torpedo_bomber` in
//! `data/factions/regency/units/air_t2.ron`): it comes down to the wave tops on its run
//! and lets a Gravitic Torpedo go from a cradle under each boom, a charge of plasma
//! held between two bronze tines; the charge's middle is the unit file's muzzle. A sonar
//! blister under its chin hears the dived hulls it hunts.
//!
//! Authored at blueprint scale (radius 7.2, height 3.0).
//!
//! From above, three points forward: two long booms with a nose blade each, and a short
//! middle pod between them on a blade of a wing; the cradles hang under the booms' heads.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::library::ModelDef;

use super::super::kit::{metal, v3};
use super::super::machine::Course;
use super::jet::{
    blade, body, cradle, down_fin, drive, nose_blade, optics, plan, plates, side, st, team_mark,
    tip, wing, workings, Station,
};

pub(super) const RADIUS: f32 = 7.2;
pub(super) const HEIGHT: f32 = 3.0;

pub(crate) const MODELS: &[ModelDef] = &[ModelDef::new(
    "regency_torpedo_bomber",
    RADIUS,
    HEIGHT,
    trident,
)];

/// The port cradle's charge: the unit file's muzzle (the starboard one mirrors it).
pub(super) const TORPEDO: Vec3 = Vec3::new(3.4, 1.9, 0.55);

const BOOM: [Station; 6] = [
    st(-5.2, 0.32, 1.35, 1.05, 0.8),
    st(-3.8, 0.55, 1.6, 1.05, 0.65),
    st(-0.6, 0.6, 1.7, 1.05, 0.6),
    st(2.4, 0.5, 1.6, 1.05, 0.62),
    st(4.2, 0.3, 1.4, 1.05, 0.8),
    tip(5.4, 1.05),
];

/// The sonar blister under the chin: a dark bronze dome with a red slot across it.
fn sonar(b: &mut MeshBuilder, at: Vec3) {
    if b.coarse() {
        return;
    }
    metal(b);
    let sides = b.sides(8);
    b.spheroid(at, v3(0.7, 0.45, 0.3), sides, if b.fine() { 4 } else { 2 });
    if b.fine() {
        super::super::machine::red_slot(
            b,
            at + v3(0.45, 0.0, -0.12),
            v3(1.0, 0.0, -0.6),
            Vec3::Y,
            0.5,
            0.06,
        );
    }
}

/// A boom about `y`, lifted `z`: far off, a flat plan of it seen from above.
fn pod(b: &mut MeshBuilder, stations: &[Station], y: f32, z: f32) {
    if b.coarse() {
        super::super::kit::dark_plate(b);
        let (tail, head) = (stations[0], stations[stations.len() - 1]);
        let wide = stations.iter().map(|s| s.w).fold(0.0, f32::max);
        let top = z + stations.iter().map(|s| s.top).fold(0.0, f32::max);
        b.face(&[
            v3(tail.x, y - tail.w, top),
            v3(-0.6, y - wide, top),
            v3(head.x, y, top),
            v3(-0.6, y + wide, top),
            v3(tail.x, y + tail.w, top),
        ]);
        return;
    }
    b.at(v3(0.0, y, z), |b| body(b, stations));
}

/// A short keel blade reaching ahead of a boom's head.
fn dark_blade_tip(b: &mut MeshBuilder, head: f32, y: f32, z: f32) {
    super::super::kit::dark_plate(b);
    blade(
        b,
        &side(
            &[
                [head + 1.3, z + 0.85],
                [head - 0.4, z + 1.15],
                [head - 1.4, z + 0.95],
                [head - 1.0, z + 0.62],
            ],
            y,
            0.0,
        ),
        Vec3::Y,
        0.06,
        0.7,
    );
}

/// The Trident.
fn trident(b: &mut MeshBuilder, _tech: u8) {
    // The short middle pod: sonar, optics, the team's mark.
    let middle_pod = [
        st(-2.8, 0.4, 1.9, 1.45, 1.05),
        st(-1.4, 0.75, 2.4, 1.45, 0.75),
        st(1.4, 0.8, 2.45, 1.45, 0.7),
        st(3.2, 0.45, 2.1, 1.45, 0.9),
        tip(4.6, 1.45),
    ];
    body(b, &middle_pod);
    nose_blade(
        b,
        &[[6.2, 1.35], [4.4, 1.8], [3.0, 1.6], [3.2, 0.95], [4.6, 1.0]],
        0.07,
    );
    optics(b, v3(3.4, 0.36, 1.9), Vec3::Y, v3(1.0, 0.0, -0.2), 0.5);
    sonar(b, v3(1.6, 0.0, 0.62));
    team_mark(b, v3(0.4, 0.0, 2.42), Vec3::X, Vec3::Z, 1.3, 0.4);
    drive(b, v3(-2.85, 0.0, 1.45), 0.36, 0.45);
    b.mirror_y(|b| {
        // The middle wing between pod and boom, then the outer wing.
        wing(
            b,
            &[[0.8, 0.6], [0.6, 1.9], [-2.2, 1.9], [-2.4, 0.6]],
            1.55,
            0.0,
            0.1,
        );
        pod(b, &BOOM, TORPEDO.y, 0.0);
        if !b.coarse() {
            dark_blade_tip(b, 5.4, TORPEDO.y, 0.1);
        }
        drive(b, v3(-5.25, TORPEDO.y, 1.05), 0.3, 0.4);
        cradle(b, v3(TORPEDO.x - 2.0, TORPEDO.y, TORPEDO.z), TORPEDO, 0.26);
        workings(
            b,
            v3(2.0, TORPEDO.y, 1.68),
            v3(-4.0, TORPEDO.y, 1.58),
            0.12,
            3,
        );
        let droop = 0.08;
        wing(
            b,
            &[[0.2, 2.3], [-2.2, 6.0], [-3.3, 6.0], [-2.6, 2.3]],
            1.4,
            droop,
            0.08,
        );
        if !b.coarse() {
            down_fin(
                b,
                &[[-2.2, 0.0], [-3.3, 0.0], [-3.7, -0.7], [-2.9, -0.7]],
                5.95,
                1.4 - 5.95 * droop,
                -0.3,
            );
        }
        if !b.coarse() {
            down_fin(
                b,
                &[[-3.8, 0.0], [-5.2, 0.0], [-5.5, -0.5], [-4.6, -0.5]],
                TORPEDO.y + 0.35,
                0.82,
                -0.4,
            );
        }
        if !b.coarse() {
            blade(
                b,
                &plan(
                    &[[-1.8, 0.0], [-2.4, 2.3], [-3.2, 2.3], [-3.0, 0.0]],
                    1.5,
                    0.0,
                ),
                Vec3::Z,
                0.05,
                0.85,
            );
        }
        plates(
            b,
            v3(-0.6, 2.9, 1.3),
            v3(-1.0, 0.85, -0.08),
            Vec3::Z,
            Course {
                count: 3,
                step: 1.0,
                len: 1.4,
                half: 0.4,
                tip: 0.35,
                thick: 0.08,
                tail: 0.6,
            },
        );
    });
}

#[cfg(test)]
mod tests {
    use super::super::jet::check;
    use super::*;

    #[test]
    fn fits_the_airframe_checks() {
        let muzzles = [TORPEDO, TORPEDO * v3(1.0, -1.0, 1.0)];
        let glow = crate::material::GLOW_LASER;
        check::airframe("regency_torpedo_bomber", RADIUS, HEIGHT, &muzzles, glow);
    }

    #[test]
    fn the_unit_files_cradles_are_the_models() {
        let bp = check::blueprint(
            "regency_t2_torpedo_bomber",
            "regency_torpedo_bomber",
            RADIUS,
            HEIGHT,
        );
        let drawn = check::muzzles(&bp);
        for m in [TORPEDO, TORPEDO * v3(1.0, -1.0, 1.0)] {
            assert!(
                drawn.iter().any(|d| d.distance(m) < 1e-3),
                "no muzzle at {m}"
            );
        }
    }
}
