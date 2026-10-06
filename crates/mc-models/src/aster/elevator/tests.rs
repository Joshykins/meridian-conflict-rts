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

/// Every level carries the jaws, the cable and the platform, and the unit file's size
/// is the model's own (the shader's constants are in model metres).
#[test]
fn every_level_has_jaws_cable_and_platform() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let blueprints = mc_data::Blueprints::load(&data).unwrap();
    let bp = blueprints.unit(blueprints.id_of("aster_t5_space_elevator").unwrap());
    assert_eq!((bp.radius.to_f32(), bp.height.to_f32()), (SIZE.0, SIZE.1));
    for key in DESIGNS {
        let model = build_model_scaled(key, SIZE.0, SIZE.1, SIZE.2).unwrap();
        for (l, lod) in model.lods.iter().enumerate() {
            for p in [part::ELEVATOR_ARM, part::ELEVATOR_CABLE, part::ELEVATOR_CAR] {
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
            // The platform.
            let car: Vec<Vec3> = of(part::ELEVATOR_CAR).collect();
            let deck = car.iter().map(|p| p.z).fold(f32::MIN, f32::max);
            let under = car.iter().map(|p| p.z).fold(f32::MAX, f32::min);
            let reach = car
                .iter()
                .map(|p| p.truncate().length())
                .fold(0.0, f32::max);
            assert!(
                (under - CAR_UNDER_Z).abs() < 0.01,
                "{name}: underside {under}"
            );
            assert!(
                reach <= DECK_R * 1.42 + 0.01,
                "{name}: platform reach {reach}"
            );
            if l < 2 {
                assert!(
                    (reach - DECK_R).abs() < 0.5,
                    "{name}: platform reach {reach}"
                );
                let edge = car
                    .iter()
                    .filter(|p| p.truncate().length() > DECK_R - 0.3)
                    .map(|p| p.z)
                    .fold(f32::MIN, f32::max);
                assert!((edge - DECK_Z).abs() < 0.2, "{name}: deck edge at {edge}");
                assert!(deck <= 32.0, "{name}: platform top {deck}");
            }
        }
    }
}

/// Swung open, every jaw stands clear of the platform's run up the cable, so the
/// platform can pass them while they are open.
#[test]
fn open_jaws_clear_the_platform() {
    for key in DESIGNS {
        let model = build_model_scaled(key, SIZE.0, SIZE.1, SIZE.2).unwrap();
        let lod = &model.lods[0];
        let nearest = lod
            .vertices
            .iter()
            .filter(|v| v.part == part::ELEVATOR_ARM)
            .map(|v| swung(pos(v), OPEN).truncate().length())
            .fold(f32::MAX, f32::min);
        assert!(nearest > DECK_R + 4.0, "{key}: open jaws {nearest} m out");
        // Nothing standing still reaches into the platform's run over the deck.
        let intruder = lod.vertices.iter().find(|v| {
            v.part == part::HULL
                && v.pos[2] > DECK_Z + 0.5
                && pos(v).truncate().length() < DECK_R + 1.0
        });
        assert!(
            intruder.is_none(),
            "{key}: {:?} in the platform's way",
            intruder.map(pos)
        );
    }
}
