//! The Behemoth (`aster_t5_titan`), the first tier 5: a giant walker that strides
//! straight over structures instead of pathing round them, crushes enemy ground units
//! under its feet, and wears an oblong bubble field that stops fire where it meets it.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, FireState, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const TITAN: &str = "aster_t5_titan";
const TANK: &str = "aster_t1_tank";
const WALL: &str = "aster_wall";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(1024, 1024, Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(7000, 7000)],
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
        seed: 5,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn add(w: &mut World, key: &str, owner: u8, x: i32, y: i32, heading: i32) -> UnitId {
    let id = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(
            id,
            owner,
            FxVec2::from_ints(x, y),
            Angle::from_degrees(heading),
            true,
        )
        .unwrap();
    w.state.units.id(row)
}

fn row(w: &World, id: UnitId) -> usize {
    w.state.units.row(id).unwrap()
}

fn health(w: &World, id: UnitId) -> f32 {
    w.state
        .units
        .row(id)
        .map_or(0.0, |r| w.state.units.health[r].to_f32())
}

fn hold_fire(w: &mut World, id: UnitId) {
    let r = row(w, id);
    w.state.units.fire_state[r] = FireState::HoldFire;
}

fn move_to(w: &mut World, id: UnitId, x: i32, y: i32) {
    let cmd = PlayerCommand {
        player: 0,
        command: Command::Move {
            units: vec![id],
            target: FxVec2::from_ints(x, y),
            queue: false,
        },
    };
    w.tick(&[cmd]).unwrap();
}

fn seconds(s: u32) -> usize {
    (s * TICKS_PER_SECOND) as usize
}

#[test]
fn the_titan_strides_straight_over_a_wall_of_structures() {
    let mut w = world();
    let titan = add(&mut w, TITAN, 0, 1000, 3000, 0);
    // A solid line of walls across its way, far longer than it would ever walk round.
    for y in (2400..=3600).step_by(12) {
        add(&mut w, WALL, 0, 1600, y, 0);
    }
    move_to(&mut w, titan, 2400, 3000);
    let mut widest = 0.0f32;
    let mut arrived = None;
    for t in 0..seconds(240) {
        w.tick(&[]).unwrap();
        let r = row(&w, titan);
        let p = w.state.units.pos[r];
        widest = widest.max((p.y.to_f32() - 3000.0).abs());
        if p.distance(FxVec2::from_ints(2400, 3000)).to_f32() < 120.0 {
            arrived = Some(t);
            break;
        }
    }
    let at = w.state.units.pos[row(&w, titan)];
    assert!(
        arrived.is_some(),
        "never got there: stopped at {:?}",
        at.to_f32()
    );
    // 1400 m at 11 m/s is ~130 s; walking round would add hundreds of metres.
    assert!(
        arrived.unwrap() < seconds(170),
        "took {} ticks",
        arrived.unwrap()
    );
    assert!(
        widest < 10.0,
        "it went round instead of over: strayed {widest} m"
    );
}

#[test]
fn a_footfall_crushes_enemy_tanks_and_spares_friends() {
    let mut w = world();
    let titan = add(&mut w, TITAN, 0, 1000, 3000, 0);
    hold_fire(&mut w, titan);
    // A column of enemy tanks and one of friendly ones lie along both feet's line
    // (the stomp's gauge either side of the walker's path).
    let stomp = w
        .blueprints
        .unit(w.blueprints.id_of(TITAN).unwrap())
        .stomp
        .unwrap();
    let gauge = stomp.gauge.to_f32() as i32;
    let mut enemies = Vec::new();
    let mut friends = Vec::new();
    for x in (1080..1480).step_by(16) {
        let e = add(&mut w, TANK, 1, x, 3000 + gauge, 0);
        hold_fire(&mut w, e);
        enemies.push(e);
        friends.push(add(&mut w, TANK, 0, x, 3000 - gauge, 0));
    }
    move_to(&mut w, titan, 1700, 3000);
    for _ in 0..seconds(80) {
        w.tick(&[]).unwrap();
    }
    let crushed = enemies.iter().filter(|&&e| health(&w, e) <= 0.0).count();
    let hurt_friends = friends
        .iter()
        .filter(|&&f| {
            let full = w
                .blueprints
                .unit(w.blueprints.id_of(TANK).unwrap())
                .health
                .to_f32();
            health(&w, f) < full
        })
        .count();
    assert!(
        crushed >= 4,
        "only {crushed} of {} enemy tanks crushed",
        enemies.len()
    );
    assert_eq!(hurt_friends, 0, "the stomp hurt its own side");
}

