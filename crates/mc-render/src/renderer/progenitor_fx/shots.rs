//! The Progenitor's hard light staged without the sim and rendered through the real
//! renderer: a Shaper's Lance, its dome under fire, its anti-nuke meeting a warhead, a
//! Seedcaster's squad warping in and the Unmaker's warhead going off, each at any tier.
//!
//! `cargo test --profile shot -p mc-render --lib progenitor_shots -- --ignored --nocapture`
//! on the machine with the GPU (it needs Vulkan and maps/crosswater.mcmap). It writes
//! PPMs to `PROG_OUT` (default artifacts/progenitor):
//!
//! - `PROG_SCENES`: lance, shield, antinuke, warpin, warpcancel (a dampening field kills
//!   the mark at 4 s), fleetprint and fleet (a Gatewright's fleet printed aloft, marked,
//!   jumping at 8 s and out at 9.5 s), warhead (default all but the fleet's), and
//!   lance-close, antinuke-close, warpin-close, warhead-close (the camera close on the
//!   beam, the body in flight or the mark);
//! - `PROG_TIERS`: the Shaper's tiers, comma-separated (default `1,5`);
//! - `PROG_TIMES`: seconds into each scene to capture (default per scene);
//! - `PROG_CAM`: `dx,dy,dz,dist,yaw,pitch` (metres off the Shaper's foot, radians), else
//!   per scene, looking across from the side;
//! - `PROG_BIG`: 2560x1440 rather than 1600x900.

use crate::camera::Camera;
use crate::overlay::Overlay;
use crate::renderer::{FrameInput, Renderer, SceneDesc, Target};
use bytemuck::Zeroable;
use glam::{Vec2, Vec3};
use mc_core::{Fx, FxVec3};
use mc_data::strategic::StrategicLook;
use mc_data::{BlueprintId, WeaponColor};
use mc_sim::mirror::*;
use mc_sim::reclaim::BeamInstance;
use mc_sim::survival::{arrival_shock, shaper_height, ShaperInstance, WarpInInstance, WarpInKind};
use std::sync::Arc;

const TICK: f32 = 0.1;
/// `owner_flags` of a unit being printed (the sim's flags sit from bit 8).
const UNDER_CONSTRUCTION: u32 = 1 << 8;

fn fx3(v: Vec3) -> FxVec3 {
    FxVec3::new(Fx::from_f32(v.x), Fx::from_f32(v.y), Fx::from_f32(v.z))
}

fn unit(bp: BlueprintId, id: u32, pos: Vec3, owner: u32) -> UnitInstance {
    let mut u = UnitInstance::zeroed();
    u.prev_pos = pos.to_array();
    u.pos = pos.to_array();
    u.blueprint = bp.0 as u32;
    u.owner_flags = owner;
    u.health = 1.0;
    u.build = 1.0;
    u.radius = 20.0;
    u.unit_id = id;
    u
}

/// A missile at `at` that was at `was` a tick ago.
fn missile(kind: u32, serial: u32, at: Vec3, was: Vec3, look: StrategicLook) -> StrategicInstance {
    StrategicInstance {
        prev_pos: was.to_array(),
        kind,
        pos: at.to_array(),
        owner: u32::from(look == StrategicLook::Progenitor),
        mark: at.to_array(),
        serial,
        scale: 1.0,
        look: look as u32,
        ..StrategicInstance::default()
    }
}

/// One scene at one tier: what the sim would hand over at tick `k` (time `t`).
struct Stage {
    scene: String,
    tier: u8,
    site: Vec3,
    target: Vec3,
    shaper_bp: BlueprintId,
    seed_bp: BlueprintId,
    gate_bp: BlueprintId,
    tank_bp: BlueprintId,
    /// The fleet's warships: corvettes at a low tier, frigates at a high.
    ship_bp: BlueprintId,
}

/// The fleet scene's ships: how many, and where each stands off the fleet's middle.
const SHIPS: usize = 6;

fn ship_off(k: usize) -> Vec3 {
    Vec3::new(
        (k % 3) as f32 * 70.0 - 70.0,
        (k / 3) as f32 * 70.0 - 35.0,
        0.0,
    )
}

impl Stage {
    fn level(&self) -> u16 {
        u16::from(self.tier - 1) * 4 + 1
    }

