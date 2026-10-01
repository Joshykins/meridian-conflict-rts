//! The Regency engineer, the Artificer: a low armoured craft that floats on gravity lift, so it
//! crosses water as it crosses land. From above it is an arrowhead: a pointed body with a
//! swept armoured sponson either side, their plates lapped and drawn back into points.
//! Under each sponson two bronze lift collars hold it off the ground, and one more under
//! the body, each round a lift bell with a dark core ringed in red. A sensor head with two pairs of red optics
//! at the nose; on the back a turning housing with bronze feed canisters at its sides,
//! and the fabricator arm reaching forward from it, a violet nanite emitter at its tip
//! where the build beam leaves.
//!
//! The higher tiers are the same craft drawn bigger (the unit files' sizes), each with its
//! own kit on it. Tech 2: twin violet prongs either side of the arm's nozzle, a feed drum
//! across the housing's back, a fourth plate in each sponson's course, and a third lift
//! collar under each sponson's trailing point. Tech 3, all of that and: a ring of violet
//! emitter fins round the nozzle, a chin ram under the nose with a third pair of optics,
//! an armoured skirt down each sponson's flank, and two lifts under the body for the one.
//!
//! Every lift bell's mouth is marked (`MeshBuilder::add_lift`): red plasma crackles under
//! it while the craft is up, trailing behind as it moves (renderer `lift_fx.rs`).
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plates (`dark_plate`), their seams dark
//! (`seam`), dark bronze on the machinery (`metal`), violet only on the emitter that
//! builds.
//!
//! Rig: a hovercraft (`MeshBuilder::set_hover`): the entity shader heaves the hull gently
//! over its lift bells (`part::LOCOMOTION`), which stay low. The housing is the turret and turns about the unit's middle; the arm pitches
//! about its trunnion (`rig::ARM_TOOL`). The numbers match `regency_t1_engineer` in
//! `data/factions/regency/units/command.ron`.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::{part, rig};

use super::kit::{cable, dark_plate, metal, seam, v3};
use super::machine::{hoop, red_slot, Course, Frame};
use super::plating::plate;

/// The fabricator arm's trunnion and its emitter's tip, where the build beam leaves: the
/// unit file's `builder.arm.pivot` and `builder.arm.emitter`.
const ARM_PIVOT: Vec3 = Vec3::new(0.35, 0.0, 2.05);
const EMITTER: Vec3 = Vec3::new(2.15, 0.0, 2.05);
/// Where the housing turns on the body.
const DECK: f32 = 1.45;
/// The lift drums under the left sponson (x, y); the right's are their mirrors.
const LIFTS: [(f32, f32); 2] = [(0.8, 1.55), (-1.45, 2.05)];
/// Tech 2's third collar under each sponson's trailing point (x, y, radius).
const TAIL_LIFT: (f32, f32, f32) = (-2.35, 2.35, 0.4);
/// Where each lift bell's mouth opens, and the lift leaves it.
const MOUTH: f32 = 0.2;

pub(super) fn engineer(b: &mut MeshBuilder, tech: u8) {
    b.set_hover();
    b.set_turret_pivot(v3(0.0, 0.0, DECK));
    b.set_arm_pivot(ARM_PIVOT);
    b.set_dust_line(0.6);
    if b.coarse() {
        coarse(b);
        return;
    }
    body(b, tech);
    b.mirror_y(|b| sponson(b, tech));
    lifts(b, tech);
    b.with_part(part::TURRET, |b| {
        housing(b, tech);
        b.with_limb(rig::ARM_TOOL, |b| arm(b, tech));
    });
}

