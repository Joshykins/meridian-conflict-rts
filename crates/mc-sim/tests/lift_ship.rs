//! Lift ships (`transport.rs`): raised on site by engineers, keep to the clouds, set down,
//! take land units up the ramp and let them out again; their guns only reach the ground
//! once they come down.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller, OrderKind};
use mc_sim::world::MapData;
use mc_sim::{Command, Handle, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

const SHIP: &str = "aster_t2_lift_ship";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(512, 512, Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(3500, 3500)],
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
        seed: 3,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

fn add(w: &mut World, key: &str, owner: u8, x: i32, y: i32) -> usize {
    let id = w.blueprints.id_of(key).unwrap();
    w.spawn_unit(id, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap()
}

fn height(w: &World, row: usize) -> i32 {
    (w.state.units.z[row] - Fx::from_int(20)).round_int()
}

fn run(w: &mut World, ticks: usize) {
    for _ in 0..ticks {
        w.tick(&[]).unwrap();
    }
}

/// Ticks until `done` holds, at most `limit`; how many it took.
fn run_until(w: &mut World, limit: usize, done: impl Fn(&World) -> bool) -> Option<usize> {
    for t in 0..limit {
        if done(w) {
            return Some(t);
        }
        w.tick(&[]).unwrap();
    }
    done(w).then_some(limit)
}

#[test]
fn engineers_raise_it_on_a_lot_and_it_stays_down_with_the_ramp_open() {
    let mut w = world();
    let bp = w.blueprints.id_of(SHIP).unwrap();
    assert!(w.blueprints.unit(bp).is_site_built_unit());
    let engineer = add(&mut w, "aster_t2_engineer", 0, 1000, 1000);
    let id = w.state.units.id(engineer);
    w.tick(&[cmd(Command::Build {
        units: vec![id],
        blueprint: bp,
        pos: FxVec2::from_ints(1100, 1000),
        heading: Angle::from_degrees(270),
        queue: false,
    })])
    .unwrap();
    let site = run_until(&mut w, 600, |w| {
        w.state
            .units
            .slots
            .iter()
            .any(|r| w.state.units.blueprint[r] == bp)
    });
    assert!(site.is_some(), "the engineer never began the site");
    let ship = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == bp)
        .unwrap();
    assert!(w.state.units.has_flag(ship, flag::UNDER_CONSTRUCTION));
    assert_eq!(height(&w, ship), 0, "the site stands on the ground");
    // Whatever heading the order carried, the hull lies along its 28 x 13 lot.
    assert_eq!(w.state.units.heading[ship], Angle::ZERO, "the ship lies along its lot");
    let ship_id = w.state.units.id(ship);
    w.tick(&[cmd(Command::DebugFreeBuild {
        player: 0,
        on: true,
    })])
    .unwrap();
    w.tick(&[cmd(Command::DebugSetBuild {
        units: vec![ship_id],
        permille: 999,
    })])
    .unwrap();
    let done = run_until(&mut w, 400, |w| {
        !w.state.units.has_flag(ship, flag::UNDER_CONSTRUCTION)
    });
    assert!(done.is_some(), "the ship was never finished");
    run(&mut w, 60);
    assert_eq!(
        height(&w, ship),
        0,
        "a finished ship with no orders stays down"
    );
    let need = w.blueprints.unit(bp).motion.unwrap().deploy_ticks;
    assert_eq!(w.state.units.deploy[ship], need, "and lowers its ramp");
}

#[test]
fn it_keeps_to_the_clouds_and_its_guns_only_reach_the_ground_on_the_way_down() {
    let mut w = world();
    let ship = add(&mut w, SHIP, 0, 1500, 1500);
    // An enemy tank 200 m off: in reach across the map, out of it along the line of sight.
    let tank = add(&mut w, "aster_t1_tank", 1, 1700, 1500);
    run(&mut w, 60);
    assert!(
        height(&w, ship) >= 550,
        "cruises in the clouds: {} m",
        height(&w, ship)
    );
    assert!(
        !w.state.units.weapon_target[ship]
            .iter()
            .any(|&t| t == w.state.units.id(tank)),
        "its guns reached the ground from {} m",
        height(&w, ship)
    );
    let full = w.state.units.health[tank];
    assert_eq!(w.state.units.health[tank], full);
    // Told to set down where it is, it starts shooting on the way down.
    let ship_id = w.state.units.id(ship);
    w.tick(&[cmd(Command::Land {
        units: vec![ship_id],
        pos: FxVec2::from_ints(1500, 1500),
        unload: false,
        queue: false,
    })])
    .unwrap();
    let tank_id = w.state.units.id(tank);
    let first_shot = run_until(&mut w, 600, |w| {
        w.state
            .units
            .row(tank_id)
            .is_none_or(|t| w.state.units.health[t] < full)
    })
    .expect("its guns never reached the tank");
    let at = height(&w, ship);
    assert!(
        at > 20,
        "it only fired once down ({at} m after {first_shot} ticks)"
    );
    assert!(
        at < 280,
        "it fired from {at} m, beyond reach along the line of sight"
    );
    let down = run_until(&mut w, 900, |w| height(w, ship) == 0);
    assert!(down.is_some(), "never set down");
}

#[test]
fn land_units_walk_up_the_ramp_ride_along_and_walk_out() {
    let mut w = world();
    let ship = add(&mut w, SHIP, 0, 1500, 1500);
    let ship_id = w.state.units.id(ship);
    let cargo: Vec<usize> = (0..4)
        .map(|i| add(&mut w, "aster_t1_tank", 0, 1350 + i * 12, 1400))
        .collect();
    let ids: Vec<Handle> = cargo.iter().map(|&r| w.state.units.id(r)).collect();
    run(&mut w, 30);
    w.tick(&[cmd(Command::Board {
        units: ids.clone(),
        carrier: ship_id,
        queue: false,
    })])
    .unwrap();
    let boarded = run_until(&mut w, 3000, |w| {
        cargo.iter().all(|&r| w.state.units.hangar[r] == ship_id)
    });
    assert!(
        boarded.is_some(),
        "not all boarded: ship at {} m, deploy {}, cargo {:?}",
        height(&w, ship),
        w.state.units.deploy[ship],
        cargo
            .iter()
            .map(|&r| (w.state.units.pos[r], w.state.units.hangar[r] == ship_id))
            .collect::<Vec<_>>()
    );
    for &r in &cargo {
        assert!(w.state.units.has_flag(r, flag::IN_FACTORY));
    }
    let room = w
        .blueprints
        .unit(w.state.units.blueprint[cargo[0]])
        .cargo_room()
        .unwrap();
    assert_eq!(w.cargo_used(ship), room * 4);

    // Off it goes, and lets them out a long way off.
    let there = FxVec2::from_ints(2600, 2400);
    w.tick(&[cmd(Command::Land {
        units: vec![ship_id],
        pos: there,
        unload: true,
        queue: false,
    })])
    .unwrap();
    run(&mut w, 100);
    assert!(height(&w, ship) > 30, "it lifted off with the hold full");
    // The hold rides along (pinned before the ship's own move, so a tick behind it).
    let hold = w.state.units.pos[ship]
        + w.blueprints
            .unit(w.state.units.blueprint[ship])
            .transport
            .unwrap()
            .hold
            .rotate(w.state.units.heading[ship]);
    for &r in &cargo {
        assert!(w.state.units.pos[r].distance(hold) < w.blueprints.unit(w.state.units.blueprint[ship]).motion.unwrap().speed / 10 + Fx::from_int(2));
    }
    let out = run_until(&mut w, 4000, |w| {
        cargo
            .iter()
            .all(|&r| w.state.units.hangar[r] == Handle::NONE)
    });
    assert!(out.is_some(), "never let them out");
    assert!(w.state.units.pos[ship].distance(there) < Fx::from_int(4));
    // They walk off down the ramp, behind the ship.
    run(&mut w, 200);
    for &r in &cargo {
        assert!(!w.state.units.has_flag(r, flag::IN_FACTORY));
        let off = w.state.units.pos[r] - w.state.units.pos[ship];
        let back = -off.dot(FxVec2::from_angle(w.state.units.heading[ship]));
        assert!(
            back > Fx::from_int(50),
            "a tank stands {back} m behind the ship, not off its ramp"
        );
    }
    assert!(w
        .state
        .orders
        .front(&w.state.units, ship)
        .is_none_or(|o| o.kind != OrderKind::Unload));
}

#[test]
fn what_is_in_the_hold_dies_with_the_ship() {
    let mut w = world();
    let ship = add(&mut w, SHIP, 0, 1500, 1500);
    let ship_id = w.state.units.id(ship);
    let tank = add(&mut w, "aster_t1_tank", 0, 1440, 1500);
    let tank_id = w.state.units.id(tank);
    w.tick(&[cmd(Command::Board {
        units: vec![tank_id],
        carrier: ship_id,
        queue: false,
    })])
    .unwrap();
    assert!(run_until(&mut w, 3000, |w| w.state.units.hangar[tank] == ship_id).is_some());
    w.tick(&[cmd(Command::DebugDamage {
        units: vec![ship_id],
        permille: 1000,
    })])
    .unwrap();
    run(&mut w, 5);
    assert!(w.state.units.row(ship_id).is_none());
    assert!(
        w.state.units.row(tank_id).is_none(),
        "the tank outlived its ship"
    );
    // Its wreck holds the tank's salvage as well as its own.
    let bp = |k: &str| w.blueprints.unit(w.blueprints.id_of(k).unwrap());
    let want = bp(SHIP).cost_mass * bp(SHIP).wreck_fraction
        + bp("aster_t1_tank").cost_mass * bp("aster_t1_tank").wreck_fraction;
    let wreck = w.state.wrecks.slots.iter().map(|r| w.state.wrecks.mass[r]).max();
    assert_eq!(wreck, Some(want), "the wreck holds the hold's mass too");
}

#[test]
fn a_dead_lift_ship_comes_down_slowly_without_tumbling() {
    let mut w = world();
    let ship = add(&mut w, SHIP, 0, 1500, 1500);
    let ship_id = w.state.units.id(ship);
    // Up at cruise height.
    assert!(run_until(&mut w, 900, |w| height(w, ship) > 500).is_some());
    w.tick(&[cmd(Command::DebugDamage { units: vec![ship_id], permille: 1000 })]).unwrap();
    run(&mut w, 2);
    assert!(w.state.units.row(ship_id).is_none());
    let fell = run_until(&mut w, 2000, |w| w.state.aircraft_crashes.is_empty()).unwrap();
    // Light aircraft drop from this height in under 7 s; a capital hull takes far longer.
    assert!(fell > 120, "it hit the ground after {fell} ticks");
}

#[test]
fn bastion_carries_the_complete_t3_land_roster_with_physical_clearance() {
    let mut w = world();
    let ship = add(&mut w, SHIP, 0, 1800, 1800);
    let ship_id = w.state.units.id(ship);
    let t = w
        .blueprints
        .unit(w.state.units.blueprint[ship])
        .transport
        .unwrap();
    assert_eq!(t.capacity, 96);
    let keys: Vec<_> = w
        .blueprints
        .units
        .iter()
        .filter(|bp| bp.tech == 3 && bp.cargo_room().is_some())
        // Refit variants are alternate loadouts of one commander, not an army of commanders.
        .filter(|bp| !bp.has(mc_data::cat::COMMANDER) || bp.key == "aster_commander+eng_3")
        .map(|bp| bp.key.clone())
        .collect();
    assert!(keys.iter().any(|k| k == "aster_t3_assault_bot"));
    let mut cargo = Vec::new();
    for (i, key) in keys.iter().enumerate() {
        let bp = w.blueprints.unit(w.blueprints.id_of(key).unwrap());
        assert!(
            bp.radius * 2 <= t.width && bp.height <= t.clearance,
            "{key} clips the hangar"
        );
        cargo.push(add(&mut w, key, 0, 1450 - i as i32 * 45, 1800));
    }
    let ids = cargo.iter().map(|&r| w.state.units.id(r)).collect();
    w.tick(&[cmd(Command::Board {
        units: ids,
        carrier: ship_id,
        queue: false,
    })])
    .unwrap();
    assert!(
        run_until(&mut w, 5000, |w| cargo.iter().all(|&r| w
            .state
            .units
            .hangar[r]
            == ship_id))
        .is_some(),
        "T3 cargo failed to board: {:?}",
        keys.iter()
            .zip(&cargo)
            .map(|(k, &r)| (k, w.state.units.hangar[r], w.state.units.pos[r]))
            .collect::<Vec<_>>()
    );
    w.tick(&[cmd(Command::Land {
        units: vec![ship_id],
        pos: FxVec2::from_ints(2600, 2400),
        unload: true,
        queue: false,
    })])
    .unwrap();
    assert!(
        run_until(&mut w, 5000, |w| cargo.iter().all(|&r| w
            .state
            .units
            .hangar[r]
            == Handle::NONE))
        .is_some(),
        "T3 cargo did not disembark"
    );
}

#[test]
fn ventral_entry_stays_under_the_hull_and_climbs_into_the_hold() {
    let mut w=world();
    let ship=add(&mut w,SHIP,0,1800,1800);
    let id=w.state.units.id(ship);
    let bp=w.blueprints.unit(w.state.units.blueprint[ship]);
    let t=bp.transport.unwrap();
    assert!(t.lip > -bp.hull.0 && t.hinge > t.lip,"entry must be under the hull");
    assert!(t.floor >= Fx::from_int(30),"vehicles need clearance under the hull");
    w.tick(&[cmd(Command::Land{units:vec![id],pos:FxVec2::from_ints(1800,1800),unload:false,queue:false})]).unwrap();
    assert!(run_until(&mut w,1200,|w|height(w,ship)==0).is_some());
    run(&mut w,60);
    let decks=w.lift_decks();
    let deck=decks.first().unwrap();
    let point=|x:Fx|[1800.0+x.to_f32(),1800.0];
    assert!(deck.lift(point(t.lip)).abs()<0.05);
    assert!((deck.lift(point((t.lip+t.hinge)/2))-t.floor.to_f32()/2.0).abs()<0.1);
    assert!((deck.lift(point(t.hinge))-t.floor.to_f32()).abs()<0.1);
}

#[test]
fn capital_ship_accelerates_climbs_with_pitch_and_levels_for_touchdown() {
    let mut w=world();
    let ship=add(&mut w,SHIP,0,1600,1600);
    let id=w.state.units.id(ship);
    w.tick(&[cmd(Command::Land{units:vec![id],pos:FxVec2::from_ints(1600,1600),unload:false,queue:false})]).unwrap();
    assert!(run_until(&mut w,1200,|w|height(w,ship)==0).is_some());
    run(&mut w,50);
    w.tick(&[cmd(Command::Land{units:vec![id],pos:FxVec2::from_ints(3100,2300),unload:false,queue:false})]).unwrap();
    let mut fastest=Fx::ZERO;
    let mut nose_up=0i32;
    let mut previous=Angle::ZERO.delta_to(w.state.units.arm_pitch[ship][0]) as i32;
    for _ in 0..1500 {
        w.tick(&[]).unwrap();
        fastest=fastest.max(w.state.units.speed[ship]);
        let pitch=Angle::ZERO.delta_to(w.state.units.arm_pitch[ship][0]) as i32;
        assert!((pitch-previous).abs()<=85,"pitch snapped from {previous} to {pitch}");
        previous=pitch;
        nose_up=nose_up.max(pitch);
    }
    assert!(fastest>Fx::from_int(65),"too slow: {fastest}");
    assert!(nose_up>500,"never pitched into climb: {nose_up}");
    assert_eq!(height(&w,ship),0);
    assert!(previous.abs()<30,"did not level for landing: {previous}");
}


#[test]
fn courier_is_an_unarmed_t1_site_built_transport_with_eight_slots() {
    let w = world();
    let id = w.blueprints.id_of("aster_t1_lift_ship").unwrap();
    let bp = w.blueprints.unit(id);
    let t = bp.transport.unwrap();
    assert_eq!(bp.tech, 1);
    assert_eq!(t.capacity, 8);
    assert!(bp.weapons.is_empty());
    assert!(bp.shield.is_none());
    assert!(bp.is_site_built_unit());
    assert_eq!(t.floor, Fx::ZERO);
    assert!(bp.motion.unwrap().deploy_ticks > 0, "stern doors need time to open");
    assert!(bp.motion.unwrap().speed >= Fx::from_int(140));
    for tier in 1..=3 {
        let factory = w.blueprints.unit(w.blueprints.id_of(&format!("aster_t{tier}_air_factory")).unwrap());
        assert!(!factory.builder.as_ref().unwrap().builds.contains(&id), "Courier must not be factory built");
        let engineer = w.blueprints.unit(w.blueprints.id_of(&format!("aster_t{tier}_engineer")).unwrap());
        assert!(engineer.builder.as_ref().unwrap().builds.contains(&id));
    }
}

#[test]
fn courier_loads_exactly_eight_slots_flies_fast_and_unloads_without_a_ramp() {
    let mut w = world();
    let ship = add(&mut w, "aster_t1_lift_ship", 0, 1600, 1600);
    let id = w.state.units.id(ship);
    // Eight light tanks fill eight slots; the ninth must stay behind.
    let cargo: Vec<_> = (0..9).map(|i| add(&mut w, "aster_t1_tank", 0, 1450-i*30, 1600)).collect();
    assert_eq!(w.blueprints.unit(w.state.units.blueprint[cargo[0]]).cargo_room(), Some(1));
    let ids = cargo.iter().map(|&r| w.state.units.id(r)).collect();
    w.tick(&[cmd(Command::Board { units: ids, carrier: id, queue: false })]).unwrap();
    assert!(run_until(&mut w, 2500, |w| w.cargo_used(ship) == 8).is_some(), "failed to fill eight slots: {:?}", cargo.iter().map(|&r|(w.state.units.pos[r],w.state.units.hangar[r])).collect::<Vec<_>>());
    run(&mut w, 150);
    assert_eq!(w.cargo_used(ship), 8);
    let aboard: Vec<_> = cargo.iter().copied().filter(|&r| w.state.units.hangar[r] == id).collect();
    assert_eq!(aboard.len(), 8);
    assert_eq!(height(&w, ship), 0);
    assert_eq!(w.state.units.deploy[ship], w.blueprints.unit(w.state.units.blueprint[ship]).motion.unwrap().deploy_ticks);
    let deck = w.lift_decks().into_iter().find(|d| d.pos == [1600.0,1600.0]).unwrap();
    assert_eq!(deck.lift([1587.0,1600.0]), 0.0, "boarding must stay at ground level");
    let there = FxVec2::from_ints(3000, 1600);
    w.tick(&[cmd(Command::Land { units: vec![id], pos: there, unload: true, queue: false })]).unwrap();
    let mut fastest = Fx::ZERO;
    let mut airborne = false;
    let mut finished = false;
    for _ in 0..2500 {
        w.tick(&[]).unwrap();
        fastest = fastest.max(w.state.units.speed[ship]);
        airborne |= height(&w, ship) > 100;
        assert!(w.cargo_used(ship) <= 8);
        for &r in &aboard {
            if w.state.units.hangar[r] == Handle::NONE {
                let local = (w.state.units.pos[r] - w.state.units.pos[ship])
                    .rotate(Angle(w.state.units.heading[ship].0.wrapping_neg()));
                if local.x > Fx::from_int(-42) {
                    assert!(local.y.abs() + w.blueprints.unit(w.state.units.blueprint[r]).radius < Fx::from_int(14),
                        "cargo cut through a bay wall: {local:?}");
                }
            }
        }
        if w.cargo_used(ship) == 0 { finished = true; break; }
    }
    assert!(airborne, "never climbed to cruise");
    assert!(fastest > Fx::from_int(135), "too slow: {fastest}");
    assert!(finished, "failed to unload");
    assert!(w.state.units.pos[ship].distance(there) < Fx::from_int(4));
    run(&mut w, 200);
    for r in aboard {
        assert!(!w.state.units.has_flag(r, flag::IN_FACTORY));
        assert_eq!(w.state.units.hangar[r], Handle::NONE);
        assert!(w.state.units.pos[r].distance(there) > Fx::from_int(20), "cargo blocked under ship");
    }
}


#[test]
fn courier_carries_a_commander_as_its_full_eight_slot_load() {
    let mut w = world();
    let ship = add(&mut w, "aster_t1_lift_ship", 0, 1600, 1600);
    let id = w.state.units.id(ship);
    let commander = add(&mut w, "aster_commander", 0, 1500, 1600);
    let cid = w.state.units.id(commander);
    let cbp = w.blueprints.unit(w.state.units.blueprint[commander]);
    let t = w.blueprints.unit(w.state.units.blueprint[ship]).transport.unwrap();
    assert_eq!(cbp.cargo_room(), Some(8));
    assert!(cbp.radius * 2 <= t.width && cbp.height <= t.clearance);
    w.tick(&[cmd(Command::Board { units: vec![cid], carrier: id, queue: false })]).unwrap();
    assert!(run_until(&mut w, 2000, |w| w.state.units.hangar[commander] == id).is_some());
    assert_eq!(w.cargo_used(ship), 8);
    let tank = add(&mut w, "aster_t1_tank", 0, 1540, 1600);
    let tid = w.state.units.id(tank);
    w.tick(&[cmd(Command::Board { units: vec![tid], carrier: id, queue: false })]).unwrap();
    run(&mut w, 100);
    assert_eq!(w.state.units.hangar[tank], Handle::NONE);
    w.tick(&[cmd(Command::Land { units: vec![id], pos: FxVec2::from_ints(2600,1600), unload: true, queue: false })]).unwrap();
    assert!(run_until(&mut w, 2500, |w| w.state.units.hangar[commander] == Handle::NONE).is_some());
    run(&mut w, 200);
    assert!(w.state.units.pos[commander].distance(FxVec2::from_ints(2600,1600)) > Fx::from_int(20));
}

#[test]
fn t1_engineer_builds_courier_on_site_ready_to_load() {
    let mut w = world();
    let bp = w.blueprints.id_of("aster_t1_lift_ship").unwrap();
    assert!(w.blueprints.unit(bp).is_site_built_unit());
    let engineer = add(&mut w, "aster_t1_engineer", 0, 1000, 1000);
    let id = w.state.units.id(engineer);
    w.tick(&[cmd(Command::Build {
        units: vec![id],
        blueprint: bp,
        pos: FxVec2::from_ints(1100, 1000),
        heading: Angle::ZERO,
        queue: false,
    })])
    .unwrap();
    let site = run_until(&mut w, 600, |w| {
        w.state
            .units
            .slots
            .iter()
            .any(|r| w.state.units.blueprint[r] == bp)
    });
    assert!(site.is_some(), "the engineer never began the site");
    let ship = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == bp)
        .unwrap();
    assert!(w.state.units.has_flag(ship, flag::UNDER_CONSTRUCTION));
    assert_eq!(height(&w, ship), 0, "the site stands on the ground");
    let ship_id = w.state.units.id(ship);
    w.tick(&[cmd(Command::DebugFreeBuild {
        player: 0,
        on: true,
    })])
    .unwrap();
    w.tick(&[cmd(Command::DebugSetBuild {
        units: vec![ship_id],
        permille: 999,
    })])
    .unwrap();
    let done = run_until(&mut w, 400, |w| {
        !w.state.units.has_flag(ship, flag::UNDER_CONSTRUCTION)
    });
    assert!(done.is_some(), "the ship was never finished");
    run(&mut w, 60);
    assert_eq!(
        height(&w, ship),
        0,
        "a finished ship with no orders stays down"
    );
    let need = w.blueprints.unit(bp).motion.unwrap().deploy_ticks;
    assert_eq!(w.state.units.deploy[ship], need, "stern doors open before boarding");
}