    fn shaper(&self, kind: u8, charge: f32) -> ShaperInstance {
        let level = self.level();
        ShaperInstance {
            unit: 1,
            kind,
            level,
            tier: self.tier,
            height: shaper_height(level).to_f32(),
            growing: 1.0,
            raised: 1.0,
            lance_charge: charge,
            lance_target: if charge > 0.0 {
                (self.target + Vec3::Z * 2.0).to_array()
            } else {
                [0.0; 3]
            },
            shield_tier: self.tier,
            fire_charge: 0.0,
            foot: 16.0 + 84.0 * ((shaper_height(level).to_f32() - 60.0) / 1100.0),
        }
    }

    /// Where the shard meets the warhead coming down on the site, at 2.5 s.
    fn meet(&self) -> Vec3 {
        self.site + Vec3::new(-300.0, 200.0, 1400.0)
    }

    /// The warhead coming down on the site, at `t` seconds.
    fn prey_at(&self, t: f32) -> Vec3 {
        self.meet() + Vec3::new(200.0, -150.0, -900.0) * (t - 2.5)
    }

    /// The shard's cell, and the shard at `t`: out of it at 0.5 s, up, then at the warhead.
    fn cell(&self) -> Vec3 {
        self.site + Vec3::new(30.0, 0.0, 8.0)
    }

    fn shard_at(&self, t: f32) -> Vec3 {
        let s = ((t - 0.5) / 2.0).clamp(0.0, 1.0);
        // Slow out of the cell, as the sim's boost is.
        let lift = self.cell() + Vec3::Z * 600.0 * (s.min(0.4) / 0.4).powi(2);
        lift.lerp(self.meet(), (s - 0.4).max(0.0) / 0.6)
    }

    /// The Unmaker's warhead coming down on the target, going off at 3 s.
    fn warhead_at(&self, t: f32) -> Vec3 {
        self.target + Vec3::Z * 20.0 + Vec3::new(-250.0, 150.0, -1100.0) * (t - 3.0)
    }

    /// A Seedcaster's squad marked at the target, coming in six seconds on.
    fn warp_in(&self) -> WarpInInstance {
        let level = self.level();
        let (radius, _) = arrival_shock(level);
        WarpInInstance {
            serial: 3,
            source: 1,
            site: 0,
            from: (self.site + Vec3::Z * shaper_height(level).to_f32()).to_array(),
            at: self.target.to_array(),
            radius: radius.to_f32(),
            reach: 40.0 + 2.0 * f32::from(level),
            spots: self.squad_spots(),
            units: 4 + level,
            level,
            ..WarpInInstance::default()
        }
    }

    /// Where the squad's units come out: rings round the mark.
    fn squad_spots(&self) -> Vec<[f32; 3]> {
        let n = 4 + usize::from(self.level());
        let reach = 40.0 + 2.0 * f32::from(self.level());
        (0..n)
            .map(|k| {
                let ring = 1 + k / 6;
                let a = k as f32 * 2.399;
                let r = (ring as f32 * 14.0).min(reach * 0.85);
                (self.target + Vec3::new(a.cos() * r, a.sin() * r, 0.0)).to_array()
            })
            .collect()
    }

    /// Where the fleet is printed, in front of the Gatewright.
    fn berth(&self) -> Vec3 {
        self.site + Vec3::new(260.0, 0.0, 220.0)
    }

    /// The fleet's mark over the target: on the ground, and its ships' spots aloft.
    fn fleet_mark(&self, t: f32) -> WarpInInstance {
        let spots = (0..SHIPS)
            .map(|k| (self.target + Vec3::Z * 260.0 + ship_off(k)).to_array())
            .collect();
        WarpInInstance {
            serial: 4,
            kind: WarpInKind::Fleet,
            source: 1,
            from: self.berth().to_array(),
            at: self.target.to_array(),
            reach: 160.0,
            spots,
            age: t - 3.0,
            eta: (8.0 - t).max(0.0),
            jumping: t >= 8.0,
            units: SHIPS as u16,
            level: self.level(),
            ..WarpInInstance::default()
        }
    }

    /// What a close-up camera follows at `t`, in a scene with a body in flight.
    fn body_at(&self, t: f32) -> Vec3 {
        match self.scene.as_str() {
            "antinuke" => self.shard_at(t),
            "warpin" | "warpcancel" => self.target + Vec3::Z * 20.0 * t,
            // Along the Lance's beam, two thirds of the way down.
            "lance" => {
                let lens =
                    self.site + Vec3::Z * crate::models::shaper::lens_height(0, self.level());
                lens.lerp(self.target, 0.66)
            }
            _ => self.warhead_at(t),
        }
    }

