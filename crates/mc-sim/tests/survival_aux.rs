//! Survival's auxiliary Shapers: the Seedcaster's replication artillery and the
//! Unmaker's warheads. Raised by the ray on their own sites from their own rounds, they
//! level like any Shaper; a Seedcaster's shells land beside the defenders and print a
//! squad there, an Unmaker's silos launch warheads the defenders' anti-nukes can stop.

use crate::survival::{rules, run, world_sites};
use mc_core::{Fx, FxVec2};
use mc_data::survival::{Domain, SiteKind};
use mc_map::Heightfield;
use mc_sim::survival::{
    aux_first_round, launch_gap_ticks, shaper_silos, shell_blast, shell_flight_ticks, shell_squad,
    shells_per_volley, volley_ticks, NodeSite,
};
use mc_sim::tables::{flag, NO_ORDER};
use mc_sim::{Command, PlayerCommand, RenderFrame, SimEvent, SurvivalRules, World};

const ARTILLERY: (i32, i32) = (5300, 3800);
const NUKE: (i32, i32) = (3700, 5300);
const HOME: (i32, i32) = (1000, 1000);

fn site(kind: SiteKind) -> NodeSite {
    let (x, y) = match kind {
        SiteKind::Nuke => NUKE,
        _ => ARTILLERY,
    };
    NodeSite {
        at: FxVec2::from_ints(x, y),
        domain: Domain::Land,
        facing: Some(mc_core::Angle::from_degrees(200)),
        kind,
        room: Fx::from_int(350),
        path: Vec::new(),
    }
}

fn aux_rules() -> SurvivalRules {
    SurvivalRules {
        rounds: 0,
        interval_secs: 20,
        ..rules()
    }
}

/// A match whose only Shaper sites are auxiliary ones of `kinds`, the defender's
/// commander made unkillable.
fn aux_world(kinds: &[SiteKind], threads: usize) -> (World, Vec<SimEvent>) {
    let sites = kinds.iter().map(|k| site(*k)).collect();
    let mut w = world_sites(aux_rules(), 1, threads, sites);
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
    (w, events)
}

/// Runs until `hit` is true of an event, at most `limit` ticks; whether it was.
fn until(
    w: &mut World,
    events: &mut Vec<SimEvent>,
    limit: u32,
    hit: impl Fn(&SimEvent) -> bool,
) -> bool {
    let mut t = 0;
    while t < limit {
        let from = events.len();
        run(w, 10, &[], events);
        t += 10;
        if events[from..].iter().any(&hit) {
            return true;
        }
    }
    false
}

fn frame(w: &World) -> RenderFrame {
    let mut f = RenderFrame::default();
    w.write_render_frame(Some(0), &mut f);
    f
}

#[test]
fn auxiliary_shapers_rise_on_their_own_sites_from_their_rounds() {
    let (mut w, mut events) = aux_world(&[SiteKind::Artillery, SiteKind::Nuke], 1);
    let mut raised = Vec::new();
    let mut t = 0;
    while raised.len() < 2 && t < 20_000 {
        let from = events.len();
        run(&mut w, 10, &[], &mut events);
        t += 10;
        for e in &events[from..] {
            if let SimEvent::NodeRaising {
                kind,
                shaper,
                product,
                ..
            } = e
            {
                let round = w.survival_status().unwrap().round;
                assert!(
                    round >= aux_first_round(*kind),
                    "{kind:?} raised in round {round}"
                );
                let key = &w.blueprints.unit(*shaper).key;
                match kind {
                    SiteKind::Artillery => {
                        assert_eq!(key, "progenitor_seedcaster");
                        assert!(product.is_some(), "a Seedcaster's shells print units");
                    }
                    SiteKind::Nuke => {
                        assert_eq!(key, "progenitor_unmaker");
                        assert!(product.is_none(), "an Unmaker prints nothing");
                    }
                    SiteKind::Printer => panic!("no printer site in this match"),
                }
                raised.push(*kind);
            }
        }
    }
    assert_eq!(raised.len(), 2, "both auxiliary Shapers raised: {raised:?}");
    // Online, each is in the render frame as its kind, with every Shaper's defences.
    run(&mut w, 400, &[], &mut events);
    let f = frame(&w);
    let mut kinds: Vec<u8> = f.shapers.iter().map(|s| s.kind).collect();
    kinds.sort_unstable();
    assert_eq!(kinds, vec![1, 2]);
    for s in &f.shapers {
        let card = w.shaper_card(mc_sim::Handle(s.unit)).unwrap();
        assert!(card.shield.is_some() && card.lance.is_some() && card.anti_nuke.is_some());
        match card.kind {
            SiteKind::Artillery => assert!(card.artillery.is_some() && card.warheads.is_none()),
            SiteKind::Nuke => assert!(card.warheads.is_some() && card.artillery.is_none()),
            SiteKind::Printer => unreachable!(),
        }
    }
}

