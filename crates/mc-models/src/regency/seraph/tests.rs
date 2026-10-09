//! The Seraph model against the unit file and the library's checks.

use super::deck::{CELLS, CELL_R, DECK};
use super::*;
use crate::build_model;

const KEY: &str = "regency_seraph";

#[test]
fn fits_the_librarys_checks() {
    super::super::check(
        KEY,
        RADIUS,
        HEIGHT,
        None,
        &[MUZZLE.to_array(), (MUZZLE * v3(1.0, -1.0, 1.0)).to_array()],
    );
}

/// The unit file builds the model `SCALE` times its authored size: its size, the
/// repeaters' muzzles and elbows, and the seekers' cells are this file's times `SCALE`.
#[test]
fn the_unit_files_guns_are_the_models() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let blueprints = mc_data::Blueprints::load(&dir).unwrap();
    let bp = blueprints.unit(blueprints.id_of("regency_t5_seraph").unwrap());
    let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
    let mirror = Vec3::new(1.0, -1.0, 1.0);
    assert_eq!(bp.visual.mesh, KEY);
    assert!((bp.radius.to_f32() - RADIUS * SCALE).abs() < 1e-3);
    assert!((bp.height.to_f32() - HEIGHT * SCALE).abs() < 1e-3);
    assert_eq!(bp.weapons.len(), 3);
    for (w, side) in bp.weapons[..2].iter().zip([Vec3::ONE, mirror]) {
        assert!(
            v(w.muzzle).distance(MUZZLE * side * SCALE) < 1e-2,
            "{}",
            w.name
        );
        assert!(v(w.pivot.unwrap()).distance(ELBOW * side * SCALE) < 1e-2);
        assert!(!w.mount, "the forearms pitch with the torso's aim");
        assert!(
            (w.rest_pitch.0 as i16) < -10923,
            "idle, the guns hang down past 60 degrees"
        );
    }
    let seekers = &bp.weapons[2];
    assert!(seekers.vertical_launch && seekers.mount);
    assert_eq!(seekers.muzzles.len(), CELLS.len());
    for (m, [x, y]) in seekers.muzzles.iter().zip(CELLS) {
        assert!(
            v(*m).distance(v3(x, y, DECK) * SCALE) < 1e-2,
            "cell {x},{y}"
        );
    }
    assert!(bp.turret_at.is_none(), "the torso turns about the waist");
    let model = build_model(KEY).unwrap();
    assert_eq!(Vec3::from(model.turret_pivot), v3(0.0, 0.0, WAIST));
    assert_eq!(Vec3::from(model.arm_pivot.unwrap()), ELBOW);
}

/// The seekers leave straight up: nothing of the model stands over a cell's mouth.
#[test]
fn the_cells_are_open_to_the_sky() {
    let model = build_model(KEY).unwrap();
    for [x, y] in CELLS {
        let over = model.lods[0].vertices.iter().find(|v| {
            v.pos[2] > DECK + 0.2 && Vec2::new(v.pos[0] - x, v.pos[1] - y).length() < CELL_R * 2.0
        });
        assert!(over.is_none(), "{over:?} over the cell at {x},{y}");
    }
}

/// It hangs in the air, its legs staggered: the left toe lowest, the right drawn up.
#[test]
fn hangs_off_the_ground() {
    let model = build_model(KEY).unwrap();
    let low = |side: f32| {
        model.lods[0]
            .vertices
            .iter()
            .filter(|v| v.pos[1] * side > 0.5 && v.pos[2] < 12.0)
            .map(|v| v.pos[2])
            .fold(f32::MAX, f32::min)
    };
    assert!(low(1.0) > 1.0, "left toe at {}", low(1.0));
    assert!(low(-1.0) > low(1.0) + 0.6, "the right leg is drawn up");
    assert!(model.hover);
}

/// Hung at their rest, the repeaters clear the legs.
#[test]
fn the_lowered_guns_clear_the_legs() {
    let model = build_model(KEY).unwrap();
    let gun = |v: &&crate::MeshVertex| v.rig & rig::LIMB_MASK == rig::ARM_GUN && v.pos[1] > 0.0;
    let rest = (-80f32).to_radians();
    let lowered = model.lods[0].vertices.iter().filter(gun).map(|v| {
        let d = Vec3::from(v.pos) - ELBOW;
        let (s, c) = rest.sin_cos();
        ELBOW + Vec3::new(d.x * c - d.z * s, d.y, d.x * s + d.z * c)
    });
    let inner = lowered.map(|p| p.y).fold(f32::MAX, f32::min);
    assert!(
        inner > 3.0,
        "a lowered gun reaches in to y {inner}, into the thigh"
    );
}
