//! Survival: the facility prints its rounds in the print bays built into it,
//! sends them down their fronts, raises Shapers with its ray (more as the
//! rounds climb), hands the defenders nothing, and the defenders win once the
//! last round is dead.

use mc_core::{Fx, FxVec2};
use mc_data::survival::Domain;
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::survival::{Bay, Front, Guard, NodeSite, Phase};
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{
    Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, SurvivalConfig, SurvivalRules,
    World,
};
use std::path::Path;
use std::sync::Arc;

const ENGINE: (i32, i32) = (4800, 4800);
const HOME: (i32, i32) = (1000, 1000);
/// The print bays: a row across the facility's face, looking south-west.
const BAYS: [(i32, i32); 4] = [(4300, 4700), (4400, 4600), (4500, 4500), (4600, 4400)];

fn world(rules: SurvivalRules) -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(768, 768, Fx::from_int(20));
    let map = MapData {
        name: "survival".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![
            FxVec2::from_ints(HOME.0, HOME.1),
            FxVec2::from_ints(ENGINE.0, ENGINE.1),
        ],
        props: Vec::new(),
    };
    let player = |name: &str, team: u8, controller| PlayerSetup {
        name: name.into(),
        faction: "Aster".into(),
        ai: Default::default(),
        team,
        controller,
        start: team,
    };
    let config = MatchConfig {
        seed: 11,
        players: vec![
            player("you", 0, Controller::Human),
            player("Replication Engine", 1, Controller::Ai),
        ],
        cheats: true,
        fog: true,
        spawn_commanders: true,
    };
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    w.begin_survival(SurvivalConfig {
        engine_player: 1,
        engine: FxVec2::from_ints(ENGINE.0, ENGINE.1),
        ray_height: Fx::from_int(700),
        bays: BAYS
            .iter()
            .map(|&(x, y)| Bay {
                at: FxVec2::from_ints(x, y),
                heading: mc_core::Angle::from_degrees(225),
                emitter: mc_core::FxVec3::new(
                    Fx::from_int(x + 50),
                    Fx::from_int(y + 50),
                    Fx::from_int(110),
                ),
                domain: Domain::Land,
                max_radius: Fx::from_int(45),
            })
            .collect(),
        guards: vec![
            Guard {
                key: "aster_t2_point_defense".into(),
                at: FxVec2::from_ints(4200, 4200),
                heading: mc_core::Angle::from_degrees(225),
            },
            Guard {
                key: "aster_t2_aa".into(),
                at: FxVec2::from_ints(4700, 4700),
                heading: mc_core::Angle::from_degrees(225),
            },
        ],
        fronts: vec![
            Front {
                domain: Domain::Land,
                path: vec![FxVec2::from_ints(4300, 4300), FxVec2::from_ints(2600, 2600)],
            },
            Front {
                domain: Domain::Air,
                path: vec![FxVec2::from_ints(4300, 4700), FxVec2::from_ints(2400, 2000)],
            },
        ],
        node_sites: vec![NodeSite {
            at: FxVec2::from_ints(3000, 3300),
            domain: Domain::Land,
            facing: Some(mc_core::Angle::from_degrees(225)),
        }],
        rules,
    })
    .unwrap();
    w
}

fn rules() -> SurvivalRules {
    SurvivalRules {
        rounds: 2,
        grace_secs: 10,
        interval_secs: 40,
        intensity: 1000,
        fronts: Domain::Land.bit() | Domain::Air.bit(),
        tier_cap: 1,
        nodes: 3,
    }
}

fn run(w: &mut World, ticks: u32, commands: &[PlayerCommand], events: &mut Vec<SimEvent>) {
    for t in 0..ticks {
        w.tick(if t == 0 { commands } else { &[] }).unwrap();
        events.extend(w.events.iter().cloned());
    }
}

