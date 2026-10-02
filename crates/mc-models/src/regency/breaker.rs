//! The Regency's tech 1 land reclaimer, the Breaker: a salvage craft on gravity lift, so it
//! works shores and shallows as it works land, with a nanite head on a turning house that
//! takes wrecks apart while the craft drives on (the unit file's `reclaimer`, one head).
//!
//! From above it is a manta: a wide flat wing swept back to two points round a keeled
//! spine, a course of lapped plates out along each wing, a sensor head with red optics at
//! the nose. Behind the head's pedestal two bronze hoppers lie along the
//! spine either side of a dark vault with a red slot in its lid, where what it takes
//! apart is held.
//!
//! The head: a bronze yaw collar on a plated pedestal, armoured cheeks either side of the
//! trunnion, and the pitching head on it: a keeled cowl with a swept plate down its back,
//! a bronze nozzle out of a seam-dark collar and a violet nanite lens at its tip, where
//! the stream leaves (`EMITTER`). Nanites build and take apart (docs/STYLE.md "The Regency
//! suite"), so its stream is the Regency's nanite strands (beams.wgsl
//! `BEAM_NANITE_RECLAIM`), not ARC's salvage beam, and the lens runs hot while it works.
//!
//! Lift: bronze collars under the wings and the body, a lift bell in each with red
//! plasma crackling under it (`engineer::lift_drum`, renderer `lift_fx.rs`).
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plates, seams dark, dark bronze on the
//! machinery, violet only on the lens that works, red optics.
//!
//! Rig: a hovercraft (`MeshBuilder::set_hover`). The head is a gun house of its own bound to
//! head 0 (`with_house`): it turns about its pivot and what is inside `with_recoil` pitches
//! about the trunnion. Authored at blueprint scale (radius 4.6, height 3.4): model metres are
//! unit metres, and `PIVOT`/`EMITTER` are `regency_t1_land_reclaimer`'s head in
//! `data/factions/regency/units/land.ron`.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::part;

use super::commander::form::{blade, ring, sleeve, KEEL};
use super::engineer::lift_drum;
use super::kit::{dark_plate, metal, seam, v3};
use super::machine::{red_slot, Course, Frame};
use super::plating::plate;

/// The head's trunnion and the lens's tip, where the nanite stream leaves: the unit file's
/// head `pivot` and `emitter`.
pub(super) const PIVOT: Vec3 = Vec3::new(-1.5, 0.0, 2.9);
pub(super) const EMITTER: Vec3 = Vec3::new(0.75, 0.0, 2.9);
/// The top of the wing's deck, where the hoppers and the pedestal stand.
const DECK: f32 = 1.3;

pub(super) fn breaker(b: &mut MeshBuilder, _tech: u8) {
    b.set_hover();
    b.set_dust_line(0.7);
    if b.coarse() {
        coarse(b);
        return;
    }
    hull(b);
    hoppers(b);
    pedestal(b);
    b.with_house(0, PIVOT, 0.0, |b| {
        house(b, PIVOT);
        b.with_recoil(|b| head(b, PIVOT, EMITTER));
    });
}

/// Far off: the wing's outline lofted, the team colour on its back, the lift under it, the
/// head a bar on its house.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    b.loft_z(
        &[
            [4.2, 0.0],
            [-1.8, 3.6],
            [-2.8, 3.5],
            [-2.8, -3.5],
            [-1.8, -3.6],
        ],
        &[Section::new(0.6, 1.0), Section::new(DECK, 0.8)],
    );
    b.with_part(part::LOCOMOTION, |b| {
        seam(b);
        b.face(&[v3(2.0, 0.0, 0.5), v3(-2.4, -2.0, 0.5), v3(-2.4, 2.0, 0.5)]);
    });
    b.paint(TEAM);
    b.face(&[
        v3(1.8, 0.0, DECK + 0.02),
        v3(0.4, 0.9, DECK + 0.02),
        v3(0.4, -0.9, DECK + 0.02),
    ]);
    b.with_house(0, PIVOT, 0.0, |b| {
        b.with_recoil(|b| {
            dark_plate(b);
            b.beam(
                PIVOT - Vec3::X * 0.9,
                EMITTER - Vec3::X * 0.3,
                Vec2::new(0.9, 0.8),
                Vec2::new(0.4, 0.4),
            );
        });
    });
}

