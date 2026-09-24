//! Combat: twin barrels take turns.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
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

#[test]
fn gunships_orbit_and_keep_the_target_in_front() {
    for key in ["aster_t1_rotor_gunship", "aster_t2_gunship"] {
        let mut w = world();
        let g = add(&mut w, key, 0, 780, 900);
        let t = add(&mut w, "aster_t1_tank", 1, 900, 900);
        w.state.units.flags[t] |= flag::PASSIVE | flag::INVULNERABLE;
        let mut positions = Vec::new();
        let mut rockets = 0;
        for tick in 0..240 {
            w.tick(&[]).unwrap();
            if tick > 70 {
                let to = w.state.units.pos[t] - w.state.units.pos[g];
                assert!(
                    w.state.units.heading[g].delta_to(to.angle()).unsigned_abs() < 5000,
                    "{key}: hull lost target"
                );
                assert!(to.length() > Fx::from_int(55) && to.length() < Fx::from_int(190));
                positions.push(w.state.units.pos[g]);
            }
            for event in &w.events {
                if let SimEvent::ShotFired {
                    blueprint,
                    weapon: 1,
                    ..
                } = event
                {
                    if *blueprint == w.state.units.blueprint[g] {
                        rockets += 1;
                        let to = w.state.units.pos[t] - w.state.units.pos[g];
                        assert!(
                            w.state.units.heading[g].delta_to(to.angle()).unsigned_abs() <= 1900
                        );
                    }
                }
            }
        }
        let travel: Fx = positions
            .windows(2)
            .map(|p| p[1].distance(p[0]))
            .fold(Fx::ZERO, |a, b| a + b);
        assert!(travel > Fx::from_int(250), "{key}: traveled {travel:?}");
        assert!(rockets > 2, "{key}: no rocket passes");
    }
}
#[test]
fn carrier_stays_airborne_and_drones_divide_and_orbit_wrecks() {
    let mut w = world();
    add(&mut w, "aster_commander", 0, 150, 150);
    let c = add(&mut w, "aster_t2_reclaim_carrier", 0, 900, 900);
    w.state.players[0].mass = Fx::ZERO;
    w.state.players[0].energy = Fx::ZERO;
    w.state.players[0].mass_capacity = Fx::from_int(100000);
    let parent = w.state.units.id(c);
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    let mut drones = Vec::new();
    let mut wrecks = Vec::new();
    for i in 0..4 {
        let x = 810 + i * 60;
        let d = add(&mut w, "aster_reclaim_drone", 0, x, 1030);
        w.state.units.drone_parent[d] = parent;
        drones.push(d);
        wrecks.push(
            w.state
                .wrecks
                .spawn(
                    tank,
                    FxVec2::from_ints(x, 1030),
                    Fx::from_int(20),
                    Angle::ZERO,
                    Fx::from_int(1000),
                )
                .unwrap(),
        );
    }
    let ids = drones.iter().map(|&d| w.state.units.id(d)).collect();
    w.tick(&[cmd(Command::Move {
        units: ids,
        target: FxVec2::from_ints(100, 100),
        queue: false,
    })])
    .unwrap();
    for &d in &drones {
        assert_eq!(w.state.units.order_head[d], mc_sim::tables::NO_ORDER);
    }
    let mut distance = [Fx::ZERO; 4];
    for _ in 0..500 {
        w.tick(&[]).unwrap();
        for (i, &d) in drones.iter().enumerate() {
            distance[i] += w.state.units.pos[d].distance(w.state.units.prev_pos[d]);
        }
        assert!(w.state.units.z[c] > Fx::from_int(30), "carrier landed");
    }
    for (i, &d) in drones.iter().enumerate() {
        assert!(distance[i] > Fx::from_int(80), "drone stationary");
        assert!(
            w.state.wrecks.mass[wrecks[i]] < Fx::from_int(950),
            "wreck {i} not reclaimed: {:?}, drone at {:?}",
            w.state.wrecks.mass[wrecks[i]],
            w.state.units.pos[d]
        );
    }
}
#[test]
fn overlapping_aircraft_reserve_only_one_landing_site() {
    let mut w = world();
    let a = add(&mut w, "aster_t1_bomber", 0, 900, 900);
    let b = add(&mut w, "aster_t1_bomber", 0, 900, 900);
    let r = w.blueprints.unit(w.state.units.blueprint[a]).radius;
    for _ in 0..400 {
        w.tick(&[]).unwrap();
        // The second one moves off to its own pad instead of sharing the first's.
        let apart = w.state.units.pos[a].distance(w.state.units.pos[b]) >= r * 2;
        assert!(
            apart || w.state.units.z[a] > Fx::from_int(23) || w.state.units.z[b] > Fx::from_int(23)
        );
    }
    assert!(
        w.state.units.z[a] <= Fx::from_int(21) || w.state.units.z[b] <= Fx::from_int(21),
        "neither aircraft landed"
    );
}
#[test]
fn cruise_vertical_velocity_changes_gradually_and_is_hashed() {
    let mut w = world();
    let a = add(&mut w, "aster_t2_reclaim_carrier", 0, 900, 900);
    w.state.units.z[a] = Fx::from_int(180);
    let mut previous = Fx::ZERO;
    for _ in 0..100 {
        w.tick(&[]).unwrap();
        let dz = w.state.units.z[a] - w.state.units.prev_z[a];
        assert!(
            (dz - previous).abs() <= Fx::ratio(151, 1000),
            "abrupt altitude acceleration"
        );
        // Cruise climb is a quarter of top speed per second, 6 to 18 m/s.
        assert!(dz.abs() <= Fx::ratio(18, mc_core::TICKS_PER_SECOND as i64));
        previous = dz;
    }
    let hash = w.hash();
    let saved = w.state.units.air_velocity[a];
    w.state.units.air_velocity[a].z += Fx::ONE;
    assert_ne!(hash, w.hash());
    w.state.units.air_velocity[a] = saved;
    let mut restored = world();
    restored
        .restore(Heightfield::flat(256, 256, Fx::from_int(20)), &w.snapshot())
        .unwrap();
    for _ in 0..80 {
        assert_eq!(w.tick(&[]).unwrap(), restored.tick(&[]).unwrap());
    }
}
#[test]
fn thunderhead_descends_fires_low_and_climbs_out_on_repeated_passes() {
    let mut w = world();
    let a = add(&mut w, "aster_t3_assault_aircraft", 0, 550, 900);
    let t = add(&mut w, "aster_t1_tank", 1, 900, 900);
    w.state.units.flags[t] |= flag::PASSIVE | flag::INVULNERABLE;
    let cruise = Fx::from_int(115); // terrain 20 + cruise 95
    w.state.units.z[a] = cruise;
    w.state.units.prev_z[a] = cruise;
    w.state.units.speed[a] = Fx::from_int(86);
    let mut low_shots = 0;
    let mut dives = 0;
    let mut recoveries = 0;
    let mut recovered = true;
    let mut lowest = cruise;
    let mut rockets = 0;
    for _ in 0..800 {
        w.tick(&[]).unwrap();
        let units = &w.state.units;
        let z = units.z[a];
        let dz = z - units.prev_z[a];
        let pitch = units.arm_pitch[a][0].0 as i16;
        lowest = lowest.min(z);
        assert!(
            z >= Fx::from_int(50),
            "strafe lost terrain clearance: {z:?}"
        );
        if dz < -Fx::ratio(1, 10) {
            assert!(pitch < 0, "descending flight must point down");
        } else if dz > Fx::ratio(1, 10) {
            assert!(pitch > 0, "climbing flight must point up");
        }
        if recovered && z < cruise - Fx::from_int(35) {
            dives += 1;
            recovered = false;
        }
        if !recovered && z > cruise - Fx::from_int(5) {
            recoveries += 1;
            recovered = true;
        }
        for e in &w.events {
            if let SimEvent::ShotFired {
                blueprint,
                weapon,
                pos: muzzle,
                ..
            } = e
            {
                if *blueprint == units.blueprint[a] && *weapon == 1 {
                    // The Talons go first, out beyond the cannon's reach.
                    let reach = w.blueprints.unit(units.blueprint[a]).weapons[0].range_max;
                    assert!((units.pos[t] - units.pos[a]).length() > reach - Fx::from_int(20));
                    rockets += 1;
                } else if *blueprint == units.blueprint[a] {
                    assert_eq!(*weapon, 0);
                    let to = units.pos[t] - units.pos[a];
                    let facing = units.heading[a] + units.weapon_yaw[a][0];
                    assert!(facing.delta_to(to.angle()).unsigned_abs() < 2300);
                    assert!(
                        to.dot(units.air_velocity[a].xy()) > Fx::ZERO,
                        "cannon fired while departing"
                    );
                    let elevation = FxVec2::from_angle(units.arm_pitch[a][0]);
                    let gun = (FxVec2::from_angle(facing) * elevation.x).extend(elevation.y);
                    let target_height = w.blueprints.unit(units.blueprint[t]).height;
                    let aim = units.pos[t].extend(units.z[t] + target_height / 2);
                    let alignment = gun.dot((aim - *muzzle).normalize());
                    // A hair of slack: this muzzle is the launched one, the gate's is
                    // modelled about the breech, and they round apart at the cone's edge.
                    assert!(alignment >= FxVec2::from_angle(Angle::from_degrees(8)).x - Fx::ratio(1, 10000),
                        "cannon fired outside its 8-degree gun cone: alignment={alignment:?}, distance={:?}",
                        to.length());
                    if z < cruise - Fx::from_int(25) && dz < Fx::ZERO {
                        low_shots += 1;
                    }
                }
            }
        }
    }
    assert!(dives >= 2, "did not repeat descent and recovery: {dives}");
    assert!(
        low_shots >= 5,
        "cannon stopped firing during the dive: {low_shots}"
    );
    assert!(
        recoveries >= 2,
        "did not repeatedly regain cruise altitude: {recoveries}"
    );
    assert!(
        rockets >= 4,
        "the Talons were not loosed on each pass: {rockets}"
    );
    eprintln!("Thunderhead: {dives} dives, {low_shots} descending low-altitude shots, {rockets} rockets, minimum height {lowest:?}");
}

