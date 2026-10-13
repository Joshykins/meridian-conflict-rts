//! Survival's auxiliary Shapers: the Seedcaster's warp-ins, the Unmaker's warheads and
//! the Gatewright's fleets. Raised by the ray on their own sites from their own rounds
//! (an `Auxiliary` site raising whichever kind fewest stand), they level like any Shaper;
//! a Seedcaster's squads warp in beside the defenders, never into their dampening fields;
//! an Unmaker's silos launch warheads the defenders' anti-nukes can stop; a Gatewright
//! builds a fleet of warships and warps it in over the defenders, through any field.

use crate::survival::{rules, run, world_sites};
use mc_core::{Fx, FxVec2};
use mc_data::survival::{Domain, SiteKind};
use mc_map::Heightfield;
use mc_sim::survival::{
    arrival_shock, aux_first_round, fleet_size, launch_gap_ticks, shaper_silos, squad_size,
    squads_per_volley, volley_ticks, warp_in_ticks, FleetStage, Hold, NodeSite, WarpInKind,
    WARP_REACH,
};
use mc_sim::tables::{flag, NO_ORDER};
use mc_sim::{Command, PlayerCommand, RenderFrame, SimEvent, SurvivalRules, World};

const ARTILLERY: (i32, i32) = (5300, 3800);
const NUKE: (i32, i32) = (3700, 5300);
const FLEET: (i32, i32) = (5400, 2400);
const HOME: (i32, i32) = (1000, 1000);
const DAMPER: &str = "aster_t2_warp_damper";

fn site(kind: SiteKind) -> NodeSite {
    let (x, y) = match kind {
        SiteKind::Nuke => NUKE,
        SiteKind::Fleet => FLEET,
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
    aux_world_with(
        aux_rules(),
        kinds.iter().map(|k| site(*k)).collect(),
        threads,
    )
}

fn aux_world_with(
    rules: SurvivalRules,
    sites: Vec<NodeSite>,
    threads: usize,
) -> (World, Vec<SimEvent>) {
    let mut w = world_sites(rules, 1, threads, sites);
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

/// A match where the rounds print at T3 and the next is the eighth, the first a
/// Gatewright may rise in: `sites` its Shaper sites.
fn fleet_world(sites: Vec<NodeSite>, threads: usize) -> (World, Vec<SimEvent>) {
    let rules = SurvivalRules {
        rounds: 9,
        tier_cap: 3,
        interval_secs: 20,
        ..rules()
    };
    let (mut w, events) = aux_world_with(rules, sites, threads);
    let now = w.state.tick;
    let s = w.state.survival.as_mut().unwrap();
    s.round = 7;
    s.next_at = now + 1;
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

/// A warp dampener of the defenders' at `at`, finished, powered and unkillable.
fn damper(w: &mut World, at: FxVec2) -> mc_sim::UnitId {
    w.state.players[0].free_build = true;
    let bp = w.blueprints.id_of(DAMPER).unwrap();
    let row = w.spawn_unit(bp, 0, at, mc_core::Angle::ZERO, true).unwrap();
    w.state.units.flags[row] |= flag::INVULNERABLE;
    w.state.units.id(row)
}

fn damper_radius(w: &World) -> Fx {
    let bp = w.blueprints.id_of(DAMPER).unwrap();
    w.blueprints.unit(bp).warp_damper.unwrap().radius
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
                        assert!(product.is_some(), "a Seedcaster's squads are units");
                    }
                    SiteKind::Nuke => {
                        assert_eq!(key, "progenitor_unmaker");
                        assert!(product.is_none(), "an Unmaker prints nothing");
                    }
                    other => panic!("no {other:?} site in this match"),
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
            other => unreachable!("{other:?}"),
        }
        assert!(card.fleet.is_none());
    }
}

/// The first `WarpInMarked` in `events`: (serial, mark, ticks, units).
fn first_mark(events: &[SimEvent]) -> (u32, mc_core::FxVec3, u32, u16) {
    events
        .iter()
        .find_map(|e| match e {
            SimEvent::WarpInMarked {
                serial,
                at,
                ticks,
                units,
                ..
            } => Some((*serial, *at, *ticks, *units)),
            _ => None,
        })
        .unwrap()
}