/// The hull: a wide flat wing swept back to two points, a keeled spine down its
/// middle, a course of plates out along each wing, and the lift under the wings.
fn hull(b: &mut MeshBuilder) {
    let deck = DECK;
    let plan = [
        [4.2, 0.0],
        [2.6, 0.9],
        [0.2, 1.9],
        [-1.6, 3.4],
        [-2.7, 3.5],
        [-2.3, 1.7],
        [-3.6, 0.6],
        [-3.6, -0.6],
        [-2.3, -1.7],
        [-2.7, -3.5],
        [-1.6, -3.4],
        [0.2, -1.9],
        [2.6, -0.9],
    ];
    seam(b);
    b.loft_z(&plan, &[Section::new(0.6, 0.7), Section::new(0.8, 0.95)]);
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &plan,
            &[
                Section::new(0.8, 1.0),
                Section::new(1.0, 0.97),
                Section::new(deck - 0.1, 0.55).shifted(-0.3, 0.0),
            ],
        )
    });
    sleeve(
        b,
        &[
            ring(v3(4.0, 0.0, 1.05), Vec3::Z, 0.15, 0.12),
            ring(v3(2.8, 0.0, deck), Vec3::Z, 0.6, 0.32),
            ring(v3(-1.0, 0.0, deck + 0.05), Vec3::Z, 0.75, 0.35),
            ring(v3(-3.5, 0.0, 1.15), Vec3::Z, 0.3, 0.18),
        ],
        &KEEL,
    );
    sensor_head(b, v3(4.2, 0.0, 0.95), 0.85);
    b.mirror_y(|b| {
        dark_plate(b);
        let f = Frame::new(v3(0.9, 1.3, 1.08), v3(-0.72, 0.7, 0.0), v3(0.0, 0.1, 1.0));
        let (count, step) = if b.fine() { (3, 0.95) } else { (2, 1.9) };
        let plates = Course {
            count,
            step,
            len: 1.5,
            half: 0.6,
            tip: -1.0,
            thick: 0.14,
            tail: 0.5,
        }
        .lay(b, &f);
        if b.fine() {
            for (g, len) in plates.iter().take(2) {
                red_slot(b, g.at(*len - 0.15, 0.0, -0.02), g.n, g.v, 0.4, 0.035);
            }
        }
        lift_drum(b, 0.6, 1.6, 0.5);
        lift_drum(b, -1.9, 2.7, 0.5);
    });
    lift_drum(b, -0.6, 0.0, 0.7);
    team_chevron(b, v3(1.6, 0.0, deck + 0.38), 0.8);
}

/// The sensor head at the nose `n`, `k` its size: a brow plate jutting over two pairs of
/// red optics.
pub(super) fn sensor_head(b: &mut MeshBuilder, n: Vec3, k: f32) {
    dark_plate(b);
    let p = |x: f32, y: f32, z: f32| n + v3(x, y, z) * k;
    plate(
        b,
        &[
            p(0.15, 0.0, 0.05),
            p(-0.9, 0.85, 0.3),
            p(-1.8, 0.95, 0.42),
            p(-1.8, -0.95, 0.42),
            p(-0.9, -0.85, 0.3),
        ],
        Vec3::Z * 0.15,
    );
    b.mirror_y(|b| {
        red_slot(
            b,
            p(-0.55, 0.42, -0.05),
            v3(0.7, 0.7, 0.0),
            Vec3::Y,
            0.26 * k,
            0.11 * k,
        );
        if b.fine() {
            red_slot(
                b,
                p(-1.05, 0.72, 0.0),
                v3(0.5, 0.86, 0.0),
                Vec3::Y,
                0.16 * k,
                0.08 * k,
            );
        }
    });
}

