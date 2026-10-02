//! The Quarrel, the Regency's tech 1 fighter: a jet with a Plasmeric Repeater pod either
//! side of its nose (one twin weapon, both pods turning a little together as the
//! blueprint's `arc` lets them). Its wing is a crescent, swept harder outboard, the tips
//! turned down; two fins are raked out over the tail and two turned down under it (the
//! user's pick of three, 2026-10-02).
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plates lapped over bronze, a nose blade,
//! red optics, red heat in the exhausts. Authored at blueprint scale (radius 3.6, height
//! 1.6): model metres are unit metres, and `MUZZLE` is `regency_t1_fighter`'s.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::library::ModelDef;
use crate::part;

use super::super::guns::repeater;
use super::super::kit::{dark_plate, metal, v3};
use super::{body, chevron, exhaust, fin, flat, lap, optics, sheet, st, tail_face, St};

const RADIUS: f32 = 3.6;
const HEIGHT: f32 = 1.6;

pub(crate) const MODELS: &[ModelDef] =
    &[ModelDef::new("regency_quarrel", RADIUS, HEIGHT, crescent)];

/// The repeater's two muzzles: the unit file's weapon `muzzles` (the left one; the right
/// mirrors it).
const MUZZLE: Vec3 = Vec3::new(3.3, 0.55, 0.62);
/// Where the pods turn: under the cockpit's place, on the centre line.
const PIVOT: Vec3 = Vec3::new(1.2, 0.0, 0.62);
/// The level the wings lie at.
const Z: f32 = 0.7;

const BODY: [St; 5] = [
    st(-3.0, 0.24, 0.5, Z, 0.92),
    st(-1.8, 0.4, 0.36, Z, 1.12),
    st(0.0, 0.42, 0.34, Z, 1.16),
    st(1.4, 0.3, 0.42, Z, 1.04),
    st(2.2, 0.16, 0.55, Z, 0.88),
];

/// What every wing shares: the body, its plates and nose blade, the exhausts, optics and
/// mark, and the pods.
fn fuselage(b: &mut MeshBuilder) {
    b.set_turret_pivot(PIVOT);
    if b.coarse() {
        dark_plate(b);
        body(b, &BODY, 0.0, Some(v3(3.7, 0.0, Z)));
        chevron(b, v3(0.2, 0.0, 1.17), 0.7);
        // A fin over the tail, seen edge on from above.
        tail_face(b, 0.0, -1.9, -3.2, 1.0, 1.6);
        // The pods as one bar across the nose, an end at each muzzle.
        b.with_part(part::TURRET, |b| {
            dark_plate(b);
            b.beam(
                MUZZLE - Vec3::X * 0.3,
                MUZZLE * v3(1.0, -1.0, 1.0) - Vec3::X * 0.3,
                glam::Vec2::new(0.5, 0.2),
                glam::Vec2::new(0.5, 0.2),
            );
        });
        return;
    }
    metal(b);
    body(b, &BODY, 0.0, Some(v3(2.8, 0.0, Z)));
    // Plates down the back and the flanks over the bronze.
    lap(
        b,
        v3(1.5, 0.0, 1.06),
        -Vec3::X,
        Vec3::Z,
        4,
        0.95,
        1.15,
        0.36,
        0.08,
    );
    b.mirror_y(|b| {
        lap(
            b,
            v3(1.6, 0.27, 0.88),
            v3(-1.0, 0.08, -0.03),
            v3(0.0, 0.8, 0.6),
            3,
            1.1,
            1.3,
            0.14,
            0.06,
        )
    });
    // The nose blade.
    dark_plate(b);
    sheet(
        b,
        &[
            [1.6, 0.16],
            [2.4, 0.24],
            [3.7, 0.0],
            [2.4, -0.24],
            [1.6, -0.16],
        ],
        Z - 0.04,
        0.08,
    );
    b.mirror_y(|b| exhaust(b, v3(-3.15, 0.17, Z), 0.14, 0.4));
    optics(
        b,
        v3(1.95, 0.2, 0.84),
        v3(0.2, 0.8, 0.4),
        v3(1.0, -0.1, -0.1),
        0.36,
        0.06,
    );
    chevron(b, v3(-0.3, 0.0, 1.25), 0.55);
    // The pods, on short plated pylons off the cheeks.
    b.with_part(part::TURRET, |b| {
        b.mirror_y(|b| {
            repeater(b, MUZZLE - Vec3::X * 1.7, MUZZLE, 0.16);
            dark_plate(b);
            b.beam(
                v3(1.9, 0.25, 0.64),
                v3(1.9, MUZZLE.y - 0.08, 0.64),
                glam::Vec2::new(0.5, 0.08),
                glam::Vec2::new(0.4, 0.08),
            );
        });
    });
}

