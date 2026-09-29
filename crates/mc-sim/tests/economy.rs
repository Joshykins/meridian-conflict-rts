//! The flow economy's stall rule: short of anything, everything slows alike, the
//! mines with it, unless the side's focus puts new power or new mines first.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::focus::{Focus, Priority};
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let map = MapData {
        name: "economy".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1800, 1800)],
        props: Vec::new(),
    };
    let player = |name: &str, team| PlayerSetup {
        name: name.into(),
        faction: "Aster".into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 9,
        players: vec![player("you", 0), player("them", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(
        Heightfield::flat(128, 128, Fx::from_int(20)),
        map,
        blueprints,
        Arc::new(Pool::new(1)),
        &config,
    )
    .unwrap()
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

/// One `key` of player 0's at (x, y), `build` permille done; its id.
fn spawn(w: &mut World, key: &str, x: i32, y: i32, build: u16) -> UnitId {
    let blueprint = w.blueprints.id_of(key).unwrap();
    let at = FxVec2::from_ints(x, y);
    w.tick(&[cmd(Command::DebugSpawn {
        owner: 0,
        blueprint,
        pos: at,
        heading: Angle::ZERO,
        count: 1,
        flags: 0,
        build,
    })])
    .unwrap();
    let u = &w.state.units;
    let row = u
        .slots
        .iter()
        .filter(|&r| u.blueprint[r] == blueprint && u.owner[r] == 0)
        .min_by_key(|&r| u.pos[r].distance_sq(at))
        .unwrap();
    u.id(row)
}

/// A reactor site and a factory site, one engineer on each, on 20 energy a second
/// with none in store and mass to spare, with new mines paid `mines` and new power `power`. How fast each goes up, in
/// build time a second, and the side's player record after.
fn race(mines: Priority, power: Priority) -> (f64, f64, mc_sim::tables::Player) {
    let mut w = world();
    spawn(&mut w, "aster_t1_power", 300, 300, 1000);
    spawn(&mut w, "aster_mass_storage", 330, 300, 1000);
    let reactor = spawn(&mut w, "aster_t1_power", 500, 500, 100);
    let factory = spawn(&mut w, "aster_t1_land_factory", 600, 500, 100);
    let a = spawn(&mut w, "aster_t1_engineer", 520, 470, 1000);
    let b = spawn(&mut w, "aster_t1_engineer", 580, 470, 1000);
    w.tick(&[
        cmd(Command::SetFocus {
            focus: Focus { mines, power },
        }),
        cmd(Command::Assist {
            units: vec![a],
            target: reactor,
            queue: false,
        }),
        cmd(Command::Assist {
            units: vec![b],
            target: factory,
            queue: false,
        }),
    ])
    .unwrap();
    let starve = |w: &mut World| {
        w.state.players[0].energy = Fx::ZERO;
        w.state.players[0].mass = Fx::from_int(1000);
        w.tick(&[]).unwrap();
    };
    for _ in 0..20 {
        starve(&mut w);
    }
    let progress =
        |w: &World, id: UnitId| w.state.units.build_progress[w.state.units.row(id).unwrap()];
    let (r0, f0) = (progress(&w, reactor), progress(&w, factory));
    for _ in 0..50 {
        starve(&mut w);
    }
    (
        (progress(&w, reactor) - r0).to_f64() / 5.0,
        (progress(&w, factory) - f0).to_f64() / 5.0,
        w.state.players[0].clone(),
    )
}

#[test]
fn power_focus_builds_power_at_full_rate_while_the_rest_stalls() {
    // The reactor wants 18 energy a second from the 20 there is: all of it. The factory
    // wants 35 and shares the 2 left over.
    let (reactor_rate, factory_rate, pl) = race(Priority::Even, Priority::First);
    assert!(
        (reactor_rate - 5.0).abs() < 0.1,
        "reactor built at {reactor_rate} a second"
    );
    assert!(
        factory_rate > 0.0 && factory_rate < 0.6,
        "factory built at {factory_rate} a second"
    );
    assert!(pl.efficiency < Fx::ONE);
    // The readout says so: the first tier at full speed, the rest crawling, nothing last.
    assert_eq!(pl.tier_speed[0], Some(Fx::ONE));
    assert!(pl.tier_speed[1].is_some_and(|s| s < Fx::ratio(2, 10)));
    assert_eq!(pl.tier_speed[2], None);
}

#[test]
fn with_no_focus_a_stall_slows_everything_alike_the_mines_too() {
    // 20 energy a second for 18 + 35 asked: everything at 20/53 of full speed.
    let (reactor_rate, _, pl) = race(Priority::Even, Priority::Even);
    let share = 20.0 / 53.0;
    assert!(
        (reactor_rate - 5.0 * share).abs() < 0.15,
        "reactor built at {reactor_rate} a second"
    );
    assert!(
        (pl.efficiency.to_f64() - share).abs() < 0.02,
        "{}",
        pl.efficiency.to_f64()
    );
    // The mines dig only as hard as the side's energy is paid, whoever asked for it.
    assert!(
        (pl.mine_power.to_f64() - share).abs() < 0.02,
        "mines powered {}",
        pl.mine_power.to_f64()
    );
}

#[test]
fn power_last_is_built_only_out_of_what_the_rest_leaves() {
    // The factory wants 35 energy a second of the 20 there is and takes it all: the
    // reactor gets nothing until the rest is paid in full.
    let (reactor_rate, factory_rate, pl) = race(Priority::Even, Priority::Last);
    assert!(
        reactor_rate < 0.01,
        "reactor built at {reactor_rate} a second"
    );
    let (_, even_factory, _) = race(Priority::Even, Priority::Even);
    assert!(
        factory_rate > even_factory * 1.3,
        "{factory_rate} vs {even_factory}"
    );
    assert!(pl.tier_speed[2].is_some_and(|s| s < Fx::ratio(1, 100)));
}

#[test]
fn power_last_builds_at_full_speed_with_energy_to_spare() {
    let mut w = world();
    spawn(&mut w, "aster_t1_power", 300, 300, 1000);
    let reactor = spawn(&mut w, "aster_t1_power", 500, 500, 100);
    let a = spawn(&mut w, "aster_t1_engineer", 520, 470, 1000);
    w.tick(&[
        cmd(Command::SetFocus {
            focus: Focus {
                mines: Priority::Last,
                power: Priority::Last,
            },
        }),
        cmd(Command::Assist {
            units: vec![a],
            target: reactor,
            queue: false,
        }),
    ])
    .unwrap();
    for _ in 0..20 {
        w.state.players[0].energy = Fx::from_int(1000);
        w.state.players[0].mass = Fx::from_int(1000);
        w.tick(&[]).unwrap();
    }
    let pl = &w.state.players[0];
    assert_eq!(pl.tier_speed, [None, None, Some(Fx::ONE)]);
}

#[test]
fn mines_first_leaves_power_sites_with_the_rest() {
    let (power_first, _, _) = race(Priority::Even, Priority::First);
    let (mines_first, _, _) = race(Priority::First, Priority::Even);
    let (neither, _, _) = race(Priority::Even, Priority::Even);
    assert!(mines_first < power_first * 0.5);
    assert!((mines_first - neither).abs() < 0.05);
}

#[test]
fn a_mass_stall_holds_energy_spending_back_and_says_so() {
    // Energy to spare and almost no mass: the factory site asks for its energy at the
    // full rate but gets only the mass-stalled share of it, and the side's record
    // reports what was really spent (the HUD's net), not what was asked for.
    let mut w = world();
    spawn(&mut w, "aster_t1_power", 300, 300, 1000);
    spawn(&mut w, "aster_mass_storage", 330, 300, 1000);
    spawn(&mut w, "aster_energy_storage", 360, 300, 1000);
    let factory = spawn(&mut w, "aster_t1_land_factory", 600, 500, 100);
    let a = spawn(&mut w, "aster_t1_engineer", 580, 470, 1000);
    w.tick(&[cmd(Command::Assist {
        units: vec![a],
        target: factory,
        queue: false,
    })])
    .unwrap();
    let hz = mc_core::TICKS_PER_SECOND as f64;
    // The engineer gets to work first.
    for _ in 0..20 {
        w.state.players[0].mass = Fx::ratio(1, 10);
        w.tick(&[]).unwrap();
    }
    for _ in 0..20 {
        let pl = &mut w.state.players[0];
        pl.mass = Fx::ratio(1, 10);
        pl.energy = pl.energy_capacity / 2;
        let before = pl.energy;
        w.tick(&[]).unwrap();
        let pl = &w.state.players[0];
        assert!(
            pl.efficiency < Fx::ratio(1, 2),
            "{}",
            pl.efficiency.to_f64()
        );
        let (income, demand, spent) = (
            pl.energy_income.to_f64(),
            pl.energy_demand.to_f64(),
            pl.energy_spent.to_f64(),
        );
        assert!(
            demand > income,
            "the old readout's drain: {demand} over {income}"
        );
        assert!(spent < income, "spent {spent} of {income} coming in");
        let rise = (pl.energy - before).to_f64() * hz;
        assert!(
            (rise - (income - spent)).abs() < 0.05,
            "store rose {rise}/s, reported net {}",
            income - spent
        );
        assert!(pl.mass_spent > Fx::ZERO && pl.mass_spent <= pl.mass_demand);
    }
}