#[test]
fn thunderhead_cruises_level_on_an_ordinary_move() {
    let mut w = world();
    // Far enough that it is still cruising, not braking, after 80 ticks.
    let a = add(&mut w, "aster_t3_assault_aircraft", 0, 100, 900);
    let cruise = Fx::from_int(115);
    w.state.units.z[a] = cruise;
    w.state.units.prev_z[a] = cruise;
    w.state.units.speed[a] = Fx::from_int(86);
    w.tick(&[cmd(Command::Move {
        units: vec![w.state.units.id(a)],
        target: FxVec2::from_ints(2000, 900),
        queue: false,
    })])
    .unwrap();
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        assert!((w.state.units.z[a] - cruise).abs() < Fx::ONE);
        assert_eq!(w.state.units.arm_pitch[a][0], Angle::ZERO);
    }
}

#[test]
fn bombers_repeat_salvos_with_turn_and_release_clearance() {
    for key in [
        "aster_t1_bomber",
        "aster_t2_fire_bomber",
        "aster_t3_strategic_bomber",
    ] {
        let mut w = world();
        let a = add(&mut w, key, 0, 400, 512);
        let t = add(&mut w, "aster_t1_tank", 1, 620, 512);
        w.state.units.flags[t] |= flag::PASSIVE | flag::INVULNERABLE;
        w.tick(&[cmd(Command::Attack {
            units: vec![w.state.units.id(a)],
            target: w.state.units.id(t),
            queue: false,
        })])
        .unwrap();
        let mut drops = 0;
        for _ in 0..1400 {
            w.tick(&[]).unwrap();
            drops+=w.events.iter().filter(|e|matches!(e,SimEvent::ShotFired{blueprint,weapon:0,..} if *blueprint==w.state.units.blueprint[a])).count();
        }
        assert!(
            drops >= w.blueprints.unit(w.state.units.blueprint[a]).weapons[0].salvo as usize * 2,
            "{key}: {drops} drops"
        );
    }
}