    fn frame(&self, k: u32, ground: &dyn Fn(Vec2) -> f32) -> RenderFrame {
        let t = k as f32 * TICK;
        let mut f = RenderFrame::default();
        let level = self.level();
        let bp = match self.scene.as_str() {
            "warpin" | "warpcancel" => self.seed_bp,
            "fleet" | "fleetprint" => self.gate_bp,
            _ => self.shaper_bp,
        };
        let mut shaper = unit(bp, 1, self.site, 1);
        // Its halo's launcher word: every interceptor held, but the one the anti-nuke
        // scene spends at 0.5 s.
        let capacity = crate::models::shaper::interceptors(level) as u32;
        let spent = u32::from(self.scene == "antinuke" && k >= 5);
        shaper.status[2] = mc_sim::nukes::LAUNCHER_MARK
            | (capacity - spent)
            | capacity << mc_sim::nukes::LAUNCHER_CAPACITY_SHIFT;
        f.units.push(shaper);
        for i in 0..4 {
            let off = Vec2::new((i % 2) as f32 * 14.0 - 7.0, (i / 2) as f32 * 14.0 - 7.0);
            let at = self.target.truncate() + off;
            f.units
                .push(unit(self.tank_bp, 10 + i, at.extend(ground(at)), 0));
        }
        let height = shaper_height(level).to_f32();
        match self.scene.as_str() {
            "lance" => {
                // Charging for three seconds, striking, then cooling.
                let charge = if t < 3.0 { (t / 3.0).min(1.0) } else { 0.0 };
                f.shapers.push(self.shaper(0, charge));
                if k == 30 {
                    f.events.push(SimEvent::LanceStrike {
                        unit: mc_sim::Handle(1),
                        owner: 1,
                        to: fx3(self.target + Vec3::Z * 2.0),
                        splash: Fx::from_int(14 + 3 * (i32::from(level) - 1).min(35)),
                        damage: Fx::from_int(1500),
                    });
                }
            }
            "shield" => {
                f.shapers.push(self.shaper(0, 0.0));
                let radius = (180.0 + 22.0 * f32::from(level - 1)).min(900.0);
                f.shields.push(ShieldInstance {
                    pos: self.site.to_array(),
                    radius,
                    prev_open: 1.0,
                    open: 1.0,
                    health: if self.tier > 3 { 0.9 } else { 1.0 },
                    packed: 1
                        | u32::from(self.tier) << 16
                        | (mc_data::ShieldLook::Progenitor as u32) << SHIELD_LOOK_SHIFT,
                    unit_id: 1,
                    projector: height * 0.86,
                    height,
                    prev_radius: 0.0,
                });
                // Shots from the south-west landing all over the near side.
                if k % 2 == 1 {
                    let a = 3.6 + (k as f32 * 0.71).sin() * 0.7;
                    let e = 0.15 + 0.5 * (k as f32 * 0.37).cos().abs();
                    let dir = Vec3::new(a.cos() * e.cos(), a.sin() * e.cos(), e.sin());
                    let flat = mc_data::dome_height_f32(radius) / radius;
                    f.events.push(SimEvent::Impact {
                        pos: fx3(self.site + Vec3::new(dir.x, dir.y, dir.z * flat) * radius),
                        target_motion: FxVec3::ZERO,
                        splash: Fx::from_f32(2.0),
                        color: WeaponColor::Orange,
                        after: Fx::from_f32(0.3),
                        on_unit: false,
                        on_shield: true,
                        on_structure: None,
                        blueprint: self.tank_bp,
                        weapon: 0,
                    });
                }
            }
            "antinuke" => {
                f.shapers.push(self.shaper(0, 0.0));
                // A warhead coming down on the site; the shard meets it at 2.5 s.
                let (meet, cell) = (self.meet(), self.cell());
                let w_at = |t: f32| self.prey_at(t);
                let i_at = |t: f32| self.shard_at(t);
                if t < 2.5 {
                    f.strategic.push(missile(
                        STRATEGIC_WARHEAD,
                        7,
                        w_at(t),
                        w_at(t - TICK),
                        StrategicLook::Fission,
                    ));
                }
                if k == 5 {
                    f.events.push(SimEvent::InterceptorLaunch {
                        from: fx3(cell),
                        owner: 1,
                        serial: 8,
                        look: StrategicLook::Progenitor,
                    });
                }
                if (0.5..2.5).contains(&t) {
                    let mut shard = missile(
                        STRATEGIC_INTERCEPTOR,
                        8,
                        i_at(t),
                        i_at(t - TICK),
                        StrategicLook::Progenitor,
                    );
                    shard.age = t - 0.5;
                    f.strategic.push(shard);
                }
                if k == 25 {
                    f.events.push(SimEvent::WarheadIntercepted {
                        pos: fx3(meet),
                        owner: 1,
                        killed: true,
                        look: StrategicLook::Fission,
                        by: StrategicLook::Progenitor,
                    });
                }
            }
            "warpin" | "warpcancel" => {
                f.shapers.push(self.shaper(1, 0.0));
                let mut s = self.warp_in();
                let (radius, _) = arrival_shock(level);
                let cancel = self.scene == "warpcancel";
                if t < 6.0 && !(cancel && t >= 4.0) {
                    s.age = t;
                    s.eta = 6.0 - t;
                    f.warp_ins.push(s.clone());
                } else if k == 40 && cancel {
                    f.events.push(SimEvent::WarpInCancelled {
                        serial: 3,
                        site: 0,
                        at: fx3(self.target),
                        by: mc_sim::Handle(99),
                        units: s.units,
                    });
                } else if k == 60 && !cancel {
                    f.events.push(SimEvent::WarpInArrived {
                        serial: 3,
                        at: fx3(self.target),
                        radius,
                        units: s.units,
                    });
                }
                if t >= 6.0 && !cancel {
                    // The squad unfolding: two thirds whole, whole a second on.
                    for (i, spot) in s.spots.iter().enumerate() {
                        let at = Vec3::from(*spot);
                        let at = at.truncate().extend(ground(at.truncate()));
                        let mut u = unit(self.tank_bp, 100 + i as u32, at, 1 | UNDER_CONSTRUCTION);
                        u.build = (0.66 + (t - 6.0) * 0.34).min(1.0);
                        if u.build < 1.0 {
                            u.status[1] = UNIT_REPLICATING;
                        } else {
                            u.owner_flags = 1;
                        }
                        f.units.push(u);
                    }
                }
            }
            "fleet" | "fleetprint" => {
                f.shapers.push(self.shaper(3, 0.0));
                let emitter = self.site + Vec3::Z * height * 0.5;
                // Printed aloft before it for three seconds, marked, jumping at 8 s and out
                // of the warp at 9.5 s.
                for n in 0..SHIPS {
                    let id = 200 + n as u32;
                    let berth = self.berth() + ship_off(n);
                    let spot = self.target + Vec3::Z * 260.0 + ship_off(n);
                    if t < 8.0 {
                        let mut u = unit(self.ship_bp, id, berth, 1);
                        if t < 3.0 {
                            u.owner_flags |= UNDER_CONSTRUCTION;
                            u.build = 0.2 + t * 0.25;
                            u.status[1] = UNIT_REPLICATING;
                            f.beams.push(BeamInstance {
                                from: emitter.to_array(),
                                kind: mc_sim::survival::BEAM_PRINT,
                                to_prev: berth.to_array(),
                                radius: 18.0,
                                to: berth.to_array(),
                                height: 12.0,
                            });
                            f.beam_sources.push(id | 1 << 31);
                        }
                        f.units.push(u);
                    } else if t >= 9.5 {
                        f.units.push(unit(self.ship_bp, id, spot, 1));
                    }
                    if k == 80 {
                        f.events.push(SimEvent::WarpJumped {
                            unit: mc_sim::Handle(id),
                            from: fx3(berth),
                            to: fx3(spot),
                            ticks: 15,
                            dampened: false,
                            blueprint: self.ship_bp,
                            owner: 1,
                        });
                    } else if k == 95 {
                        f.events.push(SimEvent::WarpArrived {
                            unit: mc_sim::Handle(id),
                            at: fx3(spot),
                            dampened: false,
                            blueprint: self.ship_bp,
                            owner: 1,
                        });
                    }
                }
                if (3.0..9.5).contains(&t) {
                    f.warp_ins.push(self.fleet_mark(t));
                }
            }
            _ => {
                // The Unmaker's warhead coming down on the target and going off at 3 s.
                let blast = 420.0 + 12.0 * f32::from(level - 1);
                let w_at = |t: f32| self.warhead_at(t);
                if t < 3.0 {
                    f.strategic.push(missile(
                        STRATEGIC_WARHEAD,
                        9,
                        w_at(t),
                        w_at(t - TICK),
                        StrategicLook::Progenitor,
                    ));
                }
                if k == 30 {
                    f.events.push(SimEvent::NuclearDetonation {
                        pos: fx3(self.target + Vec3::Z * 20.0),
                        radius: Fx::from_f32(blast),
                        owner: 1,
                        commander: false,
                        look: StrategicLook::Progenitor,
                    });
                }
            }
        }
        f.tick = k;
        f
    }
}