#[test]
fn a_seedcasters_squads_warp_in_by_the_defenders_and_attack() {
    let (mut w, mut events) = aux_world(&[SiteKind::Artillery], 1);
    assert!(
        until(&mut w, &mut events, 20_000, |e| matches!(
            e,
            SimEvent::WarpInMarked { .. }
        )),
        "a squad was marked"
    );
    let (serial, at, ticks, units) = first_mark(&events);
    assert!((100..=160).contains(&ticks), "10-16 s of warning: {ticks}");
    // Marked beside the defenders' commander, their only thing on the map.
    let home = FxVec2::from_ints(HOME.0, HOME.1);
    let acu = w
        .state
        .units
        .row(w.state.players[0].commander)
        .map_or(home, |r| w.state.units.pos[r]);
    assert!(
        at.xy().distance(acu) < Fx::from_int(400),
        "by the defenders"
    );
    // Everyone sees it coming: its mark, where each unit comes out and the countdown.
    let f = frame(&w);
    let mark = f
        .warp_ins
        .iter()
        .find(|m| m.serial == serial)
        .expect("in the render frame");
    assert_eq!(mark.kind, WarpInKind::Squad);
    assert!(mark.eta > 0.0 && mark.eta <= ticks as f32 / 10.0);
    assert_eq!(mark.units, units);
    assert_eq!(mark.spots.len(), units as usize);
    assert!(mark.reach > 0.0 && mark.radius >= 40.0);
    let card = w
        .shaper_card(mc_sim::Handle(mark.source))
        .unwrap()
        .artillery
        .unwrap();
    assert!(card.marked >= 1 && card.squads_per_volley >= 1 && card.squad.0 >= 3);
    assert!(card.interval_secs > 20.0 && card.shock_radius >= 40.0);
    assert!(card.warp_in_secs >= 10.0 && card.hold == Hold::None);

    // It comes in: a shock, and its squad out of the warp all but whole.
    assert!(until(&mut w, &mut events, 200, |e| matches!(
        e,
        SimEvent::WarpInArrived { serial: s, .. } if *s == serial
    )));
    let u = &w.state.units;
    let squad: Vec<usize> = u
        .slots
        .iter()
        .filter(|&r| u.owner[r] == 1 && u.pos[r].distance(at.xy()) < Fx::from_int(300))
        .filter(|&r| u.has_flag(r, flag::UNDER_CONSTRUCTION))
        .collect();
    assert_eq!(squad.len(), units as usize, "the whole squad coming out");
    for &r in &squad {
        let bp = w.blueprints.unit(u.blueprint[r]);
        assert!(
            u.build_progress[r] * 2 >= bp.build_time,
            "it comes out all but whole"
        );
    }
    let f = frame(&w);
    let violet = f
        .units
        .iter()
        .filter(|i| i.status[1] & mc_sim::mirror::UNIT_REPLICATING != 0)
        .count();
    assert!(violet >= squad.len(), "in the replication fill");
    assert!(
        f.warp_ins.iter().all(|m| m.serial != serial),
        "its mark gone"
    );
    // Finished a moment later, it goes at the defenders.
    run(&mut w, 20, &[], &mut events);
    let u = &w.state.units;
    let ready: Vec<usize> = squad
        .iter()
        .copied()
        .filter(|&r| u.slots.is_alive(r) && !u.has_flag(r, flag::UNDER_CONSTRUCTION))
        .collect();
    // The defenders' commander may have shot one down as it came out.
    let alive = squad.iter().filter(|&&r| u.slots.is_alive(r)).count();
    assert!(!ready.is_empty() && ready.len() == alive, "finished");
    assert!(
        ready.iter().all(|&r| u.order_head[r] != NO_ORDER),
        "and sent"
    );
}

