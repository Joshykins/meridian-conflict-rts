//! The Regent's Edict (`edict.rs`): ordered on a point in range, the ship gathers for
//! 15 s with every other gun silent, drawing power (it stalls without), and fires from the
//! point on its hull nearest the mark, wherever the hull has moved to: a blast that kills
//! everything near the mark and falls off to 1.2 km, every side's units alike, then a
//! molten wave burning out over it, weaker toward its rim.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::edict::EdictPhase;
use mc_sim::mirror::{EdictStage, RenderFrame};
use mc_sim::tables::{flag, Controller, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, Refusal, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const REGENT: &str = "regency_t5_super_destroyer";
const SCOURGE: &str = "regency_t4_space_destroyer";
const TANK: &str = "aster_t1_tank";
const DOME: &str = "aster_t3_shield";
/// The Regent's spot, heading +X: its spine runs from x 2900 to x 5450.
const AT: (i32, i32) = (4000, 8000);
/// The mark, 4.55 km off its bow.
const MARK: (i32, i32) = (10000, 8000);
const GATHER: u32 = 150;

fn world() -> World {
    world_with(|_| {})
}

/// A 16 km flat map, the Regency (player 0) against ARC, with the blueprints as `edit`
/// leaves them.
fn world_with(edit: impl FnOnce(&mut Blueprints)) -> World {
    let mut blueprints =
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    edit(&mut blueprints);
    let player = |faction: &str, team| PlayerSetup {
        name: faction.into(),
        faction: faction.into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 5,
        players: vec![player("Regency", 0), player("Aster", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let map = MapData {
        doodads: Default::default(),
        name: "edict".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(500, 500), FxVec2::from_ints(15000, 15000)],
        props: Vec::new(),
    };
    let mut w = World::with_terrain(
        Heightfield::flat(2048, 2048, Fx::from_int(30)),
        map,
        Arc::new(blueprints),
        Arc::new(Pool::new(1)),
        &config,
    )
    .unwrap();
    for p in &mut w.state.players {
        p.free_build = true;
    }
    w
}

fn add(w: &mut World, key: &str, owner: u8, (x, y): (i32, i32)) -> UnitId {
    let id = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(id, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.id(row)
}

/// A unit that never shoots and holds far more than any blast takes, to measure it.
fn dummy(w: &mut World, owner: u8, at: (i32, i32)) -> UnitId {
    let id = add(w, TANK, owner, at);
    let r = row(w, id);
    w.state.units.flags[r] |= flag::PASSIVE;
    w.state.units.health[r] = Fx::from_int(10_000_000);
    id
}

fn row(w: &World, id: UnitId) -> usize {
    w.state.units.row(id).expect("alive")
}

fn edict(w: &mut World, ship: UnitId, (x, y): (i32, i32), queue: bool) {
    let c = PlayerCommand {
        player: 0,
        command: Command::Edict {
            units: vec![ship],
            pos: FxVec2::from_ints(x, y),
            queue,
        },
    };
    w.tick(&[c]).unwrap();
}

/// Ticks until an event `pick` finds, up to `limit`; how many it took.
fn until<T>(w: &mut World, limit: u32, pick: impl Fn(&SimEvent) -> Option<T>) -> Option<(u32, T)> {
    for n in 1..=limit {
        w.tick(&[]).unwrap();
        if let Some(v) = w.events.iter().find_map(&pick) {
            return Some((n, v));
        }
    }
    None
}

fn fired(e: &SimEvent) -> Option<mc_core::FxVec3> {
    match e {
        SimEvent::EdictFired { from, .. } => Some(*from),
        _ => None,
    }
}

fn ticks(w: &mut World, n: u32) {
    for _ in 0..n {
        w.tick(&[]).unwrap();
    }
}

/// In range at once, it gathers for its 15 s and fires from its bow (the hull nearest the
/// mark), on its keel, with no warhead's blast announced.
#[test]
fn gathers_for_fifteen_seconds_then_fires_from_the_hull_nearest_the_mark() {
    let mut w = world();
    let ship = add(&mut w, REGENT, 0, AT);
    edict(&mut w, ship, MARK, false);
    assert!(w.events.iter().any(|e| matches!(
        e,
        SimEvent::EdictGathering { ticks, .. } if *ticks == GATHER as u16
    )));
    let (n, from) = until(&mut w, 400, fired).expect("it fired");
    assert_eq!(n + 1, GATHER, "it gathered for 15 s from the order's tick");
    assert_eq!(
        from.x,
        Fx::from_int(AT.0 + 1450),
        "from the bow end of its spine"
    );
    assert_eq!(from.y, Fx::from_int(AT.1));
    assert_eq!(from.z, w.state.units.z[row(&w, ship)], "on the keel");
    assert!(
        !w.events
            .iter()
            .any(|e| matches!(e, SimEvent::NuclearDetonation { .. })),
        "the Edict draws its own strike"
    );
    assert!(matches!(
        w.state.edicts[0].phase,
        EdictPhase::Fired { age: 0 }
    ));
}

/// Everything near the mark dies, and the rest takes less the farther out, out to 1.2 km;
/// past that nothing is touched. The ship that fired it is spared.
#[test]
fn the_blast_and_the_wave_fall_off_from_the_mark() {
    let mut w = world();
    let ship = add(&mut w, REGENT, 0, AT);
    let offs = [100, 600, 900, 1150, 1700];
    let ids: Vec<UnitId> = offs
        .iter()
        .map(|&d| dummy(&mut w, 1, (MARK.0, MARK.1 + d)))
        .collect();
    let full = Fx::from_int(10_000_000);
    edict(&mut w, ship, MARK, false);
    until(&mut w, 400, fired).expect("it fired");
    // The blast's front and the wave's whole burn.
    ticks(&mut w, 120);
    let lost: Vec<Fx> = ids
        .iter()
        .map(|&id| full - w.state.units.health[row(&w, id)])
        .collect();
    assert!(
        lost[0] >= Fx::from_int(400_000),
        "the core takes the full blast: {lost:?}"
    );
    assert!(
        lost[0] > lost[1] && lost[1] > lost[2] && lost[2] > lost[3],
        "{lost:?}"
    );
    assert!(lost[3] > Fx::ZERO, "the edge is still hurt: {lost:?}");
    assert_eq!(
        lost[4],
        Fx::ZERO,
        "past its reach nothing is touched: {lost:?}"
    );
    let r = row(&w, ship);
    assert_eq!(
        w.state.units.health[r],
        w.blueprints.unit(w.state.units.blueprint[r]).health,
        "the Regent is untouched"
    );
}

/// The wave burns for its roll and linger, then stops; its burn is weaker toward the rim.
#[test]
fn the_wave_burns_ten_seconds_and_weakens_toward_its_rim() {
    let mut w = world();
    let ship = add(&mut w, REGENT, 0, AT);
    let near = dummy(&mut w, 1, (MARK.0 + 400, MARK.1));
    let far = dummy(&mut w, 1, (MARK.0 + 1100, MARK.1));
    edict(&mut w, ship, MARK, false);
    until(&mut w, 400, fired).expect("it fired");
    // Past the blast's front (1.6 s), only the wave burns.
    ticks(&mut w, 41);
    let hp = |w: &World| {
        (
            w.state.units.health[row(w, near)],
            w.state.units.health[row(w, far)],
        )
    };
    let before = hp(&w);
    ticks(&mut w, 10);
    let after = hp(&w);
    let (near_burn, far_burn) = (before.0 - after.0, before.1 - after.1);
    assert!(
        near_burn > far_burn && far_burn > Fx::ZERO,
        "{near_burn:?} {far_burn:?}"
    );
    // 10 s after the shot it is out.
    ticks(&mut w, 60);
    let out = hp(&w);
    ticks(&mut w, 10);
    assert_eq!(hp(&w), out, "the wave has burnt out");
}

/// It hurts its own side's units as a warhead does.
#[test]
fn it_kills_its_own_side_too() {
    let mut w = world();
    let ship = add(&mut w, REGENT, 0, AT);
    let own = add(&mut w, "regency_t1_tank", 0, (MARK.0 + 50, MARK.1));
    edict(&mut w, ship, MARK, false);
    until(&mut w, 400, fired).expect("it fired");
    ticks(&mut w, 5);
    assert!(
        w.state.units.row(own).is_none(),
        "its own tank under the mark is dead"
    );
}

/// The Regent with the Scourge's guns: they fire, fall silent from the gather's start
/// until the quiet after the shot, then fire again.
#[test]
fn every_other_gun_is_silent_while_it_gathers() {
    let mut w = world_with(|bp| {
        let from = bp.id_of(SCOURGE).unwrap();
        let to = bp.id_of(REGENT).unwrap();
        bp.units[to.index()].weapons = bp.units[from.index()].weapons.clone();
    });
    let ship = add(&mut w, REGENT, 0, AT);
    let target = dummy(&mut w, 1, (AT.0 + 600, AT.1 + 200));
    let r = row(&w, target);
    w.state.units.flags[r] &= !flag::PASSIVE;
    let regent = w.blueprints.id_of(REGENT).unwrap();
    let shot = move |e: &SimEvent, _: UnitId| matches!(e, SimEvent::ShotFired { blueprint, .. } if *blueprint == regent);
    let shoots = |w: &mut World, n: u32| {
        (0..n).any(|_| {
            w.tick(&[]).unwrap();
            w.events.iter().any(|e| shot(e, ship))
        })
    };
    assert!(shoots(&mut w, 300), "its guns fire at first");
    edict(&mut w, ship, MARK, false);
    let mut quiet = !w.events.iter().any(|e| shot(e, ship));
    let mut fire_at = None;
    for n in 0..GATHER + 25 {
        w.tick(&[]).unwrap();
        if w.events.iter().any(|e| fired(e).is_some()) {
            fire_at = Some(n);
        }
        // Silent through the gather and the 3 s after the shot.
        if fire_at.is_none_or(|f| n < f + 29) {
            quiet &= !w.events.iter().any(|e| shot(e, ship));
        }
    }
    assert!(fire_at.is_some());
    assert!(quiet, "no gun fired while it gathered");
    assert!(shoots(&mut w, 300), "its guns fire again after the quiet");
}

/// Without power it stands still; with power back it goes on where it was.
#[test]
fn it_stalls_without_power() {
    let mut w = world();
    let ship = add(&mut w, REGENT, 0, AT);
    let p = &mut w.state.players[0];
    p.free_build = false;
    p.energy = Fx::ZERO;
    p.energy_income = Fx::ZERO;
    edict(&mut w, ship, MARK, false);
    ticks(&mut w, 30);
    let left = |w: &World| match w.state.edicts[0].phase {
        EdictPhase::Gathering { left } => left,
        EdictPhase::Fired { .. } => 0,
    };
    let stalled = left(&w);
    assert!(stalled > GATHER as u16 - 5, "it hardly gathered: {stalled}");
    assert!(!w.state.edicts[0].powered);
    w.state.players[0].free_build = true;
    until(&mut w, 400, fired).expect("it fires once it is paid for");
}

/// The firing point follows the hull: ordered away mid-gather, the ship moves on and the
/// shot leaves from where its hull is nearest the mark at the end.
#[test]
fn the_firing_point_follows_the_hull() {
    let mut w = world();
    let ship = add(&mut w, REGENT, 0, AT);
    edict(&mut w, ship, MARK, false);
    let start = w.state.edicts[0].from;
    let go = PlayerCommand {
        player: 0,
        command: Command::Move {
            units: vec![ship],
            target: FxVec2::from_ints(AT.0, AT.1 + 3000),
            queue: false,
        },
    };
    w.tick(&[go]).unwrap();
    assert!(
        matches!(w.state.edicts[0].phase, EdictPhase::Gathering { .. }),
        "a move order does not let the gather go"
    );
    let (_, from) = until(&mut w, 400, fired).expect("it fired");
    assert_ne!(from, start, "the hull moved");
    let r = row(&w, ship);
    let pos = w.state.units.pos[r];
    assert!(
        pos.distance(FxVec2::from_ints(AT.0, AT.1)) > Fx::from_int(100),
        "it moved"
    );
    assert!(
        from.xy().distance(pos) < Fx::from_int(1950),
        "the shot left its hull"
    );
}

/// Out of range it closes first; a mark off the map is refused, and so is a second shot
/// before it has reloaded unless queued, when it waits for the reload.
#[test]
fn it_closes_to_range_and_refuses_what_it_cannot_do() {
    let mut w = world();
    let ship = add(&mut w, REGENT, 0, (1500, 8000));
    let refused = |w: &World, why: Refusal| {
        w.events
            .iter()
            .any(|e| matches!(e, SimEvent::CommandRefused { reason, .. } if *reason == why))
    };
    edict(&mut w, ship, (20_000, 8000), false);
    assert!(refused(&w, Refusal::EdictOffMap));
    assert!(w.state.edicts.is_empty());
    // 13 km off: it closes before it gathers.
    edict(&mut w, ship, (14_000, 8000), false);
    assert!(
        w.state.edicts.is_empty(),
        "out of range it does not gather yet"
    );
    until(&mut w, 3000, |e| {
        matches!(e, SimEvent::EdictGathering { .. }).then_some(())
    })
    .expect("it closed and gathered");
    let from = w.state.edicts[0].from.xy();
    assert!(from.distance(FxVec2::from_ints(14_000, 8000)) <= Fx::from_int(6000));
    until(&mut w, 400, fired).expect("it fired");
    edict(&mut w, ship, MARK, false);
    assert!(refused(&w, Refusal::EdictNotReady), "not ready again yet");
    edict(&mut w, ship, MARK, true);
    let (n, _) = until(&mut w, 2000, fired).expect("the queued shot fires after the reload");
    assert!(n > 1300, "it waited for the reload: {n}");
}

/// A stop lets the gather go, and so does the ship's death.
#[test]
fn a_stop_or_its_death_lets_the_gather_go() {
    let mut w = world();
    let ship = add(&mut w, REGENT, 0, AT);
    edict(&mut w, ship, MARK, false);
    ticks(&mut w, 20);
    let stop = PlayerCommand {
        player: 0,
        command: Command::Stop { units: vec![ship] },
    };
    w.tick(&[stop]).unwrap();
    assert!(w
        .events
        .iter()
        .any(|e| matches!(e, SimEvent::EdictCancelled { .. })));
    assert!(w.state.edicts.is_empty());
    edict(&mut w, ship, MARK, false);
    ticks(&mut w, 5);
    let r = row(&w, ship);
    w.state.units.health[r] = Fx::ZERO;
    assert!(
        until(&mut w, 30, |e| matches!(e, SimEvent::EdictCancelled { .. })
            .then_some(()))
        .is_some()
    );
    assert!(w.state.edicts.is_empty());
}

/// A dome over a unit in the wave takes its burn for it.
#[test]
fn a_dome_takes_the_wave_for_what_is_under_it() {
    let mut w = world();
    let ship = add(&mut w, REGENT, 0, AT);
    let at = (MARK.0 + 1000, MARK.1);
    let dome = add(&mut w, DOME, 1, at);
    let under = dummy(&mut w, 1, (at.0 + 10, at.1));
    let open = dummy(&mut w, 1, (MARK.0, MARK.1 - 1000));
    edict(&mut w, ship, MARK, false);
    until(&mut w, 400, fired).expect("it fired");
    ticks(&mut w, 100);
    let lost = |w: &World, id| Fx::from_int(10_000_000) - w.state.units.health[row(w, id)];
    assert!(
        lost(&w, under) < lost(&w, open),
        "the dome spared what is under it: {:?} {:?}",
        lost(&w, under),
        lost(&w, open)
    );
    let _ = dome;
}

/// The enemy sees the gather once they see the ship or the mark; the owner always does.
#[test]
fn the_frame_shows_the_gather() {
    let mut w = world();
    let ship = add(&mut w, REGENT, 0, AT);
    edict(&mut w, ship, MARK, false);
    ticks(&mut w, 30);
    let mut frame = RenderFrame::default();
    w.write_render_frame(Some(0), &mut frame);
    assert_eq!(frame.edicts.len(), 1);
    let v = frame.edicts[0];
    assert_eq!(v.stage, EdictStage::Gathering);
    assert!(v.progress > 0.15 && v.progress < 0.25, "{}", v.progress);
    assert!((v.along - 1450.0).abs() < 1.0, "the bow: {}", v.along);
    w.write_render_frame(Some(1), &mut frame);
    assert_eq!(frame.edicts.len(), 1, "no fog: the enemy sees it too");
}
