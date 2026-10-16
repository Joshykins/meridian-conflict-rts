use glam::Vec3;

use super::{legs, CORE, CORE_CLEAR, HEIGHT, RADIUS};
use crate::build_model_scaled;

const KEY: &str = "regency_gravitic_lens";

#[test]
fn fits_the_librarys_checks() {
    super::super::check(KEY, RADIUS, HEIGHT, None, &[]);
}

/// The core is left open and held between prongs on opposite sides, at every level that
/// draws them (the coarse one, a few pixels across, leaves them off).
#[test]
fn the_prongs_hold_the_core() {
    let model = build_model_scaled(KEY, RADIUS, HEIGHT, 3).unwrap();
    for (lod, mesh) in model.lods.iter().enumerate().take(2) {
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

/// It walks on three pairs of shard legs and braces with its deploy: packed, its feet stand
/// in and its body rides higher, and its legs still reach (no bone stretched).
#[test]
fn walks_on_six_shards_and_braces() {
    let model = build_model_scaled(KEY, RADIUS, HEIGHT, 3).unwrap();
    let crawl = model.legs.and_then(|l| l.crawl).expect("a crawler");
    assert_eq!(crawl.pairs, 3);
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

/// Walking (packed), every foot stays where its two bones can put it, all through its
/// step as crawl.wgsl `crawl_leg` moves it (planted and passing back, then lifted and
/// swung forward), and never sweeps far round its hip. A foot nearer its hip than the
/// shin's length less the thigh's folds the leg and throws the shard down flat.
#[test]
fn every_step_is_in_reach() {
    let reach = legs::STANCE * legs::STRIDE;
    for &(hip, knee, foot, _) in &legs::CRAWL {
        let (thigh, shin) = ((knee - hip).length(), (foot - knee).length());
        let lifted = hip + Vec3::Z * legs::SINK;
        let rest = hip.with_z(0.0) + (foot - hip.with_z(0.0)) * (1.0 - legs::WIDEN);
        let mut sweep = (f32::MAX, f32::MIN);
        for i in 0..100 {
            let phase = i as f32 / 100.0;
            let step = if phase < legs::STANCE {
                Vec3::X * reach * (0.5 - phase / legs::STANCE)
            } else {
                let u = (phase - legs::STANCE) / (1.0 - legs::STANCE);
                let advance = reach * (u * u * (3.0 - 2.0 * u) - 0.5)
                    - legs::STRIDE * (1.0 - legs::STANCE) * (2.0 * u * u * u - 3.0 * u * u + u);
                Vec3::new(
                    advance,
                    0.0,
                    legs::LIFT * (u * std::f32::consts::PI).sin().powi(2),
                )
            };
            let at = rest + step;
            let d = (at - lifted).length();
            assert!(
                d > shin - thigh + 0.3 && d < shin + thigh - 0.3,
                "at {phase} of its step a foot is {d} m from its hip; the leg reaches {}..{} m",
                shin - thigh,
                shin + thigh
            );
            let az = (at.y - hip.y).atan2((at.x - hip.x).abs()).to_degrees();
            sweep = (sweep.0.min(az), sweep.1.max(az));
        }
        assert!(
            sweep.1 - sweep.0 < 55.0,
            "a foot sweeps {sweep:?} degrees round its hip"
        );
    }
}
