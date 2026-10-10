//! The Strider, the Regency assault tripod: its two cannons take turns, every charge that
//! is seen is fired, and its seekers leave from the cells on its head.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::{cat, Blueprints};
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const STRIDER: &str = "regency_t4_strider";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "strider".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1500, 1500)],
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
        seed: 3,
        players: vec![player("Regency", 0), player("Aster", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn spawn(w: &World, owner: u8, key: &str, x: i32, flags: u16) -> PlayerCommand {
    PlayerCommand {
        player: 0,
        command: Command::DebugSpawn {
            owner,
            blueprint: w.blueprints.id_of(key).unwrap(),
            pos: FxVec2::from_ints(x, 512),
            heading: Angle::ZERO,
            count: 1,
            flags,
            build: 1000,
        },
    }
}

/// What the cannons did: every (tick, weapon) they began a charge on and fired on, and
/// the last tick logged.
struct Log {
    charged: Vec<(u32, u8)>,
    fired: Vec<(u32, u8)>,
    end: u32,
}

/// What the Strider's cannons did over
/// `ticks` after a target `gap` metres off appears (after it sat idle a while).
fn cannon_log(gap: i32, ticks: u32) -> Log {
    let mut w = world();
    let strider = w.blueprints.id_of(STRIDER).unwrap();
    w.tick(&[spawn(&w, 0, STRIDER, 500, 0)]).unwrap();
    for _ in 0..100 {
        w.tick(&[]).unwrap();
    }
    w.tick(&[spawn(
        &w,
        1,
        "aster_t3_assault_bot",
        500 + gap,
        flag::PASSIVE | flag::INVULNERABLE,
    )])
    .unwrap();
    let (mut charged, mut fired) = (Vec::new(), Vec::new());
    for k in 0..=ticks {
        if k > 0 {
            w.tick(&[]).unwrap();
        }
        let tick = w.tick_count();
        for e in &w.events {
            match e {
                SimEvent::WeaponCharging {
                    blueprint, weapon, ..
                } if *blueprint == strider && *weapon < 2 => charged.push((tick, *weapon)),
                SimEvent::ShotFired {
                    blueprint, weapon, ..
                } if *blueprint == strider && *weapon < 2 => fired.push((tick, *weapon)),
                _ => {}
            }
        }
    }
    Log {
        charged,
        fired,
        end: w.tick_count(),
    }
}

/// Out of idle both cannons are seen charging, and each charge ends in that cannon's own
/// shot a charge later: the second does not sit a charged ball out while the first fires.
/// The two never fire on one tick, and both keep firing.
#[test]
fn every_charge_is_fired() {
    let w = world();
    let bp = w.blueprints.unit(w.blueprints.id_of(STRIDER).unwrap());
    let charge = bp.weapons[0].charge_ticks as u32;
    assert!(charge > 0, "the cannons charge");
    let Log {
        charged,
        fired,
        end,
    } = cannon_log(250, 400);
    for gun in 0..2u8 {
        let starts: Vec<u32> = charged.iter().filter(|c| c.1 == gun).map(|c| c.0).collect();
        let shots: Vec<u32> = fired.iter().filter(|f| f.1 == gun).map(|f| f.0).collect();
        assert!(shots.len() >= 3, "cannon {gun} fired {shots:?}");
        for shot in &shots {
            assert!(
                starts.contains(&(shot - charge)),
                "cannon {gun} fired at {shot} without a charge {charge} ticks before: \
                 charges {charged:?}, shots {fired:?}"
            );
        }
        for start in &starts {
            // The last charge may still be under way when the log ends.
            if start + charge <= end {
                assert!(
                    shots.contains(&(start + charge)),
                    "cannon {gun}'s charge at {start} never fired: charges {charged:?}, \
                     shots {fired:?}"
                );
            }
        }
    }
    for pair in fired.windows(2) {
        assert_ne!(pair[0].0, pair[1].0, "both fired on one tick: {fired:?}");
    }
}

/// The seekers leave from the cells on the back of the head wherever the head has turned:
/// the launcher rides it (a `mount` vertical launcher, `combat.rs`).
#[test]
fn seekers_leave_from_the_heads_cells() {
    let mut w = world();
    let id = w.blueprints.id_of(STRIDER).unwrap();
    let cells = w.blueprints.unit(id).weapons[2].muzzles.clone();
    assert_eq!(cells.len(), 8);
    w.tick(&[spawn(&w, 0, STRIDER, 500, 0)]).unwrap();
    let row = w
        .state
        .units
        .blueprint
        .iter()
        .position(|b| *b == id)
        .unwrap();
    // A mark off the left flank: the head turns a quarter round onto it.
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::DebugSpawn {
            owner: 1,
            blueprint: w.blueprints.id_of("aster_t3_assault_bot").unwrap(),
            pos: FxVec2::from_ints(500, 1100),
            heading: Angle::ZERO,
            count: 1,
            flags: flag::PASSIVE | flag::INVULNERABLE,
            build: 1000,
        },
    }])
    .unwrap();
    let mut turned = 0;
    for _ in 0..600 {
        w.tick(&[]).unwrap();
        let u = &w.state.units;
        let torso = u.heading[row] + u.weapon_yaw[row][0];
        for e in &w.events {
            let SimEvent::ShotFired {
                pos,
                blueprint,
                weapon: 2,
                ..
            } = e
            else {
                continue;
            };
            assert_eq!(*blueprint, id);
            let near = cells
                .iter()
                .map(|c| (u.pos[row] + c.xy().rotate(torso)).distance(pos.xy()))
                .min()
                .unwrap();
            assert!(
                near < Fx::from_int(2),
                "a seeker left {near:?} m from the nearest cell"
            );
            if u.heading[row].delta_to(torso).unsigned_abs() > Angle::from_degrees(45).0 {
                turned += 1;
            }
        }
    }
    assert!(turned >= 8, "{turned} seekers left with the head turned");
}