#[test]
fn a_seedcasters_shells_land_by_the_defenders_and_print_squads_that_attack() {
    let (mut w, mut events) = aux_world(&[SiteKind::Artillery], 1);
    assert!(
        until(&mut w, &mut events, 20_000, |e| matches!(
            e,
            SimEvent::ReplicationShellFired { .. }
        )),
        "a shell was fired"
    );
    let (serial, to, flight, units) = events
        .iter()
        .find_map(|e| match e {
            SimEvent::ReplicationShellFired {
                serial,
                to,
                flight_ticks,
                units,
                ..
            } => Some((*serial, *to, *flight_ticks, *units)),
            _ => None,
        })
        .unwrap();
    assert!((100..=200).contains(&flight), "10-20 s: {flight} ticks");
    // Aimed beside the defenders' commander, their only thing on the map.
    let home = FxVec2::from_ints(HOME.0, HOME.1);
    let acu = w
        .state
        .units
        .row(w.state.players[0].commander)
        .map_or(home, |r| w.state.units.pos[r]);
    assert!(
        to.xy().distance(acu) < Fx::from_int(400),
        "lands by the defenders"
    );
    // Everyone sees it coming: the shell, its mark and its time to land.
    let f = frame(&w);
    let shell = f
        .shells
        .iter()
        .find(|s| s.serial == serial)
        .expect("in the render frame");
    assert!(shell.eta > 0.0 && shell.eta <= shell.flight);
    assert_eq!(shell.units, units);
    let top = shell.at(0.5);
    assert!(
        top[2] > shell.from[2].max(shell.to[2]) + 1000.0,
        "a long high arc"
    );
    let card = w
        .shaper_card(mc_sim::Handle(shell.source))
        .unwrap()
        .artillery
        .unwrap();
    assert!(card.in_flight >= 1 && card.shells_per_volley >= 1 && card.squad.0 >= 3);
    assert!(card.interval_secs > 20.0 && card.blast_radius >= 40.0);

    // It lands: a burst, and its squad printing on the spot in the replication violet.
    assert!(until(&mut w, &mut events, 400, |e| matches!(
        e,
        SimEvent::ReplicationShellLanded { serial: s, .. } if *s == serial
    )));
    let u = &w.state.units;
    let squad: Vec<usize> = u
        .slots
        .iter()
        .filter(|&r| u.owner[r] == 1 && u.pos[r].distance(to.xy()) < Fx::from_int(300))
        .filter(|&r| u.has_flag(r, flag::UNDER_CONSTRUCTION))
        .collect();
    assert_eq!(squad.len(), units as usize, "the whole squad printing");
    let f = frame(&w);
    let violet = f
        .units
        .iter()
        .filter(|i| i.status[1] & mc_sim::mirror::UNIT_REPLICATING != 0)
        .count();
    assert!(violet >= squad.len(), "printed in the replication fill");
    // Quickly finished, it goes at the defenders.
    run(&mut w, 80, &[], &mut events);
    let u = &w.state.units;
    let ready: Vec<usize> = squad
        .iter()
        .copied()
        .filter(|&r| u.slots.is_alive(r) && !u.has_flag(r, flag::UNDER_CONSTRUCTION))
        .collect();
    // The defenders' commander may have shot one down as it printed.
    let alive = squad.iter().filter(|&&r| u.slots.is_alive(r)).count();
    assert!(!ready.is_empty() && ready.len() == alive, "finished");
    assert!(
        ready.iter().all(|&r| u.order_head[r] != NO_ORDER),
        "and sent"
    );
}

