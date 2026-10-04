//! The Strider model against the unit file and the library's checks.

use super::*;
use crate::{build_model, Model};

fn model() -> Model {
    build_model("regency_strider").unwrap()
}

#[test]
fn fits_the_librarys_checks() {
    {
        super::super::check_charge(
            "regency_strider",
            RADIUS,
            HEIGHT,
            None,
            &MUZZLES.map(|m| m.to_array()),
            HOLD,
        );
    }
}

/// The unit file builds the model `SCALE` times its authored size: its size, the cannons'
/// trunnion and charges, and the seekers' cells are this file's times `SCALE`.
#[test]
fn the_unit_files_guns_are_the_models() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let blueprints = mc_data::Blueprints::load(&dir).unwrap();
    let bp = blueprints.unit(blueprints.id_of("regency_t4_strider").unwrap());
    let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
    assert_eq!(bp.visual.mesh, "regency_strider");
    assert!((bp.radius.to_f32() - RADIUS * SCALE).abs() < 1e-3);
    assert!((bp.height.to_f32() - HEIGHT * SCALE).abs() < 1e-3);
    assert_eq!(bp.weapons.len(), 3);
    for (w, m) in bp.weapons.iter().zip(MUZZLES) {
        assert!(v(w.muzzle).distance(m * SCALE) < 1e-2, "{}", w.name);
        assert!(v(w.pivot.unwrap()).distance(PIVOT * SCALE) < 1e-2);
    }
    let seekers = &bp.weapons[2];
    assert!(
        seekers.vertical_launch && seekers.mount,
        "the launcher rides the head"
    );
    assert_eq!(seekers.muzzles.len(), CELLS.len());
    for (m, [x, y]) in seekers.muzzles.iter().zip(CELLS) {
        assert!(
            v(*m).distance(v3(x, y, DECK) * SCALE) < 1e-2,
            "cell {x},{y}"
        );
    }
    assert!(bp.turret_at.is_none(), "the head turns about the middle");
    let model = model();
    assert_eq!(Vec3::from(model.turret_pivot), v3(0.0, 0.0, RACE));
    assert_eq!(Vec3::from(model.arm_pivot.unwrap()), PIVOT);
    for lod in &model.lods {
        assert!(lod.vertices.iter().all(|v| v.material != GLOW_VIOLET));
    }
}

/// Three legs: a front pair, and a lone rear leg on the centreline that walks as one.
/// Each reaches the ground where its rest pose stands it, and the head's sweep never
/// meets a knee.
#[test]
fn stands_on_three_legs() {
    {
        let key = "regency_strider";
        let model = build_model(key).unwrap();
        let crawl = model.legs.and_then(|l| l.crawl).expect("a crawler");
        assert_eq!(crawl.pairs, 2);
        assert_eq!(crawl.lone, Some(1));
        assert_eq!(crawl.gpu()[5][3], 1.0, "{key}: the rear leg is marked lone");
        for lod in &model.lods[..2] {
            for pair in 0..2u32 {
                let bones = |limb: u32| {
                    lod.vertices.iter().filter(move |v| {
                        v.part == part::LOCOMOTION
                            && v.rig & rig::LIMB_MASK == limb
                            && (v.rig & rig::PAIR_MASK) >> rig::PAIR_SHIFT == pair
                    })
                };
                for limb in [rig::THIGH, rig::SHIN] {
                    assert!(bones(limb).count() > 0, "{key}: pair {pair} bone {limb}");
                }
                let [_, _, foot] = crawl.joints[pair as usize];
                let foot = Vec2::new(foot[0], foot[1]);
                let low = bones(rig::SHIN).map(|v| v.pos[2]).fold(f32::MAX, f32::min);
                assert!(low < 0.08, "{key}: pair {pair} foot at {low}");
                let reach = bones(rig::SHIN)
                    .map(|v| Vec2::new(v.pos[0], v.pos[1]).dot(foot))
                    .fold(f32::MIN, f32::max)
                    / foot.length();
                assert!(reach >= FOOT_R - 0.1, "{key}: pair {pair} reaches {reach}");
                if pair == 1 {
                    // A lone leg is its own mirror.
                    let ys: Vec<f32> = bones(rig::SHIN).map(|v| v.pos[1]).collect();
                    let (lo, hi) = ys
                        .iter()
                        .fold((f32::MAX, f32::MIN), |(a, b), &y| (a.min(y), b.max(y)));
                    assert!((lo + hi).abs() < 1e-3, "{key}: lone leg off the centreline");
                }
            }
            // The head sweeps round over the knees: everything that turns stays above
            // every leg vertex within its reach.
            let turret_low = lod
                .vertices
                .iter()
                .filter(|v| v.part == part::TURRET)
                .map(|v| v.pos[2])
                .fold(f32::MAX, f32::min);
            let reach = lod
                .vertices
                .iter()
                .filter(|v| v.part == part::TURRET)
                .map(|v| Vec2::new(v.pos[0], v.pos[1]).length())
                .fold(0.0f32, f32::max);
            let leg_high = lod
                .vertices
                .iter()
                .filter(|v| {
                    v.part == part::LOCOMOTION
                        && Vec2::new(v.pos[0], v.pos[1]).length() > HIP_R + 1.3
                        && Vec2::new(v.pos[0], v.pos[1]).length() < reach
                })
                .map(|v| v.pos[2])
                .fold(f32::MIN, f32::max);
            assert!(
                leg_high < turret_low - 0.3,
                "{key}: a leg at {leg_high} m under the head's sweep at {turret_low} m"
            );
        }
    }
}

/// Nothing of the head rises through a seeker cell's mouth: the deck stands clear over
/// the head's ridge, and only the cell's own collar and rim stand on it.
#[test]
fn the_cells_are_clear() {
    let model = model();
    for lod in &model.lods[..2] {
        for [x, y] in CELLS {
            let top = lod
                .vertices
                .iter()
                .filter(|v| v.part == part::TURRET)
                .filter(|v| Vec2::new(v.pos[0] - x, v.pos[1] - y).length() < CELL_R * 0.7)
                .map(|v| v.pos[2])
                .fold(f32::MIN, f32::max);
            assert!(top <= DECK + 0.2, "cell {x},{y}: the head stands to {top}");
        }
    }
}

/// Built at the unit file's size the legs take strides to match: the stride grows with the
/// build, or the long legs patter through tiny steps.
#[test]
fn strides_grow_with_the_build() {
    let authored = model().legs.expect("the strider walks");
    assert_eq!(authored.stride, STRIDE);
    let built = crate::build_model_scaled("regency_strider", RADIUS * SCALE, HEIGHT * SCALE, 4)
        .unwrap()
        .legs
        .unwrap();
    assert!(
        built.stride >= STRIDE * SCALE,
        "stride {} at {SCALE}x",
        built.stride
    );
}
