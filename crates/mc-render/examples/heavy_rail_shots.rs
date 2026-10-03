//! Native GPU look at the capital rail guns (`Weapon::heavy_rail`, renderer/heavy_rail_fx.rs),
//! staged tick by tick from the charge to the smoke: the Zenith firing up at a Resolute
//! in the clouds, a Resolute firing down on a structure, and one of its flank turrets
//! charging and firing on the ground below (`turret`: a light `heavy_rail`, arcs only), and
//! the commander's rail cannon refit firing on a tank (`commander`: the same, on its arm).
//!
//! Run: cargo run --release -p mc-render --example heavy_rail_shots -- maps/dev16.mcmap OUT [zenith|frigate|turret|commander]..
//! Writes `<scene>-<shot>.ppm`. `HEAVY_SIZE=WxH` (1280x800). `HEAVY_BENCH=1` renders every
//! frame of the sequence and prints the mean and worst frame time (not a pass timing).
use glam::{Vec2, Vec3};
use mc_core::{Fx, FxVec3};
use mc_data::{BlueprintId, Blueprints};
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_render::{Camera, FrameInput, Overlay, Renderer, SceneDesc, Target};
use mc_sim::mirror::{
    HousePose, ProjectileInstance, RenderFrame, SimEvent, StainInstance, UnitInstance,
    PROJECTILE_ENDS_SHIFT, PROJECTILE_RAIL, UNIT_HOUSE_SHIFT,
};
use std::{path::Path, sync::Arc};

const TICK: f32 = 0.1;

fn fixed(v: Vec3) -> FxVec3 {
    let f = |x: f32| Fx::ratio((x * 1000.0) as i64, 1000);
    FxVec3::new(f(v.x), f(v.y), f(v.z))
}

