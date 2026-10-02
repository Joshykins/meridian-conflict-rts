use glam::Vec3;

use super::*;
use crate::{build_model, material, part, rig, MeshVertex, Model};

const KEY: &str = "regency_battleship";

fn nearest(model: &Model, lod: usize, at: Vec3, keep: impl Fn(&MeshVertex) -> bool) -> f32 {
    model.lods[lod]
        .vertices
        .iter()
        .filter(|v| keep(v))
        .map(|v| Vec3::from(v.pos).distance(at))
        .fold(f32::MAX, f32::min)
}

fn of_house(model: &Model, weapon: u8) -> impl Fn(&MeshVertex) -> bool {
    let slot = model
        .houses
        .iter()
        .position(|h| h.weapon == weapon)
        .expect("a house for the weapon") as u32;
    move |v: &MeshVertex| {
        let high = v.rig & rig::HOUSE_HIGH != 0;
        let low = v.rig & rig::LIMB_MASK;
        (rig::HOUSE_FIRST..rig::HOUSE_FIRST + 4).contains(&low)
            && (low - rig::HOUSE_FIRST) + if high { 4 } else { 0 } == slot
    }
}

/// The ship floats, fits its blueprint and budget, wears dark plate and the owner's
/// colour, and keeps its houses on the unit file's pivots.
#[test]
fn it_fits() {
    let key = KEY;
    let model = build_model(key).unwrap();
    let houses: Vec<(u8, Vec3)> = model
        .houses
        .iter()
        .map(|h| (h.weapon, Vec3::from(h.pivot)))
        .collect();
    let mut want = vec![(0, MAIN[0]), (1, MAIN[1]), (2, MAIN[2]), (3, FLAK)];
    want.extend(SECONDARY.iter().enumerate().map(|(k, &p)| (4 + k as u8, p)));
    assert_eq!(houses, want, "{key}: houses");
    let tris = |lod: usize| model.lods[lod].indices.len() / 3;
    let (full, mid, coarse) = (tris(0), tris(1), tris(2));
    assert!((2000..=14000).contains(&full), "{key}: {full} triangles");
    assert!(
        mid as f32 <= full as f32 * 0.45 + 20.0,
        "{key}: reduced {mid} of {full}"
    );
    assert!(coarse < 60, "{key}: far {coarse}");
    for (lod, mesh) in model.lods.iter().enumerate() {
        let name = format!("{key} lod{lod}");
        let (lo, hi) = mesh
            .vertices
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), v| {
                (lo.min(v.pos[2]), hi.max(v.pos[2]))
            });
        assert!(
            (HEIGHT * 0.8..=HEIGHT * 1.25).contains(&hi),
            "{name}: top {hi}"
        );
        assert!((-4.5..-2.0).contains(&lo), "{name}: keel {lo}");
        let reach = mesh
            .vertices
            .iter()
            .map(|v| v.pos[0].hypot(v.pos[1]))
            .fold(0.0, f32::max);
        assert!(
            (RADIUS * 0.75..=RADIUS * 1.15).contains(&reach),
            "{name}: reach {reach}"
        );
        assert!(
            mesh.vertices
                .iter()
                .any(|v| v.material == material::TEAM && v.normal[2] > 0.5),
            "{name}: no upward team colour"
        );
        assert!(mesh
            .vertices
            .iter()
            .any(|v| v.material == material::PLATING_DARK));
        assert!(
            !mesh.vertices.iter().any(|v| v.material == material::GLOW
                || v.material == material::GLOW_ORANGE
                || v.part == part::LOCOMOTION),
            "{name}: ARC's light or running gear"
        );
        for i in 0..3 {
            assert!(
                mesh.vertices.iter().any(of_house(&model, i as u8)),
                "{name}: no main house {i}"
            );
        }
    }
}