/// Writes `frame`'s pixels as a PPM.
fn write_ppm(renderer: &mut Renderer, path: &std::path::Path, w: u32, h: u32) {
    let pixels = renderer.read_pixels().unwrap();
    let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
    for p in pixels.as_chunks::<4>().0 {
        ppm.extend_from_slice(&p[..3]);
    }
    std::fs::write(path, ppm).unwrap();
}

#[test]
#[ignore = "requires Vulkan and maps/crosswater.mcmap"]
fn progenitor_shots() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let map =
        Arc::new(mc_map::MapFile::open(root.join("maps/crosswater.mcmap")).expect("open map"));
    let blueprints = Arc::new(mc_data::Blueprints::load(&root.join("data")).unwrap());
    let (w, h) = if std::env::var("PROG_BIG").is_ok() {
        (2560u32, 1440u32)
    } else {
        (1600u32, 900u32)
    };
    let mut renderer = Renderer::new(
        Target::Headless {
            width: w,
            height: h,
        },
        SceneDesc {
            map: map.clone(),
            blueprints: blueprints.clone(),
            pool: Arc::new(mc_jobs::Pool::new(2)),
            team_colors: {
                let mut c = [[0.5; 3]; mc_core::MAX_PLAYERS];
                c[0] = [0.1, 0.6, 0.9];
                c[1] = [0.85, 0.12, 0.1];
                c
            },
        },
    )
    .unwrap();
    let out = std::path::PathBuf::from(
        std::env::var("PROG_OUT")
            .unwrap_or_else(|_| root.join("artifacts/progenitor").display().to_string()),
    );
    std::fs::create_dir_all(&out).unwrap();
    let size = Vec2::from(map.info().size_metres().to_f32());
    let at = map
        .start_positions()
        .first()
        .map_or(size * 0.5, |p| Vec2::from(p.to_f32()));
    let water = map.info().water_level.to_f32();
    let id = |key: &str| blueprints.id_of(key).unwrap_or_else(|| panic!("no {key}"));
    let list = |var: &str, default: &str| -> Vec<String> {
        std::env::var(var)
            .unwrap_or_else(|_| default.into())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect()
    };
    let overlay = Overlay::default();
    let mut base = 100.0f32;
    for scene in list("PROG_SCENES", "lance,shield,antinuke,warpin,warhead") {
        for tier in list("PROG_TIERS", "1,5") {
            let tier: u8 = tier.parse().unwrap();
            let ground = |p: Vec2| renderer.ground_height(p).max(water);
            let site = at.extend(ground(at));
            // A fleet comes in well away from its Gatewright.
            let target_xy = at
                + if scene.starts_with("fleet") {
                    Vec2::new(1700.0, -900.0)
                } else {
                    Vec2::new(380.0, -220.0)
                };
            // `-close`: the same staging, the camera close on the body in flight.
            let (staged, close) = match scene.strip_suffix("-close") {
                Some(s) => (s.to_string(), true),
                None => (scene.clone(), false),
            };
            let stage = Stage {
                scene: staged.clone(),
                tier,
                site,
                target: target_xy.extend(ground(target_xy)),
                shaper_bp: id("replication_node"),
                seed_bp: id("progenitor_seedcaster"),
                gate_bp: id("progenitor_gatewright"),
                tank_bp: id("aster_t1_tank"),
                ship_bp: id(if tier >= 4 {
                    "aster_t4_frigate"
                } else {
                    "aster_t3_corvette"
                }),
            };
            let lens = crate::models::shaper::lens_height(0, stage.level());
            let mid = stage.site.lerp(stage.target, 0.5);
            // Looking across from the side: (focus, distance, yaw, pitch).
            let (times, focus, dist, pitch): (Vec<f32>, Vec3, f32, f32) = match staged.as_str() {
                // The shard leaving its halo slot, from beside the Lens.
                _ if close && staged == "antinuke" => (
                    vec![0.75, 1.1],
                    stage.site + Vec3::Z * lens,
                    lens * 0.45 + 60.0,
                    0.05,
                ),
                _ if close => {
                    let t = match staged.as_str() {
                        "warpin" => 4.0,
                        "lance" => 3.08,
                        _ => 2.0,
                    };
                    // Drawn a tick behind, eased toward the next.
                    (vec![t], stage.body_at(t - TICK), 90.0, 0.15)
                }
                "lance" => (
                    vec![2.4, 3.08, 3.4],
                    mid + Vec3::Z * lens * 0.55,
                    (lens * 2.3).max(700.0),
                    0.1,
                ),
                "shield" => {
                    let r = (180.0 + 22.0 * f32::from(stage.level() - 1)).min(900.0);
                    (vec![2.0], stage.site + Vec3::Z * r * 0.15, r * 2.8, 0.32)
                }
                "antinuke" => (
                    vec![1.6, 2.7, 3.3],
                    stage.site + Vec3::new(-200.0, 140.0, 1050.0),
                    2600.0,
                    0.05,
                ),
                "warpin" => (
                    vec![1.5, 4.5, 5.6, 6.15, 6.6, 7.5],
                    stage.target + Vec3::Z * 25.0,
                    (stage.warp_in().reach * 6.0).max(420.0),
                    0.28,
                ),
                "warpcancel" => (
                    vec![3.6, 4.15, 4.6],
                    stage.target + Vec3::Z * 25.0,
                    (stage.warp_in().reach * 6.0).max(420.0),
                    0.28,
                ),
                // The fleet printed aloft before its Gatewright.
                "fleetprint" => (vec![1.0, 2.5], stage.berth(), 900.0, 0.12),
                // Its sky gate over the target, the jump and the arrival.
                "fleet" => (
                    vec![5.0, 7.9, 9.55, 9.9, 10.6],
                    stage.target + Vec3::Z * 180.0,
                    1300.0,
                    0.12,
                ),
                _ => (
                    vec![2.6, 3.4, 4.6, 8.0],
                    stage.target + Vec3::Z * 350.0,
                    3200.0,
                    0.18,
                ),
            };
            let times = std::env::var("PROG_TIMES")
                .map(|v| v.split(',').map(|x| x.trim().parse().unwrap()).collect())
                .unwrap_or(times);
            // `PROG_CAM`: dx,dy,dz off the Shaper's foot, distance, yaw, pitch.
            let cam: Vec<f32> = std::env::var("PROG_CAM")
                .map(|v| v.split(',').map(|x| x.trim().parse().unwrap()).collect())
                .unwrap_or_else(|_| {
                    let f = focus - stage.site;
                    vec![f.x, f.y, f.z, dist, 0.52, pitch]
                });
            let mut camera = Camera::new(size, Vec2::new(w as f32, h as f32));
            camera.focus = stage.site + Vec3::new(cam[0], cam[1], cam[2]);
            camera.distance = cam[3];
            camera.yaw = cam[4];
            camera.pitch_free = Some(cam[5]);
            let end = times.iter().copied().fold(0.0, f32::max);
            let mut next = 0;
            let mut t = 0.0f32;
            while next < times.len() && t <= end + 0.06 {
                let k = (t / TICK).round() as u32;
                let ticked = (t / TICK - k as f32).abs() < 0.01;
                let frame = stage.frame(k, &|p| renderer.ground_height(p).max(water));
                let frame = RenderFrame {
                    props_dead: vec![0; map.props().len().div_ceil(32)],
                    ..frame
                };
                renderer
                    .render(&FrameInput {
                        camera: &camera,
                        time: base + t,
                        alpha: (t / TICK - k as f32).max(0.0),
                        sim: ticked.then_some(&frame),
                        ghosts: &[],
                        marks: &[],
                        ranges: &[],
                        ranges_drawn: 0,
                        overlay: &overlay,
                        build_grid: false,
                        icons: false,
                    })
                    .unwrap();
                if t + 0.001 >= times[next] {
                    let name = format!("{scene}_t{tier}_{:.2}.ppm", times[next]);
                    write_ppm(&mut renderer, &out.join(name), w, h);
                    next += 1;
                }
                t += 0.05;
            }
            println!("{scene} tier {tier}: done");
            base += 200.0;
        }
    }
}
