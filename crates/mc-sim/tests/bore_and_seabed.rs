//! The tech 3 and 4 additions: the Paladin walks the seabed (hidden under the sea, its
//! projectors silent, its shin torpedoes hunting ships), the Argon Electric Bore strikes
//! down its tracer's channel (the AEB-2 searing everything along it), and the Fulgur
//! is raised on a lot by combat engineers and drives off it.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, UnitId, World};
use std::path::Path;
use std::sync::Arc;

/// Water level over the sea's bed (at zero): deep enough to close over a Paladin.
const WATER: i32 = 40;

fn blueprints() -> Arc<Blueprints> {
    Arc::new(Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap())
}

fn config() -> MatchConfig {
    let player = |name: &str, team| PlayerSetup {
        name: name.into(),
        faction: "Aster".into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start: team,
    };
    MatchConfig {
        seed: 11,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    }
}

fn map() -> MapData {
    MapData {
        name: "test".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(150, 300), FxVec2::from_ints(150, 1700)],
        props: Vec::new(),
    }
}

/// Deep sea (bed at zero, water at `WATER`), a shelf where the bed stands at `shelf`
/// for x from 1400 m to 1600 m, and dry land west of x = 320 m.
fn sea(shelf: u16) -> World {
    let mut samples = vec![0u16; 257 * 257];
    for y in 0..257 {
        for x in 0..257 {
            samples[y * 257 + x] = if x < 40 {
                60
            } else if (175..200).contains(&x) {
                shelf
            } else {
                0
            };
        }
    }
    let terrain =
        Heightfield::from_samples(256, 256, samples, Fx::ZERO, Fx::ONE, Fx::from_int(WATER));
    World::with_terrain(terrain, map(), blueprints(), Arc::new(Pool::new(1)), &config()).unwrap()
}

fn dry() -> World {
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    World::with_terrain(terrain, map(), blueprints(), Arc::new(Pool::new(1)), &config()).unwrap()
}

