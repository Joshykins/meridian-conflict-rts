//! Hull repair: cost, idle builders, the mint-green beam.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, RenderFrame, UnitId, World};
use std::path::Path;
use std::sync::Arc;

const ENGINEER: &str = "aster_t1_engineer";
const TANK: &str = "aster_t1_tank";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "repair".into(),
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
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    w.tick(&[
        spawn(&w, 0, "aster_mass_storage", 200, 0),
        spawn(&w, 0, "aster_energy_storage", 260, 0),
    ])
    .unwrap();
    fill_banks(&mut w);
    w
}

fn fill_banks(w: &mut World) {
    let p = &mut w.state.players[0];
    p.mass = p.mass_capacity;
    p.energy = p.energy_capacity;
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

fn spawn(w: &World, owner: u8, key: &str, x: i32, flags: u16) -> PlayerCommand {
    cmd(Command::DebugSpawn {
        owner,
        blueprint: w.blueprints.id_of(key).unwrap(),
        pos: FxVec2::from_ints(x, 512),
        heading: Angle::ZERO,
        count: 1,
        flags,
        build: 1000,
    })
}

fn ids(w: &World, owner: u8, key: &str) -> Vec<UnitId> {
    let (u, bp) = (&w.state.units, w.blueprints.id_of(key).unwrap());
    u.slots
        .iter()
        .filter(|&r| u.owner[r] == owner && u.blueprint[r] == bp)
        .map(|r| u.id(r))
        .collect()
}

fn health_of(w: &World, id: UnitId) -> Fx {
    let row = w.state.units.row(id).unwrap();
    w.state.units.health[row]
}

#[test]
fn repair_costs_a_quarter_of_the_build_and_puts_a_repair_beam_on_the_hull() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, ENGINEER, 500, 0),
        spawn(&w, 0, TANK, 540, flag::PASSIVE),
    ])
    .unwrap();
    let (engineer, tank) = (ids(&w, 0, ENGINEER), ids(&w, 0, TANK)[0]);
    w.tick(&[cmd(Command::DebugDamage {
        units: vec![tank],
        permille: 500,
    })])
    .unwrap();
    let hurt = health_of(&w, tank);
    w.tick(&[cmd(Command::Assist {
        units: engineer,
        target: tank,
        queue: false,
    })])
    .unwrap();

    let mut frame = RenderFrame::default();
    let mut saw_beam = false;
    for _ in 0..40 {
        fill_banks(&mut w);
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        if frame
            .beams
            .iter()
            .any(|b| b.kind == mc_sim::repair::BEAM_REPAIR)
        {
            saw_beam = true;
            break;
        }
    }
    assert!(
        saw_beam,
        "a repair beam is drawn once the arm is on the hull"
    );
    assert!(
        frame.build_sources.is_empty(),
        "repair is not a construction beam"
    );
    assert!(health_of(&w, tank) > hurt, "the tank is gaining health");

    let tank_bp = w.blueprints.unit_by_key(TANK).unwrap();
    let power = w
        .blueprints
        .unit_by_key(ENGINEER)
        .unwrap()
        .builder
        .as_ref()
        .unwrap()
        .power;
    let full = tank_bp.cost_mass * power / tank_bp.build_time;
    let demand = w.state.players[0].mass_demand;
    assert!(
        demand > full * Fx::ratio(24, 100) && demand <= full * Fx::ratio(1, 4),
        "repair asks for a quarter of the build's mass ({demand:?} of {full:?})"
    );
}

#[test]
fn an_idle_engineer_mends_allies_in_reach_and_never_walks() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, ENGINEER, 500, 0),
        spawn(&w, 0, TANK, 540, flag::PASSIVE),
        spawn(&w, 0, TANK, 900, flag::PASSIVE),
    ])
    .unwrap();
    let tanks = ids(&w, 0, TANK);
    w.tick(&[cmd(Command::DebugDamage {
        units: tanks.clone(),
        permille: 500,
    })])
    .unwrap();
    let near = health_of(&w, tanks[0]);
    let far = health_of(&w, tanks[1]);

    let mut mended = false;
    for _ in 0..80 {
        fill_banks(&mut w);
        w.tick(&[]).unwrap();
        if health_of(&w, tanks[0]) > near {
            mended = true;
            break;
        }
    }
    assert!(mended, "the idle engineer never started the nearby tank");
    let engineer = w.state.units.row(ids(&w, 0, ENGINEER)[0]).unwrap();
    assert_eq!(
        w.state.units.pos[engineer],
        FxVec2::from_ints(500, 512),
        "it never left its post"
    );
    assert_eq!(
        w.state.units.order_head[engineer],
        mc_sim::tables::NO_ORDER,
        "and is still idle to whoever asks"
    );
    assert_eq!(
        health_of(&w, tanks[1]),
        far,
        "the far tank is out of reach and stays hurt"
    );
}

