use glam::Vec3;

use super::{legs, CORE, CORE_CLEAR, HEIGHT, RADIUS};
use crate::build_model_scaled;

const KEY: &str = "regency_gravitic_lens";

#[test]
fn fits_the_librarys_checks() {
    super::super::check(KEY, RADIUS, HEIGHT, None, &[]);
}

/// The core is left open and held between shards on opposite sides, at every level.
#[test]
fn the_shards_hold_the_core() {
    let model = build_model_scaled(KEY, RADIUS, HEIGHT, 3).unwrap();
    for (lod, mesh) in model.lods.iter().enumerate() {
        let round: Vec<Vec3> = mesh
            .vertices
            .iter()
            .map(|v| Vec3::from(v.pos) - CORE)
            .collect();
        let near = round.iter().map(|d| d.length()).fold(f32::MAX, f32::min);
        assert!(near >= CORE_CLEAR, "lod{lod}: {near} m from the core");
        let holding: Vec<Vec3> = round
            .iter()
            .filter(|d| d.length() < 2.6 && d.z > -0.5)
            .filter_map(|d| d.with_z(0.0).try_normalize())
            .collect();
        assert!(
            holding
                .iter()
                .any(|a| holding.iter().any(|c| a.dot(*c) < -0.4)),
            "lod{lod}: nothing holds the core from both sides"
        );
    }
}

/// It walks on two pairs of shard legs and braces with its deploy: packed, its feet stand
/// in and its body rides higher, and its legs still reach (no bone stretched).
#[test]
fn walks_on_four_shards_and_braces() {
    let model = build_model_scaled(KEY, RADIUS, HEIGHT, 3).unwrap();
    let crawl = model.legs.and_then(|l| l.crawl).expect("a crawler");
    assert_eq!(crawl.pairs, 2);
    let [sink, widen] = crawl.brace.expect("it braces");
    assert!((sink - legs::SINK).abs() < 1e-4 && widen == legs::WIDEN);
    assert_eq!(crawl.gpu()[crate::CRAWL_SLOTS - 1], [sink, widen, 0.0, 0.0]);
    for &(hip, knee, foot, _) in &legs::CRAWL {
        let packed_hip = hip + Vec3::Z * sink;
        let packed_foot = hip + (foot - hip) * (1.0 - widen);
        let reach = (packed_foot.with_z(0.0) - packed_hip).length();
        let bones = (knee - hip).length() + (foot - knee).length();
        assert!(
            reach < bones * 0.95,
            "packed, the leg cannot reach its foot"
        );
    }
}
