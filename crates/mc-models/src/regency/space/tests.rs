use crate::{build_model, build_model_scaled, material, rig};
use glam::Vec3;

fn blueprints() -> mc_data::Blueprints {
    mc_data::Blueprints::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
        .unwrap()
}

#[test]
fn all_five_hulls_fit_their_blueprints_and_keep_their_identity_at_each_lod() {
    let bp = blueprints();
    for key in [
        "regency_t2_transport",
        "regency_t3_assault_transport",
        "regency_t2_space_frigate",
        "regency_t3_space_cruiser",
        "regency_t3_space_destroyer",
    ] {
        let unit = bp.unit(bp.id_of(key).unwrap());
        let model = build_model_scaled(
            &unit.visual.mesh,
            unit.radius.to_f32(),
            unit.height.to_f32(),
            unit.tech,
        )
        .unwrap();
        let full = model.lods[0].indices.len() / 3;
        let mid = model.lods[1].indices.len() / 3;
        let coarse = model.lods[2].indices.len() / 3;
        assert!((250..=9000).contains(&full), "{key}: {full} triangles");
        assert!(
            mid as f32 <= full as f32 * 0.45 + 20.0 && coarse < 60,
            "{key}: {full}/{mid}/{coarse}"
        );
        for mesh in &model.lods {
            let top = mesh
                .vertices
                .iter()
                .map(|v| v.pos[2])
                .fold(0.0f32, f32::max);
            assert!(
                top >= unit.height.to_f32() * 0.8 && top <= unit.height.to_f32() * 1.25,
                "{key}: top {top}"
            );
            assert!(mesh
                .vertices
                .iter()
                .all(|v| v.pos.iter().all(|x| x.is_finite()) && v.pos[2] >= -0.001));
            let reach = mesh
                .vertices
                .iter()
                .map(|v| v.pos[0].hypot(v.pos[1]))
                .fold(0.0f32, f32::max);
            assert!(reach <= unit.radius.to_f32() * 1.3, "{key}: reach {reach}");
            assert!(mesh
                .vertices
                .iter()
                .any(|v| v.material == material::PLATING_DARK));
            assert!(mesh
                .vertices
                .iter()
                .any(|v| v.material == material::TEAM && v.normal[2] > 0.5));
        }
        assert!(model.lods[0]
            .vertices
            .iter()
            .any(|v| v.material == material::GLOW_VIOLET));
        assert!(model.lifts.len() >= 2, "{key}: gravity propulsion");
    }
}

#[test]
fn transport_meshes_clear_the_entire_boarding_lane_and_match_the_animated_ramps() {
    let bp = blueprints();
    for (key, front) in [
        ("regency_t2_transport", 23.0),
        ("regency_t3_assault_transport", 45.0),
    ] {
        let unit = bp.unit(bp.id_of(key).unwrap());
        let t = unit.transport.unwrap();
        let model = build_model(&unit.visual.mesh).unwrap();
        let half = t.width.to_f32() * 0.5;
        let floor = t.floor.to_f32();
        let roof = floor + t.clearance.to_f32();
        for (lod, mesh) in model.lods.iter().enumerate().take(2) {
            let obstruction = mesh.vertices.iter().find(|v| {
                v.pos[1].abs() < half - 0.1
                    && v.pos[0] > t.hinge.to_f32() + 0.1
                    && v.pos[0] < front - 0.1
                    && v.pos[2] > floor + 0.1
                    && v.pos[2] < roof - 0.1
            });
            assert!(
                obstruction.is_none(),
                "{key} lod{lod}: cargo obstruction {:?}",
                obstruction.map(|v| v.pos)
            );
        }
        let rig = crate::capital_rig(&unit.visual.mesh).unwrap();
        assert_eq!(rig[6][2], t.hinge.to_f32());
        assert_eq!(rig[6][3], floor);
    }
}

#[test]
fn plasma_gun_houses_are_bound_to_their_weapons_and_end_at_their_muzzles() {
    let bp = blueprints();
    for key in ["regency_t2_space_frigate", "regency_t3_space_cruiser"] {
        let unit = bp.unit(bp.id_of(key).unwrap());
        let model = build_model(&unit.visual.mesh).unwrap();
        assert_eq!(model.houses.len(), unit.weapons.len());
        for (slot, weapon) in unit.weapons.iter().enumerate() {
            let p = weapon.pivot.unwrap();
            let pivot = Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
            assert_eq!(Vec3::from(model.houses[slot].pivot), pivot);
            assert_eq!(model.houses[slot].weapon as usize, slot);
            let p = weapon.muzzle;
            let muzzle = Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
            let near = model.lods[0]
                .vertices
                .iter()
                .filter(|v| v.material == material::GLOW_VIOLET)
                .map(|v| Vec3::from(v.pos).distance(muzzle))
                .fold(f32::MAX, f32::min);
            assert!(near < 1.2, "{key} gun {slot}: no emitter at muzzle, {near}");
        }
    }
    let unit = bp.unit(bp.id_of("regency_t3_space_destroyer").unwrap());
    let model = build_model(&unit.visual.mesh).unwrap();
    assert_eq!(model.turret_pivot, [12.0, 0.0, 16.0]);
    assert!(model.lods[0]
        .vertices
        .iter()
        .any(|v| v.rig & rig::LIMB_MASK == rig::ARM_GUN && v.material == material::GLOW_VIOLET));
}
