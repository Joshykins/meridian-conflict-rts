//! The Breacher (`aster_t4_breacher`): a tech 4 assault walker with a gatling-breach
//! cannon on each arm, each spinning up and laying on its mark on its own, and thermobaric
//! rocket pods that set the ground ahead alight.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, UnitId};
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const BREACHER: &str = "aster_t4_breacher";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(
            &Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../data"),
        )
        .unwrap(),
    );
    let terrain = Heightfield::flat(1024, 1024, Fx::from_int(20));
    let map = MapData {
        doodads: Default::default(),
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
        seed: 7,
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

fn seconds(s: u32) -> usize {
    (s * TICKS_PER_SECOND) as usize
}

/// The arms' weapon slots (the two rotary guns, in order).
fn arms(w: &World) -> [usize; 2] {
    let bp = w.blueprints.unit(w.blueprints.id_of(BREACHER).unwrap());
    let rotary: Vec<usize> = (0..bp.weapons.len())
        .filter(|&i| bp.weapons[i].spin_ticks > 0)
        .collect();
    assert_eq!(rotary.len(), 2, "two rotary arms");
    [rotary[0], rotary[1]]
}

/// Each arm keeps its own spin: it spins up a tick at a time while it has a mark (two
/// guns sharing one spin would climb two a tick), and each fires.
#[test]
fn each_arm_spins_up_on_its_own() {
    let mut w = world();
    let breacher = add(&mut w, BREACHER, 0, 1000, 3000, 0);
    for i in 0..4 {
        add(&mut w, "aster_t3_land_factory", 1, 1450, 2850 + i * 100, 0);
    }
    let bp = w.blueprints.id_of(BREACHER).unwrap();
    let arms = arms(&w);
    let full = w.blueprints.unit(bp).weapons[arms[0]].spin_ticks;
    let mut last = [0u16; 2];
    let mut shots = [0usize; 2];
    let mut topped = [false; 2];
    for _ in 0..seconds(12) {
        w.tick(&[]).unwrap();
        let spin = w.state.units.spin[row(&w, breacher)];
        for slot in 0..2 {
            let now = spin[slot][0];
            assert!(
                now <= last[slot] + 1,
                "arm {slot} spun {} ticks in one",
                now - last[slot]
            );
            topped[slot] |= now == full;
            last[slot] = now;
            shots[slot] += w
                .events
                .iter()
                .filter(|e| {
                    matches!(e,
                    SimEvent::ShotFired { blueprint, weapon, .. }
                        if *blueprint == bp && *weapon as usize == arms[slot])
                })
                .count();
        }
    }
    assert_eq!(topped, [true, true], "both arms reach full spin");
    assert!(
        shots[0] >= 15 && shots[1] >= 15,
        "both arms fire: {shots:?}"
    );
}

/// With two marks a little either side of the nose, the arms split them: each lays on a
/// mark of its own within its sway of the torso.
#[test]
fn the_arms_lay_on_marks_of_their_own() {
    let mut w = world();
    let breacher = add(&mut w, BREACHER, 0, 1000, 3000, 0);
    let left = add(&mut w, "aster_t3_land_factory", 1, 1420, 3110, 0);
    let right = add(&mut w, "aster_t3_land_factory", 1, 1420, 2890, 0);
    let arms = arms(&w);
    let mut split = 0;
    for _ in 0..seconds(10) {
        w.tick(&[]).unwrap();
        let Some(r) = w.state.units.row(breacher) else {
            break;
        };
        let marks = [
            w.state.units.weapon_target[r][arms[0]],
            w.state.units.weapon_target[r][arms[1]],
        ];
        if marks.contains(&left) && marks.contains(&right) {
            split += 1;
        }
    }
    assert!(
        split > seconds(3),
        "the arms shared one mark ({split} ticks split)"
    );
}

/// One salvo from the pods lights a field of fires on the ground ahead, spread wide.
#[test]
fn the_rockets_set_the_ground_ahead_alight() {
    let mut w = world();
    add(&mut w, BREACHER, 0, 1000, 3000, 0);
    add(&mut w, "aster_t3_land_factory", 1, 1700, 3000, 0);
    for _ in 0..seconds(14) {
        w.tick(&[]).unwrap();
    }
    let fires = &w.state.fires;
    let lit: Vec<FxVec2> = (0..fires.len())
        .filter(|&i| fires.ticks[i] > 0)
        .map(|i| fires.pos[i])
        .collect();
    assert!(lit.len() >= 20, "only {} fires burning", lit.len());
    let ys = lit.iter().map(|p| p.y.floor_int());
    let (lo, hi) = (ys.clone().min().unwrap(), ys.max().unwrap());
    assert!(hi - lo >= 120, "the fires span only {} m across", hi - lo);
    assert!(
        lit.iter().all(|p| p.x.floor_int() > 1100),
        "every fire is ahead of the walker"
    );
}

/// Every new shot of the Breacher's weapon `weapon` this tick: where it left and its
/// velocity, in floats.
fn new_shots(w: &World, weapon: usize, seen: &mut Vec<u32>) -> Vec<([f32; 3], [f32; 3])> {
    let p = &w.state.projectiles;
    let bp = w.blueprints.id_of(BREACHER).unwrap();
    let mut out = Vec::new();
    for i in 0..p.len() {
        if p.blueprint[i] != bp || p.weapon[i] as usize != weapon || seen.contains(&p.serial[i]) {
            continue;
        }
        seen.push(p.serial[i]);
        out.push((p.origin[i].to_f32(), p.vel[i].to_f32()));
    }
    out
}

/// Each arm's barrels point where its shells go: the bore hangs beside and below the
/// shoulder it turns and pitches about, and is laid through the mark, so a shell (fired
/// from the muzzle at the mark) leaves along the barrels, within the gun's spread. The
/// torso the arms lead still faces the mark, so the pods fire too.
#[test]
fn the_arms_shoot_down_their_barrels() {
    let mut w = world();
    let breacher = add(&mut w, BREACHER, 0, 1000, 3000, 0);
    add(&mut w, "aster_t3_land_factory", 1, 1260, 3040, 0);
    let arms = arms(&w);
    let mut seen = Vec::new();
    let mut checked = 0;
    let mut rockets = 0;
    for _ in 0..seconds(8) {
        w.tick(&[]).unwrap();
        let r = row(&w, breacher);
        rockets += new_shots(&w, 0, &mut seen).len();
        for (slot, &arm) in arms.iter().enumerate() {
            for (_, vel) in new_shots(&w, arm, &mut seen) {
                let units = &w.state.units;
                let yaw = (units.heading[r] + units.weapon_yaw[r][arm]).to_radians_f32();
                let pitch = Angle::ZERO.delta_to(units.arm_pitch[r][2 + arm]) as f32
                    * (std::f32::consts::TAU / 65536.0);
                let barrel = [
                    pitch.cos() * yaw.cos(),
                    pitch.cos() * yaw.sin(),
                    pitch.sin(),
                ];
                let speed = (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt();
                let dot = (barrel[0] * vel[0] + barrel[1] * vel[1] + barrel[2] * vel[2]) / speed;
                let off = dot.clamp(-1.0, 1.0).acos().to_degrees();
                assert!(
                    off < 2.5,
                    "arm {slot}'s shell left {off:.1} degrees off its barrels"
                );
                checked += 1;
            }
        }
    }
    assert!(checked >= 10, "only {checked} shells checked");
    assert!(
        rockets > 0,
        "the pods never fired with the arms leading the torso"
    );
}

/// The rockets leave the launchers' cells as they stand in the hump, which never pitch,
/// and along them (`fixed_rack`, `rake`). The mark is out of the arms' reach, so the pods
/// lead the torso: a leading rack that pitched about the shoulder threw its rockets from
/// well ahead of the hump.
#[test]
fn the_rockets_leave_the_cells_along_them() {
    let mut w = world();
    let breacher = add(&mut w, BREACHER, 0, 1000, 3000, 0);
    add(&mut w, "aster_t3_land_factory", 1, 1950, 3000, 0);
    let bp = w
        .blueprints
        .unit(w.blueprints.id_of(BREACHER).unwrap())
        .clone();
    let pods = &bp.weapons[0];
    let rake = pods.rake.to_radians_f32();
    let mut seen = Vec::new();
    let mut checked = 0;
    for _ in 0..seconds(12) {
        w.tick(&[]).unwrap();
        let r = row(&w, breacher);
        let units = &w.state.units;
        let torso = (units.heading[r] + units.weapon_yaw[r][0]).to_radians_f32();
        let at = units.pos[r].extend(units.z[r]).to_f32();
        for (origin, vel) in new_shots(&w, 0, &mut seen) {
            let near = pods.muzzles.iter().any(|m| {
                let m = m.to_f32();
                let x = at[0] + m[0] * torso.cos() - m[1] * torso.sin();
                let y = at[1] + m[0] * torso.sin() + m[1] * torso.cos();
                let z = at[2] + m[2];
                ((origin[0] - x).powi(2) + (origin[1] - y).powi(2) + (origin[2] - z).powi(2)).sqrt()
                    < 0.5
            });
            assert!(near, "a rocket left from {origin:?}, not a cell");
            let climb = vel[2].atan2((vel[0] * vel[0] + vel[1] * vel[1]).sqrt());
            assert!(
                (climb - rake).abs() < 0.05,
                "a rocket left at {climb:.2} rad, the cells are raked {rake:.2}"
            );
            checked += 1;
        }
    }
    assert!(checked >= 16, "only {checked} rockets checked");
}