#[test]
fn the_facility_prints_rounds_in_its_bays_and_raises_shapers() {
    let mut w = world(rules());
    let side = 1u8;
    // Its guns stand where the map puts them, and it has no commander.
    let guards = w
        .state
        .units
        .slots
        .iter()
        .filter(|&r| w.state.units.owner[r] == side)
        .count();
    assert_eq!(guards, 2, "the facility's guns");
    assert!(w.state.units.row(w.state.players[1].commander).is_none());

    let mut events = Vec::new();
    // Grace: nothing printed yet.
    run(&mut w, 95, &[], &mut events);
    assert!(w.survival_status().unwrap().phase == Phase::Grace);
    // Round one: units stand in the bays, being printed.
    run(&mut w, 20, &[], &mut events);
    assert!(events
        .iter()
        .any(|e| matches!(e, SimEvent::RoundPrinting { round: 1 })));
    let u = &w.state.units;
    let printing: Vec<usize> = u
        .slots
        .iter()
        .filter(|&r| u.owner[r] == side && u.has_flag(r, flag::UNDER_CONSTRUCTION))
        .filter(|&r| !w.blueprints.unit(u.blueprint[r]).has(mc_data::cat::REPLICATOR))
        .collect();
    assert!(!printing.is_empty(), "units are being printed");
    for &r in &printing {
        let d = BAYS
            .iter()
            .map(|&(x, y)| u.pos[r].distance(FxVec2::from_ints(x, y)).to_f32())
            .fold(f32::MAX, f32::min);
        assert!(d < 30.0, "printed in a bay ({d} m off)");
    }
    assert!(!w.survival_printing().is_empty());
    // The first round wakes the facility: its ray starts raising a Shaper at once.
    assert!(events
        .iter()
        .any(|e| matches!(e, SimEvent::NodeRaising { site: 0, .. })));
    assert!(w.survival_status().unwrap().activity > 0.1);

    // The round goes out together.
    run(&mut w, 600, &[], &mut events);
    let launched = events.iter().find_map(|e| match e {
        SimEvent::RoundLaunched { round: 1, units } => Some(*units),
        _ => None,
    });
    assert!(
        launched.is_some_and(|n| n > 0),
        "round one launched: {launched:?}"
    );

    // Nothing is handed out: the defender takes in only what its own commander makes.
    assert!(
        w.state.players[0].mass_income < Fx::from_int(5),
        "{:?}",
        w.state.players[0].mass_income
    );

    // Round two raises another; they come online printing one kind each.
    run(&mut w, 700, &[], &mut events);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SimEvent::NodeOnline { site: 0, .. })),
        "the node came online"
    );
    let status = w.survival_status().unwrap();
    assert_eq!(status.nodes.len(), 2);
    assert_eq!(status.round, 2);
}

#[test]
fn killing_the_last_round_wins_and_a_node_leaves_a_rich_wreck() {
    let mut w = world(rules());
    let mut events = Vec::new();
    // Keep the commander alive whatever comes.
    let acu = w.state.players[0].commander;
    let guard = PlayerCommand {
        player: 0,
        command: Command::DebugSetFlags {
            units: vec![acu],
            set: flag::INVULNERABLE,
            clear: 0,
        },
    };
    run(&mut w, 1, &[guard], &mut events);
    // Through both rounds.
    let mut t = 0;
    let settled = |w: &World| {
        let s = w.survival_status().unwrap();
        s.phase == Phase::Final && !s.nodes.is_empty() && s.nodes.iter().all(|n| n.raised >= 1.0)
    };
    while !settled(&w) && t < 3000 {
        run(&mut w, 10, &[], &mut events);
        t += 10;
    }
    assert_eq!(w.survival_status().unwrap().phase, Phase::Final);
    assert!(w.state.winner.is_none());
    // Wipe out everything the engine side fielded, node included.
    let u = &w.state.units;
    let doomed: Vec<_> = u
        .slots
        .iter()
        .filter(|&r| u.owner[r] == 1)
        .filter(|&r| {
            let bp = w.blueprints.unit(u.blueprint[r]);
            bp.is_mobile() || bp.has(mc_data::cat::REPLICATOR)
        })
        .map(|r| u.id(r))
        .collect();
    let kill = PlayerCommand {
        player: 0,
        command: Command::DebugDamage {
            units: doomed,
            permille: 1000,
        },
    };
    run(&mut w, 30, &[kill], &mut events);
    assert!(
        events.iter().any(
            |e| matches!(e, SimEvent::NodeDestroyed { wreck, raised: true, .. } if *wreck > 1000)
        ),
        "the node fell and left a rich wreck"
    );
    let node_bp = w.blueprints.id_of("replication_node").unwrap();
    let wrecks = &w.state.wrecks;
    assert!(
        wrecks.slots.iter().any(|r| wrecks.blueprint[r] == node_bp),
        "its wreck lies there to reclaim"
    );
    assert!(events
        .iter()
        .any(|e| matches!(e, SimEvent::SurvivalWon { rounds: 2 })));
    assert_eq!(w.state.winner, Some(0));
}

#[test]
fn rounds_climb_the_tiers_and_grow() {
    let r = SurvivalRules {
        rounds: 15,
        tier_cap: 3,
        ..SurvivalRules::default()
    };
    assert_eq!(r.tier_at(1), 1);
    assert_eq!(r.tier_at(6), 2);
    assert_eq!(r.tier_at(11), 3);
    assert_eq!(r.tier_at(15), 3);
    assert!(r.budget(10) > 5 * r.budget(1));
    let endless = SurvivalRules {
        rounds: 0,
        tier_cap: 5,
        ..r
    };
    assert_eq!(endless.tier_at(6), 1);
    assert_eq!(endless.tier_at(7), 2);
    assert!(endless.tier_at(200) == 5);
}