/// A field over the defenders' commander: every squad is marked clear of it, at its
/// edge, never inside.
#[test]
fn warp_ins_are_marked_clear_of_the_defenders_dampening_fields() {
    let (mut w, mut events) = aux_world(&[SiteKind::Artillery], 1);
    let home = FxVec2::from_ints(HOME.0, HOME.1);
    let field = damper(&mut w, home + FxVec2::from_ints(60, 0));
    let radius = damper_radius(&w);
    let row = w.state.units.row(field).unwrap();
    let centre = w.state.units.pos[row];
    let mut marks = Vec::new();
    for _ in 0..3 {
        let from = events.len();
        assert!(until(&mut w, &mut events, 20_000, |e| matches!(
            e,
            SimEvent::WarpInMarked { .. }
        )));
        let f = frame(&w);
        for e in &events[from..] {
            let SimEvent::WarpInMarked { serial, at, .. } = e else {
                continue;
            };
            let mark = f.warp_ins.iter().find(|m| m.serial == *serial).unwrap();
            let d = at.xy().distance(centre);
            assert!(
                d >= radius + Fx::from_f32(mark.reach),
                "marked {d} m from the dampener, inside its field ({radius} m)"
            );
            for s in &mark.spots {
                let p = FxVec2::new(Fx::from_f32(s[0]), Fx::from_f32(s[1]));
                assert!(p.distance(centre) > radius, "a unit comes out in the field");
            }
            assert!(
                d <= radius + Fx::from_int(WARP_REACH),
                "moved only to the field's edge: {d} m"
            );
            marks.push(*serial);
        }
    }
    // None was cancelled: the field was up before they were marked.
    assert!(!events
        .iter()
        .any(|e| matches!(e, SimEvent::WarpInCancelled { .. })));
    assert!(marks.len() >= 3);
}

/// A field raised over a mark after it was announced cancels it: the squad never comes.
#[test]
fn a_field_raised_over_a_mark_cancels_its_squad() {
    let (mut w, mut events) = aux_world(&[SiteKind::Artillery], 1);
    assert!(until(&mut w, &mut events, 20_000, |e| matches!(
        e,
        SimEvent::WarpInMarked { .. }
    )));
    let (serial, at, _, _) = first_mark(&events);
    let field = damper(&mut w, at.xy() + FxVec2::from_ints(200, 0));
    let from = events.len();
    run(&mut w, 10, &[], &mut events);
    let cancelled = events[from..].iter().find_map(|e| match e {
        SimEvent::WarpInCancelled { serial: s, by, .. } if *s == serial => Some(*by),
        _ => None,
    });
    assert_eq!(cancelled, Some(field), "the field cancelled it");
    assert!(frame(&w).warp_ins.iter().all(|m| m.serial != serial));
    run(&mut w, 200, &[], &mut events);
    assert!(
        !events.iter().any(|e| matches!(
            e,
            SimEvent::WarpInArrived { serial: s, .. } if *s == serial
        )),
        "the squad never came"
    );
}

/// Every target deep in the defenders' fields, too deep to move a mark out of: the
/// volley is held, and the card and the HUD say "Dampened".
#[test]
fn a_volley_is_held_when_every_target_is_dampened() {
    let (mut w, mut events) = aux_world(&[SiteKind::Artillery], 1);
    // Fields over the whole map, overlapping: nowhere within reach of any target (the
    // dampeners are targets too) is clear.
    for x in 0..5 {
        for y in 0..5 {
            damper(&mut w, FxVec2::from_ints(300 + 1400 * x, 300 + 1400 * y));
        }
    }
    assert!(
        until(&mut w, &mut events, 20_000, |e| matches!(
            e,
            SimEvent::NodeOnline { .. }
        )),
        "the Seedcaster rose"
    );
    let unit = frame(&w).shapers[0].unit;
    let mut held = false;
    for _ in 0..200 {
        run(&mut w, 10, &[], &mut events);
        let card = w.shaper_card(mc_sim::Handle(unit)).unwrap();
        if card.artillery.unwrap().hold == Hold::Dampened {
            held = true;
            break;
        }
    }
    assert!(held, "the volley was held as dampened");
    assert!(w.survival_status().unwrap().dampened);
    assert!(!events
        .iter()
        .any(|e| matches!(e, SimEvent::WarpInMarked { .. })));
}

