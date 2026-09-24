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
fn roster_is_tier_gated_and_every_aircraft_has_a_factory() {
    let w = world();
    for bp in &w.blueprints.units {
        if let Some(builder) = &bp.builder {
            for id in &builder.builds {
                // Experimentals (tech 4) are raised by combat engineers (tech 3).
                let built = w.blueprints.unit(*id);
                let experimental = built.categories & mc_data::cat::EXPERIMENTAL != 0;
                assert!(
                    built.tech <= bp.tech || (experimental && bp.tech == 3),
                    "{} builds {}",
                    bp.key,
                    w.blueprints.unit(*id).key
                );
            }
        }
        if bp
            .motion
            .is_some_and(|m| m.layer == mc_data::MoveLayer::Air)
            && bp.key != "aster_reclaim_drone"
            // Lift ships are raised by engineers on a lot of their own.
            && !bp.built_on_site()
        {
            assert!(w
                .blueprints
                .units
                .iter()
                .any(|f| f.has(mc_data::cat::FACTORY)
                    && f.builder
                        .as_ref()
                        .is_some_and(|b| b.builds.contains(&bp.id))));
        }
    }
    assert_eq!(
        w.blueprints
            .unit(w.blueprints.id_of("aster_t1_interceptor").unwrap())
            .role,
        "Fighter"
    );
}
#[test]
fn orbit_circles_follows_a_friendly_and_stop_cancels() {
    let mut w = world();
    let base = add(&mut w, "aster_t1_tank", 0, 900, 900);
    let scout = add(&mut w, "aster_t1_air_scout", 0, 1080, 900);
    let id = w.state.units.id(scout);
    let target = w.state.units.id(base);
    w.state.units.heading[scout] = Angle::from_degrees(90);
    w.tick(&[cmd(Command::Orbit {
        units: vec![id],
        pos: FxVec2::from_ints(900, 900),
        target,
        radius: Fx::ZERO,
        queue: false,
    })])
    .unwrap();
    let mut far = Fx::ZERO;
    let mut near = Fx::MAX;
    for i in 0..300 {
        w.tick(&[]).unwrap();
        if i > 100 {
            let d = w.state.units.pos[scout].distance(w.state.units.pos[base]);
            far = far.max(d);
            near = near.min(d);
        }
    }
    assert!(
        near > Fx::from_int(100) && far < Fx::from_int(260),
        "orbit band {near:?}..{far:?}"
    );
    w.state.units.pos[base] = FxVec2::from_ints(1100, 1000);
    w.tick(&[]).unwrap();
    assert_eq!(
        w.state.orders.front(&w.state.units, scout).unwrap().pos,
        w.state.units.pos[base]
    );
    w.tick(&[cmd(Command::Stop { units: vec![id] })]).unwrap();
    assert!(w.state.orders.front(&w.state.units, scout).is_none());
}
#[test]
fn carrier_pays_builds_four_drones_and_reclaims_only_inside_radius() {
    let mut w = world();
    w.state.players[0].mass = Fx::from_int(100);
    w.state.players[0].energy = Fx::from_int(100000);
    w.state.players[0].mass_capacity = Fx::from_int(20000);
    w.state.players[0].energy_capacity = Fx::from_int(200000);
    add(&mut w, "aster_commander", 0, 150, 150);
    let carrier = add(&mut w, "aster_t2_reclaim_carrier", 0, 900, 900);
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    let inside = w
        .state
        .wrecks
        .spawn(
            tank,
            FxVec2::from_ints(1050, 900),
            Fx::from_int(20),
            Angle::ZERO,
            Fx::from_int(300),
        )
        .unwrap();
    let outside = w
        .state
        .wrecks
        .spawn(
            tank,
            FxVec2::from_ints(1500, 900),
            Fx::from_int(20),
            Angle::ZERO,
            Fx::from_int(300),
        )
        .unwrap();
    let before = w.state.players[0].energy;
    for _ in 0..450 {
        w.tick(&[]).unwrap();
    }
    let parent = w.state.units.id(carrier);
    let children: Vec<_> = w
        .state
        .units
        .slots
        .iter()
        .filter(|&r| w.state.units.drone_parent[r] == parent)
        .collect();
    assert_eq!(children.len(), 4);
    assert!(w.state.players[0].energy < before);
    assert!(w.state.players[0].reclaimed_mass > Fx::from_int(100));
    assert!(
        !w.state.wrecks.slots.is_alive(inside) || w.state.wrecks.mass[inside] < Fx::from_int(100)
    );
    assert_eq!(w.state.wrecks.mass[outside], Fx::from_int(300));
    w.state.units.health[carrier] = Fx::ZERO;
    w.tick(&[]).unwrap();
    assert!(children.iter().all(|&r| !w.state.units.slots.is_alive(r)));
}
#[test]
fn carrier_reclaim_order_sends_every_drone() {
    let mut w = world();
    w.state.players[0].mass = Fx::from_int(100);
    w.state.players[0].energy = Fx::from_int(100000);
    w.state.players[0].mass_capacity = Fx::from_int(20000);
    w.state.players[0].energy_capacity = Fx::from_int(200000);
    add(&mut w, "aster_commander", 0, 150, 150);
    let carrier = add(&mut w, "aster_t2_reclaim_carrier", 0, 900, 900);
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    let far = w
        .state
        .wrecks
        .spawn(
            tank,
            FxVec2::from_ints(1500, 900),
            Fx::from_int(20),
            Angle::ZERO,
            Fx::from_int(300),
        )
        .unwrap();
    for _ in 0..450 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(w.state.wrecks.mass[far], Fx::from_int(300));
    let id = w.state.units.id(carrier);
    w.tick(&[cmd(Command::ReclaimWreck {
        units: vec![id],
        wreck: w.state.wrecks.slots.handle(far),
        queue: false,
    })])
    .unwrap();
    for _ in 0..400 {
        w.tick(&[]).unwrap();
    }
    assert!(w.state.wrecks.mass[far] < Fx::from_int(300) || !w.state.wrecks.slots.is_alive(far));
}
#[test]
fn empty_economy_cannot_create_free_drones() {
    let mut w = world();
    w.state.players[0].mass = Fx::ZERO;
    w.state.players[0].energy = Fx::ZERO;
    let c = add(&mut w, "aster_t2_reclaim_carrier", 0, 900, 900);
    for _ in 0..100 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(w.state.units.drone_progress[c], Fx::ZERO);
    assert_eq!(w.state.units.slots.live(), 1);
}
#[test]
fn sam_launches_vertically_then_curves_to_a_moving_aircraft() {
    let mut w = world();
    let sam = add(&mut w, "aster_t3_sam", 0, 600, 900);
    let target = add(&mut w, "aster_t2_reclaim_carrier", 1, 1050, 900);
    let target_id = w.state.units.id(target);
    w.state.units.flags[target] |= flag::PASSIVE;
    w.state.players[1].mass = Fx::ZERO;
    w.state.players[1].energy = Fx::ZERO;
    let go = [PlayerCommand {
        player: 1,
        command: Command::Move {
            units: vec![target_id],
            target: FxVec2::from_ints(1200, 1500),
            queue: false,
        },
    }];
    let mut launched = false;
    let mut turned = false;
    let mut hit = false;
    let bp = w.state.units.blueprint[sam];
    // The target starts in range, so the SAM may fire on the tick the move is given.
    for step in 0..241 {
        w.tick(if step == 0 { &go } else { &[] }).unwrap();
        for e in &w.events {
            match e {
                SimEvent::ShotFired { blueprint, vel, .. } if *blueprint == bp => {
                    assert_eq!(vel.x, Fx::ZERO);
                    assert!(vel.z > Fx::ZERO);
                    launched = true;
                }
                SimEvent::Impact {
                    blueprint,
                    on_unit,
                    on_shield,
                    ..
                } if *blueprint == bp && (*on_unit || *on_shield) => hit = true,
                _ => {}
            }
        }
        for i in 0..w.state.projectiles.len() {
            if w.state.projectiles.blueprint[i] == bp
                && w.state.projectiles.age[i] > 10
                && w.state.projectiles.vel[i].x > Fx::ONE
            {
                turned = true;
            }
        }
    }
    assert!(
        launched && turned && hit,
        "vertical={launched}, guided={turned}, hit={hit}"
    );
}
#[test]
fn support_intercepts_hostile_missiles_but_not_shells() {
    let mut w = world();
    let s = add(&mut w, "aster_t2_support", 0, 900, 900);
    w.state.players[0].energy = Fx::from_int(10000);
    let rocket = w.blueprints.id_of("aster_t1_rotor_gunship").unwrap();
    let pos = w.state.units.pos[s].extend(w.state.units.z[s]);
    for slot in [0, 1] {
        w.state
            .projectiles
            .spawn(
                pos,
                mc_core::FxVec3::ZERO,
                1,
                mc_sim::Handle::NONE,
                rocket,
                slot,
                30,
            )
            .unwrap();
    }
    w.tick(&[]).unwrap();
    assert!(!w.state.projectiles.weapon.contains(&1));
    assert!(w.state.projectiles.weapon.contains(&0));
    assert!(w
        .events
        .iter()
        .any(|e| matches!(e, SimEvent::MissileLased { killed: true, .. })));
}
#[test]
fn heavier_missiles_take_a_longer_laser_burst() {
    let mut w = world();
    let s = add(&mut w, "aster_t2_support", 0, 900, 900);
    w.state.players[0].free_build = true;
    let rack = w.blueprints.id_of("aster_t2_missile").unwrap();
    let pos = w.state.units.pos[s].extend(w.state.units.z[s]);
    w.state
        .projectiles
        .spawn(
            pos,
            mc_core::FxVec3::ZERO,
            1,
            mc_sim::Handle::NONE,
            rack,
            0,
            80,
        )
        .unwrap();
    // 80 casing hit points, 10 burned per tick: still flying through tick 7.
    for tick in 1..=7 {
        w.tick(&[]).unwrap();
        assert_eq!(w.state.projectiles.len(), 1, "alive on tick {tick}");
        assert!(
            w.events
                .iter()
                .any(|e| matches!(e, SimEvent::MissileLased { killed: false, .. })),
            "laser on tick {tick}"
        );
    }
    w.tick(&[]).unwrap();
    assert!(w.state.projectiles.is_empty());
    assert!(w
        .events
        .iter()
        .any(|e| matches!(e, SimEvent::MissileLased { killed: true, .. })));
}
#[test]
fn flak_splash_stays_in_the_air_and_does_not_hit_ground_hulls() {
    let mut w = world();
    let a = add(&mut w, "aster_t1_air_scout", 1, 900, 900);
    let g = add(&mut w, "aster_t1_tank", 1, 900, 900);
    w.state.units.flags[a] |= flag::PASSIVE;
    w.state.units.flags[g] |= flag::PASSIVE;
    let before = w.state.units.health[g];
    let bp = w.blueprints.id_of("aster_t3_shatter").unwrap();
    let pos = w.state.units.pos[a].extend(w.state.units.z[a] + Fx::ONE);
    w.state
        .projectiles
        .spawn(
            pos,
            mc_core::FxVec3::new(Fx::ONE, Fx::ZERO, Fx::ZERO),
            0,
            mc_sim::Handle::NONE,
            bp,
            0,
            30,
        )
        .unwrap();
    w.tick(&[]).unwrap();
    assert!(w.state.units.health[a] < Fx::from_int(55));
    assert_eq!(w.state.units.health[g], before);
}
fn drop_incendiary(w: &mut World, at: mc_core::FxVec3) {
    let bp = w.blueprints.id_of("aster_t2_fire_bomber").unwrap();
    w.state
        .projectiles
        .spawn(
            at,
            mc_core::FxVec3::new(Fx::ONE, Fx::ZERO, -Fx::ONE),
            0,
            mc_sim::Handle::NONE,
            bp,
            0,
            30,
        )
        .unwrap();
}
#[test]
fn incendiary_hit_burns_after_the_initial_explosion() {
    let mut w = world();
    let t = add(&mut w, "aster_t2_tank", 1, 900, 900);
    w.state.units.flags[t] |= flag::PASSIVE;
    let before = w.state.units.health[t];
    let pos = w.state.units.pos[t].extend(w.state.units.z[t] + Fx::from_int(2));
    drop_incendiary(&mut w, pos);
    w.tick(&[]).unwrap();
    assert_eq!(w.state.fires.len(), 1);
    assert_eq!(w.state.units.health[t], before - Fx::from_int(84));
    let after = w.state.units.health[t];
    for _ in 0..15 {
        w.tick(&[]).unwrap();
    }
    assert!(w.state.units.burn_ticks[t] > 0);
    assert!(w.state.units.health[t] < after - Fx::from_int(10));
}
#[test]
fn overlapping_incendiaries_stack_and_the_patch_has_an_edge() {
    let mut w = world();
    let t = add(&mut w, "aster_t2_tank", 1, 900, 900);
    let far = add(&mut w, "aster_t2_tank", 1, 200, 200);
    w.state.units.flags[t] |= flag::PASSIVE;
    w.state.units.flags[far] |= flag::PASSIVE;
    let pos = w.state.units.pos[t].extend(w.state.units.z[t] + Fx::from_int(2));
    drop_incendiary(&mut w, pos);
    drop_incendiary(&mut w, pos);
    let before = w.state.units.health[t];
    let far_before = w.state.units.health[far];
    w.tick(&[]).unwrap();
    assert_eq!(w.state.fires.len(), 2);
    assert_eq!(w.state.units.health[t], before - Fx::from_int(168));
    let after = w.state.units.health[t];
    for _ in 0..10 {
        w.tick(&[]).unwrap();
    }
    // Two patches, 30 each per second, for one second.
    assert_eq!(w.state.units.health[t], after - Fx::from_int(60));
    assert_eq!(w.state.units.health[far], far_before);
}
#[test]
fn carrier_and_orbit_snapshot_continue_deterministically() {
    let mut a = world();
    let mut b = world();
    a.state.players[0].mass = Fx::from_int(10000);
    a.state.players[0].energy = Fx::from_int(100000);
    add(&mut a, "aster_commander", 0, 150, 150);
    let c = add(&mut a, "aster_t2_reclaim_carrier", 0, 900, 900);
    let id = a.state.units.id(c);
    a.tick(&[cmd(Command::Orbit {
        units: vec![id],
        pos: FxVec2::from_ints(900, 900),
        target: mc_sim::Handle::NONE,
        radius: Fx::ZERO,
        queue: false,
    })])
    .unwrap();
    for _ in 0..70 {
        a.tick(&[]).unwrap();
    }
    b.restore(Heightfield::flat(256, 256, Fx::from_int(20)), &a.snapshot())
        .unwrap();
    for _ in 0..120 {
        assert_eq!(a.tick(&[]).unwrap(), b.tick(&[]).unwrap());
    }
}
#[test]
fn all_aa_structures_can_be_placed_on_water_and_stand_at_its_surface() {
    let mut w = world();
    let water = || {
        Heightfield::from_samples(
            256,
            256,
            vec![0; 257 * 257],
            Fx::from_int(-30),
            Fx::ONE,
            Fx::ZERO,
        )
    };
    let bp = w.blueprints.clone();
    let pool = w.pool.clone();
    let config = MatchConfig {
        seed: 3,
        players: vec![PlayerSetup {
            name: "you".into(),
            faction: "Aster".into(),
            ai: Default::default(),
            team: 0,
            controller: Controller::Human,
            start: 0,
        }],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let map = MapData {
        name: "water".into(),
        content_id: 2,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(500, 500)],
        props: vec![],
    };
    w = World::with_terrain(water(), map, bp, pool, &config).unwrap();
    for (i, key) in [
        "aster_t1_aa",
        "aster_t2_aa",
        "aster_t3_sam",
        "aster_t3_shatter",
    ]
    .iter()
    .enumerate()
    {
        let id = w.blueprints.id_of(key).unwrap();
        let pos = mc_sim::snap_to_build_grid(
            w.blueprints.unit(id),
            FxVec2::from_ints(500 + i as i32 * 100, 500),
        );
        assert!(w.can_place(w.blueprints.unit(id), pos), "{key}");
        let r = w.spawn_unit(id, 0, pos, Angle::ZERO, true).unwrap();
        assert_eq!(w.state.units.z[r], Fx::ZERO);
        assert!(!w.can_place(w.blueprints.unit(id), pos), "occupied {key}");
    }
    let land = w.blueprints.id_of("aster_t1_point_defense").unwrap();
    assert!(!w.can_place(w.blueprints.unit(land), FxVec2::from_ints(1200, 1200)));
}

