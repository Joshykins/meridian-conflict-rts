//! The gravitic engine's designs against the unit file and the library's checks.

use super::*;
use crate::build_model_scaled;

const DESIGNS: [&str; 3] = [
    "regency_gravitic_engine",
    "regency_gravitic_engine~b",
    "regency_gravitic_engine~c",
];
/// The unit file's lot, in build cells.
const CELLS: u8 = 22;

#[test]
fn every_design_fits_the_librarys_checks() {
    for key in DESIGNS {
        super::super::check(key, RADIUS, HEIGHT, Some(u32::from(CELLS)), &[]);
    }
}

/// The model is authored at the unit file's size, on its lot.
#[test]
fn the_unit_file_is_the_models_size() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let blueprints = mc_data::Blueprints::load(&dir).unwrap();
    let bp = blueprints.unit(blueprints.id_of("regency_t5_gravitic_engine").unwrap());
    assert_eq!(bp.visual.mesh, DESIGNS[0]);
    assert!((bp.radius.to_f32() - RADIUS).abs() < 1e-3);
    assert!((bp.height.to_f32() - HEIGHT).abs() < 1e-3);
    assert_eq!(bp.footprint, (CELLS, CELLS));
}

/// Each design leaves the room round its core open for the energy, over the well's
/// middle, below the shards' points.
#[test]
fn the_core_is_left_open() {
    for (key, core) in DESIGNS.into_iter().zip(CORE) {
        assert_eq!(core.truncate(), Vec2::ZERO, "{key}: core off the well");
        let model = build_model_scaled(key, RADIUS, HEIGHT, 5).unwrap();
        let near = model.lods[0]
            .vertices
            .iter()
            .map(|v| Vec3::from(v.pos).distance(core))
            .fold(f32::MAX, f32::min);
        assert!(near > CORE_CLEAR, "{key}: something {near} m from the core");
        let top = model.lods[0]
            .vertices
            .iter()
            .map(|v| v.pos[2])
            .fold(0.0, f32::max);
        assert!(
            core.z < top * 0.6,
            "{key}: core {core} not held among the shards"
        );
    }
}