/// A Gatewright prints a fleet of warships beside it, marks the defenders, and warps
/// the fleet in through the real warp: a field over the mark drags, hurts and stuns it.
#[test]
fn a_gatewrights_fleet_builds_warps_in_and_is_dampened_by_a_field() {
    let (mut w, mut events) = fleet_world(vec![site(SiteKind::Fleet)], 1);
    assert!(
        until(&mut w, &mut events, 3000, |e| matches!(
            e,
            SimEvent::NodeOnline {
                kind: SiteKind::Fleet,
                ..
            }
        )),
        "the Gatewright rose"
    );
    let unit = frame(&w).shapers[0].unit;
    assert_eq!(frame(&w).shapers[0].kind, 3);
    let card = w.shaper_card(mc_sim::Handle(unit)).unwrap();
    let row = w.state.units.row(card.unit).unwrap();
    assert_eq!(
        w.blueprints.unit(w.state.units.blueprint[row]).key,
        "progenitor_gatewright"
    );
    let fleet = card.fleet.expect("a fleet card");
    assert!(card.shield.is_some() && card.lance.is_some() && card.anti_nuke.is_some());
    assert!(!fleet.classes.is_empty() && fleet.size == (2, 0));
    for c in &fleet.classes {
        let bp = w.blueprints.unit(*c);
        assert!(
            bp.warp.is_some() && bp.tech <= 3,
            "{} is no warship",
            bp.key
        );
    }
    let size = fleet_size(1).0;
    assert!(
        until(&mut w, &mut events, 3000, |e| matches!(
            e,
            SimEvent::FleetMarked { .. }
        )),
        "the fleet was built and marked"
    );
    let (at, ships) = events
        .iter()
        .find_map(|e| match e {
            SimEvent::FleetMarked { at, ships, .. } => Some((*at, *ships)),
            _ => None,
        })
        .unwrap();
    assert_eq!(ships as usize, size);
    // Over the defenders: their commander is all there is.
    let acu = w
        .state
        .units
        .row(w.state.players[0].commander)
        .map(|r| w.state.units.pos[r])
        .unwrap();
    assert!(
        at.xy().distance(acu) < Fx::from_int(200),
        "over the defenders"
    );
    let f = frame(&w);
    let mark = f
        .warp_ins
        .iter()
        .find(|m| m.kind == WarpInKind::Fleet)
        .expect("its mark for everyone");
    assert_eq!(mark.spots.len(), size);
    assert!(mark.eta > 20.0 && !mark.jumping);
    let card = w.shaper_card(mc_sim::Handle(unit)).unwrap().fleet.unwrap();
    assert_eq!(card.stage, FleetStage::Marked);
    // The ships, finished, hang in front of the Gatewright at their cruise height.
    let u = &w.state.units;
    let fleet: Vec<usize> = u
        .slots
        .iter()
        .filter(|&r| u.owner[r] == 1 && w.blueprints.unit(u.blueprint[r]).warp.is_some())
        .collect();
    assert_eq!(fleet.len(), size);
    let gate = FxVec2::from_ints(FLEET.0, FLEET.1);
    for &r in &fleet {
        assert!(!u.has_flag(r, flag::UNDER_CONSTRUCTION));
        assert!(u.pos[r].distance(gate) < Fx::from_int(1500));
        let ground = w.terrain.height_at(u.pos[r]);
        assert!(u.z[r] > ground + Fx::from_int(100), "aloft");
        assert_eq!(u.order_head[r], NO_ORDER, "held for the jump");
    }
    // The defenders raise a field over the mark before the fleet comes.
    damper(&mut w, at.xy());
    let ids: Vec<mc_sim::UnitId> = fleet.iter().map(|&r| w.state.units.id(r)).collect();
    assert!(
        until(&mut w, &mut events, 2000, |e| matches!(
            e,
            SimEvent::WarpArrived { owner: 1, .. }
        )),
        "the fleet warped in"
    );
    run(&mut w, 40, &[], &mut events);
    let dampened = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                SimEvent::WarpArrived {
                    owner: 1,
                    dampened: true,
                    ..
                }
            )
        })
        .count();
    assert_eq!(dampened, size, "every ship came out dampened");
    for id in &ids {
        let Some(r) = w.state.units.row(*id) else {
            continue;
        };
        assert!(w.state.units.stun[r][0] > 0, "stunned");
        assert!(w.state.units.pos[r].distance(at.xy()) < Fx::from_int(1200));
    }
    // Out of the warp and its stun, it goes at the defenders; the Gatewright rests.
    let mut sent = false;
    for _ in 0..80 {
        run(&mut w, 10, &[], &mut events);
        let u = &w.state.units;
        if ids
            .iter()
            .filter_map(|id| u.row(*id))
            .all(|r| u.order_head[r] != NO_ORDER)
        {
            sent = true;
            break;
        }
    }
    assert!(sent, "the fleet was sent at the defenders");
    let card = w.shaper_card(mc_sim::Handle(unit)).unwrap().fleet.unwrap();
    assert_eq!(card.stage, FleetStage::Resting);
    assert!(card.in_field >= 1);
}