fn rot_z(v: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

fn rot_xz(v: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(v.x * c - v.z * s, v.y, v.x * s + v.z * c)
}

/// `look`: the way the camera faces (world angle, as `Vec2::to_angle`); `pitch` down from level.
#[derive(Clone, Copy)]
struct Cam {
    focus: Vec3,
    distance: f32,
    look: f32,
    pitch: f32,
}

fn aim(camera: &mut Camera, cam: &Cam) {
    camera.focus = cam.focus;
    camera.distance = cam.distance;
    camera.yaw = std::f32::consts::FRAC_PI_2 - cam.look;
    camera.tilt = 0.0;
    let p0 = camera.pitch();
    camera.tilt = 1.0;
    let p1 = camera.pitch();
    camera.tilt = (p0 - cam.pitch) / (p0 - p1).max(1e-3);
}

/// One gun firing once at a mark.
struct Stage {
    name: &'static str,
    gun: UnitInstance,
    others: Vec<UnitInstance>,
    blueprint: BlueprintId,
    /// The gun that fires, and the unit's gun-house poses when it turns on a house.
    weapon: u8,
    house: Option<HousePose>,
    muzzle: Vec3,
    target: Vec3,
    on_unit: bool,
    /// (shot name, seconds from the charge starting, camera)
    shots: Vec<(&'static str, f32, Cam)>,
}

fn unit(blueprints: &Blueprints, key: &str, pos: Vec3, heading: f32, id: u32) -> UnitInstance {
    let bid = blueprints.id_of(key).unwrap_or_else(|| panic!("no {key}"));
    let bp = blueprints.unit(bid);
    let mut u: UnitInstance = bytemuck::Zeroable::zeroed();
    u.pos = pos.to_array();
    u.prev_pos = u.pos;
    u.heading = heading;
    u.prev_heading = heading;
    u.blueprint = bid.0 as u32;
    u.health = 1.0;
    u.build = 1.0;
    u.radius = bp.radius.to_f32();
    u.deploy = 1.0;
    u.prev_deploy = 1.0;
    u.unit_id = id;
    u
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let map = Arc::new(MapFile::open(&args[0]).unwrap());
    let out = Path::new(&args[1]);
    let wanted: Vec<String> = args[2..].to_vec();
    std::fs::create_dir_all(out).unwrap();
    let (width, height) = std::env::var("HEAVY_SIZE")
        .ok()
        .and_then(|s| {
            s.split_once('x')
                .map(|(w, h)| (w.parse().unwrap(), h.parse().unwrap()))
        })
        .unwrap_or((1280u32, 800u32));
    let bench = std::env::var("HEAVY_BENCH").is_ok_and(|v| v == "1");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let blueprints = Arc::new(Blueprints::load(&root.join("data")).unwrap());
    let size = Vec2::from(map.info().size_metres().to_f32());
    let spot = Vec2::from(map.start_positions()[0].to_f32());
    let overlay = Overlay::default();
    let mut renderer = Renderer::new(
        Target::Headless { width, height },
        SceneDesc {
            map: map.clone(),
            blueprints: blueprints.clone(),
            pool: Arc::new(Pool::new(4)),
            team_colors: [[0.1, 0.45, 0.95]; mc_core::MAX_PLAYERS],
        },
    )
    .unwrap();
    let ground = |r: &Renderer, xy: Vec2| r.ground_height(xy);

    let mut stages = Vec::new();

    // The Zenith on the pad, firing up at a Resolute 1.6 km off in the cloud.
    {
        let zid = blueprints.id_of("aster_t4_anti_ship").unwrap();
        let w = &blueprints.unit(zid).weapons[0];
        let at = spot.extend(ground(&renderer, spot));
        let ship_xy = spot + Vec2::new(1400.0, 700.0);
        let ship = ship_xy.extend(ground(&renderer, ship_xy) + 560.0);
        let heading = (ship_xy - spot).to_angle();
        let pivot = Vec3::from(w.pivot.unwrap().to_f32());
        let local = Vec3::from(w.muzzle.to_f32());
        let mut pitch = 0.0;
        let mut muzzle = at;
        for _ in 0..4 {
            muzzle = at + rot_z(pivot + rot_xz(local - pivot, pitch), heading);
            let d = ship - muzzle;
            pitch = d.z.atan2(d.truncate().length());
        }
        let mut gun = unit(&blueprints, "aster_t4_anti_ship", at, heading, 1);
        gun.arm_pitch = [pitch, pitch, 0.0, 0.0];
        let target = unit(&blueprints, "aster_t3_frigate", ship, heading + 2.2, 2);
        // It strikes the hull's side, not its middle.
        let hit = ship + Vec3::Z * 25.0 - (ship - muzzle).normalize() * 100.0;
        let mid = (muzzle + hit) * 0.5;
        stages.push(Stage {
            name: "zenith",
            gun,
            others: vec![target],
            blueprint: zid,
            weapon: 0,
            house: None,
            muzzle,
            target: hit,
            on_unit: true,
            shots: vec![
                (
                    "charge-a",
                    0.8,
                    Cam {
                        focus: at + Vec3::Z * 60.0,
                        distance: 380.0,
                        look: heading + 1.25,
                        pitch: 0.3,
                    },
                ),
                (
                    "charge-b",
                    1.9,
                    Cam {
                        focus: at + Vec3::Z * 60.0,
                        distance: 380.0,
                        look: heading + 1.25,
                        pitch: 0.3,
                    },
                ),
                (
                    "charge-c",
                    2.42,
                    Cam {
                        focus: at + Vec3::Z * 60.0,
                        distance: 380.0,
                        look: heading + 1.25,
                        pitch: 0.3,
                    },
                ),
                (
                    "fire-a",
                    2.56,
                    Cam {
                        focus: at + Vec3::Z * 60.0,
                        distance: 420.0,
                        look: heading + 1.25,
                        pitch: 0.25,
                    },
                ),
                (
                    "fire-b",
                    2.8,
                    Cam {
                        focus: at + Vec3::Z * 80.0,
                        distance: 700.0,
                        look: heading + 1.25,
                        pitch: 0.2,
                    },
                ),
                (
                    "path-a",
                    2.95,
                    Cam {
                        focus: mid,
                        distance: 2300.0,
                        look: heading + 1.57,
                        pitch: 0.1,
                    },
                ),
                (
                    "impact-a",
                    3.12,
                    Cam {
                        focus: ship,
                        distance: 900.0,
                        look: heading + 1.9,
                        pitch: 0.25,
                    },
                ),
                (
                    "impact-b",
                    3.5,
                    Cam {
                        focus: ship,
                        distance: 900.0,
                        look: heading + 1.9,
                        pitch: 0.25,
                    },
                ),
                (
                    "impact-close",
                    3.15,
                    Cam {
                        focus: hit,
                        distance: 450.0,
                        look: heading + 1.9,
                        pitch: 0.3,
                    },
                ),
                (
                    "rts",
                    2.75,
                    Cam {
                        focus: mid,
                        distance: 1900.0,
                        look: heading + 1.4,
                        pitch: 0.95,
                    },
                ),
                (
                    "gun-after",
                    3.6,
                    Cam {
                        focus: at + Vec3::Z * 60.0,
                        distance: 420.0,
                        look: heading + 1.25,
                        pitch: 0.25,
                    },
                ),
                (
                    "late-a",
                    4.5,
                    Cam {
                        focus: mid,
                        distance: 2300.0,
                        look: heading + 1.57,
                        pitch: 0.1,
                    },
                ),
                (
                    "late-b",
                    7.0,
                    Cam {
                        focus: mid,
                        distance: 2300.0,
                        look: heading + 1.57,
                        pitch: 0.1,
                    },
                ),
                (
                    "late-c",
                    12.0,
                    Cam {
                        focus: mid,
                        distance: 2300.0,
                        look: heading + 1.57,
                        pitch: 0.1,
                    },
                ),
            ],
        });
    }

    // A Resolute at 560 m firing down on a structure a kilometre off.
    {
        let fid = blueprints.id_of("aster_t3_frigate").unwrap();
        let w = &blueprints.unit(fid).weapons[0];
        let base = spot + Vec2::new(-400.0, -2600.0);
        let ship = base.extend(ground(&renderer, base) + 560.0);
        let mark_xy = base + Vec2::new(1150.0, 150.0);
        let heading = (mark_xy - base).to_angle();
        let muzzle = ship + rot_z(Vec3::from(w.muzzle.to_f32()), heading);
        let mark = mark_xy.extend(ground(&renderer, mark_xy) + 4.0);
        let gun = unit(&blueprints, "aster_t3_frigate", ship, heading, 3);
        let target = unit(&blueprints, "aster_t2_power", mark - Vec3::Z * 4.0, 0.0, 4);
        let mid = (muzzle + mark) * 0.5;
        stages.push(Stage {
            name: "frigate",
            gun,
            others: vec![target],
            blueprint: fid,
            weapon: 0,
            house: None,
            muzzle,
            target: mark,
            on_unit: true,
            shots: vec![
                (
                    "charge-a",
                    1.5,
                    Cam {
                        focus: ship,
                        distance: 700.0,
                        look: heading + 1.2,
                        pitch: 0.9,
                    },
                ),
                (
                    "charge-b",
                    2.9,
                    Cam {
                        focus: ship,
                        distance: 700.0,
                        look: heading + 1.2,
                        pitch: 0.9,
                    },
                ),
                (
                    "charge-side",
                    2.85,
                    Cam {
                        focus: ship,
                        distance: 800.0,
                        look: heading + 1.9,
                        pitch: 0.25,
                    },
                ),
                (
                    "fire-a",
                    3.06,
                    Cam {
                        focus: ship,
                        distance: 1000.0,
                        look: heading + 1.9,
                        pitch: 0.25,
                    },
                ),
                (
                    "fire-b",
                    3.25,
                    Cam {
                        focus: mid,
                        distance: 1700.0,
                        look: heading + 1.57,
                        pitch: 0.12,
                    },
                ),
                (
                    "impact-a",
                    3.55,
                    Cam {
                        focus: mark,
                        distance: 600.0,
                        look: heading + 2.2,
                        pitch: 0.5,
                    },
                ),
                (
                    "impact-b",
                    4.0,
                    Cam {
                        focus: mark,
                        distance: 600.0,
                        look: heading + 2.2,
                        pitch: 0.5,
                    },
                ),
                (
                    "rts",
                    3.3,
                    Cam {
                        focus: mid,
                        distance: 1500.0,
                        look: heading + 1.4,
                        pitch: 0.95,
                    },
                ),
                (
                    "path-late",
                    4.5,
                    Cam {
                        focus: mid,
                        distance: 1700.0,
                        look: heading + 1.57,
                        pitch: 0.12,
                    },
                ),
                (
                    "late-a",
                    7.0,
                    Cam {
                        focus: mid,
                        distance: 1700.0,
                        look: heading + 1.57,
                        pitch: 0.12,
                    },
                ),
                (
                    "late-b",
                    14.0,
                    Cam {
                        focus: mark,
                        distance: 800.0,
                        look: heading + 2.2,
                        pitch: 0.3,
                    },
                ),
                // From over the cloud deck, before and after: the hole the slug tore.
                (
                    "cloud-before",
                    2.0,
                    Cam {
                        focus: mid,
                        distance: 2200.0,
                        look: heading + 1.2,
                        pitch: 1.1,
                    },
                ),
                (
                    "cloud-after",
                    5.0,
                    Cam {
                        focus: mid,
                        distance: 2200.0,
                        look: heading + 1.2,
                        pitch: 1.1,
                    },
                ),
                (
                    "cloud-later",
                    9.0,
                    Cam {
                        focus: mid,
                        distance: 2200.0,
                        look: heading + 1.2,
                        pitch: 1.1,
                    },
                ),
            ],
        });
    }

    // A Resolute at 560 m, its port flank turret (weapon 3) laid on the ground below.
    {
        let fid = blueprints.id_of("aster_t3_frigate").unwrap();
        let weapon = 3u8;
        let w = &blueprints.unit(fid).weapons[weapon as usize];
        let base = spot + Vec2::new(900.0, -1800.0);
        let ship = base.extend(ground(&renderer, base) + 560.0);
        let heading = 0.4f32;
        let pivot = Vec3::from(w.pivot.unwrap().to_f32());
        let reach = Vec3::from(w.muzzle.to_f32()).distance(pivot);
        let yaw = 70f32.to_radians();
        let pivot_at = ship + rot_z(pivot, heading);
        let mark_xy = pivot_at.truncate() + Vec2::from_angle(heading + yaw) * 520.0;
        let mark = mark_xy.extend(ground(&renderer, mark_xy) + 4.0);
        let mut pitch = 0.0;
        let mut muzzle = pivot_at;
        for _ in 0..4 {
            muzzle = pivot_at + rot_z(rot_xz(Vec3::X * reach, pitch), heading + yaw);
            let d = mark - muzzle;
            pitch = d.z.atan2(d.truncate().length());
        }
        let mut house = HousePose::default();
        house.pose[weapon as usize] = [yaw, yaw, pitch, pitch];
        let mut gun = unit(&blueprints, "aster_t3_frigate", ship, heading, 5);
        gun.status[1] |= 1 << UNIT_HOUSE_SHIFT;
        let target = unit(&blueprints, "aster_t1_tank", mark - Vec3::Z * 4.0, 0.0, 6);
        let g = heading + yaw;
        let side = Cam {
            focus: pivot_at + rot_z(Vec3::X * reach * 0.5, g),
            distance: 100.0,
            look: g + std::f32::consts::PI - 0.9,
            pitch: 0.3,
        };
        let behind = Cam {
            focus: pivot_at + rot_z(Vec3::X * reach * 0.5, g),
            distance: 110.0,
            look: g + std::f32::consts::PI,
            pitch: 0.55,
        };
        stages.push(Stage {
            name: "turret",
            gun,
            others: vec![target],
            blueprint: fid,
            weapon,
            house: Some(house),
            muzzle,
            target: mark,
            on_unit: true,
            shots: vec![
                ("charge-a", 0.3, side),
                ("charge-b", 0.6, side),
                ("charge-c", 0.85, side),
                ("charge-behind", 0.8, behind),
                ("fire", 0.95, side),
                ("after", 1.4, side),
                ("cooling", 3.0, side),
                (
                    "wide",
                    0.85,
                    Cam {
                        focus: ship,
                        distance: 700.0,
                        look: g + 1.2,
                        pitch: 0.5,
                    },
                ),
            ],
        });
    }

    // A commander with the rail cannon refit, its arm laid on a tank 300 m off.
    {
        let acu = blueprints.id_of("aster_commander").unwrap();
        let set = blueprints.refit_set(acu).unwrap();
        let kit = |key: &str| {
            set.slots
                .iter()
                .flat_map(|s| &s.modules)
                .find(|m| m.key == key)
                .unwrap()
                .kit
        };
        let cannon = blueprints.refit_result(acu, kit("cannon")).unwrap();
        let rail = blueprints.refit_result(cannon, kit("railgun")).unwrap();
        let bp = blueprints.unit(rail);
        let weapon = bp.weapons.iter().position(|w| w.heavy_rail > 0.0).unwrap() as u8;
        let w = &bp.weapons[weapon as usize];
        let base = spot + Vec2::new(-600.0, 400.0);
        let at = base.extend(ground(&renderer, base));
        let heading = 0.4f32;
        let pivot = Vec3::from(w.pivot.unwrap().to_f32());
        let bore = Vec3::from(w.muzzle.to_f32()) - pivot;
        let mark_xy = base + Vec2::from_angle(heading) * 300.0;
        let mark = mark_xy.extend(ground(&renderer, mark_xy) + 3.0);
        // The torso turns about its upright axis and the elbow pitches the arm, so the gun's
        // line is off to the side: turn until the muzzle's line meets the mark.
        let (mut yaw, mut pitch) = (0.0f32, 0.0f32);
        let mut muzzle = at;
        for _ in 0..8 {
            let place = |v: Vec3| at + rot_z(rot_z(pivot + rot_xz(v, pitch), yaw), heading);
            muzzle = place(bore);
            let d = mark - place(Vec3::ZERO);
            yaw += (d.truncate().to_angle() - (muzzle - place(Vec3::ZERO)).truncate().to_angle())
                .sin()
                .asin();
            pitch = d.z.atan2(d.truncate().length());
        }
        let mut gun = unit(&blueprints, "aster_commander", at, heading, 7);
        gun.blueprint = rail.0 as u32;
        gun.radius = bp.radius.to_f32();
        gun.turret_yaw = yaw;
        gun.prev_turret_yaw = yaw;
        gun.arm_pitch = [pitch, pitch, 0.0, 0.0];
        let target = unit(&blueprints, "aster_t1_tank", mark - Vec3::Z * 3.0, 0.0, 8);
        let g = heading + yaw;
        let focus = muzzle - rot_z(Vec3::X * 5.0, g);
        let side = Cam {
            focus,
            distance: 22.0,
            look: g + std::f32::consts::PI - 1.1,
            pitch: 0.35,
        };
        let behind = Cam {
            focus,
            distance: 24.0,
            look: g + std::f32::consts::PI - 0.3,
            pitch: 0.6,
        };
        stages.push(Stage {
            name: "commander",
            gun,
            others: vec![target],
            blueprint: rail,
            weapon,
            house: None,
            muzzle,
            target: mark,
            on_unit: true,
            shots: vec![
                ("charge-a", 0.25, side),
                ("charge-b", 0.5, side),
                ("charge-c", 0.75, side),
                ("charge-behind", 0.7, behind),
                ("fire", 0.85, side),
                ("after", 1.2, side),
                ("cooling", 2.5, side),
                (
                    "wide",
                    0.75,
                    Cam {
                        focus,
                        distance: 120.0,
                        look: g + 1.2,
                        pitch: 0.7,
                    },
                ),
            ],
        });
    }

    let mut clock = 30.0;
    for stage in &stages {
        if !wanted.is_empty() && !wanted.iter().any(|w| w == stage.name) {
            continue;
        }
        let bp = blueprints.unit(stage.blueprint);
        let w = &bp.weapons[stage.weapon as usize];
        let charge = w.charge_ticks as usize;
        let speed = w.projectile_speed.to_f32();
        let dir = (stage.target - stage.muzzle).normalize();
        let distance = stage.target.distance(stage.muzzle);
        let last_shot = stage.shots.iter().map(|s| s.1).fold(0.0, f32::max);
        let ticks = (last_shot / TICK).ceil() as usize + 2;
        let mut camera = Camera::new(size, Vec2::new(width as f32, height as f32));
        let mut frame = RenderFrame {
            props_dead: vec![0; map.props().len().div_ceil(32)],
            ..Default::default()
        };
        // A world with no scorch at all reads as a new one, and the renderer drops its effects.
        frame.stains.push(StainInstance {
            pos: (spot + Vec2::new(0.0, 300.0)).to_array(),
            radius: 2.0,
            strength_seed: 40,
        });
        let start = clock;
        let mut times = Vec::new();
        // `HEAVY_GUNS=N`: that many of the gun side by side, all firing at once (for cost).
        let guns: usize = std::env::var("HEAVY_GUNS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        let offsets: Vec<Vec3> = (0..guns)
            .map(|i| Vec3::new(0.0, 160.0 * i as f32, 0.0))
            .collect();
        let mut pass_ms: Vec<(&str, Vec<f32>)> = Vec::new();
        for k in 0..ticks {
            let now = start + k as f32 * TICK;
            frame.events.clear();
            frame.projectiles.clear();
            frame.units = stage.others.clone();
            frame.houses = stage.house.into_iter().collect();
            for (i, off) in offsets.iter().enumerate() {
                let mut gun = stage.gun;
                gun.pos = (Vec3::from(gun.pos) + *off).to_array();
                gun.prev_pos = gun.pos;
                gun.unit_id += 100 * i as u32;
                frame.units.push(gun);
                let (muzzle, target) = (stage.muzzle + *off, stage.target + *off);
                if k == 0 {
                    frame.events.push(SimEvent::WeaponCharging {
                        unit: mc_sim::Handle(gun.unit_id),
                        pos: fixed(Vec3::from(gun.pos) + Vec3::Z * w.muzzle.z.to_f32()),
                        owner: 0,
                        blueprint: stage.blueprint,
                        weapon: stage.weapon,
                    });
                }
                if k == charge {
                    frame.events.push(SimEvent::ShotFired {
                        pos: fixed(muzzle),
                        vel: fixed(dir * speed * TICK),
                        travel: FxVec3::ZERO,
                        color: w.color,
                        owner: 0,
                        blueprint: stage.blueprint,
                        weapon: stage.weapon,
                    });
                }
                if k < charge {
                    continue;
                }
                // The slug: out of the muzzle this tick, then a tick's flight each tick.
                let n = (k - charge) as f32;
                let step = speed * TICK;
                let remaining = distance - n * step;
                if remaining <= 0.0 {
                    continue;
                }
                let a = muzzle + dir * step * n;
                let (b, ends) = if remaining <= step {
                    (target, remaining / step)
                } else {
                    (a + dir * step, 0.0)
                };
                let mut color = 1 | PROJECTILE_RAIL;
                if ends > 0.0 {
                    color |= ((ends * 255.0) as u32).clamp(1, 255) << PROJECTILE_ENDS_SHIFT;
                }
                frame.projectiles.push(ProjectileInstance {
                    prev_pos: a.to_array(),
                    color,
                    pos: b.to_array(),
                    // A capital slug is a heavy trace; a light rail's is an ordinary one.
                    size: if w.heavy_rail >= 0.15 { 5.5 } else { 1.5 },
                    wake: 0.0,
                    plasma: 0.0,
                    _pad: [0.0; 2],
                    aim: [0.0; 4],
                    prev_aim: [0.0; 4],
                });
                if ends > 0.0 {
                    frame.events.push(SimEvent::Impact {
                        pos: fixed(target),
                        target_motion: FxVec3::ZERO,
                        splash: w.splash,
                        color: w.color,
                        after: Fx::ratio((ends * 1000.0) as i64, 1000),
                        on_unit: stage.on_unit,
                        on_shield: false,
                        blueprint: stage.blueprint,
                        weapon: stage.weapon,
                    });
                }
            }
            let t0 = std::time::Instant::now();
            // Benching: every frame through the wide view of the whole path.
            let view = if bench {
                stage
                    .shots
                    .iter()
                    .find(|s| s.0 == "path-a" || s.0 == "fire-b")
                    .map_or(&stage.shots[0].2, |s| &s.2)
            } else {
                &stage.shots[0].2
            };
            aim(&mut camera, view);
            renderer
                .render(&FrameInput {
                    camera: &camera,
                    time: now,
                    alpha: 0.0,
                    sim: Some(&frame),
                    ghosts: &[],
                    marks: &[],
                    ranges: &[],
                    ranges_drawn: 0,
                    overlay: &overlay,
                    build_grid: false,
                    icons: true,
                })
                .unwrap();
            if bench {
                let _ = renderer.read_pixels();
                times.push(t0.elapsed().as_secs_f32() * 1000.0);
                // From the shot on, while the effects are at their thickest.
                let since = k as f32 * TICK - charge as f32 * TICK;
                if (0.0..6.0).contains(&since) {
                    for &(name, ms) in &renderer.stats.gpu_passes {
                        match pass_ms.iter_mut().find(|p| p.0 == name) {
                            Some(p) => p.1.push(ms),
                            None => pass_ms.push((name, vec![ms])),
                        }
                    }
                }
            }
            // Shots that fall inside this tick.
            for (name, at, cam) in &stage.shots {
                let t = start + at;
                if t < now || t >= now + TICK {
                    continue;
                }
                aim(&mut camera, cam);
                renderer
                    .render(&FrameInput {
                        camera: &camera,
                        time: t,
                        alpha: (t - now) / TICK,
                        sim: None,
                        ghosts: &[],
                        marks: &[],
                        ranges: &[],
                        ranges_drawn: 0,
                        overlay: &overlay,
                        build_grid: false,
                        icons: true,
                    })
                    .unwrap();
                let pixels = renderer.read_pixels().expect("pixels");
                let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
                for pixel in pixels.as_chunks::<4>().0 {
                    ppm.extend_from_slice(&pixel[..3]);
                }
                std::fs::write(out.join(format!("{}-{name}.ppm", stage.name)), ppm).unwrap();
            }
        }
        for (name, mut ms) in pass_ms {
            ms.sort_by(f32::total_cmp);
            eprintln!(
                "{}: {name} pass median {:.2} ms over {} frames",
                stage.name,
                ms[ms.len() / 2],
                ms.len()
            );
        }
        if bench && !times.is_empty() {
            let mean = times.iter().sum::<f32>() / times.len() as f32;
            let worst = times.iter().copied().fold(0.0, f32::max);
            eprintln!(
                "{}: {} frames, mean {mean:.2} ms, worst {worst:.2} ms",
                stage.name,
                times.len()
            );
        }
        eprintln!("captured {}", stage.name);
        clock += 60.0;
    }
}
