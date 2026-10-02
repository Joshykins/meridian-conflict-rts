use glam::Vec3;

use super::*;
use crate::material::GLOW_VIOLET;
use crate::{build_model, part, rig};

fn muzzles() -> Vec<[f32; 3]> {
    MUZZLES.iter().map(|m| m.to_array()).collect()
}

#[test]
fn fits_the_librarys_checks() {
    super::super::check("regency_bombard", RADIUS, HEIGHT, None, &muzzles());
}

#[test]
fn the_unit_files_launcher_is_the_models() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let blueprints = mc_data::Blueprints::load(&dir).unwrap();
    let bp = blueprints.unit(blueprints.id_of("regency_t2_bombard").unwrap());
    let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
    assert_eq!(bp.visual.mesh, "regency_bombard");
    assert!((bp.radius.to_f32() - RADIUS).abs() < 1e-3);
    assert!((bp.height.to_f32() - HEIGHT).abs() < 1e-3);
    let w = &bp.weapons[0];
    assert!(v(w.pivot.expect("the pod pitches")).distance(PIVOT) < 1e-3);
    assert_eq!(w.muzzles.len(), MUZZLES.len());
    for (a, b) in w.muzzles.iter().zip(MUZZLES) {
        assert!(v(*a).distance(b) < 1e-3, "{a:?} vs {b}");
    }
    assert!(w.cluster.is_some(), "its seekers split");

    let model = build_model("regency_bombard").unwrap();
    assert_eq!(model.arm_pivot.map(Vec3::from), Some(PIVOT));
    for lod in &model.lods {
        // The pod pitches as one piece on the turret, and it does not build.
        assert!(lod
            .vertices
            .iter()
            .any(|v| v.part == part::TURRET && v.rig & rig::LIMB_MASK == rig::ARM_GUN));
        assert!(lod.vertices.iter().all(|v| v.material != GLOW_VIOLET));
    }
}

#[test]
fn strides_on_two_long_legs_in_boots() {
    let model = build_model("regency_bombard").unwrap();
    let crawl = model.legs.and_then(|l| l.crawl).expect("a walker");
    assert_eq!(crawl.pairs, 1);
    let (hip, knee, foot, _) = LEG;
    // Tall and lean: the leg is longer than the hull is high off it, the knee forward.
    assert!(hip.z > HEIGHT * 0.55, "hip at {}", hip.z);
    assert!(knee.x > hip.x + 0.5 && knee.x > foot.x + 0.5);
    for lod in &model.lods[..2] {
        let shin = |side: f32| {
            lod.vertices.iter().filter(move |v| {
                v.part == part::LOCOMOTION
                    && v.rig & rig::LIMB_MASK == rig::SHIN
                    && v.pos[1] * side > 0.0
            })
        };
        for side in [1.0, -1.0] {
            let low = shin(side).map(|v| v.pos[2]).fold(f32::MAX, f32::min);
            assert!(low < 0.1, "boot sole at {low}");
        }
    }
}
