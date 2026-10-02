//! The Sickle, the Regency's tech 1 salvage drone: a small craft hanging on lift bells over
//! a wreck field, a nanite head slung under it that turns full circle and looks straight
//! down, taking wrecks apart as it flies. A keeled pod, and off each side a blade swept back
//! and hooked forward at its tip like a sickle's, a lift bell under each hook and one under
//! the tail (the user's pick of two, 2026-10-02).
//!
//! Nanites build and take apart (docs/STYLE.md "The Regency suite"): the head's stream is
//! the Regency's nanite strands, and its lens is violet. Rig: the head is a gun house of
//! its own bound to head 0 (`with_house`); what is inside `with_recoil` pitches about its
//! trunnion. Authored at blueprint scale (radius 3.0, height 1.6): `PIVOT` and `EMITTER`
//! are `regency_t1_air_reclaimer`'s head.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::library::ModelDef;
use crate::material::*;

use super::super::commander::form::{ring, sleeve, KEEL};
use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::lift::bell;
use super::super::machine::red_slot;
use super::{body, chevron, lap, sheet, st, St};

const RADIUS: f32 = 3.0;
const HEIGHT: f32 = 1.6;

pub(crate) const MODELS: &[ModelDef] = &[ModelDef::new("regency_sickle", RADIUS, HEIGHT, hook)];

/// The head's trunnion and the lens's tip, where the nanite stream leaves: the unit file's
/// head `pivot` and `emitter`.
const PIVOT: Vec3 = Vec3::new(0.6, 0.0, 0.45);
const EMITTER: Vec3 = Vec3::new(1.4, 0.0, 0.45);
/// The underside of the hull.
const BELLY: f32 = 0.75;
/// Where each lift bell's mouth opens.
const MOUTH: f32 = 0.08;

/// The pod.
const POD: [St; 4] = [
    st(-1.9, 0.3, 0.85, 1.0, 1.15),
    st(-0.9, 0.55, BELLY, 1.0, 1.42),
    st(0.6, 0.55, BELLY, 1.0, 1.38),
    st(1.6, 0.3, 0.85, 1.0, 1.15),
];
/// One sickle blade, left side: swept back off the pod and hooked forward at the tip.
const HOOK: [[f32; 2]; 8] = [
    [0.5, 0.45],
    [-0.4, 1.4],
    [-0.6, 2.2],
    [0.2, 2.85],
    [1.1, 2.75],
    [0.5, 2.45],
    [-0.1, 2.0],
    [-1.0, 0.45],
];

fn hook(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        dark_plate(b);
        body(b, &POD, 0.0, Some(v3(2.3, 0.0, 1.0)));
        b.mirror_y(|b| {
            super::flat(
                b,
                &[[0.5, 0.45], [0.6, 2.8], [-0.6, 2.2], [-1.0, 0.45]],
                1.0,
            )
        });
        chevron(b, v3(0.2, 0.0, 1.45), 0.6);
        head_coarse(b);
        return;
    }
    dark_plate(b);
    body(b, &POD, 0.0, Some(v3(2.3, 0.0, 1.0)));
    lap(
        b,
        v3(1.3, 0.0, 1.32),
        -Vec3::X,
        Vec3::Z,
        3,
        0.8,
        0.95,
        0.4,
        0.07,
    );
    b.mirror_y(|b| {
        dark_plate(b);
        sheet(b, &HOOK, 0.94, 0.1);
        metal(b);
        // Bronze ribs along the blade, under its plates.
        b.cylinder_between(v3(-0.2, 0.5, 0.96), v3(-0.3, 1.9, 0.96), 0.07, 0.07, 6);
        lap(
            b,
            v3(-0.15, 0.7, 1.04),
            v3(-0.3, 1.0, 0.0),
            Vec3::Z,
            2,
            0.75,
            0.85,
            0.2,
            0.05,
        );
        bell(b, v3(0.3, 2.45, MOUTH), 0.32, 0.86 - MOUTH);
        red_slot(b, v3(1.6, 0.32, 1.1), v3(0.4, 0.8, 0.2), Vec3::X, 0.3, 0.06);
    });
    bell(b, v3(-1.35, 0.0, MOUTH), 0.4, BELLY + 0.05 - MOUTH);
    chevron(b, v3(-0.1, 0.0, 1.44), 0.45);
    dark_plate(b);
    b.beam(
        v3(0.0, 0.0, BELLY + 0.05),
        v3(PIVOT.x, 0.0, BELLY - 0.02),
        glam::Vec2::new(0.5, 0.18),
        glam::Vec2::new(0.4, 0.14),
    );
    head(b);
}