#[test]
fn courier_routes_commander_around_hull_and_preserves_orders_after_unloading() {
    let mut w=world();
    let ship=add(&mut w,"aster_t1_lift_ship",0,1600,1600);
    let sid=w.state.units.id(ship);
    let passenger=add(&mut w,"aster_commander",0,1740,1600);
    let cid=w.state.units.id(passenger);
    w.tick(&[cmd(Command::Board{units:vec![cid],carrier:sid,queue:false})]).unwrap();
    let mut went_around=false;
    let mut entered_stern=false;
    for _ in 0..2500 {
        w.tick(&[]).unwrap();
        let local=w.state.units.pos[passenger]-w.state.units.pos[ship];
        went_around |= local.y.abs()>Fx::from_int(45);
        entered_stern |= local.x<Fx::from_int(-55) && local.y.abs()<Fx::from_int(5);
        if w.state.units.hangar[passenger]==sid {break;}
    }
    assert!(went_around && entered_stern,"commander bypassed rear entrance: around={went_around}, rear={entered_stern}, pos={:?}, hangar={:?}",w.state.units.pos[passenger],w.state.units.hangar[passenger]);
    assert_eq!(w.state.units.hangar[passenger],sid);
    let destination=FxVec2::from_ints(1800,1800);
    w.tick(&[cmd(Command::Move{units:vec![cid],target:destination,queue:false})]).unwrap();
    w.tick(&[cmd(Command::Land{units:vec![sid],pos:FxVec2::from_ints(1600,1600),unload:true,queue:false})]).unwrap();
    let mut cleared_stern=false;
    for _ in 0..2500 {
        w.tick(&[]).unwrap();
        let local=w.state.units.pos[passenger]-w.state.units.pos[ship];
        if local.x<Fx::from_int(-60) {cleared_stern=true;}
        if w.state.units.pos[passenger].distance(destination)<Fx::from_int(15) {break;}
    }
    assert!(cleared_stern,"queued order cut through side wall");
    assert!(w.state.units.pos[passenger].distance(destination)<Fx::from_int(15),"lost queued order");
}

