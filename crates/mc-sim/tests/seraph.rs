//! The Seraph, the Regency's tier 5: it dashes into the middle of the enemies it sees and
//! lands letting go a wave that rolls out all round (`dash.rs`, `wake.rs`), its seeker
//! battery going up as it lands.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::{Blueprints, MoveLayer};
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller, FireState, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const SERAPH: &str = "regency_t5_seraph";
const BOT: &str = "aster_t3_assault_bot";
const SCOUT: &str = "aster_t1_scout";
/// The ground's height, and the map's side in 8 m cells (4 km).
const GROUND: i32 = 40;
const CELLS: u32 = 512;

/// A flat map, or one with a patch of broken ground no hull can stand on: steep cells
/// over `rough` (x and y ranges, metres).
fn world(rough: Option<((i32, i32), (i32, i32))>) -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let stride = CELLS as usize + 1;
    let mut samples = vec![GROUND as u16; stride * stride];
    if let Some(((x0, x1), (y0, y1))) = rough {
        for y in (y0 / 8) as usize..=(y1 / 8) as usize {
            for x in (x0 / 8) as usize..=(x1 / 8) as usize {
                if (x + y) % 2 == 0 {
                    samples[y * stride + x] = (GROUND + 30) as u16;
                }
            }
        }
    }
    let terrain = Heightfield::from_samples(CELLS, CELLS, samples, Fx::ZERO, Fx::ONE, Fx::ZERO);
    let map = MapData {
        name: "seraph".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(300, 300), FxVec2::from_ints(3800, 3800)],
        props: Vec::new(),
    };
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
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