#[test]
fn its_personal_field_takes_the_fire_and_the_hull_takes_nothing() {
    let mut w = world();
    let titan = add(&mut w, TITAN, 0, 2000, 3000, 0);
    hold_fire(&mut w, titan);
    // Its upkeep is paid: an unpowered field is down.
    w.state.players[0].free_build = true;
    // Let the field come up.
    for _ in 0..seconds(12) {
        w.tick(&[]).unwrap();
    }
    let shield_full = w.state.units.shield_hp[row(&w, titan)].to_f32();
    assert!(shield_full > 0.0, "the field never came up");
    let full = health(&w, titan);
    for i in 0..6 {
        add(&mut w, TANK, 1, 1900 + i * 40, 3290, 270);
    }
    let mut on_field = 0;
    let mut lowest = shield_full;
    for _ in 0..seconds(30) {
        w.tick(&[]).unwrap();
        lowest = lowest.min(w.state.units.shield_hp[row(&w, titan)].to_f32());
        on_field += w
            .events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    SimEvent::Impact {
                        on_shield: true,
                        ..
                    }
                )
            })
            .count();
    }
    assert!(on_field > 5, "hardly anything met the field ({on_field})");
    // Its regen soon makes good what light tanks do; it still took their shots.
    assert!(lowest < shield_full, "the field took nothing");
    assert_eq!(
        health(&w, titan),
        full,
        "the walker itself was hurt through its field"
    );
}

fn fire_bore_at(w: &mut World, titan: UnitId, x: i32, y: i32) {
    let cmd = PlayerCommand {
        player: 0,
        command: Command::AttackGround {
            units: vec![titan],
            pos: FxVec2::from_ints(x, y),
            queue: false,
        },
    };
    w.tick(&[cmd]).unwrap();
}

#[test]
fn the_bore_storm_spreads_and_levels_a_base_over_seconds() {
    let mut w = world();
    w.state.players[0].free_build = true;
    let titan = add(&mut w, TITAN, 0, 1000, 3000, 0);
    // A block of enemy factories 1.8 km off, spread over 600 m.
    let factory = "aster_t3_land_factory";
    let mut base = Vec::new();
    for dx in [-240, 0, 240] {
        for dy in [-240, 0, 240] {
            base.push(add(&mut w, factory, 1, 2800 + dx, 3000 + dy, 0));
        }
    }
    fire_bore_at(&mut w, titan, 2800, 3000);
    let mut storm_seen = false;
    let mut first_dead = None;
    for t in 0..seconds(60) {
        w.tick(&[]).unwrap();
        storm_seen |= !w.state.storms.is_empty();
        let dead = base.iter().filter(|&&b| health(&w, b) <= 0.0).count();
        if dead > 0 && first_dead.is_none() {
            first_dead = Some(t);
        }
    }
    let dead = base.iter().filter(|&&b| health(&w, b) <= 0.0).count();
    assert!(storm_seen, "the bore never raised a storm");
    assert!(dead >= 7, "the storm levelled only {dead} of 9 factories");
}