#[test]
fn fire_fortress_completes_a_scattered_carpet_and_aa_guns_fire_independently() {
    let mut w = world();
    let bomber = add(&mut w, "aster_t2_fire_bomber", 0, 500, 900);
    let ground = add(&mut w, "aster_t3_land_factory", 1, 1050, 900);
    let air = add(&mut w, "aster_t1_air_scout", 1, 1000, 940);
    w.state.units.flags[ground] |= flag::PASSIVE | flag::INVULNERABLE;
    w.state.units.flags[air] |= flag::PASSIVE | flag::INVULNERABLE;
    let id = w.state.units.id(bomber);
    let target = w.state.units.id(ground);
    w.tick(&[cmd(Command::Attack {
        units: vec![id],
        target,
        queue: false,
    })])
    .unwrap();
    let bp = w.state.units.blueprint[bomber];
    let mut drops = Vec::new();
    let mut guns = 0;
    for _ in 0..600 {
        w.tick(&[]).unwrap();
        for e in &w.events {
            if let SimEvent::ShotFired {
                blueprint,
                weapon,
                vel,
                ..
            } = e
            {
                if *blueprint == bp {
                    if *weapon == 0 {
                        drops.push((w.tick_count(), *vel));
                    } else {
                        guns += 1;
                    }
                }
            }
        }
    }
    assert!(drops.len() >= 24, "{} drops", drops.len());
    assert!(
        drops.windows(2).any(|s| s[0].0 != s[1].0),
        "sequential release"
    );
    assert!(
        drops.windows(2).any(|s| s[0].1 != s[1].1),
        "inaccurate carpet"
    );
    assert!(guns > 0, "fortress AA must fire during bombing");
}
#[test]
fn rotor_gunship_fires_both_weapons_from_low_altitude() {
    let mut w = world();
    let gunship = add(&mut w, "aster_t1_rotor_gunship", 0, 750, 900);
    let target = add(&mut w, "aster_t1_tank", 1, 850, 900);
    w.state.units.flags[target] |= flag::PASSIVE | flag::INVULNERABLE;
    let mut weapons = [false; 2];
    let bp = w.state.units.blueprint[gunship];
    for _ in 0..120 {
        w.tick(&[]).unwrap();
        for e in &w.events {
            if let SimEvent::ShotFired {
                blueprint, weapon, ..
            } = e
            {
                if *blueprint == bp {
                    weapons[*weapon as usize] = true;
                }
            }
        }
    }
    assert!(weapons.iter().all(|b| *b));
    assert!(
        w.state.units.z[gunship] - w.terrain.height_at(w.state.units.pos[gunship])
            < Fx::from_int(50)
    );
    assert!(
        w.state.units.speed[gunship] > Fx::from_int(10),
        "circles its target while firing"
    );
}
#[test]
fn lost_drone_is_replaced_and_projectile_guidance_survives_snapshot() {
    let mut a = world();
    let mut b = world();
    a.state.players[0].free_build = true;
    let carrier = add(&mut a, "aster_t2_reclaim_carrier", 0, 900, 900);
    let parent = a.state.units.id(carrier);
    for _ in 0..230 {
        a.tick(&[]).unwrap();
    }
    let drone = a
        .state
        .units
        .slots
        .iter()
        .find(|&r| a.state.units.drone_parent[r] == parent)
        .unwrap();
    let old = a.state.units.id(drone);
    a.state.units.health[drone] = Fx::ZERO;
    for _ in 0..60 {
        a.tick(&[]).unwrap();
    }
    assert!(a.state.units.row(old).is_none());
    assert_eq!(
        a.state
            .units
            .slots
            .iter()
            .filter(|&r| a.state.units.drone_parent[r] == parent)
            .count(),
        4
    );
    let enemy = add(&mut a, "aster_t1_air_scout", 1, 1300, 900);
    let sam = a.blueprints.id_of("aster_t3_sam").unwrap();
    a.state
        .projectiles
        .spawn(
            FxVec2::from_ints(950, 900).extend(Fx::from_int(150)),
            mc_core::FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::from_int(52)),
            0,
            parent,
            sam,
            0,
            200,
        )
        .unwrap();
    let i = a.state.projectiles.len() - 1;
    a.state.projectiles.target[i] = a.state.units.id(enemy);
    a.state.projectiles.age[i] = 7;
    b.restore(Heightfield::flat(256, 256, Fx::from_int(20)), &a.snapshot())
        .unwrap();
    for _ in 0..80 {
        assert_eq!(a.tick(&[]).unwrap(), b.tick(&[]).unwrap());
    }
}