/// The crescent wing.
const CRESCENT: [[f32; 2]; 7] = [
    [0.6, 0.38],
    [0.0, 1.3],
    [-1.0, 2.2],
    [-2.1, 2.85],
    [-2.5, 2.8],
    [-1.75, 1.6],
    [-1.8, 0.38],
];

fn crescent(b: &mut MeshBuilder, _tech: u8) {
    fuselage(b);
    dark_plate(b);
    b.mirror_y(|b| {
        if b.coarse() {
            flat(b, &CRESCENT, Z);
            return;
        }
        sheet(b, &CRESCENT, Z - 0.04, 0.07);
        lap(
            b,
            v3(0.3, 0.6, Z + 0.03),
            v3(-1.0, 1.0, 0.0),
            Vec3::Z,
            2,
            1.0,
            1.15,
            0.2,
            0.05,
        );
        fin(
            b,
            v3(-2.2, 2.84, Z - 0.02),
            0.3,
            &[[0.1, 0.0], [-0.3, 0.0], [-0.42, -0.42], [-0.05, -0.3]],
            0.05,
        );
        // Down-turned fins under the tail.
        fin(
            b,
            v3(-2.2, 0.2, 0.45),
            0.45,
            &[[0.5, 0.0], [-0.7, 0.0], [-0.9, -0.4], [-0.2, -0.3]],
            0.05,
        );
        // Twin fins raked out over the tail.
        fin(
            b,
            v3(-2.0, 0.22, 1.05),
            2.7,
            &[[0.2, 0.0], [-0.8, 0.0], [-1.0, -0.52], [-0.5, -0.5]],
            0.05,
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_the_regency_checks() {
        let muzzles = [MUZZLE.to_array(), (MUZZLE * v3(1.0, -1.0, 1.0)).to_array()];
        for key in MODELS.iter().map(|m| m.key) {
            crate::regency::check(key, RADIUS, HEIGHT, None, &muzzles);
            super::super::tests::reads_as_a_regency_jet(key);
        }
    }

    #[test]
    fn the_unit_files_guns_are_the_models() {
        use super::super::tests::{blueprint, v};
        let (b, id) = blueprint("regency_t1_fighter");
        let bp = b.unit(id);
        assert_eq!(bp.visual.mesh, "regency_quarrel");
        assert!(
            (bp.radius.to_f32() - RADIUS).abs() < 1e-3
                && (bp.height.to_f32() - HEIGHT).abs() < 1e-3
        );
        let gun = &bp.weapons[0];
        let muzzles: Vec<Vec3> = gun.muzzles.iter().map(|&m| v(m)).collect();
        assert_eq!(muzzles.len(), 2);
        assert!(muzzles.iter().any(|m| m.distance(MUZZLE) < 1e-3));
        assert!(muzzles
            .iter()
            .any(|m| m.distance(MUZZLE * v3(1.0, -1.0, 1.0)) < 1e-3));
        // The model marks its two exhausts for whatever draws their heat.
        assert_eq!(
            crate::build_model("regency_quarrel")
                .unwrap()
                .exhausts
                .len(),
            2
        );
    }
}
