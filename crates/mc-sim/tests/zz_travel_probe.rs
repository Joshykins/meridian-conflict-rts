//! How long groups take to cross real maps: a unit mix sent with one plain Move
//! (a formation, as the player's right-click) from one start to another. Prints
//! the straight distance, the time taken, the effective speed over the straight
//! line and that speed as a share of the slowest member's top speed.
//! Ships sail from harbour to harbour: the nearest point to each start with open
//! water round it for the group's biggest hull.
//!
//! `TRAVEL=the_axis cargo test --profile gate -p mc-sim --test sim -- zz_travel_probe:: --ignored --nocapture`
//! `TRAVEL` is `map[:from-to,...]` (start indices; every start from 0 when left
//! out); `TRAVEL_MIX=tanks,fleet` picks the mixes (all of them by default);
//! `TRAVEL_TRACE=1` prints the lead and the anchor every 15 s, and every member
//! while the block is not marching formed up.
use mc_core::{Angle, FxVec2};
use mc_data::{Blueprints, MoveLayer};
use mc_jobs::Pool;
use mc_sim::tables::{flag, Controller};
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

const MIXES: &[(&str, &[(&str, u32)])] = &[
    ("tank", &[("aster_t1_tank", 1)]),
    ("tanks", &[("aster_t1_tank", 9)]),
    (
        "army",
        &[
            ("aster_t1_tank", 4),
            ("aster_t2_tank", 2),
            ("aster_t1_artillery", 2),
            ("aster_t1_mobile_aa", 1),
        ],
    ),
    ("mason", &[("aster_t1_engineer", 1)]),
    ("pike", &[("aster_t1_frigate", 1)]),
    ("pikes", &[("aster_t1_frigate", 6)]),
    (
        "fleet",
        &[
            ("aster_t1_frigate", 4),
            ("aster_t2_destroyer", 2),
            ("aster_t3_battleship", 1),
        ],
    ),
    ("leviathan", &[("aster_t3_battleship", 1)]),
];

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

/// Nearest point to `start` a hull of `size` on `layer` stands on, searched in
/// rings outward: for a ship, the harbour.
fn stand(w: &World, layer: MoveLayer, size: u8, start: FxVec2) -> Option<FxVec2> {
    for r in (0..6000).step_by(40) {
        let steps = (r / 20).max(1);
        for k in 0..steps {
            let a = Angle::from_degrees(k * 360 / steps);
            let p = start + FxVec2::from_angle(a) * mc_core::Fx::from_int(r);
            // Room for the whole group round it too.
            if [0, 60, 120].iter().all(|&d| {
                (0..8).all(|j| {
                    let q = p + FxVec2::from_angle(Angle::from_degrees(j * 45))
                        * mc_core::Fx::from_int(d);
                    w.nav.passable(layer, size, q)
                })
            }) {
                return Some(p);
            }
        }
    }
    None
}

