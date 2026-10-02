//! The Winnow, the Regency's tech 2 reclaim carrier (`regency_t2_reclaim_carrier` in
//! `data/factions/regency/units/air_t2.ron`): a broad craft hung on gravity lift bells
//! (red plasma crackling under each, `lift::bell`; no rotors, no jets) that strips a wreck
//! field as it passes over. Four nanite heads hang under it, each a gun house of its own
//! (`with_house`, heads 0-3 in the unit file's order: fore port, fore starboard, aft port,
//! aft starboard), and each takes a wreck of its own; their strands make the cloud of
//! motes between the craft and the field (beams.wgsl `BEAM_NANITE_RECLAIM`).
//!
//! A head: a bronze yaw collar on a plated stem down from the hull, and on its trunnion
//! a keeled cowl, a bronze nozzle and a violet lens whose tip is the unit file's emitter.
//! Authored at blueprint scale (radius 10, height 4.5).
//!
//! From above, a manta: a plated body in a wide swept wing (the Breaker grown up), the
//! heads under the wings, a bell under each wing and two under the body.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::library::ModelDef;
use crate::material::*;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::lift::bell;
use super::super::machine::{red_slot, Course};
use super::jet::{body, plates, st, team_mark, tip, wing, workings, Station};

pub(super) const RADIUS: f32 = 10.0;
pub(super) const HEIGHT: f32 = 4.5;

pub(crate) const MODELS: &[ModelDef] = &[ModelDef::new(
    "regency_reclaim_carrier",
    RADIUS,
    HEIGHT,
    winnow,
)];

/// Each head's trunnion and its lens's tip: the unit file's head `pivot` and `emitter`.
pub(super) const HEADS: [(Vec3, Vec3); 4] = [
    (Vec3::new(3.0, 3.0, 1.1), Vec3::new(4.4, 3.0, 1.1)),
    (Vec3::new(3.0, -3.0, 1.1), Vec3::new(4.4, -3.0, 1.1)),
    (Vec3::new(-3.0, 3.0, 1.1), Vec3::new(-1.6, 3.0, 1.1)),
    (Vec3::new(-3.0, -3.0, 1.1), Vec3::new(-1.6, -3.0, 1.1)),
];

/// The heads, each on its house, hung from the hull's underside at `roof`.
fn heads(b: &mut MeshBuilder, roof: f32) {
    for (i, &(p, tip)) in HEADS.iter().enumerate() {
        b.with_house(i, p, 0.0, |b| {
            house(b, p, roof);
            b.with_recoil(|b| head(b, p, tip));
        });
    }
}

/// What turns but does not pitch: a plated stem down from the hull at `roof`, a bronze
/// yaw collar at its foot, a cheek either side of the trunnion.
fn house(b: &mut MeshBuilder, p: Vec3, roof: f32) {
    if b.coarse() {
        return;
    }
    let sides = b.sides(8);
    dark_plate(b);
    b.prism(
        v3(p.x, p.y, p.z + 0.3),
        sides,
        0.42,
        0.55,
        roof - p.z - 0.25,
    );
    if b.fine() {
        metal(b);
        b.prism(v3(p.x, p.y, p.z + 0.18), sides, 0.5, 0.46, 0.14);
        dark_plate(b);
        for s in [1.0, -1.0] {
            b.extrude_y(
                &[
                    [p.x - 0.4, p.z - 0.25],
                    [p.x + 0.3, p.z - 0.25],
                    [p.x + 0.25, p.z + 0.18],
                    [p.x - 0.45, p.z + 0.18],
                ],
                p.y + s * 0.3 - 0.04,
                p.y + s * 0.3 + 0.04,
            );
        }
    }
}

/// The pitching head from its trunnion `p` to the lens's tip `tip`, facing +x: a bronze
/// trunnion drum, a keeled cowl, red optics, the bronze nozzle and the violet lens. Far
/// off, a violet bar.
fn head(b: &mut MeshBuilder, p: Vec3, tip: Vec3) {
    if b.coarse() {
        b.paint(GLOW_VIOLET);
        b.face(&[
            p + v3(0.0, -0.25, -0.2),
            tip + v3(0.0, -0.12, -0.2),
            tip + v3(0.0, 0.12, -0.2),
            p + v3(0.0, 0.25, -0.2),
        ]);
        return;
    }
    let sides = b.sides(8);
    if b.fine() {
        metal(b);
        b.cylinder_between(p - Vec3::Y * 0.28, p + Vec3::Y * 0.28, 0.17, 0.17, sides);
    }
    dark_plate(b);
    b.loft(
        &[
            cowl(p.x - 0.35, p, 0.2, 0.18),
            cowl(p.x + 0.1, p, 0.3, 0.28),
            cowl(p.x + 0.65, p, 0.24, 0.22),
        ],
        true,
        true,
    );
    if b.fine() {
        for s in [1.0, -1.0] {
            red_slot(
                b,
                p + v3(0.4, s * 0.24, 0.05),
                v3(0.0, s, 0.0),
                Vec3::X,
                0.3,
                0.06,
            );
        }
    }
    let collar = p + Vec3::X * 0.68;
    if b.fine() {
        seam(b);
        b.cylinder_between(collar, collar + Vec3::X * 0.1, 0.2, 0.2, sides);
    }
    let lens = tip - Vec3::X * 0.24;
    metal(b);
    b.cylinder_between(collar + Vec3::X * 0.1, lens, 0.11, 0.09, sides);
    b.paint(GLOW_VIOLET);
    b.cylinder_between(lens, tip, 0.14, 0.04, sides);
}