fn spawn(w: &mut World, key: &str, owner: u8, x: i32, y: i32, flags: u16) -> UnitId {
    let bp = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(bp, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.flags[row] |= flags;
    w.state.units.id(row)
}

fn row(w: &World, id: UnitId) -> usize {
    w.state.units.row(id).expect("alive")
}

fn health(w: &World, id: UnitId) -> Fx {
    w.state.units.health[row(w, id)]
}

/// Weapon slots `shooter` fired from this tick.
fn fired(w: &World, shooter_bp: mc_data::BlueprintId, owner: u8) -> Vec<u8> {
    w.events
        .iter()
        .filter_map(|e| match e {
            SimEvent::ShotFired { blueprint, weapon, owner: o, .. }
                if *blueprint == shooter_bp && *o == owner =>
            {
                Some(*weapon)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn a_paladin_under_the_sea_is_hidden_silent_and_torpedoes_ships() {
    let mut w = sea(0);
    let paladin = spawn(&mut w, "aster_t3_assault_bot", 0, 1000, 1000, 0);
    let frigate = spawn(&mut w, "aster_t1_frigate", 1, 1250, 1000, 0);
    let bp = w.blueprints.id_of("aster_t3_assault_bot").unwrap();
    let r = row(&w, paladin);
    assert!(
        w.state.units.z[r] + w.bp(r).height < Fx::from_int(WATER),
        "it stands on the bed, under the sea"
    );
    let full = health(&w, paladin);
    let mut torpedoes = 0;
    for _ in 0..900 {
        w.tick(&[]).unwrap();
        for slot in fired(&w, bp, 0) {
            assert_eq!(slot, 2, "under the sea only the shin tubes fire");
            torpedoes += 1;
        }
        if w.state.units.row(frigate).is_none() {
            break;
        }
    }
    assert!(torpedoes > 0, "the tubes fired");
    assert!(w.state.units.row(frigate).is_none(), "the frigate went down");
    assert!(health(&w, paladin) >= full, "the frigate's guns cannot reach it");
}

#[test]
fn a_barracuda_hears_a_paladin_on_the_bed_and_torpedoes_it() {
    let mut w = sea(0);
    let paladin = spawn(&mut w, "aster_t3_assault_bot", 1, 1250, 1000, flag::PASSIVE);
    spawn(&mut w, "aster_t1_submarine", 0, 1000, 1000, 0);
    let full = health(&w, paladin) + w.state.units.shield_hp[row(&w, paladin)];
    for _ in 0..600 {
        w.tick(&[]).unwrap();
    }
    let now = w
        .state
        .units
        .row(paladin)
        .map_or(Fx::ZERO, |r| w.state.units.health[r] + w.state.units.shield_hp[r]);
    assert!(now < full, "torpedoes reach a walker on the seabed");
}

#[test]
fn a_wading_paladin_fires_its_projectors_at_ships() {
    // On the shelf the bed is 30 m up: ten metres of water, the projectors well clear of it.
    let mut w = sea(30);
    let paladin = spawn(&mut w, "aster_t3_assault_bot", 0, 1500, 1000, 0);
    spawn(&mut w, "aster_t1_attack_boat", 1, 1500, 1200, flag::PASSIVE | flag::INVULNERABLE);
    let bp = w.blueprints.id_of("aster_t3_assault_bot").unwrap();
    let r = row(&w, paladin);
    assert!(w.state.units.z[r] + w.bp(r).height > Fx::from_int(WATER), "it wades");
    let mut slots = Vec::new();
    for _ in 0..200 {
        w.tick(&[]).unwrap();
        slots.extend(fired(&w, bp, 0));
    }
    assert!(slots.iter().any(|&s| s < 2), "the projectors fire from the shallows");
    assert!(slots.contains(&2), "so do the tubes, under ten metres of water");
}

#[test]
fn an_arbalest_strikes_down_its_tracers_channel() {
    let mut w = dry();
    let arbalest = spawn(&mut w, "aster_t3_sniper", 0, 300, 512, 0);
    spawn(&mut w, "aster_t2_tank", 1, 700, 512, flag::PASSIVE);
    let at = w.state.units.pos[row(&w, arbalest)];
    for _ in 0..400 {
        w.tick(&[]).unwrap();
        for e in &w.events {
            if let SimEvent::BoreDischarge { from, to, width, .. } = e {
                assert_eq!(*width, Fx::ZERO, "the Arbalest's bore strikes only where it lands");
                assert!(
                    from.xy().distance(at) < Fx::from_int(20),
                    "the channel starts at the muzzle, not at {from:?}"
                );
                assert!(to.xy().distance(FxVec2::from_ints(700, 512)) < Fx::from_int(20));
                return;
            }
        }
    }
    panic!("the Arbalest never fired");
}

#[test]
fn the_aeb2_sears_everything_along_its_channel() {
    let mut w = dry();
    let fulgur = spawn(&mut w, "aster_t4_assault_tank", 0, 100, 512, 0);
    // A column out past the compact bores' reach, the last in it the mark.
    let near = spawn(&mut w, "aster_t1_tank", 1, 460, 512, flag::PASSIVE);
    let mid = spawn(&mut w, "aster_t1_tank", 1, 560, 512, flag::PASSIVE);
    let mark = spawn(&mut w, "aster_t2_tank", 1, 680, 512, flag::PASSIVE);
    let stains = w.state.stains.len();
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::Attack { units: vec![fulgur], target: mark, queue: false },
    }])
    .unwrap();
    let mut discharged = false;
    for _ in 0..600 {
        w.tick(&[]).unwrap();
        if w.events.iter().any(|e| matches!(e, SimEvent::BoreDischarge { .. })) {
            discharged = true;
            break;
        }
    }
    assert!(discharged, "the Fulgur fired its bore");
    let hurt = |w: &World, id: UnitId| {
        w.state
            .units
            .row(id)
            .is_none_or(|r| w.state.units.health[r] < w.bp(r).health)
    };
    assert!(hurt(&w, near) && hurt(&w, mid), "the tanks in the channel's way are seared");
    assert!(w.state.stains.len() > stains + 10, "the ground along it is scorched");
}

#[test]
fn combat_engineers_raise_a_fulgur_that_drives_off_its_lot() {
    let mut w = dry();
    let builder = spawn(&mut w, "aster_t3_engineer", 0, 400, 400, 0);
    w.tick(&[PlayerCommand { player: 0, command: Command::DebugFreeBuild { player: 0, on: true } }])
        .unwrap();
    let fulgur = w.blueprints.id_of("aster_t4_assault_tank").unwrap();
    let site = FxVec2::from_ints(520, 520);
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::Build {
            units: vec![builder],
            blueprint: fulgur,
            pos: site,
            heading: Angle::ZERO,
            queue: false,
        },
    }])
    .unwrap();
    let mut built = None;
    for _ in 0..40_000 {
        w.tick(&[]).unwrap();
        if let Some(r) = w
            .state
            .units
            .slots
            .iter()
            .find(|&r| w.state.units.blueprint[r] == fulgur)
        {
            if !w.state.units.has_flag(r, flag::UNDER_CONSTRUCTION) {
                built = Some(w.state.units.id(r));
                break;
            }
        }
    }
    let tank = built.expect("the Fulgur was finished");
    let start = w.state.units.pos[row(&w, tank)];
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::Move {
            units: vec![tank],
            target: FxVec2::from_ints(900, 520),
            queue: false,
        },
    }])
    .unwrap();
    for _ in 0..300 {
        w.tick(&[]).unwrap();
    }
    let now = w.state.units.pos[row(&w, tank)];
    assert!(now.distance(start) > Fx::from_int(40), "it drove off its lot: {start:?} to {now:?}");
}

