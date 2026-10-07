use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use crate::gpu_consts::elevator::*;
use crate::{build_model_scaled, part, MeshVertex};

/// The unit file's size (`aster_t5_space_elevator`): radius, height, tech.
const SIZE: (f32, f32, u8) = (150.0, 300.0, 5);
const DESIGNS: [&str; 1] = ["space_elevator"];

fn pos(v: &MeshVertex) -> Vec3 {
    Vec3::from(v.pos)
}

/// What entity.wgsl does to a jaw's vertex at `open` radians out (`elevator_pose`).
fn swung(p: Vec3, open: f32) -> Vec3 {
    let third = TAU / 3.0;
    let k = (p.y.atan2(p.x) / third).round();
    let dir = Vec2::from_angle(k * third);
    let side = dir.perp();
    let (s, c) = open.sin_cos();
    let r = p.truncate().dot(dir) - HINGE_R;
    let z = p.z - HINGE_Z;
    let along = p.truncate().dot(side);
    (dir * (HINGE_R + r * c + z * s) + side * along).extend(HINGE_Z - r * s + z * c)
}

/// Every level carries the jaws, the cable and the crates, and the unit file's size
/// is the model's own (the shader's constants are in model metres).
#[test]
fn every_level_has_jaws_cable_and_crates() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let blueprints = mc_data::Blueprints::load(&data).unwrap();
    let bp = blueprints.unit(blueprints.id_of("aster_t5_space_elevator").unwrap());
    assert_eq!((bp.radius.to_f32(), bp.height.to_f32()), (SIZE.0, SIZE.1));
    for key in DESIGNS {
        let model = build_model_scaled(key, SIZE.0, SIZE.1, SIZE.2).unwrap();
        for (l, lod) in model.lods.iter().enumerate() {
            for p in [
                part::ELEVATOR_ARM,
                part::ELEVATOR_CABLE,
                part::ELEVATOR_CRATE,
            ] {
                assert!(
                    lod.vertices.iter().any(|v| v.part == p),
                    "{key} lod {l}: no part {p}"
                );
            }
        }
    }
}

/// The jaws close on the cable's band at `GRIP_Z`; each swings about its own tower's
/// hinge (none reaches past a third of the turn); the cable runs from `CABLE_FOOT` up
/// over `CABLE_SPLIT`, as wide as `CABLE_R` at the grip; the platform's deck is at
/// `DECK_Z` out to `DECK_R`, and its thrusters open at `CAR_UNDER_Z`.
#[test]
fn contract_points_match_the_shader_constants() {
    for key in DESIGNS {
        let model = build_model_scaled(key, SIZE.0, SIZE.1, SIZE.2).unwrap();
        for (l, lod) in model.lods.iter().enumerate() {
            let of = |p: u32| lod.vertices.iter().filter(move |v| v.part == p).map(pos);
            let name = format!("{key} lod {l}");
            // The jaws: their tips on the cable at the grip, each within its own sector.
            let inner = of(part::ELEVATOR_ARM)
                .map(|p| p.truncate().length())
                .fold(f32::MAX, f32::min);
            assert!((inner - CABLE_R).abs() < 0.3, "{name}: jaws reach {inner}");
            for p in of(part::ELEVATOR_ARM) {
                let a = p.y.atan2(p.x);
                let off = (a / (TAU / 3.0)).round() * (TAU / 3.0) - a;
                assert!(
                    off.abs() < TAU / 6.0 - 0.05,
                    "{name}: jaw vertex {p} near a gap"
                );
            }
            let tip = of(part::ELEVATOR_ARM)
                .filter(|p| p.truncate().length() < CABLE_R + 0.3)
                .map(|p| p.z);
            let (lo, hi) = tip.fold((f32::MAX, f32::MIN), |(a, b), z| (a.min(z), b.max(z)));
            assert!(lo <= GRIP_Z && hi >= GRIP_Z, "{name}: grip {lo}..{hi}");
            // The cable.
            let (foot, top) = of(part::ELEVATOR_CABLE)
                .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.z), b.max(p.z)));
            assert!(
                (foot - CABLE_FOOT).abs() < 0.01,
                "{name}: cable foot {foot}"
            );
            assert!(
                top > CABLE_SPLIT && top <= SIZE.1,
                "{name}: cable top {top}"
            );
            if l < 2 {
                // The band the jaws close on: as wide as `CABLE_R`, across the grip.
                let band = |above: bool| {
                    of(part::ELEVATOR_CABLE).any(|p| {
                        (p.truncate().length() - CABLE_R).abs() < 0.3
                            && (p.z > GRIP_Z) == above
                            && (p.z - GRIP_Z).abs() < 20.0
                    })
                };
                assert!(band(true) && band(false), "{name}: no band at the grip");
            }
            // Nothing of the cable but its top ring reaches over the split.
            assert!(
                of(part::ELEVATOR_CABLE).all(|p| p.z < CABLE_SPLIT || p.z > CABLE_SPLIT + 0.5),
                "{name}: cable detail at the split"
            );
            // The crates: in their bays, on the ground, their thruster bells opening at
            // `CAR_UNDER_Z`, no taller than a bay and no wider than its walls.
            let shell: Vec<Vec3> = of(part::ELEVATOR_CRATE).collect();
            let under = shell.iter().map(|p| p.z).fold(f32::MAX, f32::min);
            if l < 2 {
                assert!(
                    (under - CAR_UNDER_Z).abs() < 0.01,
                    "{name}: underside {under}"
                );
            }
            for p in &shell {
                let (x, y) = bay_frame(*p);
                assert!(
                    x > BAY_FROM - 2.5 && x < BAY_FROM + BAY_LENGTH + 2.5,
                    "{name}: crate vertex {p} {x} m out"
                );
                assert!(
                    y.abs() < BAY_WIDTH / 2.0 + 3.5 && p.z <= BAY_HEIGHT,
                    "{name}: crate vertex {p} across {y}"
                );
            }
            // The door leaves close the outer end; the roof halves close the top.
            // (Far off, the crate is one block.)
            for (part, name_of) in [(part::ELEVATOR_DOOR, "door"), (part::ELEVATOR_ROOF, "roof")] {
                assert!(l >= 2 || of(part).count() > 0, "{name}: no {name_of}");
            }
            assert!(
                of(part::ELEVATOR_DOOR).all(|p| bay_frame(p).0 > BAY_FROM + BAY_LENGTH - 4.0),
                "{name}: a door vertex off the outer end"
            );
            assert!(
                of(part::ELEVATOR_ROOF).all(|p| p.z > BAY_HEIGHT - 3.0),
                "{name}: a roof vertex off the top"
            );
        }
    }
}

