//! The Trident, the Regency's tech 2 torpedo bomber (`regency_t2_torpedo_bomber` in
//! `data/factions/regency/units/air_t2.ron`): it comes down to the wave tops on its run
//! and lets a Gravitic Torpedo go from a cradle under each outer prong, a charge of plasma
//! held between two bronze tines; the charge's middle is the unit file's muzzle. A sonar
//! blister under its chin hears the dived hulls it hunts.
//!
//! Authored at blueprint scale (radius 7.2, height 3.0).
//!
//! Variants: base the trident (a long middle body and two shorter prongs reaching ahead
//! off the wing roots, a cradle under each prong's head: three points forward from
//! above), `~b` the manta (one wide lifting body, two horn blades forward, the cradles
//! under its leading edge), `~c` the twin boom (two long booms with a nose blade each, a
//! short pod between them on a straight blade of a wing, the cradles under the booms).

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

pub(crate) const MODELS: &[ModelDef] = &[
    ModelDef::new("regency_torpedo_bomber", RADIUS, HEIGHT, trident),
    ModelDef::new("regency_torpedo_bomber~b", RADIUS, HEIGHT, manta),
    ModelDef::new("regency_torpedo_bomber~c", RADIUS, HEIGHT, twin_boom),
];

/// The port cradle's charge: the unit file's muzzle (the starboard one mirrors it).
pub(super) const TORPEDO: Vec3 = Vec3::new(3.4, 1.9, 0.55);

const BODY: [Station; 6] = [
    st(-5.4, 0.5, 1.9, 1.4, 1.0),
    st(-4.0, 0.95, 2.3, 1.45, 0.75),
    st(-1.0, 1.1, 2.5, 1.45, 0.62),
    st(2.0, 0.85, 2.4, 1.45, 0.66),
    st(4.4, 0.45, 2.0, 1.45, 0.95),
    tip(5.8, 1.45),
];
/// A prong (or boom) about its own axis, tail to head.
const PRONG: [Station; 5] = [
    st(-2.6, 0.3, 1.25, 0.95, 0.75),
    st(-1.2, 0.48, 1.45, 0.95, 0.6),
    st(1.2, 0.5, 1.45, 0.95, 0.6),
    st(3.0, 0.32, 1.3, 0.95, 0.72),
    tip(4.4, 0.98),
];
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

/// The middle body's dress: nose blade, optics, spine plates over bronze, sonar, the
/// team's mark.
fn middle(b: &mut MeshBuilder, stations: &[Station], nose: &[[f32; 2]]) {
    body(b, stations);
    nose_blade(b, nose, 0.08);
    optics(b, v3(4.6, 0.38, 1.85), Vec3::Y, v3(1.0, 0.0, -0.2), 0.6);
    plates(
        b,
        v3(1.8, 0.0, 2.42),
        v3(-1.0, 0.0, -0.06),
        Vec3::Z,
        Course {
            count: 4,
            step: 1.3,
            len: 1.8,
            half: 0.6,
            tip: 0.0,
            thick: 0.12,
            tail: 0.9,
        },
    );
    b.mirror_y(|b| workings(b, v3(1.2, 0.8, 2.15), v3(-3.6, 0.85, 1.95), 0.13, 3));
    sonar(b, v3(2.6, 0.0, 0.6));
    team_mark(b, v3(2.8, 0.0, 2.38), Vec3::X, Vec3::Z, 1.3, 0.4);
}

/// A prong (port) about `y`, its head reaching to `PRONG`'s tip, a blade under its head and
/// the cradle hung below it.
fn prong(b: &mut MeshBuilder, stations: &[Station], y: f32, z: f32, drive_x: f32) {
    pod(b, stations, y, z);
    let head = stations[stations.len() - 1].x;
    if !b.coarse() {
        dark_blade_tip(b, head, y, z);
    }
    drive(b, v3(drive_x, y, z + 0.95), 0.32, 0.45);
    cradle(
        b,
        v3(TORPEDO.x - 2.0, y, TORPEDO.z),
        TORPEDO.with_y(y),
        0.26,
    );
}

/// A prong or boom about `y`, lifted `z`: far off, a flat plan of it seen from above.
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

/// A short keel blade reaching ahead of a prong's head.
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

