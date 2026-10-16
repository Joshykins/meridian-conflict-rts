//! The Fulgur (`aster_t4_assault_tank`, mesh "assault_tank"): its running gear turns,
//! every road wheel, sprocket and return roller rigged as a wheel (`rig::WHEEL`) about
//! its own axle, and the belt not.

use glam::Vec2;

use crate::gpu_consts::wheel;
use crate::{build_model_scaled, material, part, rig, MeshVertex};

fn radius(v: &MeshVertex) -> f32 {
    ((v.rig >> wheel::RADIUS_SHIFT) & wheel::RADIUS_MASK) as f32 * wheel::RADIUS_STEP
}

/// At the unit's size, each wheel's verts lie round the axle their uv carries, within
/// the rolling radius (a sprocket's teeth a little past it), and the unit is scaled
/// evenly in x and z, so a wheel stays round as it turns.
#[test]
fn fulgur_wheels_turn_about_their_axles() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let bps = mc_data::Blueprints::load(&data).unwrap();
    let bp = bps.unit(bps.id_of("aster_t4_assault_tank").unwrap());
    assert_eq!(bp.visual.mesh, "assault_tank");
    let (radius_at, height_at) = crate::authored_size("assault_tank").unwrap();
    let (r, h) = (bp.radius.to_f32(), bp.height.to_f32());
    assert!(
        (r / radius_at - h / height_at).abs() < 1e-3,
        "fulgur: scaled unevenly ({r} x {h}), its wheels would stretch as they turn"
    );
    let model = build_model_scaled("assault_tank", r, h, 1).expect("the Fulgur");
    let wheels: Vec<&MeshVertex> = model.lods[0]
        .vertices
        .iter()
        .filter(|v| v.part == part::LOCOMOTION && v.rig & rig::WHEEL != 0)
        .collect();
    for v in &wheels {
        assert_ne!(v.material, material::TREAD, "fulgur: the belt is no wheel");
        let r = radius(v);
        let off = Vec2::new(v.pos[0] - v.uv[0], v.pos[2] - v.uv[1]).length();
        assert!(
            r > 0.0 && off <= r * 1.2,
            "fulgur: a wheel vertex {off} m from its axle, radius {r}"
        );
    }
    // Two track units, each a sprocket at either end, at least two road wheels and three
    // return rollers; the two sides share their axles' x and z.
    let mut axles: Vec<[i32; 2]> = wheels
        .iter()
        .map(|v| v.uv.map(|c| (c * 100.0).round() as i32))
        .collect();
    axles.sort_unstable();
    axles.dedup();
    assert!(
        axles.len() >= 2 * (2 + 2 + 3),
        "fulgur: {} wheel axles",
        axles.len()
    );
}
