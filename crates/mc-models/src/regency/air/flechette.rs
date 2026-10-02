//! The Flechette, the Regency's tech 1 air scout: a dart, the fastest thing in the air,
//! unarmed. A needle body, a long nose blade, small fletching wings far aft with down-turned
//! tips, a swept blade standing over the tail (the user's pick of three, 2026-10-02).
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plates lapped over bronze, red optics at
//! the nose, red heat in the exhaust. Authored at blueprint scale (radius 3.5, height 1.4):
//! model metres are unit metres.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::library::ModelDef;

use super::super::kit::{dark_plate, metal, v3};
use super::{body, chevron, exhaust, fin, flat, lap, optics, sheet, st, tail_face, St};

const RADIUS: f32 = 3.5;
const HEIGHT: f32 = 1.4;

pub(crate) const MODELS: &[ModelDef] = &[ModelDef::new("regency_flechette", RADIUS, HEIGHT, dart)];

/// The level the wings and blades lie at.
const Z: f32 = 0.72;

/// The needle.
const DART: [St; 5] = [
    st(-2.7, 0.15, 0.62, Z, 0.84),
    st(-2.0, 0.24, 0.5, Z, 0.98),
    st(-0.6, 0.3, 0.44, Z, 1.04),
    st(0.8, 0.26, 0.5, Z, 0.98),
    st(1.7, 0.15, 0.6, Z, 0.86),
];

fn dart(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        dark_plate(b);
        body(b, &DART, 0.0, Some(v3(3.4, 0.0, Z)));
        b.mirror_y(|b| flat(b, &[[-0.3, 0.2], [-2.1, 1.5], [-2.5, 1.5], [-2.3, 0.2]], Z));
        chevron(b, v3(0.3, 0.0, 1.06), 0.6);
        tail_face(b, 0.0, -1.4, -2.8, 0.95, 1.42);
        return;
    }
    // The bronze under it all, its plates lapped over the back and down the flanks.
    metal(b);
    body(b, &DART, 0.0, Some(v3(2.3, 0.0, Z)));
    lap(
        b,
        v3(0.9, 0.0, 1.0),
        -Vec3::X,
        Vec3::Z,
        3,
        0.85,
        1.05,
        0.26,
        0.07,
    );
    b.mirror_y(|b| {
        if !b.fine() {
            return;
        }
        lap(
            b,
            v3(1.0, 0.2, 0.86),
            v3(-1.0, 0.05, -0.02),
            v3(0.0, 0.8, 0.6),
            3,
            0.9,
            1.1,
            0.12,
            0.06,
        )
    });
    nose_blade(b, 1.0, 3.45, 0.22);
    dark_plate(b);
    b.mirror_y(|b| {
        // Fletching: small wings far aft, their tips turned down.
        sheet(
            b,
            &[[-0.3, 0.22], [-2.0, 1.45], [-2.45, 1.5], [-2.2, 0.22]],
            Z - 0.04,
            0.07,
        );
        fin(
            b,
            v3(-2.0, 1.47, Z - 0.02),
            0.35,
            &[[0.15, 0.0], [-0.45, 0.0], [-0.55, -0.42], [-0.1, -0.3]],
            0.05,
        );
        // Canard blades.
        if !b.fine() {
            return;
        }
        sheet(
            b,
            &[[1.15, 0.22], [0.5, 0.68], [0.28, 0.68], [0.55, 0.22]],
            Z + 0.02,
            0.05,
        );
    });
    // The swept blade over the tail.
    dark_plate(b);
    b.with_facets(|b| {
        b.extrude_y(
            &[[-1.3, 0.96], [-2.3, 0.9], [-2.8, 1.42], [-2.25, 1.38]],
            -0.035,
            0.035,
        )
    });
    exhaust(b, v3(-2.8, 0.0, Z), 0.14, 0.35);
    optics(
        b,
        v3(1.35, 0.17, 0.8),
        v3(0.2, 0.8, 0.4),
        v3(1.0, -0.1, -0.1),
        0.32,
        0.05,
    );
    chevron(b, v3(-0.1, 0.0, 1.17), 0.5);
}

/// The nose blade: a flat dark blade from `root` out to a point at `tip`, `half` across
/// at its root, lying along the centre line.
fn nose_blade(b: &mut MeshBuilder, root: f32, tip: f32, half: f32) {
    dark_plate(b);
    let k = tip - root;
    sheet(
        b,
        &[
            [root, half * 0.6],
            [root + k * 0.3, half],
            [tip, 0.0],
            [root + k * 0.3, -half],
            [root, -half * 0.6],
        ],
        Z - 0.04,
        0.08,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_the_regency_checks() {
        for key in MODELS.iter().map(|m| m.key) {
            crate::regency::check(key, RADIUS, HEIGHT, None, &[]);
            super::super::tests::reads_as_a_regency_jet(key);
        }
    }

    #[test]
    fn the_unit_file_is_the_models() {
        let (b, id) = super::super::tests::blueprint("regency_t1_air_scout");
        let bp = b.unit(id);
        assert_eq!(bp.visual.mesh, "regency_flechette");
        assert!(
            (bp.radius.to_f32() - RADIUS).abs() < 1e-3
                && (bp.height.to_f32() - HEIGHT).abs() < 1e-3
        );
        assert!(bp.weapons.is_empty(), "it is unarmed");
        assert_eq!(
            crate::build_model("regency_flechette")
                .unwrap()
                .exhausts
                .len(),
            1
        );
    }
}