#[test]
fn idle_repair_leaves_enemies_alone_and_prefers_a_wounded_friend_to_a_wreck() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, ENGINEER, 500, 0),
        spawn(&w, 0, TANK, 532, flag::PASSIVE),
        spawn(&w, 1, TANK, 548, flag::PASSIVE),
        spawn(&w, 1, TANK, 556, flag::PASSIVE),
    ])
    .unwrap();
    let friend = ids(&w, 0, TANK)[0];
    let enemies = ids(&w, 1, TANK);
    w.tick(&[cmd(Command::DebugDamage {
        units: vec![friend, enemies[0]],
        permille: 500,
    })])
    .unwrap();
    w.tick(&[cmd(Command::DebugDamage {
        units: vec![enemies[1]],
        permille: 1000,
    })])
    .unwrap();
    w.tick(&[]).unwrap();
    assert_eq!(w.state.wrecks.slots.iter().count(), 1);
    let enemy_hurt = health_of(&w, enemies[0]);
    let wreck_mass = w.state.wrecks.mass[w.state.wrecks.slots.iter().next().unwrap()];

    let start = health_of(&w, friend);
    for _ in 0..80 {
        fill_banks(&mut w);
        w.tick(&[]).unwrap();
        if health_of(&w, friend) > start {
            break;
        }
    }
    assert!(
        health_of(&w, friend) > start,
        "a wounded ally is mended without an order"
    );
    assert_eq!(
        health_of(&w, enemies[0]),
        enemy_hurt,
        "an enemy is not patched unasked"
    );
    assert_eq!(
        w.state.wrecks.mass[w.state.wrecks.slots.iter().next().unwrap()],
        wreck_mass,
        "the wreck waits while there is a hull to mend"
    );
}

#[test]
fn an_armed_builder_leaves_hulls_alone_while_an_enemy_is_about() {
    let mut w = world();
    let acu = w
        .blueprints
        .unit(w.blueprints.factions[0].commander)
        .key
        .clone();
    w.tick(&[
        spawn(&w, 0, &acu, 500, 0),
        spawn(&w, 0, TANK, 530, flag::PASSIVE),
        spawn(&w, 1, TANK, 700, flag::PASSIVE | flag::INVULNERABLE),
    ])
    .unwrap();
    let friend = ids(&w, 0, TANK)[0];
    let threat = ids(&w, 1, TANK)[0];
    w.tick(&[cmd(Command::DebugDamage {
        units: vec![friend],
        permille: 500,
    })])
    .unwrap();
    let hurt = health_of(&w, friend);
    let mut frame = RenderFrame::default();
    for _ in 0..80 {
        fill_banks(&mut w);
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        assert!(
            !frame
                .beams
                .iter()
                .any(|b| b.kind == mc_sim::repair::BEAM_REPAIR),
            "its torso stays on the enemy"
        );
    }
    assert_eq!(health_of(&w, friend), hurt);

    w.tick(&[cmd(Command::DebugRemove {
        units: vec![threat],
    })])
    .unwrap();
    let mut mended = false;
    for _ in 0..80 {
        fill_banks(&mut w);
        w.tick(&[]).unwrap();
        if health_of(&w, friend) > hurt {
            mended = true;
            break;
        }
    }
    assert!(mended, "the enemy gone, it gets on with the hull");
}

#[test]
fn idle_repair_leaves_a_hull_a_friend_is_reclaiming() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, ENGINEER, 500, 0),
        spawn(&w, 0, ENGINEER, 560, 0),
        spawn(&w, 0, TANK, 540, flag::PASSIVE),
        spawn(&w, 0, TANK, 590, flag::PASSIVE),
    ])
    .unwrap();
    let engineers = ids(&w, 0, ENGINEER);
    let tanks = ids(&w, 0, TANK);
    let (scrap, mend) = (tanks[0], tanks[1]);
    w.tick(&[cmd(Command::DebugDamage {
        units: tanks.clone(),
        permille: 500,
    })])
    .unwrap();
    w.tick(&[cmd(Command::ReclaimUnit {
        units: vec![engineers[0]],
        target: scrap,
        queue: false,
    })])
    .unwrap();
    let start_scrap = health_of(&w, scrap);
    let start_mend = health_of(&w, mend);

    let mut mended = false;
    let mut scrap_high = start_scrap;
    for _ in 0..80 {
        fill_banks(&mut w);
        w.tick(&[]).unwrap();
        let Some(row) = w.state.units.row(scrap) else {
            break;
        };
        let scrap_now = w.state.units.health[row];
        assert!(
            scrap_now <= scrap_high,
            "a friend is taking it apart, so nobody patches it"
        );
        scrap_high = scrap_now;
        for r in w.state.units.slots.iter() {
            if w.state.units.flags[r] & flag::REPAIRING != 0 {
                assert_ne!(
                    w.state.units.build_target[r], scrap,
                    "idle repair must not pick the reclaim target"
                );
            }
        }
        if health_of(&w, mend) > start_mend {
            mended = true;
        }
    }
    assert!(mended, "the other wounded friend is still mended");
    assert!(scrap_high < start_scrap, "the reclaim went ahead");
}