#[test]
fn both_bores_burn_the_whole_tree_corridor_without_harming_off_path_props() {
    use mc_map::{Prop, PropKind};
    for key in ["aster_t3_sniper", "aster_t4_assault_tank"] {
        let mut terrain_map = map();
        // Three-metre spacing exposes holes caused by the former ten-metre sampling.
        for x in (350..=720).step_by(3) {
            terrain_map.props.push(Prop { kind: PropKind::TreeConifer,
                pos: FxVec2::from_ints(x, 512), heading: Angle::ZERO, scale_milli: 1000 });
        }
        let on_path = terrain_map.props.len();
        terrain_map.props.push(Prop { kind: PropKind::TreeConifer,
            pos: FxVec2::from_ints(500, 550), heading: Angle::ZERO, scale_milli: 1000 });
        let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
        let mut w = World::with_terrain(terrain, terrain_map, blueprints(), Arc::new(Pool::new(1)), &config()).unwrap();
        let gun = spawn(&mut w, key, 0, 300, 512, 0);
        let mark = spawn(&mut w, "aster_t2_tank", 1, 800, 512, flag::PASSIVE | flag::INVULNERABLE);
        w.tick(&[PlayerCommand { player: 0, command: Command::Attack {
            units: vec![gun], target: mark, queue: false } }]).unwrap();
        let mut discharged = false;
        for _ in 0..300 {
            w.tick(&[]).unwrap();
            if w.events.iter().any(|e| matches!(e, SimEvent::BoreDischarge { weapon: 0, .. })) {
                discharged = true;
                break;
            }
        }
        assert!(discharged, "{key} must discharge at the 500 m target");
        assert!(w.state.units.pos[row(&w, gun)].distance(FxVec2::from_ints(300, 512)) < Fx::from_int(5),
            "{key} should reach the target without closing range");
        for prop in 0..on_path {
            assert!(!w.is_prop_alive(prop), "{key} left a gap at tree {prop}");
        }
        assert!(w.is_prop_alive(on_path), "{key} burned an off-path tree");
    }
}

#[test]
fn fulgur_uses_independent_compact_bores_and_a_shatter_aa_mount() {
    let bp = blueprints();
    let tank = bp.unit(bp.id_of("aster_t4_assault_tank").unwrap());
    assert_eq!(tank.name, "Fulgur");
    assert!(tank.radius > Fx::from_int(30));
    assert!(tank.weapons[0].shockwave >= 4.0);
    assert!(tank.weapons[0].muzzle.y > Fx::ZERO);
    for w in &tank.weapons[1..3] {
        assert!(w.bore.is_some() && w.mount);
        assert_eq!(w.sounds.fire.as_deref(), Some("aster_bore_compact"));
    }
    assert_eq!(tank.weapons[3].sounds.fire.as_deref(), Some("aster_shatter"));
    assert!(tank.weapons[3].mount && tank.weapons[3].bolts >= 7);
}

