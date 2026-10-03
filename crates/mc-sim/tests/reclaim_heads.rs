//! Reclaim heads: each on a house of its own; the Reclaimer that works while it moves,
//! climbs a tier at a time and comes from every factory; a tower's head that looks down.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::mirror::RenderFrame;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    world_on(Heightfield::flat(256, 256, Fx::from_int(20)))
}

/// Land at 20 m with a sea channel 40 m deep across the map, x = 800 to 1360 m.
fn channel() -> World {
    let stride = 257;
    let mut samples = vec![20u16; stride * stride];
    for row in samples.chunks_exact_mut(stride) {
        for s in &mut row[100..=170] {
            *s = 0;
        }
    }
    world_on(Heightfield::from_samples(
        256,
        256,
        samples,
        Fx::from_int(-20),
        Fx::from_int(2),
        Fx::from_int(20),
    ))
}

fn world_on(terrain: Heightfield) -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let map = MapData {
        name: "heads".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1500, 1500)],
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
        seed: 7,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    w.state.players[0].mass = Fx::ZERO;
    w.state.players[0].energy = Fx::from_int(100_000);
    w.state.players[0].mass_capacity = Fx::from_int(100_000);
    w.state.players[0].energy_capacity = Fx::from_int(200_000);
    w
}

fn add(w: &mut World, key: &str, x: i32, y: i32) -> usize {
    let id = w.blueprints.id_of(key).unwrap();
    w.spawn_unit(id, 0, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap()
}

fn wreck(w: &mut World, x: i32, y: i32, mass: i32) -> usize {
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    w.state
        .wrecks
        .spawn(
            tank,
            FxVec2::from_ints(x, y),
            Fx::from_int(20),
            Angle::ZERO,
            Fx::from_int(mass),
            0,
        )
        .unwrap()
}

/// Mass stores are counted afresh each tick: keep the room the test set.
fn tick(w: &mut World, commands: &[PlayerCommand]) {
    w.tick(commands).unwrap();
    w.state.players[0].mass_capacity = Fx::from_int(100_000);
}

#[test]
fn a_reclaimer_turns_its_head_on_a_house_to_a_wreck_and_pulls_at_its_power() {
    let mut w = world();
    let reclaimer = add(&mut w, "aster_t2_mobile_reclaimer", 900, 900);
    let power = w.bp(reclaimer).reclaimer.unwrap().power;
    // Off its nose, so the head has to turn to it.
    let off = wreck(&mut w, 900, 1150, 5000);
    let mut frame = RenderFrame::default();
    let mut working = false;
    for _ in 0..600 {
        tick(&mut w, &[]);
        w.write_render_frame(None, &mut frame);
        if frame.beams.len() == 1 {
            working = true;
            break;
        }
    }
    assert!(working, "its head found the wreck");
    let before = w.state.players[0].reclaimed_mass;
    for _ in 0..100 {
        tick(&mut w, &[]);
    }
    let pulled = w.state.players[0].reclaimed_mass - before;
    assert!(
        pulled <= power * 10 + Fx::ONE && pulled > power * 9,
        "{pulled:?} in ten seconds at {power:?}"
    );
    assert!(w.state.wrecks.mass[off] < Fx::from_int(5000));
    // Its head is posed like a gun in a house of its own, turned off the nose.
    w.write_render_frame(None, &mut frame);
    let row = frame
        .units
        .iter()
        .position(|u| u.blueprint == w.state.units.blueprint[reclaimer].0 as u32)
        .unwrap();
    let house = (frame.units[row].status[1] >> mc_sim::mirror::UNIT_HOUSE_SHIFT) as usize;
    assert!(house > 0, "the Gleaner II's head is posed as a house");
    let yaw = frame.houses[house - 1].pose[0][1];
    assert!(yaw.abs() > 0.5, "the head turned to the wreck: {yaw}");
}

#[test]
fn a_reclaimer_on_a_move_order_reclaims_what_it_passes_without_stopping() {
    let mut w = world();
    let reclaimer = add(&mut w, "aster_t1_mobile_reclaimer", 400, 900);
    let id = w.state.units.id(reclaimer);
    // A field to one side of its road, far from where it is going.
    for i in 0..6 {
        wreck(&mut w, 700 + i * 60, 1100, 40);
    }
    tick(
        &mut w,
        &[PlayerCommand {
            player: 0,
            command: Command::Move {
                units: vec![id],
                target: FxVec2::from_ints(1800, 900),
                queue: false,
            },
        }],
    );
    let mut last = w.state.units.pos[reclaimer];
    let mut stopped = 0;
    for t in 0..500 {
        tick(&mut w, &[]);
        let now = w.state.units.pos[reclaimer];
        // Past its first moments getting under way, and short of where it is going.
        if now == last && t > 20 && now.x < Fx::from_int(1700) {
            stopped += 1;
        }
        last = now;
    }
    assert!(
        w.state.players[0].reclaimed_mass > Fx::from_int(40),
        "it salvaged on the way: {:?}",
        w.state.players[0].reclaimed_mass
    );
    assert_eq!(stopped, 0, "it kept moving");
    assert!(w.state.units.pos[reclaimer].x > Fx::from_int(1500));
}

#[test]
fn a_reclaimer_hovers_out_over_the_sea_to_salvage_a_wreck_lying_there() {
    let mut w = channel();
    let reclaimer = add(&mut w, "aster_t1_mobile_reclaimer", 400, 900);
    let id = w.state.units.id(reclaimer);
    let goal = FxVec2::from_ints(1080, 900);
    assert!(
        w.terrain.height_at(goal) < w.terrain.water_level(),
        "the goal is out over the water"
    );
    // A wreck in the middle of the channel, beyond its reach from either shore.
    let sunk = wreck(&mut w, 1080, 1300, 200);
    tick(
        &mut w,
        &[PlayerCommand {
            player: 0,
            command: Command::Move {
                units: vec![id],
                target: goal,
                queue: false,
            },
        }],
    );
    for _ in 0..1200 {
        tick(&mut w, &[]);
        if !w.state.wrecks.slots.is_alive(sunk) {
            break;
        }
    }
    let at = w.state.units.pos[reclaimer];
    assert!(
        at.distance(goal) < Fx::from_int(40),
        "it reached the water: {at:?}"
    );
    assert!(
        !w.state.wrecks.slots.is_alive(sunk),
        "it salvaged the wreck in the channel"
    );
}

#[test]
fn a_tower_head_pitches_down_at_a_wreck_below_it() {
    let mut w = world();
    let tower = add(&mut w, "aster_t1_reclaimer", 900, 900);
    let near = wreck(&mut w, 950, 900, 50);
    for _ in 0..1200 {
        tick(&mut w, &[]);
        if !w.state.wrecks.slots.is_alive(near) {
            break;
        }
    }
    assert!(!w.state.wrecks.slots.is_alive(near));
    let pitch = Angle::ZERO.delta_to(w.state.units.arm_pitch[tower][2]);
    assert!(
        pitch < -(Angle::from_degrees(20).0 as i16),
        "the head looks down from 36 m at 50 m: {pitch}"
    );
}

/// Reclaiming takes no energy, even a tower's: in a stall, with nothing in the store and
/// a factory going up that asks for more than comes in, the tower pulls a steep wreck at
/// its foot and a far one at full power.
#[test]
fn a_tower_reclaims_near_and_far_wrecks_through_an_energy_stall() {
    let mut w = world();
    let tower = add(&mut w, "aster_t1_reclaimer", 900, 900);
    assert_eq!(w.bp(tower).economy.energy_upkeep, Fx::ZERO);
    add(&mut w, "aster_mass_storage", 300, 300);
    let engineer = add(&mut w, "aster_t1_engineer", 400, 400);
    let factory = w.blueprints.id_of("aster_t1_land_factory").unwrap();
    let id = w.state.units.id(engineer);
    tick(
        &mut w,
        &[PlayerCommand {
            player: 0,
            command: Command::Build {
                units: vec![id],
                blueprint: factory,
                pos: FxVec2::from_ints(460, 460),
                heading: Angle::ZERO,
                queue: false,
            },
        }],
    );
    // 48 m off, from a head 36 m up: well below level. And one far across the reach.
    let near = wreck(&mut w, 948, 900, 60);
    let far = wreck(&mut w, 900, 1450, 60);
    let mut stalled = 0;
    for _ in 0..1200 {
        w.state.players[0].energy = Fx::ZERO;
        tick(&mut w, &[]);
        let p = &w.state.players[0];
        if p.energy_demand > p.energy_income {
            stalled += 1;
        }
        if !w.state.wrecks.slots.is_alive(near) && !w.state.wrecks.slots.is_alive(far) {
            break;
        }
    }
    assert!(
        stalled > 100,
        "the side was stalled on energy ({stalled} ticks)"
    );
    assert!(
        !w.state.wrecks.slots.is_alive(near),
        "the steep near wreck was taken"
    );
    assert!(
        !w.state.wrecks.slots.is_alive(far),
        "the far wreck was taken"
    );
}

/// A side that builds for free (the test range) never runs out of room: its full store
/// once left every idle tower aimed at its wreck and dark.
#[test]
fn a_free_building_side_with_a_full_store_still_reclaims() {
    let mut w = world();
    w.state.players[0].free_build = true;
    let tower = add(&mut w, "aster_t1_reclaimer", 900, 900);
    let near = wreck(&mut w, 948, 900, 60);
    let mut beam = false;
    for _ in 0..600 {
        w.tick(&[]).unwrap();
        w.state.players[0].mass = w.state.players[0].mass_capacity;
        beam |= w
            .state
            .units
            .has_flag(tower, mc_sim::tables::flag::RECLAIMING);
        if !w.state.wrecks.slots.is_alive(near) {
            break;
        }
    }
    assert!(beam && !w.state.wrecks.slots.is_alive(near));
}

/// Each faction's Reclaimer II and III (Gleaner II and III, Breaker II and III) are its
/// tech 1 one drawn 7/6 and 4/3 the size: body and head pivot alike, so the beam leaves
/// the bigger model where its head is.
#[test]
fn each_reclaimer_tier_is_the_first_scaled_up_and_upgrades_to_the_next() {
    let w = world();
    let b = &w.blueprints;
    for faction in ["aster", "regency"] {
        let key = |tier: u8| format!("{faction}_t{tier}_mobile_reclaimer");
        let unit = |tier: u8| b.unit_by_key(&key(tier)).unwrap();
        let first = unit(1);
        let head = |tier: u8| unit(tier).reclaimer.unwrap().heads()[0];
        let near = |got: Fx, want: Fx, what: &str| {
            assert!(
                (got - want).0.abs() < Fx::ratio(1, 100).0,
                "{what}: {got:?}, not {want:?}"
            );
        };
        for (tier, num, den) in [(2u8, 7, 6), (3, 4, 3)] {
            let scale = |v: Fx| v * num / den;
            let (u, k) = (unit(tier), key(tier));
            assert_eq!(u.tech, tier, "{k}");
            assert_eq!(u.visual.mesh, first.visual.mesh, "{k}: the same model");
            near(u.radius, scale(first.radius), &format!("{k} radius"));
            near(u.height, scale(first.height), &format!("{k} height"));
            let (h, h1) = (head(tier), head(1));
            assert_eq!(unit(tier).reclaimer.unwrap().heads().len(), 1, "{k}");
            let (p, p1) = (h.pivot.unwrap(), h1.pivot.unwrap());
            near(p.x, scale(p1.x), &format!("{k} pivot x"));
            near(p.z, scale(p1.z), &format!("{k} pivot z"));
            near(h.emitter.x, scale(h1.emitter.x), &format!("{k} emitter x"));
            near(h.emitter.z, scale(h1.emitter.z), &format!("{k} emitter z"));
            // A bigger tier pulls harder and reaches further.
            let (r, below) = (u.reclaimer.unwrap(), unit(tier - 1).reclaimer.unwrap());
            assert!(r.power > below.power && r.range > below.range, "{k}");
            assert!(r.mobile, "{k}");
        }
        assert_eq!(first.upgrades_to, b.id_of(&key(2)), "{faction}: I to II");
        assert_eq!(
            unit(2).upgrades_to,
            b.id_of(&key(3)),
            "{faction}: II to III"
        );
        assert_eq!(unit(3).upgrades_to, None, "{faction}: III is the last");
    }
}

/// Every land, air and naval factory of either faction makes the Reclaimer of its own
/// tier and each below it, and none above.
#[test]
fn every_factory_builds_the_reclaimers_up_to_its_tier() {
    let w = world();
    let b = &w.blueprints;
    for faction in ["aster", "regency"] {
        for kind in ["land", "air", "naval"] {
            for tier in 1..=3u8 {
                let factory = format!("{faction}_t{tier}_{kind}_factory");
                let builds = &b
                    .unit_by_key(&factory)
                    .unwrap()
                    .builder
                    .as_ref()
                    .unwrap()
                    .builds;
                for t in 1..=3u8 {
                    let reclaimer = b
                        .id_of(&format!("{faction}_t{t}_mobile_reclaimer"))
                        .unwrap();
                    assert_eq!(
                        builds.contains(&reclaimer),
                        t <= tier,
                        "{factory} and the Reclaimer of tech {t}"
                    );
                }
            }
        }
    }
}

/// A Gleaner upgrades where it hovers to a Gleaner II once the side has tech 2: the
/// same unit, carrying on with the stronger head.
#[test]
fn a_gleaner_upgrades_in_place_once_the_side_has_tech_2() {
    let mut w = world();
    // Stores that pay for the upgrade outright, with no income: set the tick after the
    // storage, once it counts in what the side can hold.
    let order = |command| PlayerCommand { player: 0, command };
    tick(
        &mut w,
        &[order(Command::DebugStorage {
            player: 0,
            mass: 10_000,
            energy: 100_000,
        })],
    );
    tick(
        &mut w,
        &[order(Command::DebugStock {
            player: 0,
            mass: Some(1000),
            energy: Some(10_000),
        })],
    );
    let reclaimer = add(&mut w, "aster_t1_mobile_reclaimer", 900, 900);
    let id = w.state.units.id(reclaimer);
    let t2 = w.blueprints.id_of("aster_t2_mobile_reclaimer").unwrap();
    let upgrade = order(Command::Upgrade { units: vec![id] });
    // On tech 1 there is no tech 2 to climb to.
    tick(&mut w, std::slice::from_ref(&upgrade));
    assert!(w.state.orders.front(&w.state.units, reclaimer).is_none());
    add(&mut w, "aster_t2_land_factory", 400, 400);
    assert_eq!(w.side_tech(0), 2);
    tick(&mut w, &[upgrade]);
    let before = w.state.units.pos[reclaimer];
    // Quickly: about 20 s at its `upgrade_power`, not the 100 s a non-builder's default
    // would take.
    for _ in 0..25 * mc_core::TICKS_PER_SECOND {
        tick(&mut w, &[]);
        if w.state
            .units
            .row(id)
            .is_some_and(|r| w.state.units.blueprint[r] == t2)
        {
            break;
        }
    }
    let row = w
        .state
        .units
        .row(id)
        .expect("the same unit after the upgrade");
    assert_eq!(w.state.units.blueprint[row], t2, "it became a Gleaner II");
    assert_eq!(w.state.units.pos[row], before, "upgraded where it hovered");
    let u = &w.state.units;
    let salvagers = u
        .slots
        .iter()
        .filter(|&r| u.owner[r] == 0 && w.bp(r).reclaimer.is_some_and(|r| r.mobile))
        .count();
    assert_eq!(salvagers, 1, "nothing left behind");
}