/// No anti-air: nothing on it shoots at aircraft.
#[test]
fn it_has_no_anti_air() {
    let w = world();
    let bp = w.blueprints.unit(w.blueprints.id_of(STRIDER).unwrap());
    assert!(bp.weapons.iter().all(|w| w.target_mask & cat::AIR == 0));
}

/// How a Strider and a Breacher, each ordered onto the other from `gap` metres apart,
/// come out: each one's health share left (shield included) when the first falls or
/// time runs out, and the seconds it took.
fn duel(gap: i32) -> (f32, f32, u32) {
    let mut w = world();
    let (strider, breacher) = (
        w.blueprints.id_of(STRIDER).unwrap(),
        w.blueprints.id_of("aster_t4_breacher").unwrap(),
    );
    let at = |w: &World, owner: u8, key: &str, x: i32| PlayerCommand {
        player: owner,
        command: Command::DebugSpawn {
            owner,
            blueprint: w.blueprints.id_of(key).unwrap(),
            pos: FxVec2::from_ints(x, 1000),
            heading: if owner == 0 {
                Angle::ZERO
            } else {
                Angle::from_degrees(180)
            },
            count: 1,
            flags: 0,
            build: 1000,
        },
    };
    w.tick(&[
        at(&w, 0, STRIDER, 1000 - gap / 2),
        at(&w, 1, "aster_t4_breacher", 1000 + gap / 2),
    ])
    .unwrap();
    let row = |w: &World, id| {
        let u = &w.state.units;
        (0..u.blueprint.len()).find(|&r| u.slots.is_alive(r) && u.blueprint[r] == id)
    };
    let (s, b) = (row(&w, strider).unwrap(), row(&w, breacher).unwrap());
    let (sid, bid) = (w.state.units.id(s), w.state.units.id(b));
    let attack = |player: u8, unit, target| PlayerCommand {
        player,
        command: Command::Attack {
            units: vec![unit],
            target,
            queue: false,
        },
    };
    // Both sides powered, so the Breacher's hull field is up as it is in a game.
    for p in &mut w.state.players {
        p.free_build = true;
    }
    // The Breacher walks in to its gatlings' reach, then turns on the Strider; the Strider
    // stands and shoots.
    let close = PlayerCommand {
        player: 1,
        command: Command::Move {
            units: vec![bid],
            target: FxVec2::from_ints(1000 - gap / 2 + 650, 1000),
            queue: false,
        },
    };
    let mut queued = attack(1, bid, sid);
    if let Command::Attack { queue, .. } = &mut queued.command {
        *queue = true;
    }
    w.tick(&[attack(0, sid, bid), close, queued]).unwrap();
    let full = |w: &World, id| {
        let bp = w.blueprints.unit(id);
        bp.health.to_f32() + bp.shield.as_ref().map_or(0.0, |s| s.health.to_f32())
    };
    let left = |w: &World, r: Option<usize>, id| {
        r.map_or(0.0, |r| {
            let u = &w.state.units;
            (u.health[r] + u.shield_hp[r]).to_f32() / full(w, id)
        })
    };
    let mut ticks = 0;
    while ticks < seconds(240) {
        w.tick(&[]).unwrap();
        ticks += 1;
        if row(&w, strider).is_none() || row(&w, breacher).is_none() {
            break;
        }
    }
    (
        left(&w, row(&w, strider), strider),
        left(&w, row(&w, breacher), breacher),
        (ticks / seconds(1)) as u32,
    )
}

fn seconds(s: u32) -> usize {
    (s * mc_core::TICKS_PER_SECOND) as usize
}