#[test]
fn sabots_fall_burst_and_leave_scrap() {
    let mut w = world();
    let titan = add(&mut w, TITAN, 0, 1000, 3000, 0);
    let sabot_bp = w.blueprints.id_of("aster_t5_titan_sabot").unwrap();
    // Enemy tanks down range for the rail gatling, out of the bore's and rockets' way.
    for i in 0..10 {
        add(&mut w, TANK, 1, 2200, 2800 + i * 40, 180);
    }
    let mut landed = 0;
    for _ in 0..seconds(40) {
        w.tick(&[]).unwrap();
        landed += w
            .events
            .iter()
            .filter(|e| matches!(e, SimEvent::SabotLanded { .. }))
            .count();
    }
    let heaps: Vec<_> = w
        .state
        .wrecks
        .slots
        .iter()
        .filter(|&r| w.state.wrecks.blueprint[r] == sabot_bp)
        .collect();
    let scrap: f32 = heaps.iter().map(|&r| w.state.wrecks.mass[r].to_f32()).sum();
    assert!(landed >= 5, "only {landed} sabots came down");
    assert!(
        !heaps.is_empty() && scrap > 0.0,
        "no scrap lies where they fell"
    );
    // They land beside the walker's gun arm, not on top of the enemy.
    let r = row(&w, titan);
    let p = w.state.units.pos[r];
    for &h in &heaps {
        let d = w.state.wrecks.pos[h].distance(p).to_f32();
        assert!(d < 450.0, "a sabot heap lies {d} m from the walker");
    }
}

#[test]
fn it_brings_down_a_starship() {
    let mut w = world();
    w.state.players[0].free_build = true;
    let _titan = add(&mut w, TITAN, 0, 1000, 3000, 0);
    let frigate = add(&mut w, "aster_t3_frigate", 1, 3200, 3000, 180);
    let r = row(&w, frigate);
    w.state.units.fire_state[r] = FireState::HoldFire;
    let full = health(&w, frigate);
    for _ in 0..seconds(60) {
        w.tick(&[]).unwrap();
    }
    let left = health(&w, frigate);
    let z = w
        .state
        .units
        .row(frigate)
        .map_or(0.0, |r| w.state.units.z[r].to_f32());
    assert!(
        left < full * 0.5,
        "the frigate ({z} m up) only lost {} of {full}",
        full - left
    );
}

#[test]
fn the_rotary_cannon_fires_faster_as_it_spins_up() {
    let mut w = world();
    let titan = add(&mut w, TITAN, 0, 1000, 3000, 0);
    let gatling = w
        .blueprints
        .unit(w.blueprints.id_of(TITAN).unwrap())
        .weapons
        .iter()
        .position(|wp| wp.spin_ramp > 0)
        .expect("a ramping rotary gun");
    // A block of hardened targets in its band, well out of the bore's way (it holds fire).
    for i in 0..6 {
        add(&mut w, "aster_t3_land_factory", 1, 3000, 2700 + i * 120, 0);
    }
    let bp = w.blueprints.id_of(TITAN).unwrap();
    let mut shots = Vec::new();
    for t in 0..seconds(12) {
        w.tick(&[]).unwrap();
        let fired = w.events.iter().filter(|e| matches!(e,
            SimEvent::ShotFired { blueprint, weapon, .. } if *blueprint == bp && *weapon as usize == gatling)).count();
        shots.push((t, fired));
    }
    let _ = titan;
    let first = shots
        .iter()
        .position(|s| s.1 > 0)
        .expect("the cannon never fired");
    // A slow heavy beat: two shells a second at full spin, fewer as it spins up.
    let early: usize = shots[first..first + 20].iter().map(|s| s.1).sum();
    let late: usize = shots[first + 60..first + 80].iter().map(|s| s.1).sum();
    assert!(
        early < late,
        "no ramp: {early} shots in its first two seconds, {late} at full spin"
    );
    assert!(late >= 4, "only {late} shots in two seconds at full spin");
}

