//! Mine points on the real maps: how many each map has, how many fall near each
//! start, and the ore fields that got none. Needs the baked maps in `maps/`.
//!
//! ```text
//! cargo test --profile gate -p mc-sim --test sim -- zz_mine_points_probe:: --ignored --nocapture
//! ```

use mc_core::FxVec2;
use mc_data::Blueprints;
use mc_sim::placement::SiteMap;
use std::path::Path;

#[test]
#[ignore]
fn probe() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bps = Blueprints::load(&root.join("data")).unwrap();
    let mut maps: Vec<_> = std::fs::read_dir(root.join("maps"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "mcmap"))
        .collect();
    maps.sort();
    for path in maps {
        let map = mc_map::MapFile::open(&path).unwrap();
        let sites = SiteMap::for_map(&map, &bps).unwrap();
        let points = sites.mine_points();
        let near = |s: FxVec2, r: i32| {
            points
                .iter()
                .filter(|p| p.distance(s) <= mc_core::Fx::from_int(r))
                .count()
        };
        let starts: Vec<String> = map
            .start_positions()
            .iter()
            .map(|&s| format!("{}/{}/{}", near(s, 1000), near(s, 2500), near(s, 5000)))
            .collect();
        println!(
            "{:<16} {:>4} fields {:>4} points; per start within 1/2.5/5 km: {}",
            path.file_stem().unwrap().to_string_lossy(),
            map.ore_regions().len(),
            points.len(),
            starts.join(" ")
        );
    }
}