/// Base: the trident.
fn trident(b: &mut MeshBuilder, _tech: u8) {
    middle(
        b,
        &BODY,
        &[[7.6, 1.35], [5.6, 1.8], [4.0, 1.6], [4.2, 0.95], [5.8, 1.0]],
    );
    drive(b, v3(-5.45, 0.0, 1.45), 0.4, 0.5);
    b.mirror_y(|b| {
        // The root wing joining the middle to the prong, then the outer wing.
        wing(
            b,
            &[[1.0, 0.9], [0.4, 1.9], [-3.2, 1.9], [-4.4, 0.9]],
            1.5,
            0.0,
            0.1,
        );
        prong(b, &PRONG, 1.9, 0.0, -2.75);
        let droop = 0.06;
        wing(
            b,
            &[[0.4, 2.3], [-3.2, 6.2], [-4.3, 6.2], [-2.8, 2.3]],
            1.5,
            droop,
            0.08,
        );
        if !b.coarse() {
            down_fin(
                b,
                &[[-3.2, 0.0], [-4.3, 0.0], [-4.7, -0.8], [-3.9, -0.8]],
                6.15,
                1.5 - 6.15 * droop,
                -0.3,
            );
        }
        plates(
            b,
            v3(-0.4, 3.0, 1.38),
            v3(-1.0, 0.75, -0.06),
            Vec3::Z,
            Course {
                count: 3,
                step: 1.0,
                len: 1.6,
                half: 0.45,
                tip: 0.35,
                thick: 0.08,
                tail: 0.6,
            },
        );
    });
}

/// B: the manta.
fn manta(b: &mut MeshBuilder, _tech: u8) {
    body(b, &BODY);
    optics(b, v3(4.6, 0.38, 1.85), Vec3::Y, v3(1.0, 0.0, -0.2), 0.6);
    plates(
        b,
        v3(1.8, 0.0, 2.42),
        v3(-1.0, 0.0, -0.06),
        Vec3::Z,
        Course {
            count: 4,
            step: 1.3,
            len: 1.8,
            half: 0.6,
            tip: 0.0,
            thick: 0.12,
            tail: 0.9,
        },
    );
    sonar(b, v3(2.6, 0.0, 0.6));
    team_mark(b, v3(2.8, 0.0, 2.38), Vec3::X, Vec3::Z, 1.3, 0.4);
    drive(b, v3(-5.45, 0.0, 1.45), 0.4, 0.5);
    b.mirror_y(|b| {
        let droop = 0.1;
        wing(
            b,
            &[
                [4.2, 0.8],
                [1.2, 4.2],
                [-1.6, 6.0],
                [-3.0, 5.7],
                [-3.6, 2.0],
                [-5.0, 0.8],
            ],
            1.5,
            droop,
            0.14,
        );
        // A horn blade forward off each side of the head.
        nose_blade_at(
            b,
            0.95,
            &[
                [7.0, 1.3],
                [5.0, 1.72],
                [3.4, 1.55],
                [3.6, 0.95],
                [5.2, 1.0],
            ],
        );
        workings(b, v3(1.2, 0.8, 2.15), v3(-3.6, 0.85, 1.95), 0.13, 3);
        plates(
            b,
            v3(1.8, 1.4, 1.48),
            v3(-1.0, 0.7, -0.1),
            Vec3::Z,
            Course {
                count: 4,
                step: 1.0,
                len: 1.6,
                half: 0.5,
                tip: 0.35,
                thick: 0.09,
                tail: 0.6,
            },
        );
        if !b.coarse() {
            down_fin(
                b,
                &[[-3.1, 0.0], [-3.6, 0.0], [-4.2, -0.75], [-3.6, -0.75]],
                5.6,
                1.5 - 5.6 * droop + 0.05,
                -0.3,
            );
        }
        cradle(b, v3(TORPEDO.x - 2.2, TORPEDO.y, TORPEDO.z), TORPEDO, 0.26);
        drive(b, v3(-3.65, 2.6, 1.25), 0.3, 0.4);
    });
}

/// A keel blade like the nose's, stood at `y` off the centreline.
fn nose_blade_at(b: &mut MeshBuilder, y: f32, outline: &[[f32; 2]]) {
    super::super::kit::dark_plate(b);
    blade(b, &side(outline, y, 0.0), Vec3::Y, 0.07, 0.7);
}

/// C: the twin boom.
fn twin_boom(b: &mut MeshBuilder, _tech: u8) {
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
    use super::*;

    #[test]
    fn fits_the_airframe_checks() {
        let muzzles = [TORPEDO, TORPEDO * v3(1.0, -1.0, 1.0)];
        for key in [
            "regency_torpedo_bomber",
            "regency_torpedo_bomber~b",
            "regency_torpedo_bomber~c",
        ] {
            let glow = crate::material::GLOW_LASER;
            super::super::jet::check::airframe(key, RADIUS, HEIGHT, &muzzles, glow);
        }
    }

    #[test]
    fn the_unit_files_cradles_are_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("regency_t2_torpedo_bomber").unwrap());
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        assert_eq!(bp.visual.mesh, "regency_torpedo_bomber");
        assert!((bp.radius.to_f32() - RADIUS).abs() < 1e-3);
        assert!((bp.height.to_f32() - HEIGHT).abs() < 1e-3);
        let drawn: Vec<Vec3> = bp.weapons[0].muzzles.iter().map(|&p| v(p)).collect();
        for m in [TORPEDO, TORPEDO * v3(1.0, -1.0, 1.0)] {
            assert!(
                drawn.iter().any(|d| d.distance(m) < 1e-3),
                "no muzzle at {m}"
            );
        }
    }
}