fn run(map: &mc_map::MapFile, bps: &Arc<Blueprints>, mix: &[(&str, u32)], from: usize, to: usize) {
    let config = MatchConfig {
        seed: 5,
        players: vec![PlayerSetup {
            name: "you".into(),
            faction: "Aster".into(),
            ai: Default::default(),
            team: 0,
            controller: Controller::Human,
            start: 0,
        }],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let mut w = World::new(map, bps.clone(), Arc::new(Pool::new(1)), &config).unwrap();
    let starts = map.start_positions();
    let motions: Vec<_> = mix
        .iter()
        .map(|(k, _)| bps.unit(bps.id_of(k).unwrap()).motion.unwrap())
        .collect();
    let layer = motions[0].layer;
    let size = motions.iter().map(|m| m.size_class).max().unwrap();
    let slowest = motions
        .iter()
        .map(|m| m.speed.to_f64())
        .fold(f64::MAX, f64::min);
    let (Some(a), Some(b)) = (
        stand(&w, layer, size, starts[from]),
        stand(&w, layer, size, starts[to]),
    ) else {
        println!("  {from}->{to}: no standing room");
        return;
    };
    let mut spawns = Vec::new();
    let mut i = 0;
    for (k, n) in mix {
        for _ in 0..*n {
            spawns.push(cmd(Command::DebugSpawn {
                owner: 0,
                blueprint: bps.id_of(k).unwrap(),
                pos: a + FxVec2::from_ints((i % 3) * 40 - 40, (i / 3) * 40 - 40),
                heading: (b - a).angle(),
                count: 1,
                flags: flag::PASSIVE,
                build: 1000,
            }));
            i += 1;
        }
    }
    let before: Vec<_> = w.state.units.slots.iter().collect();
    w.tick(&spawns).unwrap();
    let ids: Vec<_> = w
        .state
        .units
        .slots
        .iter()
        .filter(|r| !before.contains(r))
        .map(|r| w.state.units.id(r))
        .collect();
    w.tick(&[cmd(Command::Move {
        units: ids.clone(),
        target: b,
        queue: false,
    })])
    .unwrap();
    let straight = a.distance(b).to_f64();
    let mut driven = 0.0;
    let mut ticks = 0u32;
    let limit = 60 * 60 * 10;
    let trace = std::env::var("TRAVEL_TRACE").is_ok();
    while ticks < limit {
        let rows: Vec<_> = ids.iter().filter_map(|id| w.state.units.row(*id)).collect();
        if rows
            .iter()
            .all(|&r| w.state.orders.front(&w.state.units, r).is_none())
        {
            break;
        }
        let r0 = rows[0];
        let p = w.state.units.pos[r0];
        w.tick(&[]).unwrap();
        driven += w.state.units.pos[r0].distance(p).to_f64();
        ticks += 1;
        if trace && ticks.is_multiple_of(150) {
            let g = w.state.formations.values().next();
            println!(
                "    {:4} s: lead {:.0},{:.0} at {:.1} m/s; anchor phase {:?} speed {:.1}",
                ticks / 10,
                p.x.to_f64(),
                p.y.to_f64(),
                w.state.units.speed[r0].to_f64(),
                g.map(|g| g.phase),
                g.map_or(0.0, |g| g.speed.to_f64()),
            );
            if let Some(g) = g.filter(|g| g.phase != 2) {
                for &r in &rows {
                    let Some(o) = w.state.orders.front(&w.state.units, r) else {
                        continue;
                    };
                    let slot = g.anchor + o.offset.rotate(g.heading - o.heading);
                    let pos = w.state.units.pos[r];
                    println!(
                        "      {:22} at {:.0},{:.0} {:.1} m/s, {:.0} m from rank, stuck {}",
                        w.bp(r).key,
                        pos.x.to_f64(),
                        pos.y.to_f64(),
                        w.state.units.speed[r].to_f64(),
                        pos.distance(slot).to_f64(),
                        w.state.units.stuck_ticks[r],
                    );
                }
            }
        }
    }
    let secs = ticks as f64 / 10.0;
    let rows: Vec<_> = ids.iter().filter_map(|id| w.state.units.row(*id)).collect();
    let spread = rows
        .iter()
        .map(|&r| w.state.units.pos[r].distance(b).to_f64())
        .fold(0.0, f64::max);
    println!(
        "  {from}->{to}: straight {:5.0} m, lead drove {:5.0} m, {:4.0} s{} = {:4.1} m/s straight, {:4.1} m/s driven, {:3.0}% of slowest ({slowest:.0}); last member {spread:.0} m off",
        straight,
        driven,
        secs,
        if ticks >= limit { " (TIMED OUT)" } else { "" },
        straight / secs,
        driven / secs,
        100.0 * driven / secs / slowest,
    );
}

#[test]
#[ignore]
fn travel() {
    let spec = std::env::var("TRAVEL").unwrap_or_else(|_| "the_axis".into());
    let (name, pairs) = spec.split_once(':').unwrap_or((&spec, ""));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bps = Arc::new(Blueprints::load(&root.join("data")).unwrap());
    let map = mc_map::MapFile::open(root.join(format!("maps/{name}.mcmap"))).unwrap();
    let n = map.start_positions().len();
    let pairs: Vec<(usize, usize)> = if pairs.is_empty() {
        (1..n).map(|j| (0, j)).collect()
    } else {
        pairs
            .split(',')
            .map(|p| {
                let (a, b) = p.split_once('-').unwrap();
                (a.parse().unwrap(), b.parse().unwrap())
            })
            .collect()
    };
    let wanted = std::env::var("TRAVEL_MIX").ok();
    for (mix_name, mix) in MIXES {
        if wanted
            .as_ref()
            .is_some_and(|w| !w.split(',').any(|m| m == *mix_name))
        {
            continue;
        }
        println!("{name} {mix_name}:");
        for &(a, b) in &pairs {
            run(&map, &bps, mix, a, b);
        }
    }
}