/// Far off: an arrowhead of a body with the team colour on its back, and the arm as a
/// bar that still pitches.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    let plan = |k: f32, z: f32| -> Vec<Vec3> {
        [
            [3.1, 0.0],
            [0.6, 1.3],
            [-2.6, 2.6],
            [-2.0, 0.0],
            [-2.6, -2.6],
            [0.6, -1.3],
        ]
        .iter()
        .map(|p| v3(p[0] * k, p[1] * k, z))
        .collect()
    };
    b.loft(&[plan(1.0, 0.45), plan(0.8, 1.45)], true, true);
    // The lift under it, face down.
    b.with_part(part::LOCOMOTION, |b| {
        seam(b);
        b.face(&[v3(1.5, 0.0, 0.3), v3(-1.8, -1.6, 0.3), v3(-1.8, 1.6, 0.3)]);
    });
    b.paint(TEAM);
    b.face(&[
        v3(0.2, 0.0, 1.47),
        v3(-1.3, 0.8, 1.47),
        v3(-1.3, -0.8, 1.47),
    ]);
    b.with_part(part::TURRET, |b| {
        b.with_limb(rig::ARM_TOOL, |b| {
            b.paint(GLOW_VIOLET);
            b.beam(ARM_PIVOT, EMITTER, Vec2::new(0.5, 0.5), Vec2::new(0.2, 0.2));
        });
    });
}

/// The body: a pointed core lofted up from a flat seam-dark belly, the sensor head at
/// its nose, a plate swept back down its spine into the tail's point, and the team colour
/// on its back. Tech 3 hangs a chin ram under the nose, a third pair of optics on it.
fn body(b: &mut MeshBuilder, tech: u8) {
    let plan = [
        [3.1, 0.0],
        [2.2, 0.75],
        [0.6, 1.25],
        [-1.9, 1.2],
        [-2.5, 0.6],
        [-2.5, -0.6],
        [-1.9, -1.2],
        [0.6, -1.25],
        [2.2, -0.75],
    ];
    seam(b);
    b.loft_z(&plan, &[Section::new(0.45, 0.72), Section::new(0.75, 0.94)]);
    dark_plate(b);
    b.loft_z(
        &plan,
        &[
            Section::new(0.75, 1.0),
            Section::new(1.1, 1.0),
            Section::new(DECK, 0.8).shifted(-0.2, 0.0),
        ],
    );
    // The sensor head: a brow plate over the nose, two pairs of optics under it.
    plate(
        b,
        &[
            v3(3.3, 0.0, 1.05),
            v3(2.3, 0.72, 1.3),
            v3(1.5, 0.8, 1.42),
            v3(1.5, -0.8, 1.42),
            v3(2.3, -0.72, 1.3),
        ],
        Vec3::Z * 0.14,
    );
    b.mirror_y(|b| {
        red_slot(
            b,
            v3(2.62, 0.36, 0.98),
            v3(0.7, 0.7, 0.0),
            Vec3::Y,
            0.22,
            0.1,
        );
        if b.fine() {
            red_slot(
                b,
                v3(2.18, 0.64, 1.02),
                v3(0.5, 0.86, 0.0),
                Vec3::Y,
                0.14,
                0.08,
            );
        }
    });
    if tech >= 3 {
        ram(b);
    }
    // The spine plate, from behind the housing back past the body into a point.
    dark_plate(b);
    plate(
        b,
        &[
            v3(-0.9, 0.55, DECK - 0.02),
            v3(-2.3, 0.62, 1.35),
            v3(-3.1, 0.0, 1.05),
            v3(-2.3, -0.62, 1.35),
            v3(-0.9, -0.55, DECK - 0.02),
        ],
        Vec3::Z * 0.16,
    );
    // The owner's colour: a chevron on the spine plate.
    b.paint(TEAM);
    b.mirror_y(|b| {
        b.face(&[
            v3(-1.2, 0.0, DECK + 0.15),
            v3(-2.0, 0.42, 1.52),
            v3(-2.25, 0.3, 1.5),
            v3(-1.45, 0.0, DECK + 0.14),
        ])
    });
}

