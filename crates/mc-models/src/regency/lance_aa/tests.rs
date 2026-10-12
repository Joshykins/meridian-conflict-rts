use super::*;
use crate::build_model;
use crate::material::GLOW_VIOLET;

const DESIGNS: [&str; 3] = [
    "regency_lance_aa",
    "regency_lance_aa~b",
    "regency_lance_aa~c",
];

#[test]
fn every_design_holds_its_charge() {
    for key in DESIGNS {
        super::super::check_charge(key, RADIUS, HEIGHT, None, &[MUZZLE.to_array()], HOLD);
    }
}

#[test]
fn the_unit_files_lance_is_the_models() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let blueprints = mc_data::Blueprints::load(&dir).unwrap();
    let bp = blueprints.unit(blueprints.id_of("regency_t3_lance_aa").unwrap());
    let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
    assert_eq!(bp.visual.mesh, DESIGNS[0]);
    assert!((bp.radius.to_f32() - RADIUS).abs() < 1e-3);
    assert!((bp.height.to_f32() - HEIGHT).abs() < 1e-3);
    let w = &bp.weapons[0];
    assert!(v(w.muzzle).distance(MUZZLE) < 1e-3);
    assert!(v(w.pivot.unwrap()).distance(PIVOT) < 1e-3);
    assert!(bp.turret_at.is_none(), "the turret turns about the middle");
    assert_eq!(
        bp.motion.map(|m| m.layer),
        Some(mc_data::MoveLayer::Hover),
        "it rides its lift"
    );
    for key in DESIGNS {
        let model = build_model(key).unwrap();
        assert!(model.hover && model.legs.is_none() && model.treads.is_none());
        assert_eq!(Vec3::from(model.arm_pivot.unwrap()), PIVOT);
        assert!(Vec3::from(model.turret_pivot).truncate().length() < 1e-4);
        assert!(model.lifts.len() >= 4, "{key}");
        for lod in &model.lods {
            assert!(
                lod.vertices.iter().all(|v| v.material != GLOW_VIOLET),
                "{key}: it does not build"
            );
        }
    }
}
