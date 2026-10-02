//! The Wick (`regency_wick`), the Quiver's drone: one charge of pinched plasma on a small
//! lift bell, built in its bay to be spent. Flown into its mark, it lets go there
//! (mc-sim `strike_drones.rs`). The charge glows red through the plate that holds it.
//!
//! The shape is a dart: an arrowhead of plate, the charge a red cell let into its back under
//! a bronze clamp, a nose blade and two down-turned fins, one small bell under it.

use crate::builder::{MeshBuilder, Section};
use crate::library::ModelDef;
use crate::material::*;
use crate::part;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::lift::bell;
use super::super::machine::{armour, swept, Frame};
use super::{plan, team_mark};

pub(crate) const MODELS: &[ModelDef] = &[ModelDef::new("regency_wick", RADIUS, HEIGHT, dart)];

pub(crate) const RADIUS: f32 = 1.3;
pub(crate) const HEIGHT: f32 = 0.8;

/// Far off: a flat arrowhead with the team colour on its back.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    let half = [[1.2, 0.0], [-0.9, 0.65], [-0.6, 0.0]];
    b.extrude_z(&plan(&half), 0.15, 0.7);
    team_mark(b, -0.2, 0.7, 0.6);
    b.with_part(part::LOCOMOTION, |b| {
        seam(b);
        b.face(&[v3(0.3, 0.0, 0.1), v3(-0.3, -0.25, 0.1), v3(-0.3, 0.25, 0.1)]);
    });
}

fn dart(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        coarse(b);
        return;
    }
    let half = [
        [1.25, 0.0],
        [0.4, 0.42],
        [-0.95, 0.7],
        [-0.7, 0.28],
        [-1.05, 0.0],
    ];
    b.with_facets(|b| {
        seam(b);
        b.loft_z(
            &plan(&half),
            &[Section::new(0.18, 0.8), Section::new(0.28, 0.95)],
        );
        dark_plate(b);
        b.loft_z(
            &plan(&half),
            &[
                Section::new(0.28, 1.0),
                Section::new(0.5, 0.95),
                Section::new(0.66, 0.6).shifted(-0.1, 0.0),
            ],
        );
    });
    // The charge: a red cell let into its back, under a bronze clamp.
    b.paint(GLOW_LASER);
    b.frustum(
        v3(-0.15, 0.0, 0.62),
        glam::Vec2::new(0.62, 0.32),
        glam::Vec2::new(0.42, 0.2),
        0.14,
        glam::Vec2::ZERO,
    );
    metal(b);
    b.block(v3(-0.28, -0.22, 0.6), v3(-0.18, 0.22, 0.8));
    team_mark(b, -0.68, 0.6, 0.32);
    // The nose blade.
    dark_plate(b);
    armour(
        b,
        &Frame::new(v3(0.6, 0.0, 0.42), v3(1.0, 0.0, -0.12), v3(0.0, 1.0, 0.0)),
        &[[0.0, -0.12], [0.0, 0.14], [0.68, -0.02]],
        0.08,
    );
    b.mirror_y(|b| {
        // A down-turned fin off each trailing corner.
        armour(
            b,
            &Frame::new(
                v3(-0.55, 0.55, 0.4),
                v3(-0.7, 0.45, -0.55),
                v3(0.0, 0.6, 0.8),
            ),
            &swept(0.62, 0.16, 0.5, 0.5),
            0.06,
        );
    });
    bell(b, v3(0.0, 0.0, 0.0), 0.32, 0.28);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_design_fits_the_librarys_checks() {
        for key in MODELS.iter().map(|d| d.key) {
            super::super::super::check(key, RADIUS, HEIGHT, None, &[]);
            let model = crate::build_model(key).unwrap();
            assert_eq!(model.lifts.len(), 1, "{key}: one lift bell");
            assert!(
                model.lods[0]
                    .vertices
                    .iter()
                    .any(|v| v.material == GLOW_LASER && v.normal[2] > 0.3
                        || v.material == GLOW_LASER && v.normal[0] > 0.5),
                "{key}: its charge shows"
            );
        }
    }
}
