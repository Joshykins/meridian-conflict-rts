use glam::Vec3;

use super::*;
use crate::{build_model, build_model_fitted, Model};

const HEIGHT: f32 = 23.0;

fn fitted() -> Model {
    build_model_fitted("naga_commander", 10.4, HEIGHT, 1, &["eng_2", "eng_3"]).unwrap()
}

/// The unit file's commander: its weapon's muzzle and pivot, the build arm's emitter and
/// pivot, and every `arm_emitter` its loadouts move the beam to.
struct Data {
    muzzle: Vec3,
    gun_pivot: Vec3,
    emitter: Vec3,
    arm_pivot: Vec3,
    emitters: Vec<Vec3>,
    height: f32,
}

fn data() -> Data {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let blueprints = mc_data::Blueprints::load(&dir).unwrap();
    let id = blueprints.id_of("naga_commander").unwrap();
    let bp = blueprints.unit(id);
    let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
    let arm = bp.builder.as_ref().unwrap().arm.as_ref().unwrap();
    let set = blueprints.refit_set(id).unwrap();
    Data {
        muzzle: v(bp.weapons[0].muzzle),
        gun_pivot: v(bp.weapons[0].pivot.unwrap()),
        emitter: v(arm.emitter),
        arm_pivot: v(arm.pivot.unwrap()),
        emitters: set
            .loadouts
            .iter()
            .filter_map(|&l| {
                Some(v(blueprints
                    .unit(l)
                    .builder
                    .as_ref()?
                    .arm
                    .as_ref()?
                    .emitter))
            })
            .collect(),
        height: bp.height.to_f32(),
    }
}

fn limb(model: &Model, lod: usize, limb: u32) -> impl Iterator<Item = &crate::MeshVertex> {
    model.lods[lod]
        .vertices
        .iter()
        .filter(move |v| v.rig & rig::LIMB_MASK == limb)
}

#[test]
fn fits_the_librarys_checks() {
    super::super::check("naga_commander", 10.4, HEIGHT, None, &[MUZZLE.to_array()]);
    assert_eq!(data().height, HEIGHT);
}

#[test]
fn the_unit_files_muzzle_emitter_and_elbows_are_the_models() {
    let d = data();
    assert!(d.muzzle.distance(MUZZLE) < 1e-3, "muzzle {}", d.muzzle);
    assert!(d.emitter.distance(EMITTER) < 1e-3, "emitter {}", d.emitter);
    assert!(
        d.arm_pivot.distance(ELBOW) < 1e-3,
        "arm pivot {}",
        d.arm_pivot
    );
    let right = ELBOW * Vec3::new(1.0, -1.0, 1.0);
    assert!(
        d.gun_pivot.distance(right) < 1e-3,
        "gun pivot {}",
        d.gun_pivot
    );
    // Suite III's beam leaves the lance run out.
    let lance = EMITTER.with_x(LANCE_TIP + LANCE_RUN);
    assert!(
        d.emitters.iter().any(|e| e.distance(lance) < 1e-3),
        "no loadout's beam leaves the lance tip {lance}: {:?}",
        d.emitters
    );

    let model = fitted();
    assert_eq!(model.arm_pivot, Some(ELBOW.to_array()));
    assert!(model.recoil.is_some(), "the barrel kicks");
    for lod in 0..3 {
        // The cannon forearm ends at the muzzle.
        let front = limb(&model, lod, rig::ARM_GUN)
            .filter(|v| v.part == part::TURRET)
            .map(|v| v.pos[0])
            .fold(f32::MIN, f32::max);
        assert!(
            (front - MUZZLE.x).abs() < 0.1,
            "lod{lod}: cannon ends at {front}"
        );
    }
    for lod in 0..2 {
        // The build beam leaves the front of the palm's violet emitter.
        let violet: Vec<Vec3> = limb(&model, lod, rig::ARM_TOOL)
            .filter(|v| v.material == GLOW_VIOLET && v.rig & rig::MODULE_MASK == 0)
            .map(|v| Vec3::from(v.pos))
            .collect();
        let front = violet.iter().map(|p| p.x).fold(f32::MIN, f32::max);
        let near = violet
            .iter()
            .map(|p| p.distance(EMITTER))
            .fold(f32::MAX, f32::min);
        assert!(
            (front - EMITTER.x).abs() < 0.05 && near < 0.3,
            "lod{lod}: emitter front {front}, {near} m off"
        );
        // Suite III's lance tip at rest, run out while it builds.
        let lance = model.lods[lod]
            .vertices
            .iter()
            .filter(|v| v.rig & rig::WORK_MASK == rig::WORK_EXTEND)
            .map(|v| v.pos[0])
            .fold(f32::MIN, f32::max);
        assert!(
            (lance - LANCE_TIP).abs() < 0.05,
            "lod{lod}: lance tip {lance}"
        );
    }
}