/// The owner's colour: a chevron on the deck, its point at `at`.
fn team_chevron(b: &mut MeshBuilder, at: Vec3, k: f32) {
    b.paint(TEAM);
    b.mirror_y(|b| {
        b.face(&[
            at,
            at + v3(-1.0, 0.55, 0.0) * k,
            at + v3(-1.3, 0.4, 0.0) * k,
            at + v3(-0.3, 0.0, 0.0) * k,
        ])
    });
}

/// Where what it takes apart is held: two bronze hoppers along the spine behind the head's
/// pedestal, banded, and between them a dark vault with a red slot let into its lid.
fn hoppers(b: &mut MeshBuilder) {
    let (deck, x0) = (DECK, -2.35);
    b.mirror_y(|b| {
        let (front, back) = (v3(x0, 0.72, deck + 0.22), v3(x0 - 1.0, 0.64, deck + 0.18));
        metal(b);
        let sides = b.sides(10);
        b.cylinder_between(front, back, 0.27, 0.25, sides);
        if b.fine() {
            seam(b);
            for t in [0.2f32, 0.75] {
                let at = front.lerp(back, t);
                b.cylinder_between(at - Vec3::X * 0.05, at + Vec3::X * 0.05, 0.31, 0.31, 10);
            }
        }
    });
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &[
                [x0 + 0.05, 0.33],
                [x0 - 0.95, 0.33],
                [x0 - 1.2, 0.0],
                [x0 - 0.95, -0.33],
                [x0 + 0.05, -0.33],
            ],
            &[
                Section::new(deck - 0.05, 1.0),
                Section::new(deck + 0.38, 0.85).shifted(-0.08, 0.0),
            ],
        )
    });
    if b.fine() {
        red_slot(
            b,
            v3(x0 - 0.45, 0.0, deck + 0.36),
            Vec3::Z,
            Vec3::X,
            0.7,
            0.08,
        );
    }
}

/// What the head's house stands on: a plated pedestal up from the deck to the yaw collar.
fn pedestal(b: &mut MeshBuilder) {
    let top = PIVOT.z - 0.62;
    dark_plate(b);
    b.with_facets(|b| {
        b.frustum(
            v3(PIVOT.x, 0.0, DECK - 0.1),
            Vec2::new(1.7, 1.5),
            Vec2::new(1.15, 1.05),
            top - DECK + 0.1,
            Vec2::new(-0.1, 0.0),
        )
    });
}

/// What turns but does not pitch: a bronze yaw collar and an armoured cheek either side of
/// the trunnion.
fn house(b: &mut MeshBuilder, p: Vec3) {
    metal(b);
    let sides = b.sides(12);
    b.prism(v3(p.x, 0.0, p.z - 0.62), sides, 0.72, 0.66, 0.2);
    b.mirror_y(|b| {
        dark_plate(b);
        b.with_facets(|b| {
            b.extrude_y(
                &[
                    [p.x - 0.65, p.z - 0.45],
                    [p.x + 0.45, p.z - 0.45],
                    [p.x + 0.35, p.z + 0.15],
                    [p.x - 0.05, p.z + 0.45],
                    [p.x - 0.75, p.z + 0.1],
                ],
                0.5,
                0.66,
            )
        });
    });
}

