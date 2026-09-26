//! The wreckage a map starts with: laid when the match starts, weathered, and
//! on every player's screen from the first frame, fog or not.

use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::wreckage::{self, Symmetry};
use mc_map::{BakeParams, Layout, MapFile};
use mc_sim::mirror::KIND_WRECK;
use mc_sim::tables::Controller;
use mc_sim::{MatchConfig, PlayerSetup, RenderFrame, World};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn blueprints() -> Arc<Blueprints> {
    Arc::new(Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap())
}

#[test]
fn every_wreck_the_maps_lay_is_a_blueprint() {
    let bp = blueprints();
    for key in wreckage::wreck_keys() {
        assert!(bp.id_of(key).is_some(), "no blueprint {key}");
    }
}

fn baked(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("mc_map_wreckage_{name}_{}.mcmap", std::process::id()));
    let mut params = BakeParams::square(name, 4, 9);
    params.players = 2;
    mc_map::bake(&params, &path).unwrap();
    let report = wreckage::stamp(&path, Symmetry::of(Layout::Basin, 2).unwrap(), 9).unwrap();
    assert!(report.wrecks > 0, "{report:?}");
    path
}

fn world(map: &MapFile) -> World {
    let player = |name: &str, team| PlayerSetup {
        name: name.into(),
        faction: "Aster".into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 1,
        players: vec![player("you", 0), player("them", 1)],
        cheats: false,
        fog: true,
        spawn_commanders: true,
    };
    World::new(map, blueprints(), Arc::new(Pool::new(1)), &config).unwrap()
}

#[test]
fn a_map_starts_with_its_wreckage_in_sight() {
    let path = baked("start");
    let map = MapFile::open(&path).unwrap();
    map.verify().unwrap();
    let mut w = world(&map);
    assert_eq!(w.state.wrecks.slots.live(), map.wrecks().len());
    for row in w.state.wrecks.slots.iter() {
        let (mass, full) = (w.state.wrecks.mass[row], w.state.wrecks.mass_max[row]);
        assert!(mass > mc_core::Fx::ZERO && mass <= full, "weathered, never over full");
    }
    w.tick(&[]).unwrap();
    let mut frame = RenderFrame::default();
    w.write_render_frame(Some(0), &mut frame);
    let shown = frame.units.iter().filter(|u| u.owner_flags & KIND_WRECK != 0).count();
    assert_eq!(shown, map.wrecks().len(), "every map wreck shows through unexplored fog");

    // Fair: both players find the same salvage at the same reach from their
    // start. Weighted smoothly by distance: the baker snaps each start to the
    // build grid, so they can sit a few metres off true symmetry.
    let starts = map.start_positions();
    let mass_near = |s: mc_core::FxVec2| -> f32 {
        w.state
            .wrecks
            .slots
            .iter()
            .map(|r| {
                let d = (w.state.wrecks.pos[r] - s).length().to_f32();
                w.state.wrecks.mass[r].to_f32() * (-d / 1000.0).exp()
            })
            .sum()
    };
    let (a, b) = (mass_near(starts[0]), mass_near(starts[1]));
    assert!(a > 0.0 && (a - b).abs() < 0.02 * a, "{a} vs {b}");
    drop(map);
    let _ = std::fs::remove_file(path);
}


/// Salvage on each shipped map: `cargo test -p mc-sim --test map_wreckage -- --ignored --nocapture`.
#[test]
#[ignore]
fn zz_map_salvage_report() {
    let bp = blueprints();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../maps");
    let mut maps: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).collect();
    maps.sort();
    for path in maps.iter().filter(|p| p.extension().is_some_and(|e| e == "mcmap")) {
        let map = MapFile::open(path).unwrap();
        let players = map.start_positions().len().max(1) as f32;
        let mut mass = 0.0;
        for w in map.wrecks() {
            let u = bp.unit(bp.id_of(&w.blueprint).unwrap());
            mass += (u.cost_mass * u.wreck_fraction).to_f32() * w.mass_milli as f32 / 1000.0;
        }
        println!(
            "{:>16}: {:4} wrecks, {:7.0} mass, {:6.0} a player",
            path.file_stem().unwrap().to_string_lossy(),
            map.wrecks().len(),
            mass,
            mass / players
        );
    }
}
