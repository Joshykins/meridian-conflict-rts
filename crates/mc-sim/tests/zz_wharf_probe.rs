use mc_core::{Angle, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_sim::tables::Controller;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

fn world(name: &str) -> World {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bps = Arc::new(Blueprints::load(&root.join("data")).unwrap());
    let map = mc_map::MapFile::open(root.join(format!("maps/{name}.mcmap"))).unwrap();
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
        spawn_commanders: true,
    };
    World::new(&map, bps, Arc::new(Pool::new(1)), &config).unwrap()
}

fn try_build(map: &str, builder: &str, nth: usize) -> bool {
    let mut w = world(map);
    let acu = w.state.players[0].commander;
    let start = w.state.units.pos[w.state.units.row(acu).unwrap()];
    let who = if builder == "aster_commander" {
        acu
    } else {
        let bp = w.blueprints.id_of(builder).unwrap();
        let r = w
            .spawn_unit(bp, 0, start + FxVec2::from_ints(30, 0), Angle::ZERO, true)
            .unwrap();
        w.state.units.id(r)
    };
    let wharf = w.blueprints.id_of("aster_t1_naval_factory").unwrap();
    let bp = w.blueprints.unit(wharf).clone();
    // Nearest placeable site to the start.
    let mut sites = Vec::new();
    for dy in (-2000..2000).step_by(96) {
        for dx in (-2000..2000).step_by(96) {
            let p = start + FxVec2::from_ints(dx, dy);
            if p.x.round_int() < 60 || p.y.round_int() < 60 {
                continue;
            }
            let p = mc_sim::world::snap_to_build_grid(&bp, p);
            if w.can_place(&bp, p) {
                sites.push(p);
            }
        }
    }
    if sites.is_empty() {
        return true;
    }
    let site = sites[(nth * 7919) % sites.len()];
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::Build {
            units: vec![who],
            blueprint: wharf,
            pos: site,
            heading: Angle::ZERO,
            queue: false,
        },
    }])
    .unwrap();
    let r = w.state.units.row(who).unwrap();
    if false {
        eprintln!(
            "{map} {builder}: site {:?}, start {:?}, order_head after cmd {}",
            site, start, w.state.units.order_head[r]
        );
    }
    for t in 0..20000 {
        w.tick(&[]).unwrap();
        let u = &w.state.units;
        if let Some(r) = u
            .slots
            .iter()
            .find(|&r| u.blueprint[r] == wharf && u.owner[r] == 0)
        {
            if !u.has_flag(r, mc_sim::tables::flag::UNDER_CONSTRUCTION) {
                return true;
            }
        }
    }
    let r = w.state.units.row(who).unwrap();
    eprintln!(
        "{map} {builder} site {:?}: NEVER; at {:?} head {} start {:?}",
        site, w.state.units.pos[r], w.state.units.order_head[r], start
    );
    false
}

#[test]
fn probe() {
    for m in ["twin_shoals"] {
        for b in ["aster_commander", "aster_t1_engineer"] {
            let ok = (0..12).filter(|&n| try_build(m, b, n)).count();
            eprintln!("{m} {b}: {ok}/12 finished");
        }
    }
}
