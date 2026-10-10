use glam::Vec3;

use super::{gun, legs, HEIGHT, RADIUS};
use crate::{build_model_scaled, part, rig};

const KEY: &str = "regency_saker";

/// It fits the library's checks, and holds its charge between its halves' tips.
#[test]
fn holds_its_charge() {
    super::super::check_charge(
        KEY,
        RADIUS,
        HEIGHT,
        None,
        &[gun::BORE.line.muzzle().to_array()],
        gun::HOLD,
    );
}

/// It walks on two pairs of legs and braces with its deploy: packed, its feet stand in and
/// its hull rides higher, and its legs still reach (no bone stretched).
#[test]
fn walks_on_four_legs_and_braces() {
    let model = build_model_scaled(KEY, RADIUS, HEIGHT, 4).unwrap();
    let crawl = model.legs.and_then(|l| l.crawl).expect("a crawler");
    assert_eq!(crawl.pairs, 2);
    let [sink, widen] = crawl.brace.expect("it braces");
    assert!((sink - legs::SINK).abs() < 1e-4 && widen == legs::WIDEN);
    for &(hip, knee, foot, _) in &legs::LEGS.joints {
        let packed_hip = hip + Vec3::Z * sink;
        let packed_foot = hip + (foot - hip) * (1.0 - widen);
        let reach = (packed_foot.with_z(foot.z) - packed_hip).length();
        let bones = (knee - hip).length() + (foot - knee).length();
        assert!(reach < bones * 0.95, "packed, a leg cannot reach");
        // Planted, the knee stays bent.
        assert!(hip.distance(foot) < bones * 0.9, "a leg stands straight");
    }
    // Low: the hull's top is near half the unit's height, and the feet stand out at its
    // corners.
    let mesh = &model.lods[0];
    let hull_top = mesh
        .vertices
        .iter()
        .filter(|v| v.part == part::HULL)
        .map(|v| v.pos[2])
        .fold(0.0, f32::max);
    assert!(hull_top < HEIGHT * 0.55, "hull top {hull_top}");
    let feet = mesh
        .vertices
        .iter()
        .filter(|v| v.rig & rig::LIMB_MASK == rig::FOOT)
        .map(|v| v.pos[0].hypot(v.pos[1]))
        .fold(0.0, f32::max);
    assert!(feet > RADIUS * 0.9, "feet at {feet}");
}

/// The unit file's size, pivot and muzzle are the model's.
#[test]
fn the_unit_files_gun_is_the_models() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let blueprints = mc_data::Blueprints::load(&dir).unwrap();
    let bp = blueprints.unit(blueprints.id_of("regency_t4_surge_artillery").unwrap());
    assert_eq!(bp.visual.mesh, KEY);
    assert!((bp.radius.to_f32() - RADIUS).abs() < 1e-3, "radius");
    assert!((bp.height.to_f32() - HEIGHT).abs() < 1e-3, "height");
    assert!(bp.turret_at.is_none(), "the turret turns about the middle");
    let w = &bp.weapons[0];
    let v = |p: mc_core::FxVec3| Vec3::from(p.to_f32());
    let line = gun::BORE.line;
    assert!(v(w.muzzle).distance(line.muzzle()) < 0.02, "muzzle");
    assert!(v(w.pivot.unwrap()).distance(line.pivot) < 0.02, "pivot");
}