#[test]
fn an_unmaker_launches_warheads_that_anti_nukes_can_stop() {
    let (mut w, mut events) = aux_world(&[SiteKind::Nuke], 1);
    // The defenders' anti-nuke by their commander, loaded.
    w.state.players[0].free_build = true;
    let array = w.blueprints.id_of("aster_t3_nuke_defense").unwrap();
    let at = FxVec2::from_ints(HOME.0 + 300, HOME.1 + 300);
    let row = w
        .spawn_unit(array, 0, at, mc_core::Angle::ZERO, true)
        .unwrap();
    let array = w.state.units.id(row);
    w.state.strategic.launchers.entry(array).or_default().stock = 4;
    // The rounds would knock it down long before the Unmaker rises.
    w.state.units.flags[row] |= flag::INVULNERABLE;
    assert!(
        until(&mut w, &mut events, 30_000, |e| matches!(
            e,
            SimEvent::NuclearLaunch { owner: 1, .. }
        )),
        "the Unmaker launched"
    );
    // Its warhead's mark is the defenders' structure, and its blast its silo's level's.
    let f = frame(&w);
    let track = f
        .warhead_tracks
        .iter()
        .find(|t| t.owner == 1)
        .expect("in flight");
    let blast = track.blast.expect("its blast");
    assert!(blast.radius >= Fx::from_int(420));
    assert!(Fx::from_int(0) < blast.damage);
    let mark = track.path.mark.xy();
    assert!(mark.distance(at) <= blast.radius, "aimed at the array");
    let unmaker = f.shapers.iter().find(|s| s.kind == 2).unwrap().unit;
    let card = w
        .shaper_card(mc_sim::Handle(unmaker))
        .unwrap()
        .warheads
        .unwrap();
    assert!(card.silos >= 1 && card.capacity >= 1 && card.in_flight >= 1);
    assert!(card.blast_radius >= 420.0 && card.round_secs > 0.0);
    // The defenders' anti-nuke meets it.
    assert!(
        until(&mut w, &mut events, 2000, |e| matches!(
            e,
            SimEvent::WarheadIntercepted {
                owner: 0,
                killed: true,
                ..
            }
        )),
        "shot down"
    );
    assert!(!events
        .iter()
        .any(|e| matches!(e, SimEvent::NuclearDetonation { owner: 1, .. })));
}

#[test]
fn auxiliary_shapers_grow_with_their_level() {
    // The artillery: more shells, bigger squads with heavies, a quicker beat, a bigger burst.
    assert!(shells_per_volley(1) == 1 && shells_per_volley(12) > shells_per_volley(5));
    let (l1, h1) = shell_squad(1, 3, true);
    let (l12, h12) = shell_squad(12, 3, true);
    assert!(l12 > l1 && h1 == 0 && h12 >= 1);
    assert!(volley_ticks(20) < volley_ticks(10) && volley_ticks(10) < volley_ticks(1));
    assert!(volley_ticks(u16::MAX) >= 200, "never under 20 s");
    let (r1, d1) = shell_blast(1);
    let (r10, d10) = shell_blast(10);
    assert!(r10 > r1 && d10 > d1 && shell_blast(u16::MAX).0 <= Fx::from_int(100));
    assert_eq!(shell_flight_ticks(Fx::ZERO), 100);
    assert_eq!(shell_flight_ticks(Fx::from_int(30_000)), 200);
    // The warheads: more silos, a shorter gap, and each silo grows by its data.
    assert!(shaper_silos(9) > shaper_silos(1));
    assert!(launch_gap_ticks(10) < launch_gap_ticks(1));
    let w = world_sites(aux_rules(), 1, 1, vec![site(SiteKind::Nuke)]);
    let silo = w
        .blueprints
        .unit(w.blueprints.id_of("progenitor_silo").unwrap());
    let (s, g) = (silo.strategic.as_ref().unwrap(), silo.growth.unwrap());
    let blast = s.blast.unwrap();
    assert!(g.warhead_stock.at(Fx::from_int(s.stock as i32), 9) >= Fx::from_int(3));
    assert!(g.warhead_round.at(s.round_time, 10) < s.round_time);
    assert!(g.blast_radius.at(blast.radius, 10) > blast.radius);
    assert!(g.blast_damage.at(blast.damage, 10) > blast.damage);
}