/// The Tempest fires as a barrel comes up to the top of its cluster: at full spin a
/// barrel is at the top on every shot, and every shot throws out exactly one casing.
#[test]
fn the_rotary_cannon_fires_off_the_top_barrel_one_casing_a_shot() {
    let mut w = world();
    let titan = add(&mut w, TITAN, 0, 1000, 3000, 0);
    let bp = w.blueprints.id_of(TITAN).unwrap();
    let weapons = &w.blueprints.unit(bp).weapons;
    let gatling = weapons
        .iter()
        .position(|wp| wp.barrels > 0)
        .expect("a barrel-timed rotary gun");
    let (barrels, full) = (weapons[gatling].barrels as i32, weapons[gatling].spin_ticks);
    for i in 0..6 {
        add(&mut w, "aster_t3_land_factory", 1, 3000, 2700 + i * 120, 0);
    }
    let spacing = 65536 / barrels;
    let (mut shots, mut casings, mut at_speed) = (0, 0, 0);
    for _ in 0..seconds(12) {
        w.tick(&[]).unwrap();
        let fired = w.events.iter().filter(|e| matches!(e,
            SimEvent::ShotFired { blueprint, weapon, .. } if *blueprint == bp && *weapon as usize == gatling)).count();
        shots += fired;
        casings += w
            .events
            .iter()
            .filter(|e| matches!(e, SimEvent::SabotThrown { .. }))
            .count();
        let [speed, turn, _] = w.state.units.spin[row(&w, titan)];
        if fired > 0 && speed == full {
            at_speed += 1;
            let phase = (turn as i32 - spacing / 2).rem_euclid(65536) % spacing;
            assert!(
                phase.min(spacing - phase) <= 4,
                "fired at full spin {phase} steps off the top barrel"
            );
        }
    }
    assert!(at_speed >= 12, "only {at_speed} shots at full spin");
    assert_eq!(shots, casings, "{shots} shots but {casings} casings");
}

/// Shots each of the Behemoth's weapons fires in 20 s on the ground at (3200, 3000), sent
/// there by the Strike button (`strike`) or by plain fire on ground.
fn ground_shots(strike: bool) -> (usize, Vec<usize>) {
    let mut w = world();
    let titan = add(&mut w, TITAN, 0, 1000, 3000, 0);
    let bp = w.blueprints.id_of(TITAN).unwrap();
    let weapons = &w.blueprints.unit(bp).weapons;
    let bore = weapons
        .iter()
        .position(|wp| wp.bore.is_some_and(|b| b.storm.is_some()))
        .unwrap();
    let (units, pos) = (vec![titan], FxVec2::from_ints(3200, 3000));
    let command = if strike {
        Command::Strike {
            units,
            pos,
            queue: false,
        }
    } else {
        Command::AttackGround {
            units,
            pos,
            queue: false,
        }
    };
    w.tick(&[PlayerCommand { player: 0, command }]).unwrap();
    let mut fired = vec![0usize; 16];
    for _ in 0..seconds(20) {
        w.tick(&[]).unwrap();
        for e in &w.events {
            if let SimEvent::ShotFired {
                blueprint, weapon, ..
            } = e
            {
                if *blueprint == bp {
                    fired[*weapon as usize] += 1;
                }
            }
        }
    }
    (bore, fired)
}

/// The Strike button calls the storm down with the bore alone: the rocket pods and the
/// gatling hold.
#[test]
fn a_strike_fires_the_bore_and_nothing_else() {
    let (bore, fired) = ground_shots(true);
    assert!(fired[bore] > 0, "the bore never fired on the ground");
    let others: usize = fired
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != bore)
        .map(|(_, n)| n)
        .sum();
    assert_eq!(others, 0, "other guns fired on a strike: {fired:?}");
}

/// Plain fire on ground shells the spot with every gun that reaches it, bore included.
#[test]
fn fire_on_ground_fires_every_gun_that_reaches() {
    let (bore, fired) = ground_shots(false);
    assert!(fired[bore] > 0, "the bore never fired on the ground");
    let others = fired
        .iter()
        .enumerate()
        .filter(|&(i, &n)| i != bore && n > 0)
        .count();
    assert!(others >= 2, "only the bore fired on the ground: {fired:?}");
}

