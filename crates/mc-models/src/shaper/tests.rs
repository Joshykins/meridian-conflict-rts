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

#[test]
fn every_stage_hangs_a_blade_and_clamp_in_each_halo_slot() {
    use super::super::gpu_consts::halo as h;
    use super::super::part;
    for kind in 0..3u8 {
        for (i, key) in stage_keys(kind).iter().enumerate() {
            let model = build_model(key).unwrap();
            let n = halo_slots(STAGE_LEVELS[i]);
            assert!(n >= interceptors(STAGE_LEVELS[i]), "{key}: {n} slots");
            for lod in &model.lods {
                let words: Vec<u32> = lod
                    .vertices
                    .iter()
                    .map(|v| v.part)
                    .filter(|p| p & part::HALO_MASK == part::HALO)
                    .collect();
                for k in 0..n as u32 {
                    let slot = part::HALO | k << h::SLOT_SHIFT | (n as u32) << h::SLOTS_SHIFT;
                    // The coarse level hangs its blades without clamps.
                    assert!(
                        std::ptr::eq(lod, &model.lods[model.lods.len() - 1])
                            || words.contains(&slot),
                        "{key}: slot {k}'s clamp"
                    );
                    assert!(
                        words.contains(&(slot | h::BLADE)),
                        "{key}: slot {k}'s blade"
                    );
                }
                assert!(
                    words.iter().all(|w| (w >> h::SLOT_SHIFT & 0xFF) < n as u32
                        && (w >> h::SLOTS_SHIFT & 0xFF) == n as u32),
                    "{key}: slot words"
                );
            }
        }
    }
}

#[test]
fn the_sims_print_beams_leave_from_the_pieces_emitters() {
    for (k, sim) in mc_sim::survival::PAD_EMITTERS.iter().enumerate() {
        let model = pad_emitters(k);
        assert_eq!(model.len(), sim.len(), "piece {k}");
        for (m, s) in model.iter().zip(sim.iter()) {
            assert!(
                m.iter().zip(s).all(|(a, b)| (a - b).abs() < 0.05),
                "piece {k}: the model's {m:?} against the sim's {s:?}"
            );
        }
    }
}

#[test]
fn a_gatewrights_print_beams_leave_from_its_gate_ring() {
    for &level in &STAGE_LEVELS {
        let points = gate_emitters(level);
        assert_eq!(points.len(), mc_sim::survival::GATE_EMITTERS);
        let lens = lens_height(3, level);
        for p in points {
            assert!(
                p[2] > lens && p[2] <= height(level) + 0.5,
                "level {level}: {p:?} not in the crown over the Lens at {lens}"
            );
        }
    }
}
