use super::*;
use crate::models::{build_model_scaled, part, rig, Model};
use glam::Vec2;

fn fitted() -> Model {
    build_model_scaled("naga_scorpion", RADIUS, HEIGHT, 1).unwrap()
}

#[test]
fn stands_on_eight_legs_each_rigged_to_its_pair() {
    let model = fitted();
    let crawl = model.legs.and_then(|l| l.crawl).expect("a crawler");
    assert_eq!(crawl.pairs, 4);
    for lod in &model.lods[..2] {
        for pair in 0..4u32 {
            let bones = |limb: u32| {
                lod.vertices.iter().filter(move |v| {
                    v.part == part::LOCOMOTION
                        && v.rig & rig::LIMB_MASK == limb
                        && (v.rig & rig::PAIR_MASK) >> rig::PAIR_SHIFT == pair
                })
            };
            for limb in [rig::THIGH, rig::SHIN] {
                assert!(
                    bones(limb).any(|v| v.pos[1] > 0.0),
                    "pair {pair} left bone {limb}"
                );
                assert!(
                    bones(limb).any(|v| v.pos[1] < 0.0),
                    "pair {pair} right bone {limb}"
                );
            }
            // The foot's tip reaches the ground its pair's rest pose names.
            let [_, _, foot] = crawl.joints[pair as usize];
            let low = bones(rig::SHIN).map(|v| v.pos[2]).fold(f32::MAX, f32::min);
            assert!(low < 0.3, "pair {pair} foot at {low}");
            let reach = bones(rig::SHIN).map(|v| v.pos[1]).fold(0.0f32, f32::max);
            assert!(
                (reach - foot[1]).abs() < 0.6,
                "pair {pair} reaches y {reach} for {}",
                foot[1]
            );
        }
    }
    // No leg piece is a refit piece: their pair rides the `UPGRADE_AT` bits.
    assert!(model.lods[0]
        .vertices
        .iter()
        .all(|v| v.part != part::LOCOMOTION || v.rig & (rig::UPGRADE | rig::MODULE_MASK) == 0));
}

#[test]
fn tail_segments_turn_about_their_own_joints() {
    let model = fitted();
    let crawl = model.legs.unwrap().crawl.unwrap();
    assert_eq!(crawl.tail_count, TAIL.len());
    for lod in &model.lods[..2] {
        for seg in 0..TAIL.len() - 1 {
            let verts: Vec<Vec3> = lod
                .vertices
                .iter()
                .filter(|v| {
                    v.part == part::TURRET
                        && v.rig & rig::LIMB_MASK == rig::TAIL
                        && ((v.rig & rig::TAIL_SEG_MASK) >> rig::TAIL_SEG_SHIFT) as usize == seg
                })
                .map(|v| Vec3::from(v.pos))
                .collect();
            assert!(!verts.is_empty(), "segment {seg} has no geometry");
            // Every piece of it sits near its own stretch of spine, not another's.
            let (a, c) = (TAIL[seg], TAIL[seg + 1]);
            let reach = TAIL_WIDTH[seg] * 2.2 + 2.0;
            for v in &verts {
                let t = ((*v - a).dot(c - a) / (c - a).length_squared()).clamp(0.0, 1.0);
                assert!(
                    v.distance(a.lerp(c, t)) < reach,
                    "segment {seg} vertex {v} strays"
                );
            }
        }
    }
}

#[test]
fn claws_are_rigged_arm_and_jaw() {
    let model = fitted();
    let crawl = model.legs.unwrap().crawl.unwrap();
    assert_eq!(
        crawl.claw,
        Some([SHOULDER.to_array(), JAW_HINGE.to_array()])
    );
    let lod = &model.lods[0];
    let claw = |seg: u32| {
        lod.vertices.iter().filter(move |v| {
            v.part == part::HULL
                && v.rig & rig::LIMB_MASK == rig::TAIL
                && (v.rig & rig::TAIL_SEG_MASK) >> rig::TAIL_SEG_SHIFT == seg
        })
    };
    assert!(
        claw(rig::CLAW_ARM).any(|v| v.pos[1] > 0.0) && claw(rig::CLAW_ARM).any(|v| v.pos[1] < 0.0)
    );
    assert!(
        claw(rig::CLAW_JAW).any(|v| v.pos[1] > 0.0) && claw(rig::CLAW_JAW).any(|v| v.pos[1] < 0.0)
    );
    // The jaw is only the finger, forward of its hinge.
    let low = claw(rig::CLAW_JAW)
        .map(|v| v.pos[0])
        .fold(f32::MAX, f32::min);
    assert!(low > JAW_HINGE.x - 1.2, "jaw reaches back to {low}");
}

#[test]
fn projector_ends_at_its_muzzle_and_pitches_about_its_joint() {
    let model = fitted();
    let tip = model.lods[0]
        .vertices
        .iter()
        .filter(|v| v.part == part::TURRET && v.rig & rig::LIMB_MASK == rig::ARM_GUN)
        .map(|v| Vec3::from(v.pos))
        .max_by(|a, b| a.x.total_cmp(&b.x))
        .unwrap();
    assert!(tip.distance(BEAM_TIP) < 0.45, "muzzle {tip}");
    assert_eq!(model.arm_pivot, Some(BEAM_PIVOT.to_array()));
    // The sim swings the muzzle about the unit file's `turret_at`.
    assert_eq!(
        (model.turret_pivot[0], model.turret_pivot[1]),
        (AIM_PIVOT, 0.0)
    );
    // The tail bends from its root to the projector.
    let tail = model.legs.unwrap().crawl.unwrap().tail;
    assert!(tail[0] < 7.5 && tail[1] > 17.0, "{tail:?}");
}