/// A section of the head's cowl at `x`: a keel above, flat cheeks, a flat belly.
fn cowl(x: f32, p: Vec3, w: f32, h: f32) -> Vec<Vec3> {
    vec![
        v3(x, p.y, p.z + h * 1.3),
        v3(x, p.y + w, p.z + h * 0.5),
        v3(x, p.y + w, p.z - h * 0.6),
        v3(x, p.y - w, p.z - h * 0.6),
        v3(x, p.y - w, p.z + h * 0.5),
    ]
}

/// A lift bell (with its mark) under the hull, or far off its mark alone.
fn lift(b: &mut MeshBuilder, mouth: Vec3, r: f32, depth: f32) {
    if b.coarse() {
        b.add_lift(mouth, r * 0.7);
        return;
    }
    bell(b, mouth, r, depth);
}

/// The Winnow.
fn winnow(b: &mut MeshBuilder, _tech: u8) {
    const BODY: [Station; 5] = [
        st(-6.6, 0.8, 2.6, 2.0, 1.6),
        st(-4.0, 1.6, 3.7, 2.1, 1.3),
        st(0.0, 1.9, 4.2, 2.1, 1.2),
        st(3.6, 1.4, 3.8, 2.1, 1.3),
        tip(6.4, 2.1),
    ];
    body(b, &BODY);
    team_mark(b, v3(0.4, 0.0, 4.15), Vec3::X, Vec3::Z, 2.0, 0.6);
    if b.fine() {
        for s in [1.0, -1.0] {
            red_slot(
                b,
                v3(4.6, s * 0.95, 3.2),
                v3(0.4, s, 0.3),
                v3(1.0, 0.0, -0.4),
                0.8,
                0.12,
            );
        }
    }
    plates(
        b,
        v3(2.6, 0.0, 4.05),
        v3(-1.0, 0.0, -0.08),
        Vec3::Z,
        Course {
            count: 4,
            step: 1.6,
            len: 2.2,
            half: 0.8,
            tip: 0.0,
            thick: 0.16,
            tail: 1.0,
        },
    );
    b.mirror_y(|b| {
        let droop = 0.08;
        wing(
            b,
            &[
                [4.6, 1.2],
                [2.0, 6.5],
                [-0.8, 9.2],
                [-2.2, 8.6],
                [-4.4, 3.2],
                [-6.6, 1.2],
            ],
            2.0,
            droop,
            0.3,
        );
        plates(
            b,
            v3(2.4, 2.0, 2.05),
            v3(-0.7, 1.0, -0.1),
            Vec3::Z,
            Course {
                count: 3,
                step: 1.4,
                len: 2.0,
                half: 0.6,
                tip: 0.35,
                thick: 0.12,
                tail: 1.2,
            },
        );
        workings(b, v3(1.0, 1.8, 2.4), v3(-4.0, 1.7, 2.35), 0.18, 3);
        lift(b, v3(-1.0, 5.6, 0.9), 0.9, 0.75);
    });
    lift(b, v3(0.4, 0.0, 0.5), 1.25, 0.8);
    lift(b, v3(-4.6, 0.0, 0.7), 0.8, 0.8);
    heads(b, 1.6);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::part;

    #[test]
    fn fits_the_airframe_checks() {
        let lenses: Vec<Vec3> = HEADS.iter().map(|h| h.1).collect();
        {
            let key = "regency_reclaim_carrier";
            super::super::jet::check::airframe(key, RADIUS, HEIGHT, &lenses, GLOW_VIOLET);
            let model = crate::build_model(key).unwrap();
            assert!(
                model.lifts.len() >= 3,
                "{key}: {} lift bells",
                model.lifts.len()
            );
            assert_eq!(model.houses.len(), 4, "{key}: one house per head");
            for (h, &(p, _)) in model.houses.iter().zip(HEADS.iter()) {
                assert!(
                    Vec3::from(h.pivot).distance(p) < 1e-3,
                    "{key}: house at {p}"
                );
            }
            assert!(
                model.lods[0]
                    .vertices
                    .iter()
                    .any(|v| v.part == part::LOCOMOTION),
                "{key}: no bells"
            );
        }
    }

    #[test]
    fn the_unit_files_heads_are_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("regency_t2_reclaim_carrier").unwrap());
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        assert_eq!(bp.visual.mesh, "regency_reclaim_carrier");
        assert!((bp.radius.to_f32() - RADIUS).abs() < 1e-3);
        assert!((bp.height.to_f32() - HEIGHT).abs() < 1e-3);
        let heads = bp.reclaimer.unwrap().heads().to_vec();
        assert_eq!(heads.len(), HEADS.len());
        for (h, &(p, e)) in heads.iter().zip(HEADS.iter()) {
            assert!(v(h.pivot.unwrap()).distance(p) < 1e-3, "pivot {p}");
            assert!(v(h.emitter).distance(e) < 1e-3, "emitter {e}");
        }
    }
}
