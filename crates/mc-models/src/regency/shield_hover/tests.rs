use super::*;
use crate::build_model;
use crate::material::GLOW_VIOLET;

const KEY: &str = "regency_shield_hover";

#[test]
fn fits_the_librarys_checks() {
    super::super::check(KEY, 5.4, 6.8, None, &[]);
}

#[test]
fn floats_on_its_lift_bells() {
    let model = build_model(KEY).unwrap();
    assert!(model.hover && model.legs.is_none() && model.treads.is_none());
    // Two under each sponson and one under the body.
    assert_eq!(model.lifts.len(), 5);
    for lod in &model.lods {
        let low = lod
            .vertices
            .iter()
            .map(|v| v.pos[2])
            .fold(f32::MAX, f32::min);
        assert!(low > 0.15, "hangs clear of the ground: lowest {low}");
        assert!(lod.vertices.iter().all(|v| v.material != GLOW_VIOLET));
    }
}
