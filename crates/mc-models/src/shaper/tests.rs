use super::super::build_model;
use super::*;

#[test]
fn stages_stand_at_their_heights_within_budget() {
    for kind in 0..3u8 {
        for (i, key) in stage_keys(kind).iter().enumerate() {
            let level = STAGE_LEVELS[i];
            let model = build_model(key).unwrap();
            let tris: Vec<usize> = model.lods.iter().map(|l| l.indices.len() / 3).collect();
            let (mut top, mut out) = (f32::MIN, 0.0f32);
            for v in &model.lods[0].vertices {
                top = top.max(v.pos[2]);
                out = out.max(v.pos[0].hypot(v.pos[1]));
            }
            println!("{key}: level {level}, triangles {tris:?}, top {top:.0}, out {out:.0}");
            assert!(tris[0] <= 40_000, "{key}: {} triangles", tris[0]);
            let h = height(level);
            assert!(
                (top - h).abs() < h * 0.02 + 1.0,
                "{key}: top {top}, height {h}"
            );
            assert!(out <= reach(kind, level) * 1.6, "{key}: reaches {out}");
        }
    }
}

#[test]
fn a_level_is_drawn_with_the_last_stage_it_reached() {
    assert_eq!(stage(1), 0);
    assert_eq!(stage(0), 0);
    assert_eq!(STAGE_LEVELS[stage(9)], 8);
    assert_eq!(STAGE_LEVELS[stage(10)], 10);
    assert_eq!(stage(500), STAGES - 1);
}
