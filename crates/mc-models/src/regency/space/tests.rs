use super::destroyer::{seeker_muzzles, CORE_MUZZLE};
use crate::{build_model, build_model_scaled, material};
use glam::Vec3;

fn blueprints() -> mc_data::Blueprints {
    mc_data::Blueprints::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
        .unwrap()
}

#[test]
fn the_hulls_fit_their_blueprints_and_keep_their_identity_at_each_lod() {
    let bp = blueprints();
    for key in [
        "regency_t2_transport",
        "regency_t2_space_frigate",
        "regency_t3_space_cruiser",
        "regency_t3_space_destroyer",
    ] {
        let unit = bp.unit(bp.id_of(key).unwrap());
        let mesh = &unit.visual.mesh;
        let model = build_model_scaled(mesh, unit.radius.to_f32(), unit.height.to_f32(), unit.tech)
            .unwrap();
        let full = model.lods[0].indices.len() / 3;
        let mid = model.lods[1].indices.len() / 3;
        let coarse = model.lods[2].indices.len() / 3;
        let most = super::super::triangles(mesh).unwrap_or(9000);
        assert!((250..=most).contains(&full), "{mesh}: {full} triangles");
        assert!(
            mid as f32 <= full as f32 * 0.45 + 20.0 && coarse < 60,
            "{mesh}: {full}/{mid}/{coarse}"
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
fn the_coffer_clears_the_entire_boarding_lane_and_matches_its_animated_ramp() {
    let bp = blueprints();
    for (key, front) in [("regency_t2_transport", 23.0)] {
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
}

#[test]
fn the_destroyers_lance_leaves_its_core_and_its_seekers_leave_their_cells() {
    let bp = blueprints();
    let unit = bp.unit(bp.id_of("regency_t3_space_destroyer").unwrap());
    let model = build_model(&unit.visual.mesh).unwrap();
    let at = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
    // The lance is laid from the core's lens at the ship's middle: no turret, nothing
    // turns or pitches (a pivot would pitch the hull: `capital_ship` in entity.wgsl).
    let lance = &unit.weapons[0];
    let core = Vec3::from(CORE_MUZZLE);
    assert!(lance.pivot.is_none() && core.truncate().length() < 0.01);
    assert!(at(lance.muzzle).distance(core) < 0.01, "lance muzzle");
    // The lens's face lies in the muzzle's plane, round it.
    let face: Vec<Vec3> = model.lods[0]
        .vertices
        .iter()
        .filter(|v| v.material == material::GLOW_PRISM && (v.pos[2] - core.z).abs() < 0.05)
        .map(|v| Vec3::from(v.pos))
        .collect();
    let middle = face.iter().sum::<Vec3>() / face.len().max(1) as f32;
    assert!(
        face.len() >= 8 && middle.distance(core) < 0.1,
        "no lens at the core's muzzle: {} points about {middle}",
        face.len()
    );
    assert!(model.lods[0]
        .vertices
        .iter()
        .all(|v| v.part != crate::part::TURRET));
    // Each seeker battery fires from its own block's open cells, port block first.
    for (want, weapon) in seeker_muzzles().iter().zip(&unit.weapons[1..3]) {
        assert_eq!(weapon.hatch_ticks, 0, "open cells");
        let have: Vec<Vec3> = weapon.muzzles.iter().map(|&p| at(p)).collect();
        assert!(
            have.len() == want.len() && have.iter().zip(want).all(|(h, w)| h.distance(*w) < 0.02),
            "{}: muzzles {:?}",
            weapon.name,
            want.iter()
                .map(|p| format!("({:.2}, {:.2}, {:.2})", p.x, p.y, p.z))
                .collect::<Vec<_>>()
        );
        assert!(at(weapon.muzzle).distance(want[0]) < 0.02);
    }
}