#[test]
fn redesigned_factory_beams_leave_the_visible_assembly_heads() {
    let mut w = world();
    w.state.players[0].free_build = true;
    let f = add(&mut w, "aster_t3_air_factory", 0, 900, 900);
    let id = w.state.units.id(f);
    let bp = w.blueprints.id_of("aster_t1_air_scout").unwrap();
    w.tick(&[cmd(Command::Produce {
        factories: vec![id],
        blueprint: bp,
        count: 1,
    })])
    .unwrap();
    w.tick(&[]).unwrap();
    let mut frame = mc_sim::RenderFrame::default();
    w.write_render_frame(None, &mut frame);
    let heads: Vec<_> = frame
        .projectiles
        .iter()
        .filter(|p| p.color & mc_sim::mirror::PROJECTILE_BEAM != 0)
        .map(|p| p.prev_pos)
        .collect();
    // Every beam leaves one of the fabricator tips the model stands on its mounts.
    let factory = mc_sim::print_heads::factory_heads("factory_air").unwrap();
    let tips: Vec<[f32; 3]> = factory
        .heads
        .iter()
        .map(|h| mc_sim::print_heads::nozzle(h, factory.aim))
        .collect();
    assert_eq!(heads.len(), tips.len());
    let origin = [900.0, 900.0, w.state.units.z[f].to_f32()];
    for p in heads {
        let local = [p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]];
        assert!(
            tips.iter()
                .any(|t| (0..3).all(|i| (t[i] - local[i]).abs() < 0.05)),
            "beam from {local:?}, not a fabricator tip"
        );
    }
}