/// Local position of `row` in the frame of `ship` (x along its heading).
fn local_of(w: &World, ship: usize, row: usize) -> FxVec2 {
    (w.state.units.pos[row] - w.state.units.pos[ship])
        .rotate(Angle(w.state.units.heading[ship].0.wrapping_neg()))
}

#[test]
fn it_glides_down_onto_a_distant_site_rather_than_stopping_and_dropping() {
    let mut w = world();
    let ship = add(&mut w, SHIP, 0, 800, 1600);
    let id = w.state.units.id(ship);
    run(&mut w, 60);
    assert!(height(&w, ship) >= 550);
    let site = FxVec2::from_ints(3000, 1600);
    w.tick(&[cmd(Command::Land { units: vec![id], pos: site, unload: false, queue: false })]).unwrap();
    let descent = w.blueprints.unit(w.state.units.blueprint[ship]).transport.unwrap().descent;
    let mut last = w.state.units.z[ship];
    let mut began = None;
    let mut near_height = None;
    let mut fastest_drop = Fx::ZERO;
    for t in 0..3000 {
        w.tick(&[]).unwrap();
        let z = w.state.units.z[ship];
        let d = w.state.units.pos[ship].distance(site);
        assert!(z <= last + Fx::ratio(1, 20), "climbed on the way in at tick {t}: {last} -> {z}, {d} m out");
        fastest_drop = fastest_drop.max(last - z);
        if began.is_none() && z < last - Fx::ONE {
            began = Some(d);
        }
        if near_height.is_none() && d < Fx::from_int(5) {
            near_height = Some(height(&w, ship));
        }
        last = z;
        if height(&w, ship) == 0 && w.state.units.deploy[ship] > 0 {
            break;
        }
    }
    let began = began.expect("never came down");
    assert!(began > Fx::from_int(600), "began its descent only {began} m out");
    let near = near_height.expect("never reached the site");
    assert!(near < 30, "was still {near} m up within 5 m of the site: it stopped, then dropped");
    assert!(fastest_drop <= descent / 10 + Fx::ratio(1, 100), "came down faster than its descent rate: {fastest_drop} m/tick");
    assert!(w.state.units.pos[ship].distance(site) < Fx::from_int(4));
    assert_eq!(height(&w, ship), 0);
}