/// A point in its bay's frame: out along the bay's bearing and across it (the shader
/// finds the bay the same way, by the nearest of the three bearings).
fn bay_frame(p: Vec3) -> (f32, f32) {
    let third = TAU / 3.0;
    let first = BAY_BEARING_DEG.to_radians();
    let k = ((p.y.atan2(p.x) - first) / third).round();
    let dir = Vec2::from_angle(first + k * third);
    (p.truncate().dot(dir), p.truncate().dot(dir.perp()))
}

/// The bays' numbers are the sim's (`mc_core::elevator_bay`), which seats units in them.
#[test]
fn the_bays_are_the_sims() {
    use mc_core::elevator_bay as sim;
    assert_eq!(BAY_FROM, sim::FROM as f32);
    assert_eq!(BAY_LENGTH, sim::LENGTH as f32);
    assert_eq!(BAY_WIDTH, sim::WIDTH as f32);
    assert_eq!(BAY_HEIGHT, sim::HEIGHT as f32);
    assert_eq!(DOORS_OPEN, sim::DOORS_OPEN);
    assert_eq!(sim::COUNT, 3);
}

/// With its jaws swung open, a jaw never reaches a bay, and no tower or foundation piece
/// stands in one above the pad (the units walk out of it and the crates land in it).
#[test]
fn nothing_stands_in_the_bays() {
    for key in DESIGNS {
        let model = build_model_scaled(key, SIZE.0, SIZE.1, SIZE.2).unwrap();
        let lod = &model.lods[0];
        let jaw = lod
            .vertices
            .iter()
            .filter(|v| v.part == part::ELEVATOR_ARM)
            .map(|v| swung(pos(v), OPEN))
            .find(|&p| in_bay(p));
        assert!(jaw.is_none(), "{key}: an open jaw in a bay at {jaw:?}");
        let intruder = lod
            .vertices
            .iter()
            .find(|v| v.part == part::HULL && v.pos[2] > DECK_Z + 0.5 && in_bay(pos(v)));
        assert!(
            intruder.is_none(),
            "{key}: {:?} in a crate's bay",
            intruder.map(pos)
        );
    }
}

/// Inside a bay's box, a little in from its walls and past the well's rim.
fn in_bay(p: Vec3) -> bool {
    let (x, y) = bay_frame(p);
    x > BAY_FROM + 1.0
        && x < BAY_FROM + BAY_LENGTH - 1.0
        && y.abs() < BAY_WIDTH / 2.0 - BAY_WALL
        && p.z < BAY_HEIGHT
}