#[test]
fn tempest_uses_all_fixed_tubes_and_curves_while_accelerating() {
    let mut w = world();
    let aa = add(&mut w, "aster_t2_aa", 0, 600, 900);
    let target = add(&mut w, "aster_t2_reclaim_carrier", 1, 950, 1050);
    w.state.units.flags[target] |= flag::PASSIVE | flag::INVULNERABLE;
    w.state.units.heading[aa] = Angle::from_degrees(37);
    let bp = w.state.units.blueprint[aa];
    let cruise = w.blueprints.unit(bp).weapons[0].projectile_speed / 10;
    let origin = w.state.units.pos[aa];
    let mut mouths = Vec::new();
    let mut speeds = std::collections::BTreeMap::new();
    for _ in 0..100 {
        w.tick(&[]).unwrap();
        for e in &w.events {
            if let SimEvent::ShotFired {
                pos,
                blueprint,
                vel,
                ..
            } = e
            {
                if *blueprint == bp && mouths.len() < 16 {
                    let local = (pos.xy() - origin)
                        .rotate(Angle(0u16.wrapping_sub(Angle::from_degrees(37).0)));
                    mouths.push((local.x.to_f32(), local.y.to_f32()));
                    assert!(vel.length() < cruise / 5, "must leave the tube slowly");
                }
            }
        }
        for i in 0..w.state.projectiles.len() {
            let p = &w.state.projectiles;
            if p.blueprint[i] == bp {
                let age = p.age[i];
                if age == 1 {
                    assert!(p.vel[i].xy().length() > Fx::ZERO, "must arc on first step");
                }
                speeds.entry(age).or_insert(p.vel[i].length());
            }
        }
        if mouths.len() == 16 && speeds.contains_key(&11) {
            break;
        }
    }
    assert_eq!(mouths.len(), 16);
    for (i, (x, y)) in mouths.iter().enumerate() {
        assert!(
            (*x - (-3.0 + 2.0 * (i / 4) as f32)).abs() < 0.02,
            "{mouths:?}"
        );
        assert!(
            (*y - (-3.0 + 2.0 * (i % 4) as f32)).abs() < 0.02,
            "{mouths:?}"
        );
    }
    assert!(speeds[&1] < speeds[&5] && speeds[&5] < speeds[&11]);
    assert!((speeds[&11] - cruise).abs() < Fx::ratio(1, 100));
}