#[test]
fn it_settles_softly_where_it_stands_and_lifts_off_before_moving_away() {
    let mut w = world();
    let ship = add(&mut w, SHIP, 0, 1600, 1600);
    let id = w.state.units.id(ship);
    run(&mut w, 60);
    w.tick(&[cmd(Command::Land { units: vec![id], pos: FxVec2::from_ints(1600, 1600), unload: false, queue: false })]).unwrap();
    // Ease in: the first second is slow, and no tick's change of speed is a jolt.
    let mut speeds = Vec::new();
    let mut last = w.state.units.z[ship];
    for _ in 0..1500 {
        w.tick(&[]).unwrap();
        let z = w.state.units.z[ship];
        speeds.push(last - z);
        last = z;
        if height(&w, ship) == 0 {
            break;
        }
    }
    assert_eq!(height(&w, ship), 0, "never set down");
    assert!(speeds[5] < Fx::ONE, "dropped away at once: {} m in a tick", speeds[5]);
    for pair in speeds.windows(2) {
        assert!((pair[1] - pair[0]).abs() <= Fx::ratio(6, 100), "jolted: {} -> {}", pair[0], pair[1]);
    }
    let touchdown = speeds[speeds.len().saturating_sub(4)..].iter().copied().max().unwrap();
    assert!(touchdown < Fx::ratio(3, 10), "touched down at {touchdown} m/tick");
    run(&mut w, 60);
    // Take off for somewhere else: straight up first, then away while climbing.
    let start = w.state.units.pos[ship];
    w.tick(&[cmd(Command::Move { units: vec![id], target: FxVec2::from_ints(3200, 1600), queue: false })]).unwrap();
    let mut moved_low = Fx::ZERO;
    for _ in 0..400 {
        w.tick(&[]).unwrap();
        if height(&w, ship) < 30 {
            moved_low = moved_low.max(w.state.units.pos[ship].distance(start));
        }
    }
    assert!(moved_low < Fx::from_int(3), "slid {moved_low} m across the ground lifting off");
    assert!(height(&w, ship) > 200, "climbed only to {} m", height(&w, ship));
    assert!(w.state.units.pos[ship].distance(start) > Fx::from_int(200), "never moved off");
}

