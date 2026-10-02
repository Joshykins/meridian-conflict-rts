use super::*;
use crate::build_model;

const KEY: &str = "regency_seeker_hover";

#[test]
fn fits_the_librarys_checks() {
    let muzzles: Vec<[f32; 3]> = muzzles().iter().map(|m| m.to_array()).collect();
    super::super::check(KEY, 5.0, 5.6, None, &muzzles);
}

#[test]
fn the_unit_files_battery_is_the_models() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let blueprints = mc_data::Blueprints::load(&dir).unwrap();
    let bp = blueprints.unit(blueprints.id_of("regency_t2_mobile_aa").unwrap());
    let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
    assert_eq!(bp.visual.mesh, KEY);
    assert!((bp.radius.to_f32() - 5.0).abs() < 1e-3 && (bp.height.to_f32() - 5.6).abs() < 1e-3);
    let w = &bp.weapons[0];
    assert!(
        w.missile && w.guided && w.plasma_grade.is_some(),
        "Gravitic Seekers"
    );
    assert!(v(w.pivot.unwrap()).distance(PIVOT) < 1e-3);
    let ours = muzzles();
    assert_eq!(w.muzzles.len(), ours.len());
    for (theirs, ours) in w.muzzles.iter().zip(&ours) {
        assert!(
            v(*theirs).distance(*ours) < 1e-3,
            "{} vs {ours}",
            v(*theirs)
        );
    }
    assert!(v(w.muzzle).distance(ours[0]) < 1e-3);
    assert_eq!(w.salvo as usize, ours.len(), "one seeker from each cradle");
    assert!(bp.turret_at.is_none(), "the battery turns about the middle");
    assert_eq!(
        bp.motion.map(|m| m.layer),
        Some(mc_data::MoveLayer::Hover),
        "it crosses water on its lift"
    );
    let model = build_model(KEY).unwrap();
    assert_eq!(model.arm_pivot, Some(PIVOT.to_array()));
    assert!(Vec3::from(model.turret_pivot).truncate().length() < 1e-4);
}

#[test]
fn floats_on_its_lift_bells() {
    let model = build_model(KEY).unwrap();
    assert!(model.hover && model.legs.is_none() && model.treads.is_none());
    // Two under each sponson and one under the body, mirrored.
    assert_eq!(model.lifts.len(), 5);
    let left = model.lifts.iter().filter(|l| l.at[1] > 0.5).count();
    let right = model.lifts.iter().filter(|l| l.at[1] < -0.5).count();
    assert_eq!(left, right);
    for lod in &model.lods {
        let low = lod
            .vertices
            .iter()
            .map(|v| v.pos[2])
            .fold(f32::MAX, f32::min);
        assert!(low > 0.15, "hangs clear of the ground: lowest {low}");
        assert!(lod.vertices.iter().all(|v| v.material != GLOW_VIOLET));
    }
}
