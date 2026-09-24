//! Survival mode's replicator effects, staged without the sim: the Replication
//! Engine under its veil, the replication ray raising a node, and print beams.
//! Test-only: `survival_shots` renders them through the real renderer.

#[cfg(test)]
mod survival_shots {
    use super::super::{FrameInput, Renderer, SceneDesc, Target};
    use crate::camera::Camera;
    use crate::overlay::Overlay;
    use bytemuck::Zeroable;
    use glam::{Vec2, Vec3};
    use mc_core::{Fx, FxVec3};
    use mc_data::{BlueprintId, WeaponColor};
    use mc_sim::mirror::*;
    use mc_sim::reclaim::BeamInstance;
    use std::sync::Arc;

    const TICK: f32 = 0.1;
    /// `owner_flags` for a site (the sim's flags sit from bit 8).
    const UNDER_CONSTRUCTION: u32 = 1 << 8;

    fn fx3(v: Vec3) -> FxVec3 {
        FxVec3::new(Fx::from_f32(v.x), Fx::from_f32(v.y), Fx::from_f32(v.z))
    }

    fn unit(bp: BlueprintId, id: u32, pos: Vec3, heading: f32, radius: f32, owner: u32) -> UnitInstance {
        let mut u = UnitInstance::zeroed();
        u.prev_pos = pos.to_array();
        u.pos = pos.to_array();
        u.prev_heading = heading;
        u.heading = heading;
        u.prev_turret_yaw = heading;
        u.turret_yaw = heading;
        u.blueprint = bp.0 as u32;
        u.owner_flags = owner;
        u.health = 1.0;
        u.build = 1.0;
        u.radius = radius;
        u.unit_id = id;
        u
    }

