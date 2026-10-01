//! Reclaim heads: several per unit, each on a house of its own; salvage units that
//! work while they move; heads that look down from a tower or an aircraft.

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
fn a_three_headed_thresher_works_three_wrecks_at_once_on_the_units_power() {
    let mut w = world();
    let thresher = add(&mut w, "aster_t2_land_reclaimer", 900, 900);
    let power = w.bp(thresher).reclaimer.unwrap().power;
    assert_eq!(w.bp(thresher).reclaimer.unwrap().heads().len(), 3);
    let wrecks = [
        wreck(&mut w, 1100, 900, 5000),
        wreck(&mut w, 900, 1150, 5000),
        wreck(&mut w, 700, 800, 5000),
    ];
    let mut frame = RenderFrame::default();
    let mut all_three = false;
    for _ in 0..600 {
        tick(&mut w, &[]);
        w.write_render_frame(None, &mut frame);
        if frame.beams.len() == 3 {
            all_three = true;
            break;
        }
    }
    assert!(all_three, "every head found a wreck of its own");
    // The pull is the unit's, shared between its heads.
    let before = w.state.players[0].reclaimed_mass;
    for _ in 0..100 {
        tick(&mut w, &[]);
    }
    let pulled = w.state.players[0].reclaimed_mass - before;
    assert!(
        pulled <= power * 10 + Fx::ONE && pulled > power * 9,
        "{pulled:?} in ten seconds at {power:?}"
    );
    for &r in &wrecks {
        assert!(
            w.state.wrecks.mass[r] < Fx::from_int(5000),
            "each wreck was worked"
        );
    }
    // Its heads are posed like guns in houses of their own.
    let row = frame
        .units
        .iter()
        .position(|u| u.blueprint == w.state.units.blueprint[thresher].0 as u32)
        .unwrap();
    let house = (frame.units[row].status[1] >> mc_sim::mirror::UNIT_HOUSE_SHIFT) as usize;
    assert!(house > 0, "the Thresher's heads are posed as houses");
    let yaws: Vec<f32> = (0..3).map(|i| frame.houses[house - 1].pose[i][1]).collect();
    assert!(
        yaws[0] != yaws[1] && yaws[1] != yaws[2],
        "each head turned its own way: {yaws:?}"
    );
}

#[test]
fn a_gleaner_on_a_move_order_reclaims_what_it_passes_without_stopping() {
    let mut w = world();
    let gleaner = add(&mut w, "aster_t1_land_reclaimer", 400, 900);
    let id = w.state.units.id(gleaner);
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
    let mut last = w.state.units.pos[gleaner];
    let mut stopped = 0;
    for t in 0..500 {
        tick(&mut w, &[]);
        let now = w.state.units.pos[gleaner];
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
    assert!(w.state.units.pos[gleaner].x > Fx::from_int(1500));
}

#[test]
fn a_gleaner_hovers_out_over_the_sea_to_salvage_a_wreck_lying_there() {
    let mut w = channel();
    let gleaner = add(&mut w, "aster_t1_land_reclaimer", 400, 900);
    let id = w.state.units.id(gleaner);
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
    let at = w.state.units.pos[gleaner];
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
fn an_argus_looks_straight_down_from_its_cruise_height_at_a_wreck_below() {
    let mut w = world();
    let argus = add(&mut w, "aster_t3_support", 900, 900);
    let reach = w.bp(argus).reclaimer.unwrap().range;
    assert!(reach >= Fx::from_int(1000));
    let under = wreck(&mut w, 960, 900, 300);
    let mut deepest = 0i16;
    for _ in 0..600 {
        tick(&mut w, &[]);
        let pitch = Angle::ZERO.delta_to(w.state.units.arm_pitch[argus][2]);
        deepest = deepest.min(pitch);
        if !w.state.wrecks.slots.is_alive(under) {
            break;
        }
    }
    assert!(
        !w.state.wrecks.slots.is_alive(under),
        "the Argus cleared the wreck under it"
    );
    // At 220 m up, a wreck a few hundred metres off is well below level.
    assert!(
        deepest < -(Angle::from_degrees(20).0 as i16),
        "the head pitched down: {deepest}"
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