#[test]
fn walks_on_reverse_kneed_legs_with_long_high_steps() {
    let model = build_model("naga_commander").unwrap();
    let legs = model.legs.expect("it walks");
    let (hock, _) = legs.hock.expect("reverse-kneed");
    assert_eq!(hock, HOCK.to_array());
    assert!(legs.crawl.is_none());
    assert!(
        legs.stride >= 16.0 && legs.lift > 1.5,
        "{} / {}",
        legs.stride,
        legs.lift
    );
    assert!(
        legs.stance < 0.5,
        "a moment in the air: stance {}",
        legs.stance
    );
    assert!(
        legs.foot[2] > 2.0 && legs.foot[1] - legs.foot[0] > 4.0,
        "{:?}",
        legs.foot
    );
    for lod in 0..3 {
        let bones: &[u32] = if lod < 2 {
            &[rig::THIGH, rig::SHIN, rig::TARSUS, rig::FOOT]
        } else {
            &[rig::THIGH, rig::SHIN, rig::TARSUS]
        };
        for &bone in bones {
            let on = |left: bool| {
                limb(&model, lod, bone)
                    .filter(|v| v.part == part::LOCOMOTION)
                    .any(|v| (v.pos[1] > 0.0) == left)
            };
            assert!(on(true) && on(false), "lod{lod}: bone {bone} on both legs");
        }
        // The feet stand on the ground.
        let low = model.lods[lod]
            .vertices
            .iter()
            .filter(|v| v.part == part::LOCOMOTION)
            .map(|v| v.pos[2])
            .fold(f32::MAX, f32::min);
        assert!(low < 0.05, "lod{lod}: lowest foot {low}");
    }
    // Nothing on a leg is a refit piece.
    assert!(fitted().lods[0]
        .vertices
        .iter()
        .all(|v| v.part != part::LOCOMOTION || v.rig & (rig::UPGRADE | rig::MODULE_MASK) == 0));
}

#[test]
fn every_suite_piece_rides_the_claw_arm() {
    let model = fitted();
    for (lod, mesh) in model.lods.iter().enumerate().take(2) {
        let suite: Vec<_> = mesh
            .vertices
            .iter()
            .filter(|v| v.rig & rig::MODULE_MASK != 0)
            .collect();
        assert!(!suite.is_empty(), "lod{lod}: no suite kit");
        assert!(
            suite
                .iter()
                .all(|v| v.part == part::TURRET && v.rig & rig::LIMB_MASK == rig::ARM_TOOL),
            "lod{lod}: suite kit off the claw arm"
        );
        assert!(
            suite.iter().any(|v| v.material == GLOW_VIOLET),
            "lod{lod}: suites add no nanite emitters"
        );
    }
}

#[test]
fn fits_the_triangle_budgets() {
    // As the library's budget test counts it: the bare unit, no suites fitted.
    let model = build_model("naga_commander").unwrap();
    let [full, mid, coarse] = [0, 1, 2].map(|i| model.lods[i].indices.len() / 3);
    let suited = fitted().lods[0].indices.len() / 3;
    println!("naga_commander: {full}/{mid}/{coarse}, suited {suited}");
    assert!(full <= super::super::COMMANDER_TRIANGLES, "{full}");
    assert!(
        suited <= super::super::COMMANDER_TRIANGLES + 1000,
        "suited {suited}"
    );
    assert!(
        mid as f32 <= full as f32 * 0.45 + 20.0 && coarse < 60,
        "{full}/{mid}/{coarse}"
    );
}
