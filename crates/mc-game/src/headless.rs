//! Headless runs on the same runtime as the game: screenshots and benchmarks.
//! The sim is stepped synchronously here so results are reproducible.

use crate::setup::{self, Options};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_render::{Camera, FrameInput, Overlay, Renderer, SceneDesc, Target};
use mc_sim::{RenderFrame, World};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

pub struct Shot {
    pub path: String,
    /// Focus x, y; distance; yaw in degrees.
    pub camera: Option<[f32; 4]>,
    pub width: u32,
    pub height: u32,
    /// Match screenshots: select player 0's units whose blueprint key contains
    /// this (the first one, or all of them with a trailing `*`) instead of the commander.
    pub select: Option<String>,
    /// Match screenshots: where the pointer is, in pixels.
    pub cursor: Option<[f32; 2]>,
    /// Match screenshots: show the pause card.
    pub paused: bool,
    /// Match screenshots: the commander has structures planned and a way to walk, and shift is held.
    pub plans: bool,
    /// With `plans`: the order under `cursor` has been dragged to this pixel.
    pub drag: Option<[f32; 2]>,
    /// Play this many more ticks through the renderer before the frame that is
    /// kept, so what builds up over time (smoke, dust, track marks, a shell's
    /// flight) is in the picture; and how far into the last tick that frame is.
    pub follow: u32,
    pub alpha: f32,
}

