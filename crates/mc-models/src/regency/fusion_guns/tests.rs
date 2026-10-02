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