#[test]
fn sam_cold_lobs_aims_and_holds_until_ignition() {
    use mc_sim::mirror::{RenderFrame, PROJECTILE_COLD, PROJECTILE_MISSILE};
    let mut w = world();
    let sam = add(&mut w, "aster_t3_sam", 0, 600, 900);
    let target = add(&mut w, "aster_t2_reclaim_carrier", 1, 1200, 900);
    w.state.units.flags[target] |= flag::PASSIVE | flag::INVULNERABLE;
    let bp = w.state.units.blueprint[sam];
    let cold = w.blueprints.unit(bp).weapons[0].cold_launch_ticks;
    let cruise = w.blueprints.unit(bp).weapons[0].projectile_speed / 10;
    let mut ignition = 0;
    let mut saw_cold = false;
    let mut saw_powered = false;
    let mut saw_aim = false;
    let mut saw_render_aim = false;
    let mut launch_x = None;
    let mut vz = std::collections::BTreeMap::new();
    let mut frame = RenderFrame::default();
    for _ in 0..60 {
        w.tick(&[]).unwrap();
        for e in &w.events {
            if let SimEvent::MissileIgnited {
                pos,
                vel,
                blueprint,
                ..
            } = e
            {
                if *blueprint == bp {
                    ignition += 1;
                    assert!(pos.z > w.state.units.z[sam] + Fx::from_int(90));
                    assert!((pos.x - launch_x.expect("the lob started")).abs() < Fx::ONE);
                    let aim = w.state.units.pos[target].extend(
                        w.state.units.z[target]
                            + w.blueprints.unit(w.state.units.blueprint[target]).height / 2,
                    );
                    assert!(vel.normalize().dot((aim - *pos).normalize()) > Fx::ratio(95, 100));
                    assert!(
                        vel.xy().length() > Fx::ZERO,
                        "the burn is what starts the intercept"
                    );
                }
            }
        }
        for i in 0..w.state.projectiles.len() {
            let p = &w.state.projectiles;
            if p.blueprint[i] != bp {
                continue;
            }
            if p.age[i] == 1 {
                launch_x = Some(p.pos[i].x);
            }
            if p.age[i] <= cold {
                assert_eq!(
                    p.vel[i].xy().length(),
                    Fx::ZERO,
                    "the lob does not chase until ignition"
                );
                vz.entry(p.age[i]).or_insert(p.vel[i].z);
            }
            if p.age[i] <= 2 {
                assert!(p.aim[i].z > Fx::ratio(9, 10), "it clears the tube nose-up");
            }
            if p.age[i] + 4 >= cold && p.age[i] <= cold {
                saw_aim |= p.aim[i].x > Fx::ratio(3, 5);
            }
            if p.age[i] > cold + 5 {
                assert!(p.vel[i].length() > cruise / 2);
            }
        }
        w.write_render_frame(None, &mut frame);
        for p in &frame.projectiles {
            if p.color & PROJECTILE_MISSILE != 0 {
                if p.color & PROJECTILE_COLD != 0 {
                    saw_cold = true;
                    saw_render_aim |= p.aim[0] > 0.6;
                } else {
                    saw_powered = true;
                }
            }
        }
    }
    assert_eq!(ignition, 1);
    assert!(saw_cold && saw_aim && saw_render_aim && saw_powered);
    let drop = vz[&1] - vz[&2];
    assert!(drop > Fx::ZERO, "gravity is pulling from the first step");
    for age in 1..cold {
        let step = vz[&age] - vz[&(age + 1)];
        assert!(
            (step - drop).abs() < Fx::ratio(1, 50),
            "the toss stays under constant gravity"
        );
    }
    assert!(vz[&1] > vz[&(cold / 2)] && vz[&(cold / 2)] > vz[&cold]);
    assert!(
        vz[&cold] < Fx::ZERO,
        "it has crested and started to fall before ignition"
    );
}

