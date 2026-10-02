//! The Pilum, the Regency's tech 2 interceptor (`regency_t2_interceptor` in
//! `data/factions/regency/units/air_t2.ron`): a fast jet that runs fliers down with
//! Gravitic Seekers, each a plasma charge held between two bronze tines until it is let go.
//!
//! A needle of a chined body behind a nose blade, swept plates lapped back down its spine
//! and over its wings, bronze workings in the gaps, red optics either side of the nose and
//! two drives burning red in bronze rings. From above, a crescent: the wings sweep harder
//! outboard and fall away into spikes, long canards ahead of them, fins turned down under
//! the tail, and a seeker cradle either side under the chin. Authored at blueprint scale
//! (radius 5.5, height 2.2): the cradles' charges are the unit file's muzzles.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::library::ModelDef;

use super::super::kit::v3;
use super::super::machine::Course;
use super::jet::{
    blade, body, cradle, down_fin, drive, nose_blade, optics, plan, plates, st, team_mark, tip,
    wing, workings, Station,
};

pub(super) const RADIUS: f32 = 5.5;
pub(super) const HEIGHT: f32 = 2.2;

pub(crate) const MODELS: &[ModelDef] =
    &[ModelDef::new("regency_interceptor", RADIUS, HEIGHT, pilum)];

/// The port cradle's charge: the unit file's muzzle (the starboard one mirrors it).
const SEEKER: Vec3 = Vec3::new(2.6, 0.55, 0.42);

/// The body, tail to the needle nose.
const BODY: [Station; 6] = [
    st(-4.5, 0.55, 1.45, 1.0, 0.7),
    st(-3.2, 0.85, 1.7, 1.05, 0.5),
    st(-1.0, 0.92, 1.85, 1.05, 0.4),
    st(1.5, 0.72, 1.8, 1.05, 0.45),
    st(3.6, 0.42, 1.55, 1.05, 0.66),
    tip(5.2, 1.06),
];

/// The nose blade, side on: a keel ahead of the needle, its edge raked down and back.
const NOSE: [[f32; 2]; 5] = [
    [6.6, 0.88],
    [5.0, 1.26],
    [3.4, 1.08],
    [3.6, 0.52],
    [5.0, 0.58],
];

/// The body, the nose blade and optics, the spine's plates over its bronze workings, two
/// drives and the team's mark.
fn common(b: &mut MeshBuilder) {
    body(b, &BODY);
    nose_blade(b, &NOSE, 0.07);
    optics(b, v3(3.9, 0.33, 1.36), Vec3::Y, v3(1.0, 0.0, -0.15), 0.55);
    plates(
        b,
        v3(1.3, 0.0, 1.8),
        v3(-1.0, 0.0, -0.04),
        Vec3::Z,
        Course {
            count: 4,
            step: 1.1,
            len: 1.5,
            half: 0.5,
            tip: 0.0,
            thick: 0.1,
            tail: 0.8,
        },
    );
    b.mirror_y(|b| {
        workings(b, v3(0.8, 0.62, 1.62), v3(-3.4, 0.72, 1.46), 0.11, 3);
        drive(b, v3(-4.55, 0.42, 1.05), 0.36, 0.5);
    });
    team_mark(b, v3(2.2, 0.0, 1.77), Vec3::X, Vec3::Z, 1.1, 0.32);
}

/// The Pilum.
fn pilum(b: &mut MeshBuilder, _tech: u8) {
    common(b);
    b.mirror_y(|b| {
        let inner = [[1.0, 0.55], [-0.6, 2.5], [-2.4, 2.5], [-3.4, 0.55]];
        wing(b, &inner, 1.05, 0.0, 0.08);
        // Outboard the wing falls away, swept harder, its tip drawn back into a spike.
        let outer = [
            [-0.6, 2.5],
            [-2.6, 4.1],
            [-4.2, 4.5],
            [-3.4, 3.6],
            [-2.4, 2.5],
        ];
        wing(b, &outer, 1.05 + 2.5 * 0.25, 0.25, 0.06);
        if !b.coarse() {
            down_fin(
                b,
                &[[-2.6, 0.0], [-4.2, 0.0], [-4.6, -0.5], [-3.6, -0.5]],
                0.6,
                0.72,
                -0.4,
            );
            blade(
                b,
                &plan(
                    &[[3.4, 0.35], [2.0, 1.9], [1.5, 1.9], [2.3, 0.35]],
                    1.22,
                    0.12,
                ),
                Vec3::Z,
                0.05,
                0.8,
            );
        }
        plates(
            b,
            v3(0.4, 1.0, 1.12),
            v3(-1.0, 0.9, -0.02),
            Vec3::Z,
            Course {
                count: 3,
                step: 0.9,
                len: 1.4,
                half: 0.38,
                tip: 0.3,
                thick: 0.08,
                tail: 0.6,
            },
        );
        cradle(b, v3(0.4, SEEKER.y, SEEKER.z), SEEKER, 0.2);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(m: Vec3) -> [Vec3; 2] {
        [m * v3(1.0, -1.0, 1.0), m]
    }

    #[test]
    fn fits_the_airframe_checks() {
        let glow = crate::material::GLOW_LASER;
        super::super::jet::check::airframe(
            "regency_interceptor",
            RADIUS,
            HEIGHT,
            &pair(SEEKER),
            glow,
        );
    }

    #[test]
    fn the_unit_files_cradles_are_the_models() {
        let bp = super::super::jet::check::blueprint(
            "regency_t2_interceptor",
            "regency_interceptor",
            RADIUS,
            HEIGHT,
        );
        let drawn = super::super::jet::check::muzzles(&bp);
        for m in pair(SEEKER) {
            assert!(
                drawn.iter().any(|d| d.distance(m) < 1e-3),
                "no muzzle at {m}"
            );
        }
    }
}
