use glam::Vec3;
use mc_map::city::structure;
use mc_map::PropKind;

use super::MODELS;
use crate::gpu_consts::city as pat;
use crate::{build_model, material, prop_model_key, Model};

fn city_kinds() -> impl Iterator<Item = PropKind> {
    PropKind::ALL.iter().copied().filter(|k| k.is_city())
}

/// The kind a catalogue key (or a design variant of one) is the model of.
fn kind_of(key: &str) -> PropKind {
    let base = key.split('~').next().unwrap_or(key);
    city_kinds()
        .find(|k| structure(*k).is_some_and(|s| s.model == base))
        .unwrap_or_else(|| panic!("{key} is no city kind's model"))
}

fn pattern(v: &crate::MeshVertex) -> u32 {
    v.surface & 0xFF
}

/// Glass the shader can break: windows, shopfronts, curtain walls, glass roofs.
fn glazed(p: u32) -> bool {
    (pat::HOUSE..=pat::ARCHED).contains(&p)
        || p == pat::ROOF_GLASS
        || p == pat::SHOP
        || p == pat::LOBBY
}

#[test]
fn every_city_kind_has_its_model() {
    for kind in city_kinds() {
        let key = prop_model_key(kind.raw());
        assert_eq!(key, structure(kind).unwrap().model);
        assert!(MODELS.iter().any(|d| d.key == key), "{key} is in the kit");
        assert!(build_model(key).is_some(), "{key} builds");
    }
}

/// Each model's solid parts are the plan's: every part reaches its top, nothing
/// spreads more than a few metres past the plan (eaves, canopies, a cornice), and no
/// wall rises past the highest top (only spires, masts and crowns may).
#[test]
fn city_models_keep_to_their_plans() {
    for def in MODELS {
        let kind = kind_of(def.key);
        let s = structure(kind).unwrap();
        let model = build_model(def.key).unwrap();
        let verts = &model.lods[0].vertices;
        if s.plan.is_empty() {
            let high = verts.iter().map(|v| v.pos[2]).fold(f32::MIN, f32::max);
            assert!(high < 3.0, "{}: rubble stands {high} m high", def.key);
            continue;
        }
        let mut lo = glam::Vec2::splat(f32::MAX);
        let mut hi = glam::Vec2::splat(f32::MIN);
        for (i, &(cx, cy, hx, hy)) in s.plan.iter().enumerate() {
            let (c, h) = (
                glam::Vec2::new(cx as f32, cy as f32),
                glam::Vec2::new(hx as f32, hy as f32),
            );
            lo = lo.min(c - h);
            hi = hi.max(c + h);
            let inside = |p: Vec3| {
                let d = (p.truncate() - c).abs();
                d.x <= h.x && d.y <= h.y
            };
            let top = s.tops[i] as f32;
            let reach = verts
                .iter()
                .map(|v| Vec3::from(v.pos))
                .filter(|p| inside(*p))
                .map(|p| p.z)
                .fold(f32::MIN, f32::max);
            assert!(
                reach >= top - 1.0,
                "{} part {i}: reaches {reach} m of its {top} m top",
                def.key
            );
        }
        let highest = s.tops.iter().copied().max().unwrap_or(0) as f32;
        for v in verts {
            let p = Vec3::from(v.pos).truncate();
            assert!(
                p.cmpge(lo - 3.2).all() && p.cmple(hi + 3.2).all(),
                "{}: {p} is off its plan {lo}..{hi}",
                def.key
            );
            let wall = (pat::RENDER..=pat::GUTTED).contains(&pattern(v))
                || pattern(v) >= pat::HOUSE + pat::BLANK;
            if v.material == material::CONCRETE && wall {
                assert!(
                    v.pos[2] <= highest + 1.0,
                    "{}: a wall at {} m over its {highest} m top",
                    def.key,
                    v.pos[2]
                );
            }
        }
    }
}

/// Glass where the city's numbers say there is glazing, none where they say none.
#[test]
fn glazing_follows_the_numbers() {
    for def in MODELS {
        let s = structure(kind_of(def.key)).unwrap();
        let model: Model = build_model(def.key).unwrap();
        let glass = model
            .lods
            .iter()
            .flat_map(|l| &l.vertices)
            .any(|v| v.material == material::CONCRETE && glazed(pattern(v)));
        if s.glazing == 0 {
            assert!(!glass, "{}: glass on a structure with none", def.key);
        } else if s.glazing >= 40 {
            assert!(glass, "{}: no glass on a glazed structure", def.key);
        }
    }
}

/// Every city face is the shader's: city concrete with a city pattern (lamps aside).
#[test]
fn city_faces_carry_city_patterns() {
    for def in MODELS {
        let model = build_model(def.key).unwrap();
        for lod in model.lods.iter().chain(&model.far) {
            for v in &lod.vertices {
                if v.material == material::GLOW_RED {
                    continue;
                }
                assert_eq!(v.material, material::CONCRETE, "{}", def.key);
                let p = pattern(v);
                assert!(
                    (pat::FIRST..=pat::LAST).contains(&p),
                    "{}: pattern {p}",
                    def.key
                );
            }
        }
    }
}

/// The far level, for a structure a few pixels across, is cheaper than the coarse one.
#[test]
fn city_far_levels_are_cheaper() {
    for def in MODELS {
        let model = build_model(def.key).unwrap();
        let far = model.far.as_ref().expect("a far level");
        let coarse = model.lods[2].indices.len() / 3;
        let tris = far.indices.len() / 3;
        assert!(
            tris > 0 && tris <= coarse,
            "{}: far {tris} of coarse {coarse}",
            def.key
        );
    }
}

/// Wall segments meet end to end: the cross-section runs the segment's whole length.
#[test]
fn wall_segments_join_end_to_end() {
    let h = mc_map::city::WALL_SEGMENT_M as f32 * 0.5;
    for key in ["city_wall", "city_wall~casemate", "city_wall~glacis"] {
        let model = build_model(key).unwrap();
        for lod in &model.lods {
            let (lo, hi) = lod.vertices.iter().fold((f32::MAX, f32::MIN), |(a, b), v| {
                (a.min(v.pos[0]), b.max(v.pos[0]))
            });
            assert!(
                (lo + h).abs() < 1e-3 && (hi - h).abs() < 1e-3,
                "{key}: {lo}..{hi}"
            );
        }
    }
}

/// The gate leaves its road clear up to its bridge.
#[test]
fn the_gate_passage_is_clear() {
    let road = mc_map::city::GATE_PASSAGE_M as f32 * 0.5;
    for key in ["city_gate", "city_gate~casemate", "city_gate~glacis"] {
        let model = build_model(key).unwrap();
        for v in &model.lods[0].vertices {
            let p = Vec3::from(v.pos);
            let in_road = p.y.abs() < road - 2.5 && p.x.abs() < 14.0;
            assert!(
                !in_road || p.z > 20.0 || p.z < 0.1,
                "{key}: {p} stands in the road"
            );
        }
    }
}
