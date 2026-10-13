//! The yard against the unit file and the library's checks.

use super::*;
use crate::build_model_scaled;

const KEY: &str = "regency_sky_yard";
/// The unit file's lot, in build cells.
const CELLS: u8 = 24;

#[test]
fn it_fits_the_librarys_checks() {
    super::super::check(KEY, RADIUS, HEIGHT, Some(u32::from(CELLS)), &[]);
}

/// The model is authored at the unit file's size, on its lot, and its emitters are the
/// unit file's: the great beam's crown first, then the four intakes.
#[test]
fn the_unit_file_is_the_models_size_and_emitters() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let blueprints = mc_data::Blueprints::load(&dir).unwrap();
    let bp = blueprints.unit(blueprints.id_of("regency_t5_sky_yard").unwrap());
    assert_eq!(bp.visual.mesh, KEY);
    assert!((bp.radius.to_f32() - RADIUS).abs() < 1e-3);
    assert!((bp.height.to_f32() - HEIGHT).abs() < 1e-3);
    assert_eq!(bp.footprint, (CELLS, CELLS));
    let emitters: Vec<Vec3> = bp
        .builder
        .as_ref()
        .unwrap()
        .emitters
        .iter()
        .map(|e| Vec3::new(e.x.to_f32(), e.y.to_f32(), e.z.to_f32()))
        .collect();
    assert_eq!(emitters[0], EMITTER);
    let intakes: Vec<Vec3> = (0..4)
        .map(|k| {
            let a = std::f32::consts::FRAC_PI_2 * k as f32;
            Vec3::new(a.cos() * INTAKE_R, a.sin() * INTAKE_R, INTAKE_Z)
        })
        .collect();
    for (e, i) in emitters[1..].iter().zip(&intakes) {
        assert!(e.distance(*i) < 1e-3, "intake {e} is not at {i}");
    }
}

/// Nothing stands over the emitter: the beam goes straight up from it clear of the model.
#[test]
fn the_beam_goes_up_clear() {
    let model = build_model_scaled(KEY, RADIUS, HEIGHT, 5).unwrap();
    let over = model.lods[0].vertices.iter().find(|v| {
        let p = Vec3::from(v.pos);
        p.z > EMITTER.z + 0.5 && p.truncate().length() < 5.0
    });
    assert!(over.is_none(), "{over:?} over the emitter");
}