#[test]
fn electric_bores_elevate_and_depress_with_their_actual_muzzles_on_hills() {
    for key in ["aster_t3_sniper", "aster_t4_assault_tank"] {
        for (uphill, target_y) in [(true, 512), (false, 512), (true, 660), (false, 660)] {
            let mut samples = vec![0u16; 257 * 257];
            for y in 0..257 {
                for x in 0..257 {
                    let ramp = ((x as i32 * 8 - 320).clamp(0, 400) / 5) as u16;
                    samples[y * 257 + x] = if uphill { 20 + ramp } else { 100 - ramp };
                }
            }
            let terrain = Heightfield::from_samples(256, 256, samples, Fx::ZERO, Fx::ONE, Fx::ZERO);
            let mut w = World::with_terrain(terrain, map(), blueprints(), Arc::new(Pool::new(1)), &config()).unwrap();
            let gun = spawn(&mut w, key, 0, 200, 512, 0);
            let target = spawn(&mut w, "aster_t2_tank", 1, 620, target_y, flag::PASSIVE | flag::INVULNERABLE);
            w.tick(&[PlayerCommand { player: 0, command: Command::Attack {
                units: vec![gun], target, queue: false } }]).unwrap();
            let mut fired = false;
            for _ in 0..300 {
                w.tick(&[]).unwrap();
                let row = row(&w, gun);
                let bp = w.bp(row);
                for event in &w.events {
                    let SimEvent::ShotFired { pos, vel, blueprint, weapon: 0, .. } = event else { continue };
                    if *blueprint != w.state.units.blueprint[row] { continue; }
                    let pitch = w.state.units.arm_pitch[row][0];
                    let weapon = &bp.weapons[0];
                    let pivot = weapon.pivot.expect("the bore has elevation trunnions");
                    let arm = FxVec2::new(weapon.muzzle.x - pivot.x, weapon.muzzle.z - pivot.z).rotate(pitch);
                    let local = mc_core::FxVec3::new(pivot.x + arm.x, weapon.muzzle.y, pivot.z + arm.y);
                    let facing = w.state.units.heading[row] + w.state.units.weapon_yaw[row][0];
                    // This fixture slopes only on x. Independently reconstruct the
                    // shader's terrain basis, including when the gun rolls onto the ramp.
                    let center = w.state.units.pos[row];
                    let step = bp.radius.max(Fx::from_int(4));
                    let rise = w.terrain.height_at(center + FxVec2::new(step, Fx::ZERO))
                        - w.terrain.height_at(center - FxVec2::new(step, Fx::ZERO));
                    let terrain_pitch = FxVec2::new(step * 2, rise).angle();
                    let horizontal = local.xy().rotate(facing);
                    let leaned = FxVec2::new(horizontal.x, local.z).rotate(terrain_pitch);
                    let expected = (center + FxVec2::new(leaned.x, horizontal.y))
                        .extend(w.state.units.z[row] + leaned.y);
                    assert!((*pos - expected).length() < Fx::ratio(1, 10),
                        "{key}: shot {pos:?} disagrees with tilted muzzle {expected:?}");
                    let elevation = FxVec2::from_angle(pitch);
                    let forward = FxVec2::from_angle(facing) * elevation.x;
                    let tilted = FxVec2::new(forward.x, elevation.y).rotate(terrain_pitch);
                    let barrel_pitch = FxVec2::new(FxVec2::new(tilted.x, forward.y).length(), tilted.y).angle();
                    let signed = Angle::ZERO.delta_to(barrel_pitch);
                    assert!(if uphill { signed > 300 } else { signed < -300 }, "{key} did not aim toward the hill");
                    let launch_pitch = FxVec2::new(vel.xy().length(), vel.z).angle();
                    assert!(barrel_pitch.delta_to(launch_pitch).unsigned_abs() < 550,
                        "{key}: shot leaves sideways from the barrel");
                    fired = true;
                }
                if fired { break; }
            }
            assert!(fired, "{key} failed to fire {}", if uphill { "uphill" } else { "downhill" });
        }
    }
}
