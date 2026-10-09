use glam::Vec3;

use super::{howitzer, skyspear};
use crate::build_model;

#[test]
fn skyspear_holds_its_charge() {
    super::super::check_charge(
        "regency_skyspear",
        skyspear::RADIUS,
        skyspear::HEIGHT,
        None,
        &[skyspear::LINE.muzzle().to_array()],
        skyspear::HOLD,
    );
}

#[test]
fn howitzer_holds_its_charge() {
    super::super::check_charge(
        "regency_fusion_howitzer",
        howitzer::RADIUS,
        howitzer::HEIGHT,
        None,
        &[howitzer::LINE.muzzle().to_array()],
        howitzer::HOLD,
    );
}

/// The unit file's size, pivot and muzzle are the model's, and the gun is Pinch-fusion.
#[test]
fn the_unit_files_guns_are_the_models() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let blueprints = mc_data::Blueprints::load(&dir).unwrap();
    for (key, mesh, line, radius, height) in [
        (
            "regency_t3_mobile_aa",
            "regency_skyspear",
            skyspear::LINE,
            skyspear::RADIUS,
            skyspear::HEIGHT,
        ),
        (
            "regency_t3_artillery",
            "regency_fusion_howitzer",
            howitzer::LINE,
            howitzer::RADIUS,
            howitzer::HEIGHT,
        ),
    ] {
        let bp = blueprints.unit(blueprints.id_of(key).unwrap());
        assert_eq!(bp.visual.mesh, mesh);
        assert_eq!(bp.tech, 3);
        assert!((bp.radius.to_f32() - radius).abs() < 1e-3, "{key} radius");
        assert!((bp.height.to_f32() - height).abs() < 1e-3, "{key} height");
        assert!(
            bp.turret_at.is_none(),
            "{key}: the turret turns about the middle"
        );
        let w = &bp.weapons[0];
        assert_eq!(w.plasma_grade, Some(mc_data::PlasmaGrade::PinchFusion));
        let v = |p: mc_core::FxVec3| Vec3::from(p.to_f32());
        assert!(v(w.muzzle).distance(line.muzzle()) < 0.02, "{key} muzzle");
        assert!(
            v(w.pivot.unwrap()).distance(line.pivot) < 0.02,
            "{key} pivot"
        );
        let model = build_model(mesh).unwrap();
        assert_eq!(model.arm_pivot, Some(line.pivot.to_array()));
        assert!(model.charge_gear.is_some(), "{key}: its core spins up");
    }
}

/// Both run on tracks.
#[test]
fn both_run_on_tracks() {
    for key in ["regency_skyspear", "regency_fusion_howitzer"] {
        let model = build_model(key).unwrap();
        assert!(model.treads.is_some(), "{key} runs on tracks");
        assert!(!model.hover && model.legs.is_none(), "{key}");
    }
}

/// The Spire plants on four pods: its siege rig is declared, its arms lift the body, and
/// every arm and pod lies clear of the centre lines, since the shader picks the corner by
/// the side a vertex is on.
#[test]
fn the_spire_plants_on_four_pods() {
    use crate::gpu_consts::siege;
    let model = build_model("regency_skyspear").unwrap();
    let rig = model.siege.expect("the Spire has a siege rig");
    assert!(rig.lift() > 2.0 && rig.stow < 0.0 && rig.retract > 0.0);
    // Far off the arms are dropped; the pods still turn out.
    for (lod, mesh) in model.lods.iter().enumerate() {
        let mut pieces = [0usize; 4];
        for v in &mesh.vertices {
            if v.rig & crate::rig::DEPLOY == 0 {
                continue;
            }
            let piece = (v.rig >> siege::SHIFT) & siege::MASK;
            pieces[piece as usize] += 1;
            if piece != 0 {
                assert!(
                    v.pos[0].abs() > 1.0 && v.pos[1].abs() > 1.0,
                    "a leg vertex at {:?} sits on a centre line",
                    v.pos
                );
            }
        }
        let legs = if lod < 2 {
            &pieces[1..3]
        } else {
            &pieces[2..3]
        };
        assert!(legs.iter().all(|&n| n > 0), "lod{lod} pieces {pieces:?}");
    }
}