/// The left sponson: a seam-dark deck out from the body's flank, a bronze frame on it
/// (a ribbed spar out to the lift collars and ribs from the body), and over the frame a
/// course of three armour plates lapped like feathers and swept back and out, the last
/// drawn out past the deck into a point. From tech 2 the course has four plates and the
/// spar runs on back to the third collar; tech 3 hangs an armoured skirt down its flank.
fn sponson(b: &mut MeshBuilder, tech: u8) {
    seam(b);
    b.extrude_z(
        &[
            [1.9, 0.9],
            [1.15, 1.8],
            [-1.3, 2.35],
            [-2.6, 2.55],
            [-2.0, 1.9],
            [-1.9, 0.9],
        ],
        0.72,
        0.92,
    );
    // The spar: bronze, ribbed, from the front lift collar back to the rear one.
    metal(b);
    let (front, back) = (
        v3(LIFTS[0].0, LIFTS[0].1, 1.0),
        v3(LIFTS[1].0, LIFTS[1].1, 1.0),
    );
    let sides = b.sides(8);
    b.cylinder_between(front, back, 0.14, 0.14, sides);
    if tech >= 2 {
        let tail = v3(TAIL_LIFT.0, TAIL_LIFT.1, 1.0);
        b.cylinder_between(back, tail, 0.12, 0.12, sides);
    }
    if b.fine() {
        for t in [0.2f32, 0.45, 0.7] {
            let at = front.lerp(back, t);
            let d = (back - front).normalize() * 0.06;
            b.cylinder_between(at - d, at + d, 0.19, 0.19, 8);
        }
        // The ribs in the gap between the sponson and the body's flank.
        for x in [0.3f32, -0.5, -1.3] {
            b.cylinder_between(v3(x, 1.0, 1.0), v3(x, 1.6, 1.0), 0.08, 0.08, 5);
        }
    }
    dark_plate(b);
    let f = Frame::new(v3(1.45, 1.3, 1.08), v3(-1.0, 0.3, 0.0), v3(0.0, 0.3, 1.0));
    // The fourth plate laps closer, so the course ends where three would.
    let (count, step) = match (b.fine(), tech >= 2) {
        (true, true) => (4, 0.77),
        (true, false) => (3, 1.15),
        (false, _) => (2, 2.3),
    };
    let plates = Course {
        count,
        step,
        len: 1.7,
        half: 0.62,
        tip: -1.0,
        thick: 0.14,
        tail: 0.55,
    }
    .lay(b, &f);
    if b.fine() {
        // A red line in the seam under each plate's trailing edge.
        for (g, len) in plates.iter().take(2) {
            red_slot(b, g.at(*len - 0.15, 0.0, -0.02), g.n, g.v, 0.45, 0.035);
        }
    }
    if tech >= 3 {
        skirt(b);
    }
}

/// Tech 3's chin ram: a heavy wedge of plate under the nose, its point out ahead of the
/// brow, and a third pair of optics on its shoulders.
fn ram(b: &mut MeshBuilder) {
    dark_plate(b);
    plate(
        b,
        &[
            v3(3.55, 0.0, 0.5),
            v3(2.65, 0.9, 0.5),
            v3(1.7, 1.05, 0.52),
            v3(1.7, -1.05, 0.52),
            v3(2.65, -0.9, 0.5),
        ],
        Vec3::Z * 0.2,
    );
    if b.fine() {
        seam(b);
        plate(
            b,
            &[
                v3(3.2, 0.0, 0.7),
                v3(2.55, 0.62, 0.7),
                v3(2.0, 0.7, 0.7),
                v3(2.0, -0.7, 0.7),
                v3(2.55, -0.62, 0.7),
            ],
            Vec3::Z * 0.06,
        );
    }
    b.mirror_y(|b| {
        red_slot(
            b,
            v3(2.95, 0.38, 0.62),
            v3(0.75, 0.66, 0.0),
            Vec3::Y,
            0.18,
            0.08,
        );
    });
}

/// Tech 3's armoured skirt: a dark plate hung down the sponson's outer flank, outside
/// the front and middle collars, from the deck nearly to the bells' mouths.
fn skirt(b: &mut MeshBuilder) {
    let (front, back) = (Vec3::new(1.6, 1.98, 0.0), Vec3::new(-1.1, 2.64, 0.0));
    let out = Vec3::new(back.y - front.y, front.x - back.x, 0.0).normalize();
    dark_plate(b);
    plate(
        b,
        &[
            front + Vec3::Z * 0.97,
            front + Vec3::Z * 0.55 - (back - front) * 0.08,
            back + Vec3::Z * 0.42,
            back + Vec3::Z * 0.97,
        ],
        out * 0.12,
    );
    if b.fine() {
        // Its lower edge lit, where the lift's light catches it.
        red_slot(
            b,
            front.lerp(back, 0.55) + Vec3::Z * 0.5 + out * 0.05,
            out,
            back - front,
            1.2,
            0.04,
        );
    }
}