#[test]
fn wasp_projectiles_leave_the_rotating_chin_barrel() {
    let mut w = world();
    let a = add(&mut w, "aster_t1_rotor_gunship", 0, 800, 900);
    let t = add(&mut w, "aster_t1_tank", 1, 900, 900);
    w.state.units.heading[a] = Angle::from_degrees(110);
    w.state.units.flags[t] |= flag::PASSIVE | flag::INVULNERABLE;
    let mut checked = 0;
    for _ in 0..60 {
        w.tick(&[]).unwrap();
        for event in &w.events {
            if let SimEvent::ShotFired {
                blueprint,
                weapon: 0,
                pos,
                ..
            } = event
            {
                if *blueprint != w.state.units.blueprint[a] {
                    continue;
                }
                let yaw = w.state.units.weapon_yaw[a][0].to_radians_f32();
                let pitch = w.state.units.arm_pitch[a][0].to_radians_f32();
                let heading = w.state.units.heading[a].to_radians_f32();
                let forward = (w.state.units.pos[a] - w.state.units.prev_pos[a])
                    .dot(FxVec2::from_angle(w.state.units.heading[a]))
                    .to_f32();
                let lean = -(forward * 0.035).clamp(-0.12, 0.12);
                let x = 2.1 + 1.1 * pitch.cos() * yaw.cos();
                let y = 1.1 * pitch.cos() * yaw.sin();
                let z = 0.5 + 1.1 * pitch.sin();
                let x2 = x * lean.cos() - z * lean.sin();
                let z2 = x * lean.sin() + z * lean.cos();
                let expected = [
                    w.state.units.pos[a].x.to_f32() + x2 * heading.cos() - y * heading.sin(),
                    w.state.units.pos[a].y.to_f32() + x2 * heading.sin() + y * heading.cos(),
                    w.state.units.z[a].to_f32() + z2,
                ];
                let actual = pos.to_f32();
                for axis in 0..3 {
                    assert!(
                        (expected[axis] - actual[axis]).abs() < 0.025,
                        "barrel/muzzle mismatch: {expected:?} vs {actual:?}"
                    );
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 5);
}
#[test]
fn guided_aa_can_hit_a_fast_crossing_fighter() {
    let mut w = world();
    let a = add(&mut w, "aster_t3_sam", 0, 900, 900);
    let t = add(&mut w, "aster_t3_air_superiority", 1, 1050, 900);
    w.state.units.flags[t] |= flag::PASSIVE | flag::INVULNERABLE;
    let target = w.state.units.id(t);
    w.tick(&[PlayerCommand {
        player: 1,
        command: Command::Move {
            units: vec![target],
            target: FxVec2::from_ints(1400, 1600),
            queue: false,
        },
    }])
    .unwrap();
    let mut hits = 0;
    for _ in 0..100 {
        w.tick(&[]).unwrap();
        hits+=w.events.iter().filter(|e|matches!(e,SimEvent::Impact{blueprint,on_unit:true,..} if *blueprint==w.state.units.blueprint[a])).count();
    }
    assert!(hits > 0, "guided AA missed fast crossing aircraft");
}

#[test]
fn fighters_dive_and_climb_to_their_targets_and_fire_below_cruise() {
    for key in [
        "aster_t1_interceptor",
        "aster_t2_interceptor",
        "aster_t3_air_superiority",
    ] {
        let mut w = world();
        let a = add(&mut w, key, 0, 780, 900);
        let t = add(&mut w, "aster_t2_reclaim_carrier", 1, 900, 900);
        w.state.units.flags[t] |= flag::PASSIVE | flag::INVULNERABLE;
        let motion = w
            .blueprints
            .unit(w.state.units.blueprint[a])
            .motion
            .unwrap();
        w.state.units.speed[a] = motion.speed;
        w.tick(&[cmd(Command::Attack {
            units: vec![w.state.units.id(a)],
            target: w.state.units.id(t),
            queue: false,
        })])
        .unwrap();
        let mut low_shots = 0;
        for _ in 0..260 {
            let previous = w.state.units.air_velocity[a].z;
            w.tick(&[]).unwrap();
            assert!((w.state.units.air_velocity[a].z - previous).abs() <= Fx::ratio(151, 1000));
            if w.state.units.z[a] < Fx::from_int(150) {
                low_shots += w.events.iter().filter(|e| matches!(e,
                    SimEvent::ShotFired { blueprint, .. } if *blueprint == w.state.units.blueprint[a]
                )).count();
            }
        }
        assert!(
            (w.state.units.z[a] - w.state.units.z[t]).abs() < Fx::from_int(4),
            "{key} did not descend to target: {:?}",
            w.state.units.z[a]
        );
        // Line up a fresh firing pass at the acquired low altitude.
        w.state.units.pos[a] = w.state.units.pos[t] - FxVec2::from_ints(150, 0);
        w.state.units.heading[a] = Angle::ZERO;
        w.state.units.bank[a] = 0;
        w.state.units.speed[a] = motion.speed;
        for _ in 0..30 {
            w.tick(&[]).unwrap();
            low_shots += w.events.iter().filter(|e| matches!(e,
                SimEvent::ShotFired { blueprint, .. } if *blueprint == w.state.units.blueprint[a]
            )).count();
        }
        assert!(low_shots > 0, "{key} could not fire below cruise altitude: z={:?}, pos={:?}, speed={:?}, flags={}, target={:?}", w.state.units.z[a], w.state.units.pos[a], w.state.units.speed[a], w.state.units.flags[a], w.state.units.weapon_target[a]);

        // A target climbing above every fighter's cruise band must draw pursuit up.
        for _ in 0..350 {
            w.state.units.z[t] = Fx::from_int(420);
            w.tick(&[]).unwrap();
        }
        assert!(
            (w.state.units.z[a] - Fx::from_int(420)).abs() < Fx::from_int(4),
            "{key} did not climb to target: {:?}",
            w.state.units.z[a]
        );

        // A normal move releases attack altitude and returns to the cruise band.
        w.tick(&[cmd(Command::Move {
            units: vec![w.state.units.id(a)],
            target: FxVec2::from_ints(1800, 1800),
            queue: false,
        })])
        .unwrap();
        let before = w.state.units.z[a];
        for _ in 0..50 {
            w.tick(&[]).unwrap();
        }
        assert!(
            w.state.units.z[a] < before - Fx::from_int(30),
            "{key} kept attack altitude"
        );
    }
}

#[test]
fn fighter_pursuit_clears_ground_and_restores_deterministically() {
    let mut w = world();
    let a = add(&mut w, "aster_t1_interceptor", 0, 780, 900);
    let t = add(&mut w, "aster_t1_rotor_gunship", 1, 900, 900);
    w.state.units.flags[t] |= flag::PASSIVE | flag::INVULNERABLE;
    w.state.units.z[t] = Fx::from_int(20);
    w.tick(&[cmd(Command::Attack {
        units: vec![w.state.units.id(a)],
        target: w.state.units.id(t),
        queue: false,
    })])
    .unwrap();
    for _ in 0..50 {
        w.tick(&[]).unwrap();
    }
    let mut restored = world();
    restored.pool = Arc::new(Pool::new(4));
    restored
        .restore(Heightfield::flat(256, 256, Fx::from_int(20)), &w.snapshot())
        .unwrap();
    for _ in 0..250 {
        assert_eq!(w.tick(&[]).unwrap(), restored.tick(&[]).unwrap());
        assert!(
            w.state.units.z[a] >= Fx::from_int(43),
            "fighter descended into terrain: z={:?}, target_z={:?}, flags={}",
            w.state.units.z[a],
            w.state.units.z[t],
            w.state.units.flags[a]
        );
    }
    assert!((w.state.units.z[a] - Fx::from_int(44)).abs() < Fx::ONE);
}

#[test]
fn thunderhead_looses_its_talons_on_an_attack_ground_run() {
    let mut w = world();
    let a = add(&mut w, "aster_t3_assault_aircraft", 0, 400, 900);
    let cruise = Fx::from_int(115);
    w.state.units.z[a] = cruise;
    w.state.units.prev_z[a] = cruise;
    w.state.units.speed[a] = Fx::from_int(188);
    w.tick(&[cmd(Command::AttackGround {
        units: vec![w.state.units.id(a)],
        pos: FxVec2::from_ints(1300, 900),
        queue: false,
    })])
    .unwrap();
    let (mut rockets, mut landed) = (0, 0);
    for _ in 0..300 {
        w.tick(&[]).unwrap();
        let bp = w.state.units.blueprint[a];
        for e in &w.events {
            match e {
                SimEvent::ShotFired {
                    blueprint,
                    weapon: 1,
                    ..
                } if *blueprint == bp => rockets += 1,
                SimEvent::Impact {
                    blueprint,
                    weapon: 1,
                    pos,
                    ..
                } if *blueprint == bp => {
                    // Nothing to home on: each goes straight down the line to the point.
                    assert!(pos.xy().distance(FxVec2::from_ints(1300, 900)) < Fx::from_int(40));
                    landed += 1;
                }
                _ => {}
            }
        }
    }
    assert!(
        rockets >= 2 && landed >= 2,
        "rockets {rockets}, landed {landed}"
    );
}
