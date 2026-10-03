//! What a lone tier 1 core mine on each ore field of the real maps would work:
//! hectares of land and of ore in its territory, so the mine numbers (`ground`,
//! `per_hectare`, `base`) can be set against real fields. Needs the baked maps.
//!
//! ```text
//! cargo test --profile gate -p mc-sim --test sim -- zz_mine_yield_probe:: --ignored --nocapture
//! ```

use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_sim::tables::Controller;
use mc_sim::{MatchConfig, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

#[test]
#[ignore]
fn probe() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bps = Arc::new(Blueprints::load(&root.join("data")).unwrap());
    let mine = bps.id_of("aster_core_mine").unwrap();
    let mut maps: Vec<_> = std::fs::read_dir(root.join("maps"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "mcmap"))
        .collect();
    maps.sort();
    let mut all = Vec::new();
    for path in maps {
        let map = mc_map::MapFile::open(&path).unwrap();
        let config = MatchConfig {
            seed: 1,
            players: vec![PlayerSetup {
                name: "probe".into(),
                faction: "Aster".into(),
                ai: Default::default(),
                team: 0,
                controller: Controller::Human,
                start: 0,
            }],
            cheats: false,
            fog: false,
            spawn_commanders: false,
        };
        let w = World::new(&map, bps.clone(), Arc::new(Pool::new(1)), &config).unwrap();
        let bp = w.blueprints.unit(mine);
        let mut ore: Vec<f32> = Vec::new();
        let mut ground: Vec<f32> = Vec::new();
        for c in w.ore_centres() {
            let s = w.mine_share_at(bp, c);
            ore.push(s.ore_alone.to_f32());
            ground.push(s.ground_alone.to_f32());
        }
        let mut o = ore.clone();
        o.sort_by(f32::total_cmp);
        let med = |v: &[f32]| v.get(v.len() / 2).copied().unwrap_or(0.0);
        let mut g = ground.clone();
        g.sort_by(f32::total_cmp);
        println!(
            "{:<16} fields {:>4}  ore ha p10/p50/p90 {:.1}/{:.1}/{:.1}  land ha p50 {:.0}",
            path.file_stem().unwrap().to_string_lossy(),
            ore.len(),
            o.get(o.len() / 10).copied().unwrap_or(0.0),
            med(&o),
            o.get(o.len() * 9 / 10).copied().unwrap_or(0.0),
            med(&g),
        );
        all.extend(ore);
    }
    all.sort_by(f32::total_cmp);
    println!(
        "all: ore ha p10/p50/p90 {:.1}/{:.1}/{:.1}",
        all[all.len() / 10],
        all[all.len() / 2],
        all[all.len() * 9 / 10]
    );
}