/// A finished unit; `passive` ones never shoot back.
fn add(w: &mut World, key: &str, owner: u8, x: i32, y: i32, passive: bool) -> UnitId {
    let id = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(id, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    if passive {
        w.state.units.flags[row] |= flag::PASSIVE;
    }
    w.state.units.id(row)
}

fn row(w: &World, id: UnitId) -> usize {
    w.state.units.row(id).expect("alive")
}

/// Five enemy assault bots round `(x, y)`: one in the middle and four 80 m out.
fn cluster(w: &mut World, x: i32, y: i32) -> Vec<UnitId> {
    [(0, 0), (80, 0), (-80, 0), (0, 80), (0, -80)]
        .into_iter()
        .map(|(dx, dy)| add(w, BOT, 1, x + dx, y + dy, true))
        .collect()
}

/// Ticks until the Seraph lands from a dash, up to `limit`: the tick it landed and where.
fn until_landed(w: &mut World, limit: u32) -> Option<(u32, FxVec2)> {
    for _ in 0..limit {
        w.tick(&[]).unwrap();
        if let Some(to) = w.events.iter().find_map(|e| match e {
            SimEvent::DashLanded { to, .. } => Some(to.xy()),
            _ => None,
        }) {
            return Some((w.tick_count(), to));
        }
    }
    None
}

fn command(player: u8, command: Command) -> PlayerCommand {
    PlayerCommand { player, command }
}

/// Idle, it sees a cluster 1200 m off and comes down in its middle, where it then stands.
#[test]
fn dashes_into_the_middle_of_a_cluster() {
    let mut w = world(None);
    let seraph = add(&mut w, SERAPH, 0, 500, 2000, false);
    cluster(&mut w, 1700, 2000);
    let (_, to) = until_landed(&mut w, 60).expect("it dashed");
    let middle = FxVec2::from_ints(1700, 2000);
    // Clear of the middle bot's hull, on the side it came from.
    let r = &w.state.units;
    let gap = to.distance(middle);
    assert!(
        gap <= Fx::from_int(90),
        "landed {gap} m from the cluster's middle"
    );
    assert_eq!(r.pos[row(&w, seraph)], to, "it stands where it landed");
}

/// It charges, is gone, and lands: the windup's and the transit's ticks after it was
/// seen charging. While it is gone it is out of the world, already at the mark.
#[test]
fn winds_up_then_lands() {
    let mut w = world(None);
    let seraph = add(&mut w, SERAPH, 0, 500, 2000, false);
    cluster(&mut w, 1700, 2000);
    let dash = w.blueprints.unit_by_key(SERAPH).unwrap().dash.unwrap();
    let windup = dash.windup_ticks as u32 + dash.transit_ticks as u32;
    let mut charged = None;
    for _ in 0..30 {
        w.tick(&[]).unwrap();
        if w.events
            .iter()
            .any(|e| matches!(e, SimEvent::DashCharging { .. }))
        {
            charged = Some(w.tick_count());
            break;
        }
    }
    let charged = charged.expect("it wound up");
    let mut gone = None;
    for _ in 0..60 {
        w.tick(&[]).unwrap();
        if let Some(to) = w.events.iter().find_map(|e| match e {
            SimEvent::DashVanished { to, .. } => Some(to.xy()),
            _ => None,
        }) {
            gone = Some((w.tick_count(), to));
            break;
        }
    }
    let (vanished, mark) = gone.expect("it vanished");
    assert_eq!(
        vanished - charged,
        dash.windup_ticks as u32,
        "it goes as the charge ends"
    );
    let r = row(&w, seraph);
    assert!(
        w.state.units.has_flag(r, flag::IN_FACTORY),
        "out of the world"
    );
    assert_eq!(w.state.units.pos[r], mark, "already at the mark");
    let (landed, to) = until_landed(&mut w, 60).expect("it landed");
    assert_eq!(landed - charged, windup, "it lands as the transit ends");
    assert_eq!(to, mark);
    assert!(!w.state.units.has_flag(row(&w, seraph), flag::IN_FACTORY));
}

/// Once landed it may not dash again until its recharge is out, and then it does.
#[test]
fn the_cooldown_holds_the_next_dash() {
    let mut w = world(None);
    add(&mut w, SERAPH, 0, 2000, 2000, false);
    // Scouts the landing kills outright, then a second cluster farther on.
    for (dx, dy) in [(0, 0), (40, 0), (-40, 0)] {
        add(&mut w, SCOUT, 1, 1000 + dx, 2000 + dy, true);
    }
    let (first, _) = until_landed(&mut w, 60).expect("first dash");
    cluster(&mut w, 1000, 3200);
    let cooldown = w
        .blueprints
        .unit_by_key(SERAPH)
        .unwrap()
        .dash
        .unwrap()
        .cooldown_ticks as u32;
    let (second, to) = until_landed(&mut w, cooldown + 40).expect("second dash");
    assert!(
        second - first >= cooldown,
        "dashed again {} ticks after landing, recharge {cooldown}",
        second - first
    );
    assert!(to.distance(FxVec2::from_ints(1000, 3200)) <= Fx::from_int(90));
}

/// A move order, hold fire and hold position keep it where the player put it.
#[test]
fn no_dash_unless_free_to_pick_its_fight() {
    for how in ["move", "hold fire", "hold position"] {
        let mut w = world(None);
        let seraph = add(&mut w, SERAPH, 0, 500, 2000, false);
        cluster(&mut w, 1700, 2000);
        let order = match how {
            "move" => Command::Move {
                units: vec![seraph],
                target: FxVec2::from_ints(500, 3800),
                queue: false,
            },
            "hold fire" => Command::SetFireState {
                units: vec![seraph],
                state: FireState::HoldFire,
            },
            _ => Command::SetFireState {
                units: vec![seraph],
                state: FireState::HoldPosition,
            },
        };
        w.tick(&[command(0, order)]).unwrap();
        for _ in 0..60 {
            w.tick(&[]).unwrap();
            assert!(
                !w.events.iter().any(|e| matches!(
                    e,
                    SimEvent::DashCharging { .. } | SimEvent::DashLanded { .. }
                )),
                "it dashed under {how}"
            );
        }
    }
}

/// Under an attack order the mark picks the cluster: it goes for the one named, not the
/// nearer one.
#[test]
fn an_attack_order_picks_the_cluster() {
    let mut w = world(None);
    let seraph = add(&mut w, SERAPH, 0, 2000, 2000, false);
    cluster(&mut w, 2000, 2800);
    let far = cluster(&mut w, 2000, 600);
    w.tick(&[command(
        0,
        Command::Attack {
            units: vec![seraph],
            target: far[0],
            queue: false,
        },
    )])
    .unwrap();
    let (_, to) = until_landed(&mut w, 60).expect("it dashed");
    assert!(
        to.distance(FxVec2::from_ints(2000, 600)) <= Fx::from_int(90),
        "landed at {to:?}, not by the mark"
    );
}

/// Ticks for the landing's wave to roll out to its reach.
fn roll_out(w: &mut World) {
    for _ in 0..16 {
        w.tick(&[]).unwrap();
    }
}

/// The landing's wave strikes the enemies round it, stunning all but experimentals; a
/// friend in the middle of them is not touched.
#[test]
fn the_wave_hurts_enemies_not_friends() {
    let mut w = world(None);
    add(&mut w, SERAPH, 0, 500, 2000, false);
    let bots = cluster(&mut w, 1700, 2000);
    let tank = add(&mut w, "aster_t4_assault_tank", 1, 1700, 2150, true);
    let friend = add(&mut w, BOT, 0, 1700, 1880, true);
    until_landed(&mut w, 60).expect("it dashed");
    roll_out(&mut w);
    let u = &w.state.units;
    for &b in &bots {
        // Killed outright, or hurt and stunned.
        if let Some(r) = u.row(b) {
            assert!(u.health[r] < w.unit_max_health(r), "a bot was not struck");
            assert!(u.stun[r][0] > 0, "a bot was not stunned");
        }
    }
    let r = row(&w, tank);
    assert!(
        u.health[r] < w.unit_max_health(r),
        "the tank was not struck"
    );
    assert_eq!(u.stun[r][0], 0, "an experimental was stunned");
    let r = row(&w, friend);
    assert_eq!(u.health[r], w.unit_max_health(r), "the friend was struck");
    assert_eq!(u.stun[r][0], 0, "the friend was stunned");
}

/// The wave rolls out from where it lands: an enemy near its reach is struck (and
/// stunned) only once the front gets there, not as it lands.
#[test]
fn the_wave_rolls_out_from_the_landing() {
    let mut w = world(None);
    add(&mut w, SERAPH, 0, 500, 2000, false);
    cluster(&mut w, 1700, 2000);
    // Past the cluster's gather, inside the wave's reach of where it lands.
    let far = add(&mut w, BOT, 1, 1700, 2290, true);
    until_landed(&mut w, 60).expect("it dashed");
    let r = row(&w, far);
    assert_eq!(
        w.state.units.stun[r][0], 0,
        "struck before the front got there"
    );
    roll_out(&mut w);
    if let Some(r) = w.state.units.row(far) {
        assert!(w.state.units.stun[r][0] > 0, "the front never reached it");
    }
}

/// Four heavy tanks the landing does not kill, 150 m about `(x, y)`.
fn heavy_cluster(w: &mut World, x: i32, y: i32) {
    for (dx, dy) in [(150, 0), (-150, 0), (0, 150), (0, -150)] {
        add(w, "aster_t4_assault_tank", 1, x + dx, y + dy, true);
    }
}

/// Its seeker battery goes up as it lands, though it was deep in its reload.
#[test]
fn the_battery_fires_as_it_lands() {
    let mut w = world(None);
    let seraph = add(&mut w, SERAPH, 0, 500, 2000, false);
    heavy_cluster(&mut w, 1700, 2000);
    let bp = w.blueprints.unit_by_key(SERAPH).unwrap();
    let battery = bp
        .weapons
        .iter()
        .position(|w| w.on_dash)
        .expect("a battery");
    let salvo = bp.weapons[battery].salvo as usize;
    let seraph_bp = bp.id;
    let r = row(&w, seraph);
    w.state.units.weapon_cooldown[r][battery] = u16::MAX;
    until_landed(&mut w, 60).expect("it dashed");
    // The first goes up on the tick it lands.
    let mut launched = 0;
    for k in 0..30 {
        if k > 0 {
            w.tick(&[]).unwrap();
        }
        launched += w
            .events
            .iter()
            .filter(|e| {
                matches!(e, SimEvent::ShotFired { blueprint, weapon, .. }
                if *blueprint == seraph_bp && *weapon as usize == battery)
            })
            .count();
    }
    assert_eq!(launched, salvo, "the whole battery went up on landing");
}

/// A cluster whose middle is broken ground: it comes down on ground it can stand on.
#[test]
fn lands_on_ground_it_can_stand_on() {
    let rough = ((1600, 1800), (1900, 2100));
    let mut w = world(Some(rough));
    let seraph = add(&mut w, SERAPH, 0, 300, 2000, false);
    for (x, y) in [(1560, 2000), (1840, 2000), (1700, 1860), (1700, 2140)] {
        add(&mut w, BOT, 1, x, y, true);
    }
    let (_, to) = until_landed(&mut w, 60).expect("it dashed");
    let m = w.bp(row(&w, seraph)).motion.unwrap();
    assert_eq!(m.layer, MoveLayer::Hover);
    assert!(
        w.nav.passable(m.layer, m.size_class, to),
        "landed on ground it cannot stand on: {to:?}"
    );
}

/// Its state rides a snapshot: restored mid-windup, it lands on the same tick as one that
/// was never stopped.
#[test]
fn a_windup_survives_a_snapshot() {
    let run = |restore: bool| {
        let mut w = world(None);
        add(&mut w, SERAPH, 0, 500, 2000, false);
        cluster(&mut w, 1700, 2000);
        loop {
            w.tick(&[]).unwrap();
            if w.events
                .iter()
                .any(|e| matches!(e, SimEvent::DashCharging { .. }))
            {
                break;
            }
        }
        if restore {
            let bytes = w.snapshot();
            let mut again = world(None);
            again.restore(world_terrain(), &bytes).unwrap();
            w = again;
        }
        until_landed(&mut w, 60).expect("it landed")
    };
    assert_eq!(run(false), run(true));
}

fn world_terrain() -> Heightfield {
    let stride = CELLS as usize + 1;
    Heightfield::from_samples(
        CELLS,
        CELLS,
        vec![GROUND as u16; stride * stride],
        Fx::ZERO,
        Fx::ONE,
        Fx::ZERO,
    )
}

/// Power for side 0's hull fields: two reactors far off in a corner.
fn power(w: &mut World) {
    add(w, "regency_t3_power", 0, 150, 150, false);
    add(w, "regency_t3_power", 0, 300, 150, false);
}

/// A Seraph against four Striders (a little under a quarter of its price): it dashes
/// in among them, wins and lives.
#[test]
fn outfights_a_pack_of_tier_4s() {
    let mut w = world(None);
    power(&mut w);
    let seraph = add(&mut w, SERAPH, 0, 500, 2000, false);
    let striders: Vec<UnitId> = (0..4)
        .map(|i| {
            add(
                &mut w,
                "regency_t4_strider",
                1,
                1900 + i * 100,
                1900 + (i % 2) * 200,
                false,
            )
        })
        .collect();
    for _ in 0..1200 {
        w.tick(&[]).unwrap();
        if striders.iter().all(|&s| w.state.units.row(s).is_none()) {
            break;
        }
    }
    assert!(
        striders.iter().all(|&s| w.state.units.row(s).is_none()),
        "the Striders still stand"
    );
    assert!(w.state.units.row(seraph).is_some(), "the Seraph fell");
}