#[test]
fn boarding_units_line_up_behind_the_stern_and_walk_up_the_centre_line() {
    let mut w = world();
    let ship = add(&mut w, SHIP, 0, 1800, 1800);
    let id = w.state.units.id(ship);
    let t = w.blueprints.unit(w.state.units.blueprint[ship]).transport.unwrap();
    // Down first, ramp open.
    w.tick(&[cmd(Command::Land { units: vec![id], pos: FxVec2::from_ints(1800, 1800), unload: false, queue: false })]).unwrap();
    assert!(run_until(&mut w, 1500, |w| w.ramp_down(ship)).is_some());
    // Tanks beside the hull, off both flanks and ahead of the nose.
    let cargo: Vec<usize> = [(1800, 1700), (1760, 1900), (1700, 1690), (2000, 1800)]
        .iter()
        .map(|&(x, y)| add(&mut w, "aster_t1_tank", 0, x, y))
        .collect();
    let ids = cargo.iter().map(|&r| w.state.units.id(r)).collect();
    w.tick(&[cmd(Command::Board { units: ids, carrier: id, queue: false })]).unwrap();
    let half = t.width / 2;
    let hull_half = w.blueprints.unit(w.state.units.blueprint[ship]).hull.1;
    for tick in 0..4000 {
        w.tick(&[]).unwrap();
        for &r in &cargo {
            if w.state.units.hangar[r] == id {
                continue;
            }
            let l = local_of(&w, ship, r);
            // Under the hull alongside the ramp is where a unit would cut in from the side.
            if l.x >= t.lip && l.x <= t.hinge && l.y.abs() < hull_half {
                assert!(l.y.abs() <= half, "tick {tick}: a tank came onto the ramp from the side at {l:?}");
            }
        }
        if cargo.iter().all(|&r| w.state.units.hangar[r] == id) {
            break;
        }
    }
    assert!(cargo.iter().all(|&r| w.state.units.hangar[r] == id), "not all boarded");
    // And out again: down the centre line to behind the stern before spreading.
    w.tick(&[cmd(Command::Land { units: vec![id], pos: FxVec2::from_ints(1800, 1800), unload: true, queue: false })]).unwrap();
    for _ in 0..1500 {
        w.tick(&[]).unwrap();
        for &r in &cargo {
            if w.state.units.hangar[r] != id {
                let l = local_of(&w, ship, r);
                if l.x >= t.lip && l.y.abs() < hull_half {
                    assert!(l.y.abs() <= half, "a tank left the ramp by its side at {l:?}");
                }
            }
        }
    }
    assert!(cargo.iter().all(|&r| w.state.units.hangar[r] == Handle::NONE));
}