    /// Stages the replicators on dev16 and writes PPMs.
    /// `SURV_SCENES`: veil, ray, print (default all). `SURV_CAMS`: `;`-separated
    /// `dx,dy,dist,yaw,tilt` (metres off the engine, radians); `SURV_TIMES` seconds;
    /// `SURV_OUT` folder; `SURV_RAY` metres from the engine to the node site;
    /// `SURV_BIG` for 2560x1440; `SURV_PLAIN` draws the veil as an ordinary dome;
    /// `SURV_NOFX` leaves the veil, beams and hits out (to measure what they cost).
    /// Prints the mean GPU pass times over the last 20 frames of each scene.
    #[test]
    #[ignore = "requires Vulkan and maps/dev16.mcmap"]
    fn survival_shots() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let map = Arc::new(mc_map::MapFile::open(root.join("maps/dev16.mcmap")).expect("open map"));
        let blueprints = Arc::new(mc_data::Blueprints::load(&root.join("data")).unwrap());
        let (w, h) = if std::env::var("SURV_BIG").is_ok() { (2560u32, 1440u32) } else { (1600u32, 900u32) };
        let mut renderer = Renderer::new(
            Target::Headless { width: w, height: h },
            SceneDesc {
                map: map.clone(),
                blueprints: blueprints.clone(),
                pool: Arc::new(mc_jobs::Pool::new(2)),
                team_colors: [[0.85, 0.12, 0.1], [0.1, 0.6, 0.9], [0.9, 0.7, 0.1], [0.3, 0.9, 0.3], [0.8, 0.3, 0.9], [0.9, 0.5, 0.2], [0.5, 0.5, 0.5], [0.2, 0.9, 0.9]],
            },
        )
        .unwrap();
        let out = std::path::PathBuf::from(
            std::env::var("SURV_OUT").unwrap_or_else(|_| root.join("artifacts/survival").display().to_string()),
        );
        std::fs::create_dir_all(&out).unwrap();
        let size = Vec2::from(map.info().size_metres().to_f32());
        let at = map.start_positions().first().map_or(size * 0.5, |p| Vec2::from(p.to_f32()));
        let ground = |r: &Renderer, p: Vec2| r.ground_height(p).max(map.info().water_level.to_f32());
        let id = |key: &str| blueprints.id_of(key).unwrap_or_else(|| panic!("no {key}"));
        let (engine_bp, node_bp, tank_bp, bot_bp) =
            (id("replication_engine"), id("replication_node"), id("aster_t1_tank"), id("aster_t1_bot"));
        let radius = |bp: BlueprintId| blueprints.unit(bp).radius.to_f32();
        let height = |bp: BlueprintId| blueprints.unit(bp).height.to_f32();
        let engine_at = at.extend(ground(&renderer, at));
        let ray_len: f32 = std::env::var("SURV_RAY").ok().and_then(|v| v.parse().ok()).unwrap_or(2600.0);
        let site_xy = at + Vec2::new(0.8, 0.6) * ray_len;
        let site = site_xy.extend(ground(&renderer, site_xy));
        let overlay = Overlay::default();
        let scenes = std::env::var("SURV_SCENES").unwrap_or_else(|_| "veil,ray,print".into());
        let mut base = 100.0f32;
        for scene in scenes.split(',') {
            let captures: Vec<f32> = match std::env::var("SURV_TIMES") {
                Ok(t) => t.split(',').map(|v| v.trim().parse().unwrap()).collect(),
                Err(_) => vec![2.0],
            };
            let cams: Vec<Vec<f32>> = std::env::var("SURV_CAMS")
                .unwrap_or_else(|_| match scene {
                    "veil" => "0,0,700,0.6,0.15".into(),
                    "ray" => "0,0,2200,0.9,0".into(),
                    _ => "150,0,260,0.5,0".into(),
                })
                .split(';')
                .map(|c| c.split(',').map(|v| v.trim().parse().unwrap()).collect())
                .collect();
            let end = captures.iter().copied().fold(0.0, f32::max);
            for (ci, cam) in cams.iter().enumerate() {
                let mut camera = Camera::new(size, Vec2::new(w as f32, h as f32));
                let focus = at + Vec2::new(cam[0], cam[1]);
                camera.focus = focus.extend(ground(&renderer, focus));
                camera.distance = cam[2];
                camera.yaw = cam[3];
                camera.tilt = cam[4];
                let mut next = 0;
                let mut sums: Vec<f32> = Vec::new();
                let mut frames = 0;
                let mut t = 0.0f32;
                while next < captures.len() {
                    let k = (t / TICK).round() as u32;
                    let ticked = (t / TICK - k as f32).abs() < 0.01;
                    let mut frame = RenderFrame::default();
                    frame.props_dead = vec![0; map.props().len().div_ceil(32)];
                    // The engine, hostile (player 0 draws red here).
                    frame.units.push(unit(engine_bp, 1, engine_at, 0.0, radius(engine_bp), 0));
                    match scene {
                        "veil" => {
                            let veil = std::env::var("SURV_PLAIN").is_err();
                            frame.shields.push(ShieldInstance {
                                pos: engine_at.to_array(),
                                radius: 170.0,
                                prev_open: 1.0,
                                open: 1.0,
                                health: 1.0,
                                packed: 5 << 16 | if veil { SHIELD_VEIL } else { 0 },
                                unit_id: 1,
                                projector: 32.0,
                                height: 150.0,
                                _pad: 0.0,
                            });
                            // Shots from the south-west landing all over the near side.
                            if k % 3 == 1 {
                                let a = 3.4 + (k as f32 * 0.71).sin() * 0.9;
                                let e = 0.25 + 0.5 * (k as f32 * 0.37).cos().abs();
                                let dir = Vec3::new(a.cos() * e.cos(), a.sin() * e.cos(), e.sin());
                                frame.events.push(SimEvent::Impact {
                                    pos: fx3(engine_at + dir * 170.0),
                                    target_motion: FxVec3::ZERO,
                                    splash: Fx::from_f32(2.0),
                                    color: WeaponColor::Orange,
                                    after: Fx::from_f32(0.3),
                                    on_unit: false,
                                    on_shield: true,
                                    blueprint: tank_bp,
                                    weapon: 0,
                                });
                            }
                        }
                        "ray" => {
                            let raise = (t / 6.0).min(1.0);
                            let mut node = unit(node_bp, 2, site, 0.3, radius(node_bp), UNDER_CONSTRUCTION);
                            node.build = raise;
                            node._pad3[1] = UNIT_REPLICATING;
                            frame.units.push(node);
                            frame.beams.push(BeamInstance {
                                from: (engine_at + Vec3::new(0.0, 0.0, 140.0)).to_array(),
                                kind: 4,
                                to_prev: site.to_array(),
                                radius: 7.0,
                                to: site.to_array(),
                                height: raise,
                            });
                            frame.beam_sources.push(1);
                        }
                        _ => {
                            // Bay 0 prints a Warden; the node beside it prints a bot.
                            let spot = engine_at.truncate() + Vec2::new(150.0, 0.0);
                            let spot = spot.extend(ground(&renderer, spot));
                            let mut u = unit(tank_bp, 3, spot, 0.0, radius(tank_bp), UNDER_CONSTRUCTION);
                            u.build = (0.2 + t * 0.12).min(0.95);
                            u._pad3[1] = UNIT_REPLICATING;
                            frame.units.push(u);
                            frame.beams.push(BeamInstance {
                                from: (engine_at + Vec3::new(100.0, 0.0, 55.0)).to_array(),
                                kind: 5,
                                to_prev: spot.to_array(),
                                radius: radius(tank_bp),
                                to: spot.to_array(),
                                height: height(tank_bp),
                            });
                            frame.beam_sources.push(3);
                            let node_xy = engine_at.truncate() + Vec2::new(210.0, 90.0);
                            let node_at = node_xy.extend(ground(&renderer, node_xy));
                            frame.units.push(unit(node_bp, 4, node_at, 0.0, radius(node_bp), 0));
                            let bot_xy = node_xy + Vec2::new(50.0, 0.0);
                            let bot_at = bot_xy.extend(ground(&renderer, bot_xy));
                            let mut bot = unit(bot_bp, 5, bot_at, 0.0, radius(bot_bp), UNDER_CONSTRUCTION);
                            bot.build = (0.1 + t * 0.15).min(0.95);
                            bot._pad3[1] = UNIT_REPLICATING;
                            frame.units.push(bot);
                            frame.beams.push(BeamInstance {
                                from: (node_at + Vec3::new(0.0, 0.0, 40.0)).to_array(),
                                kind: 5,
                                to_prev: bot_at.to_array(),
                                radius: radius(bot_bp),
                                to: bot_at.to_array(),
                                height: height(bot_bp),
                            });
                            frame.beam_sources.push(5);
                        }
                    }
                    if std::env::var("SURV_NOFX").is_ok() {
                        // For cost: the same scene with no veil and no beams.
                        frame.shields.clear();
                        frame.beams.clear();
                        frame.beam_sources.clear();
                        frame.events.clear();
                    }
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
                        })
                        .unwrap();
                    if t + 20.0 * 0.05 > end {
                        frames += 1;
                        for (i, (_, ms)) in renderer.stats.gpu_passes.iter().enumerate() {
                            if sums.len() <= i {
                                sums.push(0.0);
                            }
                            sums[i] += ms;
                        }
                    }
                    if t + 0.001 >= captures[next] {
                        let pixels = renderer.read_pixels().unwrap();
                        let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
                        for p in pixels.chunks_exact(4) {
                            ppm.extend_from_slice(&p[..3]);
                        }
                        std::fs::write(out.join(format!("{scene}_c{ci}_{:.2}.ppm", captures[next])), ppm).unwrap();
                        next += 1;
                    }
                    t += 0.05;
                }
                let names: Vec<_> = renderer.stats.gpu_passes.iter().map(|p| p.0).collect();
                let mean: Vec<String> =
                    names.iter().zip(&sums).map(|(n, ms)| format!("{n} {:.3}", ms / frames.max(1) as f32)).collect();
                println!("{scene} cam{ci}: {}", mean.join(", "));
                base += 100.0;
            }
        }
    }
}