#[test]
fn sparrow_hits_circling_aircraft_reliably() {
    for key in [
        "aster_t1_air_scout",
        "aster_t1_interceptor",
        "aster_t1_rotor_gunship",
    ] {
        let mut w = world();
        let gun = add(&mut w, "aster_t1_aa", 0, 900, 900);
        let target = add(&mut w, key, 1, 1080, 900);
        let motion = w
            .blueprints
            .unit(w.state.units.blueprint[target])
            .motion
            .unwrap();
        w.state.units.z[target] = Fx::from_int(20) + motion.altitude;
        w.state.units.heading[target] = Angle::from_degrees(90);
        w.state.units.speed[target] = motion.speed;
        w.state.units.flags[target] |= flag::PASSIVE;
        let id = w.state.units.id(target);
        w.tick(&[PlayerCommand {
            player: 1,
            command: Command::Orbit {
                units: vec![id],
                pos: FxVec2::from_ints(900, 900),
                target: mc_sim::tables::Handle::NONE,
                radius: Fx::ZERO,
                queue: false,
            },
        }])
        .unwrap();
        let gun_bp = w.state.units.blueprint[gun];
        let damage = w.blueprints.unit(gun_bp).weapons[0].damage;
        w.state.units.flags[gun] |= flag::PASSIVE;
        for _ in 0..60 {
            w.tick(&[]).unwrap();
        }
        w.state.units.flags[gun] &= !flag::PASSIVE;
        let mut shots = 0;
        let mut hits = 0;
        for tick in 0..620 {
            // Stop firing and let the final measured shots finish their flight.
            if tick == 600 {
                w.state.units.flags[gun] |= flag::PASSIVE;
            }
            w.state.units.health[target] = Fx::from_int(10000);
            w.tick(&[]).unwrap();
            shots += w
                .events
                .iter()
                .filter(|e| {
                    matches!(e,
                SimEvent::ShotFired { blueprint, .. } if *blueprint == gun_bp)
                })
                .count();
            hits += ((Fx::from_int(10000) - w.state.units.health[target]) / damage).round_int();
        }
        eprintln!("Sparrow vs {key}: {hits}/{shots} hits");
        assert!(shots >= 50, "Sparrow rarely fired at {key}: {shots}");
        assert!(
            hits as usize * 100 >= shots * 75,
            "Sparrow hit fewer than 75% of shots against {key}: {hits}/{shots}"
        );
    }
}