#[test]
fn each_gun_house_rests_its_own_way_and_only_takes_what_is_in_its_cone() {
    let mut w = world();
    let ship = add(&mut w, SHIP, 0, 2000, 2000);
    let id = w.state.units.id(ship);
    let bp = w.blueprints.unit(w.state.units.blueprint[ship]).clone();
    assert_eq!(bp.weapons.len(), 4);
    let facings: Vec<i32> = bp.weapons.iter().map(|g| Angle::ZERO.delta_to(g.facing) as i32).collect();
    assert_eq!(facings, vec![0, 0x4000, -0x4000, -0x8000], "nose, port, starboard, stern");
    for (i, g) in bp.weapons.iter().enumerate() {
        assert!(g.mount && g.slant && g.half_arc < 0x8000, "gun {i} is a coned, slanting house");
        assert!(g.half_arc >= Angle::from_degrees(75).0 && g.half_arc <= Angle::from_degrees(90).0);
        assert_eq!(w.state.units.weapon_yaw[ship][i], g.facing, "gun {i} starts at rest");
    }
    w.tick(&[cmd(Command::Land { units: vec![id], pos: FxVec2::from_ints(2000, 2000), unload: false, queue: false })]).unwrap();
    assert!(run_until(&mut w, 1500, |w| w.ramp_down(ship)).is_some());
    run(&mut w, 60);
    for (i, g) in bp.weapons.iter().enumerate() {
        assert_eq!(w.state.units.weapon_yaw[ship][i], g.facing, "gun {i} rests facing its own way");
    }
    // A tank off the port side: only the port sponson takes it.
    let heading = w.state.units.heading[ship];
    let centre = w.state.units.pos[ship];
    let at = |x: i32, y: i32| centre + FxVec2::from_ints(x, y).rotate(heading);
    let port = at(-40, 200);
    let tank = w.spawn_unit(w.blueprints.id_of("aster_t1_tank").unwrap(), 1, port, Angle::ZERO, true).unwrap();
    let tank_id = w.state.units.id(tank);
    // It neither shoots back nor dies, so the guns keep on it.
    w.state.units.flags[tank] |= flag::PASSIVE | flag::INVULNERABLE;
    let mut fired = [false; 4];
    for _ in 0..30 {
        w.tick(&[]).unwrap();
        for (g, f) in fired.iter_mut().enumerate() {
            *f |= w.state.units.weapon_cooldown[ship][g] > 0;
        }
    }
    let on: Vec<bool> = (0..4).map(|g| w.state.units.weapon_target[ship][g] == tank_id).collect();
    assert_eq!(on, vec![false, true, false, false], "only the port gun bears to port");
    let yaw = Angle::ZERO.delta_to(w.state.units.weapon_yaw[ship][1]) as i32;
    assert!((yaw - 0x4000).abs() < 0x800, "the port gun swung to it: {yaw}");
    assert_eq!(fired, [false, true, false, false], "only the port gun fired");
    // One astern: the stern gun.
    let aft = at(-400, 0);
    let second = w.spawn_unit(w.blueprints.id_of("aster_t1_tank").unwrap(), 1, aft, Angle::ZERO, true).unwrap();
    let second_id = w.state.units.id(second);
    w.state.units.flags[second] |= flag::PASSIVE | flag::INVULNERABLE;
    run(&mut w, 30);
    assert_eq!(w.state.units.weapon_target[ship][3], second_id, "the stern gun covers the wake");
    assert_ne!(w.state.units.weapon_target[ship][2], second_id);
    assert_ne!(w.state.units.weapon_target[ship][0], second_id);
    // No gun ever turns past the edge of its cone.
    for _ in 0..100 {
        w.tick(&[]).unwrap();
        for (i, g) in bp.weapons.iter().enumerate() {
            let off = g.facing.delta_to(w.state.units.weapon_yaw[ship][i]).unsigned_abs();
            assert!(off <= g.half_arc, "gun {i} turned {off} off its facing");
        }
    }
}

