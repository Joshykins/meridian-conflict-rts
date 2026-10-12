//! Survival: the facility prints its rounds in the print bays built into it,
//! sends them down their fronts, raises Shapers with its ray (more as the
//! rounds climb), hands the defenders nothing, and the defenders win once the
//! last round is dead.

use mc_core::{Fx, FxVec2};
use mc_data::survival::{Domain, SiteKind};
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
/// A second defender's start, for co-op.
const ALLY: (i32, i32) = (1000, 3200);
/// The print bays: a row across the facility's face, looking south-west.
const BAYS: [(i32, i32); 4] = [(4300, 4700), (4400, 4600), (4500, 4500), (4600, 4400)];

fn world(rules: SurvivalRules) -> World {
    world_with(rules, 1)
}

/// `defenders` people on team 0 (players 0..), the facility's side after them.
fn world_with(rules: SurvivalRules, defenders: u8) -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(768, 768, Fx::from_int(20));
    let map = MapData {
        name: "survival".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: [HOME, ALLY][..defenders as usize]
            .iter()
            .chain([&ENGINE])
            .map(|&(x, y)| FxVec2::from_ints(x, y))
            .collect(),
        props: Vec::new(),
    };
    let player = |name: &str, team: u8, controller, start| PlayerSetup {
        name: name.into(),
        faction: "Aster".into(),
        ai: Default::default(),
        team,
        controller,
        start,
    };
    let mut players: Vec<PlayerSetup> = (0..defenders)
        .map(|i| player("you", 0, Controller::Human, i))
        .collect();
    players.push(player("Replication Engine", 1, Controller::Ai, defenders));
    let config = MatchConfig {
        seed: 11,
        players,
        cheats: true,
        fog: true,
        spawn_commanders: true,
    };
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    w.begin_survival(SurvivalConfig {
        engine_player: defenders,
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
            kind: SiteKind::Printer,
            room: Fx::from_int(450),
            path: vec![FxVec2::from_ints(2600, 2900)],
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
        .filter(|&r| {
            !w.blueprints
                .unit(u.blueprint[r])
                .has(mc_data::cat::REPLICATOR)
        })
        .collect();
    // Whole formations at once: more units printing than there are bays.
    assert!(
        printing.len() > BAYS.len(),
        "{} units printing on {} bays",
        printing.len(),
        BAYS.len()
    );
    for &r in &printing {
        let d = BAYS
            .iter()
            .map(|&(x, y)| u.pos[r].distance(FxVec2::from_ints(x, y)).to_f32())
            .fold(f32::MAX, f32::min);
        assert!(d < 240.0, "printed on a bay's apron or past it ({d} m off)");
    }
    // A formation prints together: every unit on one bay started at once.
    let progress: Vec<Fx> = printing.iter().map(|&r| u.build_progress[r]).collect();
    assert!(progress.iter().any(|p| *p > Fx::ZERO));
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
        w.state.players[0].materials_income < Fx::from_int(5),
        "{:?}",
        w.state.players[0].materials_income
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
fn heavy_waves_grow_without_a_ceiling() {
    let r = SurvivalRules {
        rounds: 30,
        tier_cap: 5,
        ..SurvivalRules::default()
    };
    let waves: Vec<(u16, mc_sim::survival::HeavyWave)> = (1..=30)
        .filter_map(|k| r.heavy_wave(k).map(|w| (k, w)))
        .collect();
    // Only once the rounds reach T4, never above the round's tier, T5 squads only at T5.
    assert!(waves
        .iter()
        .all(|(k, w)| r.tier_at(*k) >= 4 && w.top == r.tier_at(*k)));
    assert!(waves
        .iter()
        .all(|(k, w)| (w.t5_share > 0) == (r.tier_at(*k) >= 5)));
    // They come more often as the rounds climb, and each is worth more than the last.
    let gaps: Vec<u16> = waves.windows(2).map(|w| w[1].0 - w[0].0).collect();
    assert!(gaps.windows(2).all(|g| g[1] <= g[0]), "{gaps:?}");
    assert!(
        waves.windows(2).all(|w| w[1].1.value > w[0].1.value),
        "{waves:?}"
    );
    // A T3 ceiling never sends one; a harder engine sends them more often, more of them.
    let t3 = SurvivalRules { tier_cap: 3, ..r };
    assert!((1..=30).all(|k| t3.heavy_wave(k).is_none()));
    let hard = SurvivalRules {
        intensity: 3200,
        ..r
    };
    assert!((1..=30).filter(|k| hard.heavy_wave(*k).is_some()).count() > waves.len());
    assert!(hard.heavy_wave(waves[0].0).unwrap().floor > waves[0].1.floor);
    // Endless keeps growing: a wave every round late on, ever bigger.
    let endless = SurvivalRules { rounds: 0, ..r };
    assert!((40..60).all(|k| endless.heavy_wave(k).is_some()));
    assert!(endless.heavy_wave(50).unwrap().value > 8 * endless.heavy_wave(40).unwrap().value);
}

#[test]
fn a_heavy_wave_comes_in_squads_of_one_kind() {
    // Ten rounds to T5: T4 from round 7 (a wave), T5 from round 9, the next wave (round 10) with T5s open.
    let rules = SurvivalRules {
        rounds: 10,
        grace_secs: 10,
        interval_secs: 20,
        tier_cap: 5,
        nodes: 0,
        ..rules()
    };
    assert!(rules.heavy_wave(7).is_some_and(|w| w.top == 4));
    assert!(rules.heavy_wave(8).is_none());
    assert!(rules
        .heavy_wave(10)
        .is_some_and(|w| w.top == 5 && w.t5_share > 0));
    let mut w = world(rules);
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
    // Heavies (T4 + T5) of the round being printed, as each round starts.
    let mut heavies = std::collections::BTreeMap::new();
    let mut t = 0;
    while heavies.len() < 10 && t < 30_000 {
        run(&mut w, 1, &[], &mut events);
        t += 1;
        let s = w.survival_status().unwrap();
        if s.phase == Phase::Printing && !heavies.contains_key(&s.round) {
            let tech = |k: usize| s.incoming.iter().map(|d| d[k]).sum::<u16>();
            heavies.insert(s.round, (tech(3) + tech(4), s.heavy.clone()));
        }
    }
    for round in 1..=10u16 {
        let (count, squads) = heavies.get(&round).cloned().unwrap_or_default();
        let wave = rules.heavy_wave(round);
        assert_eq!(count > 0, wave.is_some(), "round {round}: {heavies:?}");
        assert_eq!(
            count,
            squads.iter().map(|q| q.1).sum::<u16>(),
            "round {round}: every heavy is in a squad"
        );
        if let Some(wave) = wave {
            // At least the floor of heavies (a T5 squad counts for two).
            let worth: u16 = squads
                .iter()
                .map(|(b, n)| {
                    n * if w.blueprints.unit(*b).tech >= 5 {
                        2
                    } else {
                        1
                    }
                })
                .sum();
            assert!(worth >= wave.floor, "round {round}: {squads:?}");
            assert!(squads
                .iter()
                .all(|(b, _)| w.blueprints.unit(*b).tech <= wave.top));
        }
    }
    assert!(events
        .iter()
        .any(|e| matches!(e, SimEvent::HeavyWave { round: 7, .. })));
    // The tier changes are announced as they come.
    let tiers: Vec<(u16, u8)> = events
        .iter()
        .filter_map(|e| match e {
            SimEvent::TierReached { round, tier } => Some((*round, *tier)),
            _ => None,
        })
        .collect();
    assert_eq!(
        tiers.iter().map(|t| t.1).collect::<Vec<_>>(),
        vec![2, 3, 4, 5]
    );
    assert!(tiers
        .iter()
        .all(|(r, t)| rules.tier_at(*r) == *t && rules.tier_at(r - 1) < *t));
}

#[test]
fn the_engine_never_sends_a_transport() {
    let mut w = world(SurvivalRules {
        rounds: 6,
        tier_cap: 3,
        interval_secs: 20,
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
    let mut seen = 0;
    for _ in 0..300 {
        run(&mut w, 10, &[], &mut events);
        let u = &w.state.units;
        for r in u.slots.iter().filter(|&r| u.owner[r] == 1) {
            let bp = w.blueprints.unit(u.blueprint[r]);
            assert!(bp.transport.is_none(), "the engine sent a {}", bp.key);
            seen += bp.is_mobile() as usize;
        }
    }
    assert!(seen > 0);
}

#[test]
fn the_facility_wakes_as_the_rounds_climb() {
    let raised = |nodes: u8| {
        let r = SurvivalRules {
            nodes,
            ..SurvivalRules::default()
        };
        (1..=15)
            .map(|round| r.nodes_raised(round))
            .collect::<Vec<_>>()
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
        && w.survival_status()
            .unwrap()
            .nodes
            .iter()
            .filter(|n| n.raised >= 1.0)
            .count()
            < 3
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
    // Each prints a formation at a time: at level 1, four T1s.
    run(&mut w, 400, &[], &mut events);
    let status = w.survival_status().unwrap();
    assert!(status.nodes.iter().any(|n| n.printed >= 4));
    assert!(
        status
            .nodes
            .iter()
            .all(|n| n.level > 1 || n.printed % 4 == 0),
        "{:?}",
        status.nodes
    );
}

#[test]
fn shapers_grow_the_longer_they_stand() {
    let mut w = world(SurvivalRules {
        rounds: 12,
        interval_secs: 30,
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
    while w.survival_status().unwrap().round < 9 && t < 20_000 {
        run(&mut w, 10, &[], &mut events);
        t += 10;
    }
    let status = w.survival_status().unwrap();
    let top = status.nodes.iter().map(|n| n.level).max().unwrap_or(0);
    assert!(top >= 3, "{:?}", status.nodes);
    // Every level is announced; at level 3 it takes on another kind of unit.
    let levels: Vec<(u16, bool)> = events
        .iter()
        .filter_map(|e| match e {
            SimEvent::ShaperLevelled { level, added, .. } => Some((*level, added.is_some())),
            _ => None,
        })
        .collect();
    assert!(levels.iter().any(|l| l.0 == 3 && l.1), "{levels:?}");
    let grown = status.nodes.iter().find(|n| n.level >= 3).unwrap();
    assert!(grown.products.len() >= 2, "{grown:?}");
    // A Shaper prints for one domain: all its products are of it.
    for n in &status.nodes {
        for p in &n.products {
            let layer = w.blueprints.unit(*p).motion.unwrap().layer;
            let air = layer == mc_data::MoveLayer::Air;
            assert_eq!(air, n.domain == Domain::Air, "{n:?}");
        }
    }
}

#[test]
fn defenders_hold_out_together_against_one_facility() {
    let mut w = world_with(rules(), 2);
    let side = 2usize;
    // Both defenders deploy with a commander; the facility has none.
    let acus = [w.state.players[0].commander, w.state.players[1].commander];
    assert!(acus.iter().all(|&c| w.state.units.row(c).is_some()));
    assert!(w.state.units.row(w.state.players[side].commander).is_none());
    let mut events = Vec::new();
    let guard = PlayerCommand {
        player: 0,
        command: Command::DebugSetFlags {
            units: acus.to_vec(),
            set: flag::INVULNERABLE,
            clear: 0,
        },
    };
    run(&mut w, 1, &[guard], &mut events);
    let mut t = 0;
    while w.survival_status().unwrap().phase != Phase::Final && t < 3000 {
        run(&mut w, 10, &[], &mut events);
        t += 10;
    }
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SimEvent::RoundLaunched { round: 1, units } if *units > 0)),
        "the facility sends its rounds at two defenders as at one"
    );
    // Wipe out what it fielded: the defenders win together.
    let u = &w.state.units;
    let doomed: Vec<_> = u
        .slots
        .iter()
        .filter(|&r| u.owner[r] as usize == side)
        .filter(|&r| {
            let bp = w.blueprints.unit(u.blueprint[r]);
            bp.is_mobile() || bp.has(mc_data::cat::REPLICATOR)
        })
        .map(|r| u.id(r))
        .collect();
    let kill = PlayerCommand {
        player: 1,
        command: Command::DebugDamage {
            units: doomed,
            permille: 1000,
        },
    };
    run(&mut w, 30, &[kill], &mut events);
    assert!(events
        .iter()
        .any(|e| matches!(e, SimEvent::SurvivalWon { rounds: 2 })));
    assert_eq!(w.state.winner, Some(0));
    assert!(!w.state.players[0].defeated && !w.state.players[1].defeated);
}