#[test]
fn steep_aa_shots_hit_the_vertical_hull_but_near_misses_do_not() {
    for (offset, rise, vertical_step, should_hit) in [
        (-3, -10, 24, true),
        (-3, 14, -24, true),
        (9, -10, 24, false),
        (-3, 14, 24, false),
    ] {
        let mut w = world();
        let gun = add(&mut w, "aster_t1_aa", 0, 900, 900);
        let target = add(&mut w, "aster_t1_rotor_gunship", 1, 1000, 900);
        w.state.units.flags[gun] |= flag::PASSIVE;
        w.state.units.flags[target] |= flag::PASSIVE;
        w.state.units.z[target] = Fx::from_int(58);
        let before = w.state.units.health[target];
        w.state
            .projectiles
            .spawn(
                FxVec2::from_ints(1000 + offset, 900).extend(Fx::from_int(58 + rise)),
                FxVec2::from_ints(9, 0).extend(Fx::from_int(vertical_step)),
                0,
                w.state.units.id(gun),
                w.state.units.blueprint[gun],
                0,
                10,
            )
            .unwrap();
        w.tick(&[]).unwrap();
        assert_eq!(
            w.state.units.health[target] < before,
            should_hit,
            "offset {offset}, rise {rise}, vertical step {vertical_step}"
        );
    }
}