/// With shells in the air and an Unmaker standing, survival hashes the same at any
/// worker count and after a restore.
#[test]
fn auxiliary_shapers_hash_the_same_at_any_worker_count_and_after_a_restore() {
    const TICKS: u32 = 3600;
    let kinds = [SiteKind::Artillery, SiteKind::Nuke];
    let play = |threads: usize| {
        let (mut w, _) = aux_world(&kinds, threads);
        (1..TICKS)
            .map(|_| w.tick(&[]).unwrap())
            .collect::<Vec<u64>>()
    };
    let reference = play(0);
    for threads in [1, 3] {
        let hashes = play(threads);
        let first = reference.iter().zip(&hashes).position(|(a, b)| a != b);
        assert_eq!(
            first, None,
            "desync with {threads} workers at tick {first:?}"
        );
    }
    // A snapshot while a shell is in the air.
    let (mut a, _) = aux_world(&kinds, 2);
    let mut at = 1;
    while frame(&a).shells.is_empty() {
        a.tick(&[]).unwrap();
        at += 1;
        assert!(at < TICKS - 200, "a shell in the air before the end");
    }
    let blob = a.snapshot();
    let (mut b, _) = aux_world(&kinds, 3);
    b.restore(Heightfield::flat(768, 768, Fx::from_int(20)), &blob)
        .unwrap();
    assert_eq!(a.hash(), b.hash());
    let after: Vec<u64> = (at..TICKS).map(|_| b.tick(&[]).unwrap()).collect();
    let tail = &reference[at as usize - 1..];
    let first = tail.iter().zip(&after).position(|(a, b)| a != b);
    assert_eq!(first, None, "diverged {first:?} ticks after the snapshot");
}

/// A Shaper's foot spreads as it builds up: its cells blocked, a unit standing there
/// shoved out, and the ground open again once it dies.
#[test]
fn a_shapers_foot_spreads_as_it_grows_and_opens_when_it_dies() {
    use mc_data::MoveLayer;
    let (mut w, mut events) = aux_world(&[SiteKind::Artillery], 1);
    assert!(until(&mut w, &mut events, 20_000, |e| matches!(
        e,
        SimEvent::NodeOnline { .. }
    )));
    let unit = |w: &World| frame(w).shapers[0].unit;
    let shaper = mc_sim::Handle(unit(&w));
    let row = w.state.units.row(shaper).unwrap();
    let at = w.state.units.pos[row];
    // A tank of the defenders' parked against it, kept alive through its Lance.
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    let parked = at + FxVec2::from_ints(22, 0);
    let t = w
        .spawn_unit(tank, 0, parked, mc_core::Angle::ZERO, true)
        .unwrap();
    w.state.units.flags[t] |= flag::INVULNERABLE;
    let tank_id = w.state.units.id(t);
    let mut foot = 0.0;
    for _ in 0..400 {
        run(&mut w, 10, &[], &mut events);
        foot = frame(&w).shapers[0].foot;
        if foot >= 24.0 {
            break;
        }
    }
    assert!(foot >= 24.0, "the foot spread to {foot} m");
    let card = w.shaper_card(shaper).unwrap();
    assert_eq!(card.foot, foot);
    let inside = at + FxVec2::new(Fx::from_f32(foot - 6.0), Fx::ZERO);
    assert!(
        !w.nav.passable(MoveLayer::Land, 0, inside),
        "its foot is blocked"
    );
    let t = w.state.units.row(tank_id).unwrap();
    assert!(
        w.state.units.pos[t].distance(at) > Fx::from_f32(foot),
        "the tank was shoved out"
    );
    // Down, its foot opens.
    w.state.units.health[row] = Fx::ZERO;
    run(&mut w, 3, &[], &mut events);
    assert!(w.state.units.row(shaper).is_none());
    assert!(w.nav.passable(MoveLayer::Land, 0, inside), "open again");
}