#[test]
fn one_unit_can_be_let_out_of_the_hold_and_take_off_lifts_the_ship() {
    let mut w = world();
    let ship = add(&mut w, SHIP, 0, 1600, 1600);
    let id = w.state.units.id(ship);
    let cargo: Vec<usize> = (0..3).map(|i| add(&mut w, "aster_t1_tank", 0, 1400, 1560 + i * 40)).collect();
    let ids: Vec<Handle> = cargo.iter().map(|&r| w.state.units.id(r)).collect();
    w.tick(&[cmd(Command::Board { units: ids.clone(), carrier: id, queue: false })]).unwrap();
    assert!(run_until(&mut w, 3000, |w| cargo.iter().all(|&r| w.state.units.hangar[r] == id)).is_some());
    // Up and away, then let only the middle one out: the ship comes down where it is for it.
    w.tick(&[cmd(Command::TakeOff { units: vec![id] })]).unwrap();
    assert!(run_until(&mut w, 600, |w| height(w, ship) > 200).is_some(), "Take Off never lifted it");
    w.tick(&[cmd(Command::Unload { units: vec![ids[1]] })]).unwrap();
    assert!(run_until(&mut w, 2000, |w| w.state.units.hangar[cargo[1]] == Handle::NONE).is_some(), "never let it out");
    run(&mut w, 100);
    assert_eq!(height(&w, ship), 0);
    assert_eq!(w.state.units.hangar[cargo[0]], id, "let out one it was not asked to");
    assert_eq!(w.state.units.hangar[cargo[2]], id, "let out one it was not asked to");
    let view = w.cargo_view(ship).unwrap();
    assert_eq!(view.phase, mc_sim::mirror::LiftPhase::Ready);
    assert_eq!(view.stored.len(), 2);
    // Take off from the ground: the ramp comes up first, then it rises.
    w.tick(&[cmd(Command::TakeOff { units: vec![id] })]).unwrap();
    run(&mut w, 5);
    assert_eq!(w.cargo_view(ship).unwrap().phase, mc_sim::mirror::LiftPhase::RampClosing);
    assert!(run_until(&mut w, 800, |w| height(w, ship) > 300).is_some(), "never climbed back up");
}

#[test]
fn it_does_not_turn_its_hull_toward_what_it_shoots() {
    let mut w = world();
    let ship = add(&mut w, SHIP, 0, 1500, 1500);
    let ship_id = w.state.units.id(ship);
    assert!(run_until(&mut w, 900, |w| height(w, ship) > 500).is_some());
    let before = w.state.units.heading[ship];
    // An aircraft off its port beam, and an attack order on it.
    let foe = add(&mut w, "aster_t1_interceptor", 1, 1500, 1700);
    let foe_id = w.state.units.id(foe);
    w.tick(&[cmd(Command::Attack { units: vec![ship_id], target: foe_id, queue: false })]).unwrap();
    for _ in 0..200 {
        w.tick(&[]).unwrap();
        if let Some(r) = w.state.units.row(ship_id) {
            let turned = w.state.units.heading[r].delta_to(before).unsigned_abs();
            assert!(turned < mc_core::Angle::from_degrees(5).0, "the hull swung toward its target");
        }
    }
}