/// Each arm swings a little off the torso on its own (`sway`): with marks on either side
/// of it, the gatling and the bore lay on their own, never further than their sway.
#[test]
fn the_arms_aim_a_little_apart() {
    let mut w = world();
    let titan = add(&mut w, TITAN, 0, 1000, 3000, 0);
    let bp = w.blueprints.id_of(TITAN).unwrap();
    let weapons = w.blueprints.unit(bp).weapons.clone();
    let gatling = weapons.iter().position(|wp| wp.barrels > 0).unwrap();
    let bore = weapons
        .iter()
        .position(|wp| wp.bore.is_some_and(|b| b.storm.is_some()))
        .unwrap();
    // Two hardened marks some 10 degrees apart, too tough to fall in the window: one
    // inside the bore's least range (the Tempest's and the pods'), one only the bore takes,
    // so each arm has its own and neither can simply follow the other onto one mark.
    for (x, y) in [(2056, 3092), (2992, 2826)] {
        let mark = add(&mut w, "aster_t3_land_factory", 1, x, y, 0);
        let r = row(&w, mark);
        w.state.units.health[r] = Fx::from_int(10_000_000);
    }
    let mut apart = 0;
    for _ in 0..seconds(15) {
        w.tick(&[]).unwrap();
        let yaw = w.state.units.weapon_yaw[row(&w, titan)];
        for arm in [gatling, bore] {
            let off = yaw[0].delta_to(yaw[arm]).unsigned_abs();
            assert!(
                off <= weapons[arm].sway.0 + 1,
                "arm {arm} swung {off} steps off the torso"
            );
        }
        if yaw[gatling].delta_to(yaw[bore]).unsigned_abs() > Angle::from_degrees(2).0 {
            apart += 1;
        }
    }
    assert!(apart > 0, "the arms never laid apart");
}

/// The rocket pods loft their rockets: they climb well over the pods on the way out.
#[test]
fn the_rockets_fly_an_arc() {
    let mut w = world();
    add(&mut w, TITAN, 0, 1000, 3000, 0);
    let bp = w.blueprints.id_of(TITAN).unwrap();
    let pods = w
        .blueprints
        .unit(bp)
        .weapons
        .iter()
        .position(|wp| wp.missile)
        .unwrap();
    add(&mut w, "aster_t3_land_factory", 1, 3000, 3000, 0);
    let mut top = Fx::ZERO;
    for _ in 0..seconds(30) {
        w.tick(&[]).unwrap();
        let p = &w.state.projectiles;
        for i in 0..p.len() {
            if p.blueprint[i] == bp && p.weapon[i] as usize == pods {
                top = top.max(p.pos[i].z);
            }
        }
    }
    // The pods stand some 480 m up (20 m ground); a flat shot at 2 km never climbs over them.
    assert!(
        top > Fx::from_int(700),
        "rockets topped out at {} m",
        top.to_f32()
    );
}

/// Its air defence is two twin flak turrets on the shoulders, riding the torso: they
/// fire on a gunship overhead, and there are only the two.
#[test]
fn two_shoulder_flak_guns_fire_on_aircraft() {
    let mut w = world();
    add(&mut w, TITAN, 0, 1000, 3000, 0);
    let bp = w.blueprints.id_of(TITAN).unwrap();
    let weapons = &w.blueprints.unit(bp).weapons;
    let flak: Vec<usize> = (0..weapons.len())
        .filter(|&i| weapons[i].target_mask == mc_data::cat::AIR)
        .collect();
    assert_eq!(flak.len(), 2, "not two flak guns");
    assert!(
        flak.iter().all(|&i| weapons[i].mount),
        "the flak turn on houses of their own"
    );
    let gunship = add(&mut w, "aster_t2_gunship", 1, 1300, 3100, 180);
    let r = row(&w, gunship);
    w.state.units.flags[r] |= mc_sim::tables::flag::PASSIVE;
    let mut shots = 0;
    for _ in 0..seconds(10) {
        w.tick(&[]).unwrap();
        shots += w.events.iter().filter(|e| matches!(e,
            SimEvent::ShotFired { blueprint, weapon, .. } if *blueprint == bp && flak.contains(&(*weapon as usize)))).count();
    }
    assert!(shots > 0, "the flak never fired on the gunship");
}