/// A Strider and a Breacher are a fair fight one on one (user, 2026-10-06): the Strider less
/// tanky but outranging it a little. Met from past the Breacher's reach the Strider wins,
/// caught inside its gatlings' the Breacher does, and either way the winner comes out
/// badly hurt.
#[test]
fn a_strider_and_a_breacher_are_a_close_fight() {
    let w = world();
    let unit = |key: &str| w.blueprints.unit(w.blueprints.id_of(key).unwrap());
    let reach = |key: &str| unit(key).weapons.iter().map(|w| w.range_max).max().unwrap();
    assert!(
        reach(STRIDER) > reach("aster_t4_breacher"),
        "the Strider outranges it"
    );
    assert!(
        unit(STRIDER).health < unit("aster_t4_breacher").health,
        "less tanky"
    );
    let (strider, breacher, secs) = duel(1300);
    assert!(
        breacher == 0.0 && strider < 0.6,
        "from range: Strider {strider:.2} left, Breacher {breacher:.2} after {secs} s"
    );
    let (strider, breacher, secs) = duel(900);
    assert!(
        strider == 0.0 && breacher < 0.6,
        "up close: Strider {strider:.2} left, Breacher {breacher:.2} after {secs} s"
    );
}

/// Ordered onto one mark among several, the seekers open on it and spread the rest of the
/// volley over the others in reach (user, 2026-10-06); the cannons stay on the mark.
#[test]
fn seekers_spread_over_the_marks_in_reach() {
    let mut w = world();
    let id = w.blueprints.id_of(STRIDER).unwrap();
    let bot = w.blueprints.id_of("aster_t3_assault_bot").unwrap();
    let mut spawns = vec![spawn(&w, 0, STRIDER, 500, 0)];
    for k in 0..4 {
        spawns.push(PlayerCommand {
            player: 0,
            command: Command::DebugSpawn {
                owner: 1,
                blueprint: bot,
                pos: FxVec2::from_ints(1300, 400 + 80 * k),
                heading: Angle::ZERO,
                count: 1,
                flags: flag::PASSIVE | flag::INVULNERABLE,
                build: 1000,
            },
        });
    }
    w.tick(&spawns).unwrap();
    let u = &w.state.units;
    let rows: Vec<usize> = (0..u.blueprint.len())
        .filter(|&r| u.slots.is_alive(r))
        .collect();
    let strider = *rows.iter().find(|&&r| u.blueprint[r] == id).unwrap();
    let mark = *rows.iter().find(|&&r| u.blueprint[r] == bot).unwrap();
    let (sid, mid) = (u.id(strider), u.id(mark));
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::Attack {
            units: vec![sid],
            target: mid,
            queue: false,
        },
    }])
    .unwrap();
    // Each seeker's mark as it leaves its cell (by its serial, the first tick it is seen).
    let mut launched = std::collections::BTreeMap::new();
    for _ in 0..seconds(30) {
        w.tick(&[]).unwrap();
        let p = &w.state.projectiles;
        for i in 0..p.len() {
            if p.source[i] == sid && p.weapon[i] == 2 {
                launched.entry(p.serial[i]).or_insert(p.target[i]);
            }
        }
    }
    let chased: std::collections::BTreeSet<_> = launched.values().collect();
    let on_mark = launched.values().filter(|&&t| t == mid).count();
    assert!(on_mark > 0, "no seeker went at the mark");
    assert!(
        chased.len() >= 3,
        "the seekers chased {} marks",
        chased.len()
    );
}

/// No Regency missile is drawn as a bare ball of plasma (user, 2026-10-06: "no energy balls
/// for rockets"): each is a missile body with the seeker's look, the Strider's included.
#[test]
fn every_regency_missile_has_a_body() {
    use mc_sim::mirror::{plasma_look, PLASMA_LOOK_FUSION_SEEKER, PLASMA_LOOK_GRAVITIC_SEEKER};
    let w = world();
    let mut seekers = 0;
    for unit in &w.blueprints.units {
        for weapon in unit.weapons.iter().filter(|w| w.seeker()) {
            seekers += 1;
            // One carrying pinch fusion has the seeker's body, its thrust fusion-hot.
            let look = if weapon.fusion_strike {
                PLASMA_LOOK_FUSION_SEEKER
            } else {
                PLASMA_LOOK_GRAVITIC_SEEKER
            };
            assert_eq!(plasma_look(weapon), look, "{}: {}", unit.key, weapon.name);
        }
    }
    assert!(seekers >= 8, "Regency missiles: {seekers}");
    let bp = w.blueprints.unit(w.blueprints.id_of(STRIDER).unwrap());
    assert!(bp.weapons[2].seeker() && bp.weapons[2].fusion_strike);
}