#[test]
fn heavies_come_one_at_a_time_and_rarely() {
    let r = SurvivalRules {
        rounds: 30,
        tier_cap: 5,
        ..SurvivalRules::default()
    };
    let heavies: Vec<(u16, u8)> = (1..=30).filter_map(|k| r.heavy_at(k).map(|t| (k, t))).collect();
    // Only once the rounds reach T4, never two rounds running, T5 no more than T4.
    assert!(heavies.iter().all(|(k, _)| r.tier_at(*k) >= 4), "{heavies:?}");
    assert!(heavies.windows(2).all(|w| w[1].0 - w[0].0 >= 3), "{heavies:?}");
    let t5 = heavies.iter().filter(|h| h.1 == 5).count();
    assert!(t5 >= 1 && t5 <= heavies.len() - t5, "{heavies:?}");
    assert!(heavies.iter().all(|(k, t)| *t <= r.tier_at(*k)), "{heavies:?}");
    // A T3 ceiling never sends one; a harder engine sends them more often.
    let t3 = SurvivalRules { tier_cap: 3, ..r };
    assert!((1..=30).all(|k| t3.heavy_at(k).is_none()));
    let hard = SurvivalRules { intensity: 2200, ..r };
    assert!((1..=30).filter(|k| hard.heavy_at(*k).is_some()).count() > heavies.len());
}

#[test]
fn a_t4_and_then_a_t5_come_in_their_rounds() {
    // Ten rounds to T5: T4 from round 7 (a heavy), T5 from round 9, the next heavy (round 10) a T5.
    let rules = SurvivalRules {
        rounds: 10,
        grace_secs: 10,
        interval_secs: 20,
        tier_cap: 5,
        nodes: 0,
        ..rules()
    };
    assert_eq!((rules.heavy_at(7), rules.heavy_at(8), rules.heavy_at(10)), (Some(4), None, Some(5)));
    let mut w = world(rules);
    let mut events = Vec::new();
    let acu = w.state.players[0].commander;
    let guard = PlayerCommand {
        player: 0,
        command: Command::DebugSetFlags { units: vec![acu], set: flag::INVULNERABLE, clear: 0 },
    };
    run(&mut w, 1, &[guard], &mut events);
    // Units per tech (T4, T5) of the round being printed, as each round starts.
    let mut heavies = std::collections::BTreeMap::new();
    let mut t = 0;
    while heavies.len() < 10 && t < 20_000 {
        run(&mut w, 1, &[], &mut events);
        t += 1;
        let s = w.survival_status().unwrap();
        if s.phase == Phase::Printing && !heavies.contains_key(&s.round) {
            let tech = |k: usize| s.incoming.iter().map(|d| d[k]).sum::<u16>();
            heavies.insert(s.round, (tech(3), tech(4)));
        }
    }
    for round in 1..=10u16 {
        let want = match round {
            7 => (1, 0),
            10 => (0, 1),
            _ => (0, 0),
        };
        assert_eq!(heavies.get(&round), Some(&want), "round {round}: {heavies:?}");
    }
}

#[test]
fn the_facility_wakes_as_the_rounds_climb() {
    let raised = |nodes: u8| {
        let r = SurvivalRules { nodes, ..SurvivalRules::default() };
        (1..=15).map(|round| r.nodes_raised(round)).collect::<Vec<_>>()
    };
    assert!(raised(0).iter().all(|n| *n == 0));
    for nodes in 1..=3 {
        let per = raised(nodes);
        // Never fewer as the rounds climb (rare raises one every third round).
        if nodes > 1 {
            assert!(per.windows(2).all(|w| w[1] >= w[0]), "{nodes}: {per:?}");
            assert!(per[14] > per[1], "{nodes}: {per:?}");
        }
        assert!(per.iter().sum::<usize>() >= 4, "{nodes}: {per:?}");
    }
}

#[test]
fn one_site_holds_a_row_of_shapers_printing_in_batches() {
    let mut w = world(SurvivalRules {
        rounds: 8,
        ..rules()
    });
    let mut events = Vec::new();
    let acu = w.state.players[0].commander;
    let guard = PlayerCommand {
        player: 0,
        command: Command::DebugSetFlags {
            units: vec![acu],
            set: flag::INVULNERABLE,
            clear: 0,
        },
    };
    run(&mut w, 1, &[guard], &mut events);
    let mut t = 0;
    while t < 4000
        && w.survival_status().unwrap().nodes.iter().filter(|n| n.raised >= 1.0).count() < 3
    {
        run(&mut w, 10, &[], &mut events);
        t += 10;
    }
    let status = w.survival_status().unwrap();
    // The map has one site: every Shaper stands on it, side by side, lots apart.
    assert!(status.nodes.len() >= 3, "{} shapers", status.nodes.len());
    assert!(status.nodes.iter().all(|n| n.site == 0));
    for (i, a) in status.nodes.iter().enumerate() {
        for b in &status.nodes[i + 1..] {
            let d = (a.pos[0] - b.pos[0]).hypot(a.pos[1] - b.pos[1]);
            assert!(d >= 60.0, "shapers {d} m apart");
        }
    }
    // Light units come three at a time.
    run(&mut w, 400, &[], &mut events);
    let status = w.survival_status().unwrap();
    assert!(status.nodes.iter().any(|n| n.printed >= 3));
    assert!(status.nodes.iter().all(|n| n.printed % 3 == 0), "{:?}", status.nodes);
}