/// The flak ride the torso: with it swung round onto a mark off the side, each shell still
/// leaves from its own shoulder, and the barrel lies along the shell's path.
#[test]
fn shoulder_flak_fire_from_a_turned_torso() {
    let mut w = world();
    let titan = add(&mut w, TITAN, 0, 1000, 3000, 0);
    // A tank off the left side turns the torso; a gunship behind that shoulder draws flak.
    let tank = add(&mut w, TANK, 1, 1000, 5000, 180);
    hold_fire(&mut w, tank);
    let gunship = add(&mut w, "aster_t2_gunship", 1, 600, 3600, 180);
    let r = row(&w, gunship);
    w.state.units.flags[r] |= mc_sim::tables::flag::PASSIVE;
    let bp = w.blueprints.id_of(TITAN).unwrap();
    let weapons = w.blueprints.unit(bp).weapons.clone();
    let mut shots = 0;
    for _ in 0..seconds(20) {
        // Both kept alive: the tank holds the torso round, the gunship draws the flak.
        for id in [tank, gunship] {
            let r = row(&w, id);
            w.state.units.health[r] = Fx::from_int(1_000_000);
        }
        w.tick(&[]).unwrap();
        let t = row(&w, titan);
        let units = &w.state.units;
        let torso = units.heading[t] + units.weapon_yaw[t][0];
        if units.heading[t].delta_to(torso).unsigned_abs() < Angle::from_degrees(60).0 {
            continue;
        }
        for e in &w.events {
            let SimEvent::ShotFired {
                pos,
                vel,
                blueprint,
                weapon,
                ..
            } = e
            else {
                continue;
            };
            let gun = &weapons[*weapon as usize];
            if *blueprint != bp || !gun.mount {
                continue;
            }
            let trunnion = units.pos[t] + gun.pivot.unwrap().xy().rotate(torso);
            assert!(
                pos.xy().distance(trunnion) < Fx::from_int(45),
                "flak {weapon} fired {} m from its shoulder",
                pos.xy().distance(trunnion).to_f32()
            );
            let barrel = units.heading[t] + units.weapon_yaw[t][*weapon as usize];
            let off = barrel.delta_to(vel.xy().angle()).unsigned_abs();
            assert!(
                off < Angle::from_degrees(6).0,
                "flak {weapon} laid {} degrees off its shell",
                off as f32 * 360.0 / 65536.0
            );
            shots += 1;
        }
    }
    assert!(shots > 0, "the flak never fired with the torso turned");
}

/// When the bore's mark dies part way through a charge, it swings onto another and says
/// where it will now land (`StormRetargeted`), so the strike warning follows it rather than
/// waiting for the bolt.
#[test]
fn a_charge_that_loses_its_mark_says_where_it_lands_now() {
    let mut w = world();
    let titan = add(&mut w, TITAN, 0, 1000, 3000, 0);
    let first = add(&mut w, TANK, 1, 3000, 3000, 180);
    let second = add(&mut w, TANK, 1, 3000, 3700, 180);
    for id in [first, second] {
        hold_fire(&mut w, id);
    }
    // Wait for the charge to begin, and note which tank it is on.
    let mut on = None;
    for _ in 0..seconds(20) {
        w.tick(&[]).unwrap();
        on = w.events.iter().find_map(|e| match e {
            SimEvent::StormCharging { target, .. } => Some(target.xy()),
            _ => None,
        });
        if on.is_some() {
            break;
        }
    }
    let on = on.expect("the bore never began to charge");
    let (marked, other) = if on.distance(FxVec2::from_ints(3000, 3000)) < Fx::from_int(50) {
        (first, second)
    } else {
        (second, first)
    };
    w.tick(&[]).unwrap();
    let r = row(&w, marked);
    w.state.units.health[r] = Fx::ZERO;
    let mut moved = None;
    for _ in 0..seconds(3) {
        w.tick(&[]).unwrap();
        for e in &w.events {
            match e {
                SimEvent::StormRetargeted { unit, target, .. } if *unit == titan => {
                    moved = *target;
                }
                SimEvent::BoreDischarge { .. } => panic!("the bolt landed before the mark moved"),
                _ => {}
            }
        }
        if moved.is_some() {
            break;
        }
    }
    let moved = moved.expect("the charge swung onto nothing it told of");
    let there = w.state.units.pos[row(&w, other)];
    assert!(
        moved.xy().distance(there) < Fx::from_int(5),
        "the mark moved to {:?}, not the other tank at {there:?}",
        moved.xy()
    );
}