/// Each main house holds both its charges between projectors, clear of them, and reaches
/// no further than its charges; the secondaries', flak's and torpedo doors' muzzles are
/// drawn; each counter-seeker head is red.
#[test]
fn it_holds_its_weapons() {
    let key = KEY;
    let model = build_model(key).unwrap();
    for lod in 0..2 {
        let name = format!("{key} lod{lod}");
        for i in 0..3 {
            let house: Vec<Vec3> = model.lods[lod]
                .vertices
                .iter()
                .filter(|v| of_house(&model, i as u8)(v))
                .map(|v| Vec3::from(v.pos))
                .collect();
            let past = house.iter().map(|p| p.x).fold(f32::MIN, f32::max);
            assert!(
                past <= charge(i).x + 1.0,
                "{name}: house {i} reaches {past}"
            );
            for c in charges(i) {
                let near = house.iter().map(|p| p.distance(c)).fold(f32::MAX, f32::min);
                assert!(near > TWIN * 0.4, "{name}: house {i} {near} m into {c}");
                let round: Vec<Vec3> = house
                    .iter()
                    .map(|p| *p - c)
                    .filter(|d| d.length() < TWIN * 1.2)
                    .map(|d| d.with_x(0.0))
                    .collect();
                assert!(
                    round.iter().any(|a| round.iter().any(|b| a.dot(*b) < 0.0)),
                    "{name}: nothing holds {c} from both sides"
                );
            }
        }
        for (k, &p) in SECONDARY.iter().enumerate() {
            for y in [-SECONDARY_TWIN, SECONDARY_TWIN] {
                let m = p + SECONDARY_REACH + Vec3::Y * y;
                let house = of_house(&model, 4 + k as u8);
                let near = nearest(&model, lod, m, |v| house(v) && v.rig & rig::RECOIL != 0);
                assert!(near < 0.4, "{name}: secondary {k} {near} m from {m}");
            }
        }
        let flak = of_house(&model, 3);
        let near = nearest(&model, lod, FLAK_MUZZLE, flak);
        assert!(near < 0.5, "{name}: flak tubes {near} m from the muzzle");
        for t in TUBES {
            let t = Vec3::from(t);
            let near = nearest(&model, lod, t, |v| v.rig & rig::LIMB_MASK == 0);
            assert!(near < 0.4, "{name}: no torpedo door within {near} m of {t}");
        }
        for at in DEFENCE {
            let red = nearest(&model, lod, at, |v| v.material == material::GLOW_LASER);
            assert!(red < 0.9, "{name}: no red at the counter-seeker {at}");
        }
    }
}

/// The unit file's weapon points are the model's.
#[test]
fn the_unit_files_weapons_are_the_models() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let blueprints = mc_data::Blueprints::load(&dir).unwrap();
    let bp = blueprints.unit(blueprints.id_of("regency_t3_battleship").unwrap());
    assert_eq!(bp.visual.mesh, "regency_battleship");
    let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
    let close = |a: Vec3, b: Vec3| a.distance(b) < 0.02;
    let w = &bp.weapons;
    for i in 0..3 {
        assert!(close(v(w[i].pivot.unwrap()), MAIN[i]), "main {i} pivot");
        assert!(close(v(w[i].muzzle), charge(i)), "main {i} muzzle");
        let m: Vec<Vec3> = w[i].muzzles.iter().map(|&p| v(p)).collect();
        assert_eq!(m.len(), 2);
        assert!(
            m.iter().zip(charges(i)).all(|(a, b)| close(*a, b)),
            "main {i} charges"
        );
    }
    assert!(close(v(w[3].pivot.unwrap()), FLAK) && close(v(w[3].muzzle), FLAK_MUZZLE));
    for (k, &p) in SECONDARY.iter().enumerate() {
        let s = &w[4 + k];
        assert!(close(v(s.pivot.unwrap()), p), "secondary {k} pivot");
        assert!(
            close(v(s.muzzle), p + SECONDARY_REACH),
            "secondary {k} muzzle"
        );
        let ys: Vec<f32> = s.muzzles.iter().map(|&m| v(m).y - p.y).collect();
        assert!(
            ys.len() == 2 && ys.iter().all(|y| (y.abs() - SECONDARY_TWIN).abs() < 0.02),
            "secondary {k} muzzles {ys:?}"
        );
    }
    let tubes: Vec<Vec3> = w[8].muzzles.iter().map(|&p| v(p)).collect();
    assert_eq!(tubes.len(), TUBES.len());
    assert!(tubes
        .iter()
        .zip(TUBES)
        .all(|(a, b)| close(*a, Vec3::from(b))));
    let mounts: Vec<Vec3> = bp.anti_missile_mounts.iter().map(|&m| v(m)).collect();
    assert_eq!(mounts.len(), DEFENCE.len());
    assert!(mounts.iter().zip(DEFENCE).all(|(a, b)| close(*a, b)));
}

/// Prints the ship's triangles per level of detail.
#[test]
#[ignore = "prints numbers: cargo test -p mc-models battleship_numbers -- --ignored --nocapture"]
fn battleship_numbers() {
    let key = KEY;
    let model = build_model(key).unwrap();
    let tris = model.lods.each_ref().map(|l| l.indices.len() / 3);
    println!("{key}: {tris:?}");
    for lod in 0..2 {
        let mesh = &model.lods[lod];
        let mut by = [0usize; 9];
        for t in mesh.indices.chunks(3) {
            let v = mesh.vertices[t[0] as usize];
            let slot = (0..8u8).position(|w| of_house(&model, w)(&v)).unwrap_or(8);
            by[slot] += 1;
        }
        println!("  lod{lod} by house (8 = hull): {by:?}");
    }
}
