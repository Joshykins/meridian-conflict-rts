use super::{FLOOR, HINGE, LIP};
use crate::{build_model, build_model_scaled, material, part};

fn ark() -> mc_data::UnitBlueprint {
    let bp = mc_data::Blueprints::load(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
    )
    .unwrap();
    bp.unit(bp.id_of("regency_t3_assault_transport").unwrap())
        .clone()
}

#[test]
fn the_ark_is_drawn_at_its_unit_files_size_with_its_detail_kept_to_budget() {
    let unit = ark();
    assert_eq!(unit.visual.mesh, "regency_ark");
    assert_eq!(unit.radius.to_f32(), super::RADIUS);
    assert_eq!(unit.height.to_f32(), super::HEIGHT);
    let model = build_model_scaled("regency_ark", super::RADIUS, super::HEIGHT, 3).unwrap();
    let tris: Vec<usize> = model.lods.iter().map(|m| m.indices.len() / 3).collect();
    assert!((6000..=14000).contains(&tris[0]), "{tris:?}");
    assert!(tris[1] as f32 <= tris[0] as f32 * 0.5, "{tris:?}");
    assert!(tris[2] < 60, "{tris:?}");
    for mesh in &model.lods {
        let top = mesh.vertices.iter().map(|v| v.pos[2]).fold(0.0, f32::max);
        assert!((80.0..=125.0).contains(&top), "top {top}");
        assert!(mesh
            .vertices
            .iter()
            .all(|v| v.pos.iter().all(|x| x.is_finite()) && v.pos[2] >= -0.001));
        let reach = mesh
            .vertices
            .iter()
            .map(|v| v.pos[0].hypot(v.pos[1]))
            .fold(0.0, f32::max);
        assert!(reach <= super::RADIUS * 1.3, "reach {reach}");
        assert!(mesh
            .vertices
            .iter()
            .any(|v| v.material == material::TEAM && v.normal[2] > 0.5));
    }
    // Red light, never construction violet: the Ark builds nothing.
    let lod0 = &model.lods[0].vertices;
    assert!(lod0.iter().any(|v| v.material == material::GLOW_LASER));
    assert!(lod0.iter().all(|v| v.material != material::GLOW_VIOLET));
    assert!(model.lifts.len() >= 8, "{} lifts", model.lifts.len());
}

#[test]
fn the_ark_leaves_its_hold_lane_and_the_way_in_under_its_stern_clear() {
    let unit = ark();
    let t = unit.transport.unwrap();
    assert_eq!(t.hinge.to_f32(), HINGE);
    assert_eq!(t.lip.to_f32(), LIP);
    assert_eq!(t.floor.to_f32(), FLOOR);
    // The ramp drops one in two, so it swings shut flush with the belly.
    assert!(((FLOOR / (HINGE - LIP)) - 0.5).abs() < 1e-4);
    let model = build_model("regency_ark").unwrap();
    let half = t.width.to_f32() * 0.5;
    let roof = FLOOR + t.clearance.to_f32();
    // As far forward as cargo stows (`mc_sim::transport`, its stow reach twice over).
    let front = t.hold.x.to_f32() + 8.0;
    for (lod, mesh) in model.lods.iter().enumerate().take(2) {
        let blocked = mesh.vertices.iter().find(|v| {
            let [x, y, z] = v.pos;
            let lane = y.abs() < half - 0.1;
            let hold = x > HINGE + 0.1 && x < front && z > FLOOR + 0.1 && z < roof - 0.1;
            let walk_in = x < LIP - 0.1 && z < FLOOR - 1.0;
            lane && (hold || walk_in) && v.part != part::RAMP
        });
        assert!(blocked.is_none(), "lod{lod}: {:?}", blocked.map(|v| v.pos));
    }
    let rig = crate::capital_rig("regency_ark").unwrap();
    assert_eq!([rig[6][2], rig[6][3]], [HINGE, FLOOR]);
    assert!(model.lods[0].vertices.iter().any(|v| v.part == part::RAMP));
}