#[test]
fn the_bomb_leaves_from_between_the_claws_fingers() {
    let model = fitted();
    let lod = &model.lods[0];
    // The lit emitter in the throat points at `BOMB_AT`, and each claw has one.
    let emitter = |y: f32| {
        lod.vertices
            .iter()
            .filter(|v| {
                v.material == crate::models::material::GLOW_LASER
                    && v.part == part::HULL
                    && v.rig & rig::LIMB_MASK == rig::TAIL
                    && v.pos[1] * y > 0.0
            })
            .map(|v| Vec3::from(v.pos).distance(BOMB_AT * Vec3::new(1.0, y, 1.0)))
            .fold(f32::MAX, f32::min)
    };
    assert!(emitter(1.0) < 1.3 && emitter(-1.0) < 1.3);
}

#[test]
fn the_unit_files_numbers_are_the_models_scaled() {
    let bp = mc_data::Blueprints::load(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
    )
    .unwrap();
    let unit = bp
        .units
        .iter()
        .find(|u| u.visual.mesh == "naga_scorpion")
        .expect("a unit drawn as the scorpion");
    let near = |a: f32, b: f32| (a - b).abs() < 0.02;
    assert!(near(unit.radius.to_f32(), RADIUS * SCALE), "radius");
    assert!(near(unit.height.to_f32(), HEIGHT * SCALE), "height");
    let scaled = |p: Vec3| p * SCALE;
    let v = |p: mc_core::FxVec3| Vec3::from(p.to_f32());
    let beam = &unit.weapons[0];
    assert!(
        v(beam.muzzle).distance(scaled(BEAM_TIP)) < 0.02,
        "beam muzzle"
    );
    assert!(
        v(beam.pivot.unwrap()).distance(scaled(BEAM_PIVOT)) < 0.02,
        "beam pivot"
    );
    let at = unit.turret_at.unwrap();
    assert!(near(at.x.to_f32(), AIM_PIVOT * SCALE) && near(at.y.to_f32(), 0.0));
    // Each claw snaps on its own weapon's shots (`Crawl::throws`): left then right.
    let throws = fitted().legs.unwrap().crawl.unwrap().throws;
    assert_eq!(throws, Some([1, 2]));
    for (w, y) in [(1, 1.0), (2, -1.0)] {
        let bomb = &unit.weapons[w];
        assert!(
            bomb.curve.0 > 0 && bomb.muzzle.y.to_f32() * y > 0.0,
            "bomb {w} side"
        );
        let muzzle = scaled(BOMB_AT * Vec3::new(1.0, y, 1.0));
        assert!(v(bomb.muzzle).distance(muzzle) < 0.02, "bomb {w} muzzle");
        // It turns about its own muzzle: whatever way it throws, it leaves the claw.
        assert!(
            v(bomb.pivot.unwrap()).distance(muzzle) < 0.02,
            "bomb {w} pivot"
        );
    }
}

#[test]
fn the_sims_single_pivot_follows_the_bending_tail() {
    // The shader turns each of the top four joints by its share of the aim; the sim
    // turns the muzzles about `AIM_PIVOT` by the whole aim. Over the unit file's
    // `aim_arc` (60 degrees either side) the two stay within half a metre.
    let turn = |p: Vec2, c: Vec2, a: f32| c + Vec2::from_angle(a).rotate(p - c);
    let joints = &TAIL[TAIL.len() - 4..];
    let mut worst = 0.0f32;
    for deg in (5..=60).step_by(5) {
        let a = (deg as f32).to_radians();
        for tip in [BEAM_TIP] {
            let mut bent = tip.truncate();
            for (j, share) in joints.iter().zip(AIM_SHARE).rev() {
                bent = turn(bent, j.truncate(), a * share);
            }
            let swung = turn(tip.truncate(), Vec2::new(AIM_PIVOT, 0.0), a);
            worst = worst.max(bent.distance(swung));
        }
    }
    assert!(worst < 0.55, "muzzle off the drawn tip by {worst} m");
    assert!((AIM_SHARE.iter().sum::<f32>() - 1.0).abs() < 1e-6);
}

#[test]
fn fits_the_triangle_budgets() {
    let model = fitted();
    let budget = super::super::triangles("naga_scorpion").unwrap();
    let [full, mid, coarse] = [0, 1, 2].map(|i| model.lods[i].indices.len() / 3);
    assert!(
        full <= budget && mid as f32 <= full as f32 * 0.45 + 20.0 && coarse < 60,
        "{full}/{mid}/{coarse}"
    );
    for lod in &model.lods {
        assert!(
            lod.vertices.iter().all(|v| v.pos[2] >= -1e-3),
            "below ground"
        );
    }
}