/// Builds the world for `opts` and runs it for `ticks`.
pub fn run_sim(
    opts: &Options,
    map: &Arc<MapFile>,
    blueprints: &Arc<Blueprints>,
    pool: &Arc<Pool>,
    ticks: u32,
    report: bool,
) -> Result<World, String> {
    let config = setup::match_config(opts, map);
    let mut world =
        World::new(map, blueprints.clone(), pool.clone(), &config).map_err(|e| e.to_string())?;
    let opening = setup::opening_commands(opts, map, blueprints, &config);
    let mut worst = 0u64;
    let mut total = 0u64;
    let mut phase_totals: Vec<(&'static str, u64)> = Vec::new();
    for t in 0..ticks {
        let commands = match t {
            0 => opening.clone(),
            1 => setup::scene_orders(opts, map, blueprints, &world),
            _ => Vec::new(),
        };
        world.tick(&commands).map_err(|e| e.to_string())?;
        if world.timings.total_ns > worst && report {
            let slowest = world
                .timings
                .phases
                .iter()
                .max_by_key(|p| p.1)
                .map_or(("", 0), |p| *p);
            log::debug!(
                "tick {t}: {:.2} ms, mostly {} ({:.2} ms)",
                world.timings.total_ns as f64 / 1e6,
                slowest.0,
                slowest.1 as f64 / 1e6
            );
        }
        worst = worst.max(world.timings.total_ns);
        total += world.timings.total_ns;
        for (i, (name, ns)) in world.timings.phases.iter().enumerate() {
            if phase_totals.len() <= i {
                phase_totals.push((name, 0));
            }
            phase_totals[i].1 += ns;
        }
    }
    if report && ticks > 0 {
        let s = &world.state;
        println!(
            "{ticks} ticks: mean {:.2} ms, worst {:.2} ms | {} units, {} projectiles, {} wrecks, {} stains | hash {:016x}",
            total as f64 / ticks as f64 / 1e6,
            worst as f64 / 1e6,
            s.units.slots.live(),
            s.projectiles.len(),
            s.wrecks.slots.live(),
            s.stains.len(),
            world.hash()
        );
        for (name, ns) in &phase_totals {
            println!(
                "  {name:<12} {:>8.3} ms/tick",
                *ns as f64 / ticks as f64 / 1e6
            );
        }
        let nav = world.nav.stats();
        println!(
            "  paths: {} live fields, {} tiles, {} late joins",
            nav.live_fields, nav.total_tiles, nav.late_joins
        );
    }
    Ok(world)
}

pub fn screenshot(
    opts: &Options,
    map: Arc<MapFile>,
    blueprints: Arc<Blueprints>,
    pool: Arc<Pool>,
    ticks: u32,
    shot: &Shot,
) -> Result<(), String> {
    let mut world = run_sim(opts, &map, &blueprints, &pool, ticks, true)?;
    // A selected factory gets a queue to show: a few of its first two products.
    if let Some(key) = &shot.select {
        let u = &world.state.units;
        let factory = u.slots.iter().find(|&r| {
            u.owner[r] == 0
                && world.bp(r).key.contains(key.trim_end_matches('*'))
                && world.bp(r).has(mc_data::cat::FACTORY)
        });
        if let Some(row) = factory {
            let builds = world
                .bp(row)
                .builder
                .as_ref()
                .map(|b| b.builds.clone())
                .unwrap_or_default();
            let factories = vec![u.id(row)];
            let orders: Vec<mc_sim::PlayerCommand> = [(0usize, 3u8), (1, 2), (0, 1)]
                .iter()
                .filter_map(|(i, count)| {
                    builds.get(*i).map(|b| mc_sim::PlayerCommand {
                        player: 0,
                        command: mc_sim::Command::Produce {
                            factories: factories.clone(),
                            blueprint: *b,
                            count: *count,
                        },
                    })
                })
                .collect();
            world.tick(&orders).map_err(|e| e.to_string())?;
            for _ in 0..25 {
                world.tick(&[]).map_err(|e| e.to_string())?;
            }
        }
    }
    if shot.plans {
        plan_a_base(&mut world)?;
    }
    let mut frame = RenderFrame::default();
    world.write_render_frame(if opts.fog { Some(0) } else { None }, &mut frame);

    let scene = SceneDesc {
        map: map.clone(),
        blueprints,
        pool,
        team_colors: setup::TEAM_COLORS,
    };
    let mut renderer = Renderer::new(
        Target::Headless {
            width: shot.width,
            height: shot.height,
        },
        scene,
    )
    .map_err(|e| e.to_string())?;
    let size = map.info().size_metres().to_f32();
    let mut camera = Camera::new(
        glam::Vec2::from(size),
        glam::Vec2::new(shot.width as f32, shot.height as f32),
    );
    if let Some([x, y, distance, yaw]) = shot.camera {
        camera.focus = glam::Vec3::new(x, y, renderer.ground_height(glam::Vec2::new(x, y)));
        camera.distance = distance.clamp(mc_render::camera::MIN_DISTANCE, camera.max_distance());
        camera.yaw = yaw.to_radians();
    }

    // The same HUD the game draws, with `select` selected (the commander by default) so the panels show.
    let (mut overlay, mut memory) = (Overlay::default(), crate::ui::Memory::default());
    overlay.set_image(
        crate::hud::MINIMAP_SLOT,
        crate::ui::preview::SIZE,
        crate::ui::preview::SIZE,
        &crate::ui::preview::render(&map),
    );
    let mut view = crate::game::View::new(0, setup::TEAM_COLORS, true);
    view.status = crate::sim_thread::status_of(&world, world.timings.total_ns);
    view.status.owns_clock = true;
    view.index_of = frame
        .units
        .iter()
        .enumerate()
        .filter(|(_, u)| u.owner_flags & mc_sim::mirror::KIND_WRECK == 0)
        .map(|(i, u)| (u.unit_id, i))
        .collect();
    view.frame = frame.clone();
    view.paused = shot.paused;
    if opts.scene == setup::Scene::Range {
        let subject = world
            .blueprints
            .id_of(&opts.subject)
            .ok_or("unknown --unit")?;
        view.range = Some(crate::range::Range::new(setup::range_pad(&map), subject));
    }
    let wanted = |u: &mc_sim::mirror::UnitInstance| match &shot.select {
        Some(key) => {
            u.owner_flags & 0xFF == 0
                && world
                    .blueprints
                    .unit(mc_data::BlueprintId(u.blueprint as u16))
                    .key
                    .contains(key.trim_end_matches('*'))
        }
        None => world
            .state
            .players
            .first()
            .is_some_and(|p| p.commander.0 == u.unit_id),
    };
    view.selection = frame
        .units
        .iter()
        .filter(|u| u.owner_flags & mc_sim::mirror::KIND_WRECK == 0 && wanted(u))
        .map(|u| u.unit_id)
        .collect();
    if shot.select.as_deref().is_some_and(|k| !k.ends_with('*')) {
        view.selection.truncate(1);
    }
    view.groups[1] = view.selection.clone();
    view.shift = shot.plans;
    world.write_orders(
        Some(0),
        &view.selection,
        shot.plans.then_some(0),
        &mut view.status.queues,
    );
    world.write_plans(0, &mut view.status.plans);
    let marks: Vec<mc_render::Mark> = view
        .selection
        .iter()
        .filter_map(|id| view.index_of.get(id).copied())
        .map(|i| {
            let u = &frame.units[i];
            mc_render::Mark {
                unit_index: i as u32,
                kind: 0,
                work: crate::game::unit_bar_work(u, &view.status.queues),
                _pad: 0,
            }
        })
        .collect();
    let (ranges, ranges_drawn) = crate::rings::Rings::new(&world.blueprints).collect(
        view.selection
            .iter()
            .filter_map(|id| view.index_of.get(id))
            .map(|&i| &frame.units[i]),
        1.0,
        true,
    );
    view.reaches = crate::rings::Rings::key(&ranges);
    let mut hud = crate::hud::Hud::default();
    let audio = crate::audio::Audio::silent();
    let input = crate::ui::Input {
        cursor: shot
            .cursor
            .map_or(glam::Vec2::splat(-100.0), glam::Vec2::from),
        ..Default::default()
    };
    let (mut order_map, mut ghosts) = (crate::orders::OrderMap::default(), Vec::new());
    let started = Instant::now();
    // A few frames so streamed terrain tiles arrive (and hover glows settle) before the one we keep.
    for i in 0..40 {
        overlay.clear();
        memory.begin_frame();
        let mut ui = crate::ui::Ui::new(
            &mut overlay,
            &input,
            &mut memory,
            &audio,
            camera.viewport,
            1.0,
            10.0 + i as f32 * 0.016,
            0.016,
        );
        // The orders on the map, as the game draws them; with `drag`, one of them in hand.
        let field = crate::orders::Field {
            view: &view,
            blueprints: &world.blueprints,
            map: &map,
            camera: &camera,
            renderer: &renderer,
        };
        if let (Some(to), 39) = (shot.drag, i) {
            order_map.press(&field, input.cursor);
            order_map.update(&field, glam::Vec2::from(to), false);
        } else {
            order_map.update(&field, input.cursor, false);
        }
        ghosts.clear();
        order_map.ghosts(&field, &mut ghosts);
        order_map.draw(&mut ui, &field);
        let scene = crate::hud::Scene {
            view: &view,
            blueprints: &world.blueprints,
            map: &map,
            camera: &camera,
            gpu: &renderer.stats,
        };
        hud.draw(&mut ui, &scene, 0.016);
        memory.end_frame(&input);
        let input = FrameInput {
            camera: &camera,
            time: 10.0 + i as f32 * 0.016,
            alpha: 1.0,
            sim: (i == 0).then_some(&frame),
            ghosts: &ghosts,
            marks: &marks,
            ranges: &ranges,
            ranges_drawn,
            overlay: &overlay,
            build_grid: false,
        };
        renderer.render(&input).map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    // The followed ticks, at the pace of a live match: two frames a tick.
    let mut time = 10.0 + 40.0 * 0.016;
    for k in 0..shot.follow {
        // With `--ticks 1` the scene's orders are still owed: given here, what they
        // set off (a self-destruct, say) happens where the renderer sees it.
        let owed = if k == 0 && ticks == 1 {
            setup::scene_orders(opts, &map, &world.blueprints.clone(), &world)
        } else {
            Vec::new()
        };
        world.tick(&owed).map_err(|e| e.to_string())?;
        world.write_render_frame(if opts.fog { Some(0) } else { None }, &mut frame);
        let last = k + 1 == shot.follow;
        for (i, alpha) in [
            if last { shot.alpha * 0.5 } else { 0.5 },
            if last { shot.alpha } else { 1.0 },
        ]
        .into_iter()
        .enumerate()
        {
            let at = time + alpha / mc_core::TICKS_PER_SECOND as f32;
            let input = FrameInput {
                camera: &camera,
                time: at,
                alpha,
                sim: (i == 0).then_some(&frame),
                ghosts: &ghosts,
                marks: &marks,
                ranges: &ranges,
                ranges_drawn,
                overlay: &overlay,
                build_grid: false,
            };
            renderer.render(&input).map_err(|e| e.to_string())?;
        }
        time += 1.0 / mc_core::TICKS_PER_SECOND as f32;
    }
    if overlay.overflowed {
        log::warn!("the overlay ran out of vertices");
    }
    println!("{} overlay vertices", overlay.vertices.len());
    let pixels = renderer
        .read_pixels()
        .ok_or("no pixels from a headless target")?;
    println!(
        "rendered on {} in {:.1} s; gpu passes: {:?}",
        renderer.device_name(),
        started.elapsed().as_secs_f32(),
        renderer.stats.gpu_passes
    );
    write_png(Path::new(&shot.path), shot.width, shot.height, &pixels)
}

/// `--plans`: player 0's commander is told to build a few structures around itself and
/// then to walk on. Two of the build orders overlap on purpose: the sim keeps the first.
fn plan_a_base(world: &mut World) -> Result<(), String> {
    use mc_core::{Angle, FxVec2};
    use mc_sim::{Command, PlayerCommand};
    let acu = world
        .state
        .players
        .first()
        .map(|p| p.commander)
        .ok_or("no players")?;
    let row = world
        .state
        .units
        .row(acu)
        .ok_or("player 0 has no commander")?;
    let at = world.state.units.pos[row];
    let builds = world
        .bp(row)
        .builder
        .as_ref()
        .map(|b| b.builds.clone())
        .unwrap_or_default();
    let pick = |category: u32| {
        builds.iter().copied().find(|b| {
            world.blueprints.unit(*b).has(category) && !world.blueprints.unit(*b).needs_deposit
        })
    };
    let (power, factory) = (
        pick(mc_data::cat::POWER).ok_or("the commander builds no power")?,
        pick(mc_data::cat::FACTORY).ok_or("the commander builds no factory")?,
    );
    let build = |blueprint, dx: i32, dy: i32, queue| PlayerCommand {
        player: 0,
        command: Command::Build {
            units: vec![acu],
            blueprint,
            pos: at + FxVec2::from_ints(dx, dy),
            heading: Angle::from_degrees(270),
            queue,
        },
    };
    let commands = [
        build(power, 140, -40, false),
        build(power, 140, 8, true),
        build(factory, 150, 110, true),
        // Across the factory: refused.
        build(power, 170, 120, true),
        PlayerCommand {
            player: 0,
            command: Command::Move {
                units: vec![acu],
                target: at + FxVec2::from_ints(-60, 160),
                queue: true,
            },
        },
    ];
    world.tick(&commands).map_err(|e| e.to_string())?;
    println!("planned a base around {:?}", at.to_f32());
    Ok(())
}

/// Draws a front-end screen over its backdrop battle, the way the game would
/// after the screen has been up for a couple of seconds.
pub fn ui_screenshot(
    screen: crate::ui::front::Screen,
    blueprints: Arc<Blueprints>,
    pool: Arc<Pool>,
    ticks: u32,
    shot: &Shot,
    cursor: Option<[f32; 2]>,
) -> Result<(), String> {
    use crate::ui::{self, backdrop::Director, front::Front, menu};
    let path = setup::backdrop_map().ok_or("no maps found")?;
    let map = Arc::new(MapFile::open(&path).map_err(|e| format!("{}: {e}", path.display()))?);
    let opts = Options {
        map: path,
        scene: setup::Scene::Backdrop,
        seed: 7,
        army: 0,
        fog: false,
        ..Default::default()
    };
    let world = run_sim(&opts, &map, &blueprints, &pool, ticks.max(2), false)?;
    let mut frame = RenderFrame::default();
    world.write_render_frame(None, &mut frame);
    let status = crate::sim_thread::status_of(&world, 0);

    let scene = SceneDesc {
        map: map.clone(),
        blueprints,
        pool,
        team_colors: setup::TEAM_COLORS,
    };
    let mut renderer = Renderer::new(
        Target::Headless {
            width: shot.width,
            height: shot.height,
        },
        scene,
    )
    .map_err(|e| e.to_string())?;
    let viewport = glam::Vec2::new(shot.width as f32, shot.height as f32);
    let mut camera = Camera::new(
        glam::Vec2::from(map.info().size_metres().to_f32()),
        viewport,
    );

    let mut settings = crate::settings::Settings::default();
    let mut front = Front::new(Director::new(&map, true));
    front.show(screen, &settings);
    let audio = crate::audio::Audio::silent();
    let (mut overlay, mut memory) = (Overlay::default(), ui::Memory::default());
    overlay.set_image(
        menu::PREVIEW_SLOT,
        ui::preview::SIZE,
        ui::preview::SIZE,
        &ui::preview::render(&map),
    );
    let input = ui::Input {
        cursor: cursor.map_or(glam::Vec2::splat(-100.0), glam::Vec2::from),
        ..Default::default()
    };
    for i in 0..90 {
        let (time, dt) = (20.0 + i as f32 / 30.0, 1.0 / 30.0);
        front.director.apply(&mut camera);
        camera.focus.z = renderer
            .ground_height(camera.focus.truncate())
            .max(map.info().water_level.to_f32());
        overlay.clear();
        memory.begin_frame();
        let mut ui = ui::Ui::new(
            &mut overlay,
            &input,
            &mut memory,
            &audio,
            viewport,
            settings.ui_scale,
            time,
            dt,
        );
        let telemetry = menu::Telemetry {
            map_name: map.name(),
            tick: status.tick,
            units: status.units,
            camera: camera.focus.truncate(),
            altitude: camera.eye().z - camera.focus.z,
            preview: true,
        };
        front.frame(&mut ui, &mut settings, &telemetry);
        memory.end_frame(&input);
        let input = FrameInput {
            camera: &camera,
            time,
            alpha: 1.0,
            sim: (i == 0).then_some(&frame),
            ghosts: &[],
            marks: &[],
            ranges: &[],
            ranges_drawn: 0,
            overlay: &overlay,
            build_grid: false,
        };
        renderer.render(&input).map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(3));
    }
    let pixels = renderer
        .read_pixels()
        .ok_or("no pixels from a headless target")?;
    if overlay.overflowed {
        log::warn!("the overlay ran out of vertices");
    }
    println!("{} overlay vertices", overlay.vertices.len());
    write_png(Path::new(&shot.path), shot.width, shot.height, &pixels)
}

pub fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())?;
    println!("wrote {}", path.display());
    Ok(())
}