/// The lift: a bronze collar under each end of each sponson and a bigger one under the
/// body, and in each a lift bell (`part::LOCOMOTION`), a dark core ringed in red where
/// the lift leaves it. The shader heaves the hull over its bells (`set_hover`): each bell
/// runs deep enough into its collar that no gap opens as it does. Tech 2 adds a collar
/// under each sponson's trailing point; tech 3 two under the body in place of the one.
/// Each bell's mouth is marked for its plasma (`add_lift`).
fn lifts(b: &mut MeshBuilder, tech: u8) {
    let drum = |b: &mut MeshBuilder, x: f32, y: f32, r: f32| {
        b.add_lift(v3(x, y, MOUTH), r * 0.7);
        metal(b);
        let sides = b.sides(10);
        b.prism(v3(x, y, 0.5), sides, r, r * 0.95, 0.28);
        b.with_part(part::LOCOMOTION, |b| {
            seam(b);
            b.prism(v3(x, y, 0.28), sides, r * 0.86, r * 0.84, 0.46);
            b.prism(v3(x, y, MOUTH), sides, r * 0.55, r * 0.7, 0.08);
            if b.fine() {
                b.paint(GLOW_LASER);
                hoop(b, v3(x, y, 0.3), r * 0.7, 0.08, 0.05, 12);
            }
        });
    };
    b.mirror_y(|b| {
        for (x, y) in LIFTS {
            drum(b, x, y, 0.48);
        }
        if tech >= 2 {
            drum(b, TAIL_LIFT.0, TAIL_LIFT.1, TAIL_LIFT.2);
        }
    });
    if tech >= 3 {
        drum(b, 0.65, 0.0, 0.55);
        drum(b, -1.2, 0.0, 0.6);
    } else {
        drum(b, -0.3, 0.0, 0.7);
    }
}

/// The housing (turret): a bronze turntable, a dark armoured block swept back to a point
/// behind, a bronze feed canister along each side, and the arm's trunnion cheeks. From
/// tech 2 a banded feed drum lies across its back, a red slot round its middle.
fn housing(b: &mut MeshBuilder, tech: u8) {
    metal(b);
    let sides = b.sides(12);
    b.prism(v3(0.0, 0.0, DECK - 0.05), sides, 0.85, 0.8, 0.2);
    dark_plate(b);
    b.loft_z(
        &[
            [0.75, 0.5],
            [-0.9, 0.62],
            [-1.75, 0.0],
            [-0.9, -0.62],
            [0.75, -0.5],
        ],
        &[
            Section::new(DECK + 0.15, 1.0),
            Section::new(DECK + 0.6, 0.95),
            Section::new(2.35, 0.7).shifted(-0.15, 0.0),
        ],
    );
    b.mirror_y(|b| {
        // A cheek plate either side of the trunnion.
        dark_plate(b);
        b.extrude_y(
            &[
                [ARM_PIVOT.x - 0.5, DECK + 0.2],
                [ARM_PIVOT.x + 0.4, DECK + 0.2],
                [ARM_PIVOT.x + 0.35, ARM_PIVOT.z + 0.2],
                [ARM_PIVOT.x - 0.15, ARM_PIVOT.z + 0.4],
                [ARM_PIVOT.x - 0.75, ARM_PIVOT.z + 0.1],
            ],
            0.34,
            0.46,
        );
        // The feed canister, banded.
        let (front, back) = (v3(0.1, 0.72, 1.85), v3(-1.3, 0.62, 1.8));
        metal(b);
        let sides = b.sides(8);
        b.cylinder_between(front, back, 0.2, 0.2, sides);
        if b.fine() {
            seam(b);
            for t in [0.25f32, 0.7] {
                let at = front.lerp(back, t);
                b.cylinder_between(at - Vec3::X * 0.05, at + Vec3::X * 0.05, 0.23, 0.23, 8);
            }
            metal(b);
            cable(
                b,
                &[front, v3(0.25, 0.5, 1.95), ARM_PIVOT + v3(-0.1, 0.3, -0.1)],
                0.05,
            );
        }
    });
    if tech >= 2 {
        let (l, r) = (v3(-1.2, -0.5, 2.25), v3(-1.2, 0.5, 2.25));
        metal(b);
        let sides = b.sides(12);
        b.cylinder_between(l, r, 0.3, 0.3, sides);
        seam(b);
        for y in [-0.42f32, 0.42] {
            let at = v3(-1.2, y, 2.25);
            b.cylinder_between(at - Vec3::Y * 0.05, at + Vec3::Y * 0.05, 0.34, 0.34, sides);
        }
        if b.fine() {
            red_slot(b, v3(-1.2, 0.0, 2.55), Vec3::Z, Vec3::Y, 0.5, 0.07);
        }
    }
}