/// Three `Auxiliary` sites in the eighth round: one of each auxiliary kind rises, the
/// kind fewest stand each time.
#[test]
fn auxiliary_sites_raise_the_kind_fewest_of_them_stand() {
    let sites = [(5300, 3800), (3700, 5300), (5400, 2400)]
        .iter()
        .map(|&(x, y)| NodeSite {
            at: FxVec2::from_ints(x, y),
            kind: SiteKind::Auxiliary,
            ..site(SiteKind::Artillery)
        })
        .collect();
    let (mut w, mut events) = fleet_world(sites, 1);
    let mut kinds = Vec::new();
    for _ in 0..30 {
        let from = events.len();
        run(&mut w, 10, &[], &mut events);
        for e in &events[from..] {
            if let SimEvent::NodeRaising { kind, shaper, .. } = e {
                kinds.push((*kind, w.blueprints.unit(*shaper).key.clone()));
            }
        }
        if kinds.len() == 3 {
            break;
        }
    }
    let names: Vec<&str> = kinds.iter().map(|k| k.1.as_str()).collect();
    assert_eq!(
        names,
        [
            "progenitor_seedcaster",
            "progenitor_unmaker",
            "progenitor_gatewright"
        ],
        "{kinds:?}"
    );
    // An Auxiliary site in an early round raises only what that round allows.
    let one = vec![NodeSite {
        kind: SiteKind::Auxiliary,
        ..site(SiteKind::Artillery)
    }];
    let (mut w, mut events) = aux_world_with(aux_rules(), one, 1);
    assert!(until(&mut w, &mut events, 20_000, |e| matches!(
        e,
        SimEvent::NodeRaising { .. }
    )));
    let kind = events.iter().find_map(|e| match e {
        SimEvent::NodeRaising { kind, .. } => Some(*kind),
        _ => None,
    });
    assert_eq!(kind, Some(SiteKind::Artillery));
    assert_eq!(aux_first_round(SiteKind::Auxiliary), 3);
}

#[test]
fn auxiliary_shapers_grow_with_their_level() {
    // The warp-ins: more squads, bigger with heavies, a quicker beat, a shorter but never
    // short warning, a bigger shock.
    assert!(squads_per_volley(1) == 1 && squads_per_volley(12) > squads_per_volley(5));
    let (l1, h1) = squad_size(1, 3, true);
    let (l12, h12) = squad_size(12, 3, true);
    assert!(l12 > l1 && h1 == 0 && h12 >= 1);
    assert!(volley_ticks(20) < volley_ticks(10) && volley_ticks(10) < volley_ticks(1));
    assert!(volley_ticks(u16::MAX) >= 200, "never under 20 s");
    assert!(warp_in_ticks(1) > warp_in_ticks(5) && warp_in_ticks(u16::MAX) == 100);
    let (r1, d1) = arrival_shock(1);
    let (r10, d10) = arrival_shock(10);
    assert!(r10 > r1 && d10 > d1 && arrival_shock(u16::MAX).0 <= Fx::from_int(100));
    // The fleets: more ships, capital ships from level 10.
    assert!(fleet_size(12).0 > fleet_size(1).0 && fleet_size(9).1 == 0 && fleet_size(10).1 == 1);
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

/// With squads marked, an Unmaker standing and a fleet in warp, survival hashes the same
/// at any worker count and after a restore.
#[test]
fn auxiliary_shapers_hash_the_same_at_any_worker_count_and_after_a_restore() {
    const TICKS: u32 = 2600;
    let sites = || {
        vec![
            site(SiteKind::Artillery),
            site(SiteKind::Nuke),
            site(SiteKind::Fleet),
        ]
    };
    let play = |threads: usize| {
        let (mut w, _) = fleet_world(sites(), threads);
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
    // A snapshot while the fleet is in its jump.
    let (mut a, _) = fleet_world(sites(), 2);
    let mut at = 1;
    while !frame(&a)
        .warp_ins
        .iter()
        .any(|m| m.kind == WarpInKind::Fleet && m.jumping)
    {
        a.tick(&[]).unwrap();
        at += 1;
        assert!(at < TICKS - 100, "a fleet in its jump before the end");
    }
    // A tick on, so the jump is under way.
    a.tick(&[]).unwrap();
    at += 1;
    let blob = a.snapshot();
    let (mut b, _) = fleet_world(sites(), 3);
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