/// The pitching head from its trunnion `p` to the lens's tip `tip`, built facing +x: a
/// bronze trunnion drum, a keeled cowl, a swept plate down its back, red optics either
/// side, a seam-dark collar, the bronze nozzle and the violet lens.
fn head(b: &mut MeshBuilder, p: Vec3, tip: Vec3) {
    metal(b);
    let sides = b.sides(10);
    b.cylinder_between(p - Vec3::Y * 0.5, p + Vec3::Y * 0.5, 0.25, 0.25, sides);
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(p - Vec3::X * 0.95, Vec3::Z, 0.34, 0.3),
            ring(p - Vec3::X * 0.55, Vec3::Z, 0.46, 0.44),
            ring(p + Vec3::X * 0.35, Vec3::Z, 0.48, 0.46),
            ring(p + Vec3::X * 1.05, Vec3::Z, 0.36, 0.34),
        ],
        &KEEL,
    );
    blade(
        b,
        p + v3(0.5, 0.0, 0.45),
        v3(-1.0, 0.0, -0.1),
        Vec3::Z,
        1.7,
        0.36,
        0.0,
        0.14,
    );
    if b.fine() {
        b.mirror_y(|b| {
            red_slot(b, p + v3(0.75, 0.38, 0.02), Vec3::Y, Vec3::X, 0.42, 0.07);
        });
    }
    let collar = p + Vec3::X * 1.08;
    seam(b);
    b.cylinder_between(collar, collar + Vec3::X * 0.16, 0.3, 0.3, sides);
    let lens = tip - Vec3::X * 0.34;
    metal(b);
    b.cylinder_between(collar + Vec3::X * 0.16, lens, 0.17, 0.13, sides);
    b.paint(GLOW_VIOLET);
    b.cylinder_between(lens, tip, 0.2, 0.05, sides);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_model, rig};

    #[test]
    fn fits_the_librarys_checks() {
        super::super::check("regency_breaker", 4.6, 3.4, None, &[]);
    }

    #[test]
    fn the_unit_files_head_is_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("regency_t1_land_reclaimer").unwrap());
        assert_eq!(bp.visual.mesh, "regency_breaker");
        assert!((bp.radius.to_f32() - 4.6).abs() < 1e-3 && (bp.height.to_f32() - 3.4).abs() < 1e-3);
        assert_eq!(bp.motion.map(|m| m.layer), Some(mc_data::MoveLayer::Hover));
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        let reclaimer = bp.reclaimer.expect("it reclaims");
        assert!(reclaimer.mobile, "it works what it passes");
        let head = &reclaimer.heads()[0];
        assert!(v(head.pivot.expect("a head turns about its pivot")).distance(PIVOT) < 1e-3);
        assert!(v(head.emitter).distance(EMITTER) < 1e-3);

        let model = build_model("regency_breaker").unwrap();
        assert!(model.hover && model.legs.is_none() && model.treads.is_none());
        assert_eq!(model.houses.len(), 1);
        assert_eq!(Vec3::from(model.houses[0].pivot), PIVOT);
        for lod in &model.lods[..2] {
            // The lens's tip is the emitter, on the house, pitching; nothing else is violet.
            let violet: Vec<Vec3> = lod
                .vertices
                .iter()
                .filter(|v| v.material == GLOW_VIOLET)
                .inspect(|v| assert!(v.rig & rig::RECOIL != 0, "violet off the head"))
                .map(|v| Vec3::from(v.pos))
                .collect();
            let near = violet
                .iter()
                .map(|p| p.distance(EMITTER))
                .fold(f32::MAX, f32::min);
            assert!(near < 0.06, "lens {near} m from the emitter");
            let front = violet.iter().map(|p| p.x).fold(f32::MIN, f32::max);
            assert!((front - EMITTER.x).abs() < 0.02, "violet ends at {front}");
        }
    }

    #[test]
    fn every_bell_is_marked_for_its_plasma_and_hangs_clear() {
        let model = build_model("regency_breaker").unwrap();
        assert_eq!(model.lifts.len(), 5);
        let left = model.lifts.iter().filter(|l| l.at[1] > 0.5).count();
        let right = model.lifts.iter().filter(|l| l.at[1] < -0.5).count();
        assert_eq!(left, right, "mirrored");
        for lod in &model.lods {
            let low = lod
                .vertices
                .iter()
                .map(|v| v.pos[2])
                .fold(f32::MAX, f32::min);
            assert!(low > 0.15, "hangs clear of the ground: lowest {low}");
        }
    }
}
