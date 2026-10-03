//! The placement preview's map-only check agrees with the sim's own
//! `can_place` wherever no structure stands: on a land map and on one with
//! sea, for every kind of lot (land, floating, naval). Each test bakes its own
//! small map, so none needs a baked map from `maps/`.

use mc_core::FxVec2;
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::BakeParams;
use mc_sim::placement::SiteMap;
use mc_sim::tables::Controller;
use mc_sim::{MatchConfig, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

/// Bakes `params` to a file of its own and removes it again once it is open.
fn bake(params: &BakeParams) -> mc_map::MapFile {
    let path = std::env::temp_dir().join(format!(
        "mc_site_map_{}_{}.mcmap",
        params.name,
        std::process::id()
    ));
    mc_map::bake(params, &path).unwrap();
    let map = mc_map::MapFile::open(&path).unwrap();
    std::fs::remove_file(&path).ok();
    map
}

/// Returns how many sites of a water-only kind the sim took.
fn agree_on(params: &BakeParams) -> usize {
    let name = &params.name;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bps = Arc::new(Blueprints::load(&root.join("data")).unwrap());
    let map = bake(params);
    let player = |n: &str, team| PlayerSetup {
        name: n.into(),
        faction: "Aster".into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 7,
        players: vec![player("you", 0), player("them", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let w = World::new(&map, bps.clone(), Arc::new(Pool::new(1)), &config).unwrap();
    let sites = SiteMap::new(&mc_map::Heightfield::load(&map).unwrap(), map.props());

    // One blueprint of each kind of lot, in each footprint size.
    let mut kinds = std::collections::BTreeMap::new();
    // Core mines are left out: past the map's ground they go only on its mine
    // points, which the interface takes from `mines::mine_points`.
    for bp in bps
        .units
        .iter()
        .filter(|u| u.is_structure() && u.mine.is_none())
    {
        kinds
            .entry((bp.water_only(), bp.water_build, bp.footprint))
            .or_insert(bp);
    }
    let size = map.info().size_metres();
    let (sx, sy) = (size.x.round_int(), size.y.round_int());
    let (mut yes, mut no, mut wet) = (0, 0, 0);
    for bp in kinds.values() {
        for y in (-20..sy + 20).step_by(37) {
            for x in (-20..sx + 20).step_by(41) {
                let pos = mc_sim::world::snap_to_build_grid(bp, FxVec2::from_ints(x, y));
                let sim = w.can_place(bp, pos);
                let preview = sites.check(bp, pos);
                assert_eq!(
                    sim,
                    preview.is_ok(),
                    "{name}: {} at {pos:?}: preview says {preview:?}",
                    bp.key
                );
                if sim {
                    yes += 1;
                    wet += usize::from(bp.water_only());
                } else {
                    no += 1
                }
            }
        }
    }
    assert!(yes > 0 && no > 0, "{name}: {yes} placeable, {no} not");
    wet
}

#[test]
fn preview_matches_the_sim_on_land() {
    agree_on(&BakeParams::square("land", 1, 16));
}

#[test]
fn preview_matches_the_sim_by_the_sea() {
    let wet = agree_on(&BakeParams::islands("sea", 3, 5));
    assert!(
        wet > 0,
        "no naval lot was placeable, so the sea went unchecked"
    );
}
