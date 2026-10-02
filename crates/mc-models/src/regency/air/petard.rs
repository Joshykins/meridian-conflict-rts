//! The Petard, the Regency's tech 1 bomber: a blade of a wing that lays a short stick of
//! Plasmeric Bombs out of a bay in its belly. A cleaver of a flying wing (the user's pick
//! of three, 2026-10-02): broad and swept round a keeled body, its trailing edge cranked,
//! the tips turned down, plates lapped along its leading edge.
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plates lapped over bronze, a nose blade,
//! red optics, red heat in the exhausts, a red slot where the bay is. Authored at
//! blueprint scale (radius 5.6, height 2.2): model metres are unit metres, and `BAY` is
//! `regency_t1_bomber`'s weapon `muzzle`.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::library::ModelDef;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::red_slot;
use super::{body, chevron, exhaust, fin, flat, lap, optics, sheet, st, St};

const RADIUS: f32 = 5.6;
const HEIGHT: f32 = 2.2;

pub(crate) const MODELS: &[ModelDef] = &[ModelDef::new("regency_petard", RADIUS, HEIGHT, cleaver)];

/// Where the bombs leave the bay: the unit file's weapon `muzzle`.
const BAY: Vec3 = Vec3::new(0.4, 0.0, 0.5);
/// The level the wings lie at.
const Z: f32 = 0.95;

const BODY: [St; 5] = [
    st(-3.7, 0.4, 0.75, Z, 1.3),
    st(-2.0, 0.75, 0.48, Z, 1.68),
    st(0.6, 0.8, 0.45, Z, 1.76),
    st(2.4, 0.45, 0.6, Z, 1.4),
    st(3.2, 0.22, 0.75, Z, 1.14),
];

/// The body, its plates, nose blade, bay, exhausts, optics and mark: what every wing has.
fn fuselage(b: &mut MeshBuilder) {
    if b.coarse() {
        dark_plate(b);
        body(b, &BODY, 0.0, Some(v3(5.6, 0.0, Z)));
        chevron(b, v3(0.8, 0.0, 1.8), 1.0);
        return;
    }
    metal(b);
    body(b, &BODY, 0.0, Some(v3(3.9, 0.0, Z)));
    lap(
        b,
        v3(2.3, 0.0, 1.62),
        -Vec3::X,
        Vec3::Z,
        4,
        1.4,
        1.7,
        0.6,
        0.1,
    );
    b.mirror_y(|b| {
        lap(
            b,
            v3(2.4, 0.42, 1.22),
            v3(-1.0, 0.1, -0.04),
            v3(0.0, 0.8, 0.6),
            3,
            1.7,
            2.0,
            0.24,
            0.08,
        )
    });
    dark_plate(b);
    sheet(
        b,
        &[
            [2.6, 0.28],
            [3.6, 0.4],
            [5.6, 0.0],
            [3.6, -0.4],
            [2.6, -0.28],
        ],
        Z - 0.05,
        0.1,
    );
    // The bay: two dark doors in the belly, a red slot between them where the bombs drop.
    seam(b);
    b.mirror_y(|b| {
        sheet(
            b,
            &[[1.5, 0.06], [1.5, 0.42], [-0.9, 0.42], [-0.9, 0.06]],
            0.42,
            0.06,
        );
    });
    red_slot(b, BAY - Vec3::Z * 0.06, -Vec3::Z, Vec3::X, 0.7, 0.08);
    b.mirror_y(|b| exhaust(b, v3(-3.85, 0.3, Z + 0.05), 0.2, 0.5));
    optics(
        b,
        v3(2.85, 0.32, 1.2),
        v3(0.25, 0.8, 0.4),
        v3(1.0, -0.12, -0.1),
        0.5,
        0.08,
    );
    chevron(b, v3(0.2, 0.0, 1.92), 0.8);
}

/// The cleaver: the wing's plan, left half.
const CLEAVER: [[f32; 2]; 6] = [
    [1.8, 0.6],
    [-2.4, 5.0],
    [-3.1, 5.1],
    [-2.9, 3.5],
    [-3.7, 2.5],
    [-3.5, 0.6],
];

fn cleaver(b: &mut MeshBuilder, _tech: u8) {
    fuselage(b);
    dark_plate(b);
    b.mirror_y(|b| {
        if b.coarse() {
            flat(b, &CLEAVER, Z);
            return;
        }
        sheet(b, &CLEAVER, Z - 0.06, 0.12);
        // Plates along the leading edge, lapped back toward the tip.
        lap(
            b,
            v3(1.2, 1.0, Z + 0.06),
            v3(-0.68, 0.73, 0.0),
            Vec3::Z,
            4,
            1.25,
            1.5,
            0.45,
            0.07,
        );
        fin(
            b,
            v3(-2.45, 5.05, Z - 0.04),
            0.3,
            &[[0.1, 0.0], [-0.6, 0.0], [-0.75, -0.7], [-0.15, -0.5]],
            0.07,
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_model;
    use crate::material::*;

    #[test]
    fn fits_the_regency_checks() {
        for key in MODELS.iter().map(|m| m.key) {
            crate::regency::check(key, RADIUS, HEIGHT, None, &[]);
            super::super::tests::reads_as_a_regency_jet(key);
            // The bombs leave a red slot in the belly, under the bay.
            let model = build_model(key).unwrap();
            let near = model.lods[0]
                .vertices
                .iter()
                .filter(|v| v.material == GLOW_LASER)
                .map(|v| Vec3::from(v.pos).distance(BAY))
                .fold(f32::MAX, f32::min);
            assert!(near < 0.5, "{key}: bay {near} m from its slot");
        }
    }

    #[test]
    fn the_unit_files_bay_is_the_models() {
        use super::super::tests::{blueprint, v};
        let (b, id) = blueprint("regency_t1_bomber");
        let bp = b.unit(id);
        assert_eq!(bp.visual.mesh, "regency_petard");
        assert!(
            (bp.radius.to_f32() - RADIUS).abs() < 1e-3
                && (bp.height.to_f32() - HEIGHT).abs() < 1e-3
        );
        assert!(v(bp.weapons[0].muzzle).distance(BAY) < 1e-3);
        assert_eq!(build_model("regency_petard").unwrap().exhausts.len(), 2);
    }
}
