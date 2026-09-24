//! Survival: the engine prints its rounds on its bays, sends them down their
//! fronts, raises nodes with its ray, feeds the defenders, takes no harm, and
//! the defenders win once the last round is dead.

use mc_core::{Fx, FxVec2};
use mc_data::survival::Domain;
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::survival::{Front, NodeSite, Phase};
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
        harbor: None,
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

fn engine_row(w: &World) -> usize {
    let id = w.blueprints.id_of("replication_engine").unwrap();
    let u = &w.state.units;
    u.slots
        .iter()
        .find(|&r| u.blueprint[r] == id)
        .expect("the engine stands")
}

fn run(w: &mut World, ticks: u32, commands: &[PlayerCommand], events: &mut Vec<SimEvent>) {
    for t in 0..ticks {
        w.tick(if t == 0 { commands } else { &[] }).unwrap();
        events.extend(w.events.iter().cloned());
    }
}

#[test]
fn the_engine_prints_rounds_raises_nodes_and_cannot_be_harmed() {
    let mut w = world(rules());
    let engine = engine_row(&w);
    let side = 1u8;
    assert!(w.state.units.has_flag(engine, flag::INVULNERABLE));
    // Its guard stands round it, and it has no commander.
    let guards = w
        .state
        .units
        .slots
        .iter()
        .filter(|&r| w.state.units.owner[r] == side)
        .count();
    assert!(guards > 10, "the engine is defended ({guards} structures)");
    assert!(w.state.units.row(w.state.players[1].commander).is_none());

    let mut events = Vec::new();
    // Grace: nothing printed yet.
    run(&mut w, 95, &[], &mut events);
    assert!(w.survival_status().unwrap().phase == Phase::Grace);
    // Round one: units stand on the bays, being printed.
    run(&mut w, 20, &[], &mut events);
    assert!(events
        .iter()
        .any(|e| matches!(e, SimEvent::RoundPrinting { round: 1 })));
    let u = &w.state.units;
    let printing: Vec<usize> = u
        .slots
        .iter()
        .filter(|&r| u.owner[r] == side && u.has_flag(r, flag::UNDER_CONSTRUCTION))
        .collect();
    assert!(!printing.is_empty(), "units are being printed");
    for &r in &printing {
        let d = u.pos[r].distance(u.pos[engine]).to_f32();
        assert!((120.0..190.0).contains(&d), "printed on a bay ({d} m out)");
    }
    assert!(!w.survival_printing().is_empty());

    // The engine shrugs off anything: heavy guns under its veil fire at it.
    let before = (
        w.state.units.health[engine],
        w.state.units.shield_hp[engine],
    );
    let guns = PlayerCommand {
        player: 0,
        command: Command::DebugSpawn {
            owner: 0,
            blueprint: w.blueprints.id_of("aster_t2_point_defense").unwrap(),
            pos: FxVec2::from_ints(ENGINE.0, ENGINE.1 + 560),
            heading: mc_core::Angle::ZERO,
            count: 1,
            flags: flag::INVULNERABLE,
            build: 1000,
        },
    };
    run(&mut w, 150, &[guns], &mut events);
    // Only the veil is hit: every shot at the engine stops on it.
    let gun = {
        let u = &w.state.units;
        u.slots
            .iter()
            .find(|&r| u.owner[r] == 0 && u.blueprint[r] != u.blueprint[0])
            .map(|r| u.id(r))
            .unwrap()
    };
    let aim = PlayerCommand {
        player: 0,
        command: Command::Attack {
            units: vec![gun],
            target: w.state.units.id(engine),
            queue: false,
        },
    };
    run(&mut w, 150, &[aim], &mut events);
    assert!(
        events.iter().any(|e| matches!(
            e,
            SimEvent::Impact {
                on_shield: true,
                ..
            }
        )),
        "the veil was hit"
    );
    assert_eq!(
        (
            w.state.units.health[engine],
            w.state.units.shield_hp[engine]
        ),
        before
    );

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

    // Round two raises a node with the ray; it comes online printing one kind.
    run(&mut w, 700, &[], &mut events);
    assert!(events
        .iter()
        .any(|e| matches!(e, SimEvent::NodeRaising { site: 0, .. })));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SimEvent::NodeOnline { site: 0, .. })),
        "the node came online"
    );
    let status = w.survival_status().unwrap();
    assert_eq!(status.nodes.len(), 1);
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
    let engine_bp = w.blueprints.id_of("replication_engine").unwrap();
    let doomed: Vec<_> = u
        .slots
        .iter()
        .filter(|&r| u.owner[r] == 1 && u.blueprint[r] != engine_bp)
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