/// The head: a bronze slewing ring under the hull, the house hanging from it, and in the
/// house the pitching head: a trunnion drum, a keeled cowl, a seam-dark collar, the
/// bronze nozzle and the violet lens at `EMITTER`.
fn head(b: &mut MeshBuilder) {
    let p = PIVOT;
    b.with_house(0, p, 0.0, |b| {
        metal(b);
        let sides = b.sides(10);
        b.prism(v3(p.x, 0.0, BELLY - 0.12), sides, 0.32, 0.3, 0.12);
        dark_plate(b);
        b.mirror_y(|b| {
            b.with_facets(|b| {
                b.extrude_y(
                    &[
                        [p.x - 0.3, BELLY - 0.1],
                        [p.x + 0.25, BELLY - 0.1],
                        [p.x + 0.2, p.z - 0.05],
                        [p.x - 0.1, p.z - 0.18],
                        [p.x - 0.35, p.z],
                    ],
                    0.2,
                    0.27,
                )
            })
        });
        b.with_recoil(|b| {
            metal(b);
            b.cylinder_between(p - Vec3::Y * 0.2, p + Vec3::Y * 0.2, 0.12, 0.12, sides);
            dark_plate(b);
            sleeve(
                b,
                &[
                    ring(p - Vec3::X * 0.3, Vec3::Z, 0.13, 0.12),
                    ring(p - Vec3::X * 0.1, Vec3::Z, 0.19, 0.18),
                    ring(p + Vec3::X * 0.3, Vec3::Z, 0.18, 0.17),
                    ring(p + Vec3::X * 0.5, Vec3::Z, 0.13, 0.12),
                ],
                &KEEL,
            );
            seam(b);
            let collar = p + Vec3::X * 0.5;
            b.cylinder_between(collar, collar + Vec3::X * 0.06, 0.12, 0.12, sides);
            let lens = EMITTER - Vec3::X * 0.16;
            metal(b);
            b.cylinder_between(collar + Vec3::X * 0.06, lens, 0.07, 0.06, sides);
            b.paint(GLOW_VIOLET);
            b.cylinder_between(lens, EMITTER, 0.09, 0.025, sides);
        });
    });
}

/// Far off, the head as one bar on its house.
fn head_coarse(b: &mut MeshBuilder) {
    b.with_house(0, PIVOT, 0.0, |b| {
        b.with_recoil(|b| {
            dark_plate(b);
            b.beam(
                PIVOT - Vec3::X * 0.3,
                EMITTER,
                glam::Vec2::new(0.3, 0.25),
                glam::Vec2::new(0.12, 0.12),
            );
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_model;

    #[test]
    fn fits_the_regency_checks() {
        for key in MODELS.iter().map(|m| m.key) {
            crate::regency::check(key, RADIUS, HEIGHT, None, &[]);
            let model = build_model(key).unwrap();
            assert_eq!(model.houses.len(), 1, "{key}: one head");
            assert!(model.lifts.len() >= 3, "{key}: its lift bells");
            let near = model.lods[0]
                .vertices
                .iter()
                .filter(|v| v.material == GLOW_VIOLET)
                .map(|v| Vec3::from(v.pos).distance(EMITTER))
                .fold(f32::MAX, f32::min);
            assert!(near < 0.05, "{key}: lens {near} m from the emitter");
        }
    }

    #[test]
    fn the_unit_files_head_is_the_models() {
        use super::super::tests::{blueprint, v};
        let (b, id) = blueprint("regency_t1_air_reclaimer");
        let bp = b.unit(id);
        assert_eq!(bp.visual.mesh, "regency_sickle");
        assert!(
            (bp.radius.to_f32() - RADIUS).abs() < 1e-3
                && (bp.height.to_f32() - HEIGHT).abs() < 1e-3
        );
        assert!(bp.motion.is_some_and(|m| m.hover), "it hangs on its lift");
        let reclaimer = bp.reclaimer.as_ref().expect("it reclaims");
        assert!(reclaimer.mobile, "it works what it passes");
        let head = &reclaimer.heads()[0];
        assert!(v(head.pivot.expect("the head turns")).distance(PIVOT) < 1e-3);
        assert!(v(head.emitter).distance(EMITTER) < 1e-3);
        let model = build_model("regency_sickle").unwrap();
        assert_eq!(Vec3::from(model.houses[0].pivot), PIVOT);
    }
}