/// The fabricator arm from its trunnion to its tip: a bronze trunnion drum, a plated
/// sheath swept back over the trunnion, a bronze nozzle through a seam-dark collar, and
/// the violet emitter at the tip (where the beam leaves); close up, a guide rail either
/// side. Tech 2 sets twin violet prongs either side of the nozzle, converging on the
/// tip; tech 3 a ring of four violet fins round it behind them, on a seam-dark collar.
fn arm(b: &mut MeshBuilder, tech: u8) {
    let (p, tip) = (ARM_PIVOT, EMITTER);
    metal(b);
    let sides = b.sides(10);
    b.cylinder_between(p - Vec3::Y * 0.33, p + Vec3::Y * 0.33, 0.24, 0.24, sides);
    dark_plate(b);
    let sides = b.sides(8);
    b.cylinder_between(p + Vec3::X * 0.05, p + Vec3::X * 1.1, 0.26, 0.22, sides);
    plate(
        b,
        &[
            v3(p.x + 1.15, -0.2, p.z + 0.2),
            v3(p.x + 1.15, 0.2, p.z + 0.2),
            v3(p.x - 0.2, 0.28, p.z + 0.26),
            v3(p.x - 0.75, 0.0, p.z + 0.38),
            v3(p.x - 0.2, -0.28, p.z + 0.26),
        ],
        Vec3::Z * 0.1,
    );
    seam(b);
    b.cylinder_between(p + Vec3::X * 1.1, p + Vec3::X * 1.3, 0.2, 0.2, sides);
    metal(b);
    b.cylinder_between(p + Vec3::X * 1.3, tip - Vec3::X * 0.3, 0.13, 0.11, sides);
    b.paint(GLOW_VIOLET);
    b.cylinder_between(tip - Vec3::X * 0.32, tip, 0.15, 0.05, sides);
    if tech >= 2 {
        b.mirror_y(|b| {
            b.cylinder_between(
                p + v3(1.3, 0.2, 0.0),
                tip - v3(0.08, -0.05, 0.0),
                0.05,
                0.025,
                6,
            )
        });
    }
    if tech >= 3 {
        let ring = tip - Vec3::X * 0.62;
        seam(b);
        b.cylinder_between(
            ring - Vec3::X * 0.06,
            ring + Vec3::X * 0.06,
            0.2,
            0.2,
            sides,
        );
        b.paint(GLOW_VIOLET);
        for k in 0..4 {
            let a = std::f32::consts::FRAC_PI_4 + std::f32::consts::FRAC_PI_2 * k as f32;
            let d = v3(0.0, a.cos(), a.sin());
            b.cylinder_between(
                ring + d * 0.15,
                ring + Vec3::X * 0.35 + d * 0.32,
                0.045,
                0.02,
                5,
            );
        }
    }
    if b.fine() {
        metal(b);
        b.mirror_y(|b| {
            b.cylinder_between(
                p + v3(0.2, 0.22, 0.0),
                p + v3(1.2, 0.16, 0.0),
                0.04,
                0.04,
                5,
            )
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_model;

    #[test]
    fn fits_the_librarys_checks() {
        super::super::check("regency_engineer", 3.6, 2.8, None, &[]);
        super::super::check_at("regency_engineer", 2, 4.2, 3.267, None, &[]);
        super::super::check_at("regency_engineer", 3, 4.8, 3.733, None, &[]);
    }

    #[test]
    fn every_bell_is_marked_for_its_plasma() {
        // Two collars a sponson and one under the body; a third a sponson at tech 2; two
        // under the body at tech 3.
        for (tech, bells) in [(1, 5), (2, 7), (3, 8)] {
            let model = crate::build_model_scaled("regency_engineer", 3.6, 2.8, tech).unwrap();
            assert_eq!(model.lifts.len(), bells, "tech {tech}");
            for l in &model.lifts {
                assert!((l.at[2] - MOUTH).abs() < 1e-4 && l.radius > 0.2);
            }
            let left = model.lifts.iter().filter(|l| l.at[1] > 0.5).count();
            let right = model.lifts.iter().filter(|l| l.at[1] < -0.5).count();
            assert_eq!(left, right, "tech {tech}: mirrored");
        }
        // Drawn bigger, the mouths move out with the hull.
        let big = crate::build_model_scaled("regency_engineer", 4.8, 3.733, 1).unwrap();
        assert!((big.lifts[0].at[0] / LIFTS[0].0 - 4.8 / 3.6).abs() < 1e-3);
    }

    #[test]
    fn hovers_with_nothing_on_the_ground() {
        let model = build_model("regency_engineer").unwrap();
        assert!(model.hover, "it floats on its lift");
        assert!(model.legs.is_none() && model.treads.is_none());
        for lod in &model.lods {
            let low = lod
                .vertices
                .iter()
                .map(|v| v.pos[2])
                .fold(f32::MAX, f32::min);
            assert!(low > 0.15, "hangs clear of the ground: lowest {low}");
        }
    }

    #[test]
    fn the_beam_leaves_the_unit_files_emitter() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        for tech in 1..=3u8 {
            let bp = blueprints.unit(
                blueprints
                    .id_of(&format!("regency_t{tech}_engineer"))
                    .unwrap(),
            );
            assert_eq!(bp.tech, tech);
            // Each tier is the tech 1 craft drawn bigger, evenly.
            let (radius, height) = (bp.radius.to_f32(), bp.height.to_f32());
            let k = radius / 3.6;
            assert!(
                (height / 2.8 - k).abs() < 1e-3,
                "tech {tech}: {radius} x {height}"
            );
            let arm = bp.builder.as_ref().unwrap().arm.as_ref().unwrap();
            let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
            assert!(
                v(arm.emitter).distance(EMITTER * k) < 2e-3,
                "tech {tech}: emitter {}",
                v(arm.emitter)
            );
            assert!(v(arm.pivot.unwrap()).distance(ARM_PIVOT * k) < 2e-3);
            assert!(bp.turret_at.is_none(), "the housing turns about the middle");
            assert_eq!(
                bp.motion.map(|m| m.layer),
                Some(mc_data::MoveLayer::Hover),
                "it crosses water"
            );
            assert_eq!(bp.visual.mesh, "regency_engineer");
            check_violet(
                &crate::build_model_scaled("regency_engineer", 3.6, 2.8, tech).unwrap(),
                tech,
            );
        }
        let model = build_model("regency_engineer").unwrap();
        assert_eq!(model.arm_pivot, Some(ARM_PIVOT.to_array()));
        assert!(Vec3::from(model.turret_pivot).truncate().length() < 1e-4);
    }

    /// The violet ends at the emitter, and only what builds is violet.
    fn check_violet(model: &crate::Model, tech: u8) {
        for lod in &model.lods {
            let violet: Vec<Vec3> = lod
                .vertices
                .iter()
                .filter(|v| {
                    v.material == GLOW_VIOLET
                        && v.part == part::TURRET
                        && v.rig & rig::LIMB_MASK == rig::ARM_TOOL
                })
                .map(|v| Vec3::from(v.pos))
                .collect();
            let front = violet.iter().map(|p| p.x).fold(f32::MIN, f32::max);
            assert!(
                (front - EMITTER.x).abs() < 0.05,
                "tech {tech}: violet ends at {front}"
            );
            // Nothing else is violet: only what builds is.
            assert!(lod
                .vertices
                .iter()
                .filter(|v| v.material == GLOW_VIOLET)
                .all(|v| v.rig & rig::LIMB_MASK == rig::ARM_TOOL));
        }
    }
}
