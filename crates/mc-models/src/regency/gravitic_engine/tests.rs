//! The gravitic engine's designs against the unit file and the library's checks.

use super::*;
use crate::build_model_scaled;

const KEY: &str = "regency_gravitic_engine";
/// The unit file's lot, in build cells.
const CELLS: u8 = 22;

#[test]
fn fits_the_librarys_checks() {
    super::super::check(KEY, RADIUS, HEIGHT, Some(u32::from(CELLS)), &[]);
}

/// The model is authored at the unit file's size, on its lot.
#[test]
fn the_unit_file_is_the_models_size() {
    let dir =
        std::path::Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../data");
    let blueprints = mc_data::Blueprints::load(&dir).unwrap();
    let bp = blueprints.unit(blueprints.id_of("regency_t5_gravitic_engine").unwrap());
    assert_eq!(bp.visual.mesh, KEY);
    assert!((bp.radius.to_f32() - RADIUS).abs() < 1e-3);
    assert!((bp.height.to_f32() - HEIGHT).abs() < 1e-3);
    assert_eq!(bp.footprint, (CELLS, CELLS));
}

/// The room round the core is left open for the energy, over the well's middle, below
/// the shards' points.
#[test]
fn the_core_is_left_open() {
    assert_eq!(CORE.truncate(), Vec2::ZERO, "core off the well");
    let model = build_model_scaled(KEY, RADIUS, HEIGHT, 5).unwrap();
    let near = model.lods[0]
        .vertices
        .iter()
        .map(|v| Vec3::from(v.pos).distance(CORE))
        .fold(f32::MAX, f32::min);
    assert!(near > CORE_CLEAR, "something {near} m from the core");
    let top = model.lods[0]
        .vertices
        .iter()
        .map(|v| v.pos[2])
        .fold(0.0, f32::max);
    assert!(CORE.z < top * 0.6, "core {CORE} not held among the shards");
}
