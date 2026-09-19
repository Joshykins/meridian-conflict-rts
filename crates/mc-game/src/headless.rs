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
}

fn unit_list(world: &World) -> Vec<(u8, mc_sim::UnitId, bool)> {
    let u = &world.state.units;
    u.slots.iter().map(|r| (u.owner[r], u.id(r), world.bp(r).is_mobile())).collect()
}

/// Builds the world for `opts` and runs it for `ticks`.
pub fn run_sim(opts: &Options, map: &Arc<MapFile>, blueprints: &Arc<Blueprints>, pool: &Arc<Pool>, ticks: u32, report: bool) -> Result<World, String> {
    let config = setup::match_config(opts, map);
    let mut world = World::new(map, blueprints.clone(), pool.clone(), &config).map_err(|e| e.to_string())?;
    let opening = setup::opening_commands(opts, map, blueprints, &config);
    let mut worst = 0u64;
    let mut total = 0u64;
    let mut phase_totals: Vec<(&'static str, u64)> = Vec::new();
    for t in 0..ticks {
        let commands = match t {
            0 => opening.clone(),
            1 => setup::scene_orders(opts, map, &unit_list(&world)),
            _ => Vec::new(),
        };
        world.tick(&commands).map_err(|e| e.to_string())?;
        if world.timings.total_ns > worst && report {
            let slowest = world.timings.phases.iter().max_by_key(|p| p.1).map_or(("", 0), |p| *p);
            log::debug!("tick {t}: {:.2} ms, mostly {} ({:.2} ms)", world.timings.total_ns as f64 / 1e6, slowest.0, slowest.1 as f64 / 1e6);
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
            println!("  {name:<12} {:>8.3} ms/tick", *ns as f64 / ticks as f64 / 1e6);
        }
        let nav = world.nav.stats();
        println!("  paths: {} live fields, {} tiles, {} late joins", nav.live_fields, nav.total_tiles, nav.late_joins);
    }
    Ok(world)
}

pub fn screenshot(opts: &Options, map: Arc<MapFile>, blueprints: Arc<Blueprints>, pool: Arc<Pool>, ticks: u32, shot: &Shot) -> Result<(), String> {
    let world = run_sim(opts, &map, &blueprints, &pool, ticks, true)?;
    let mut frame = RenderFrame::default();
    world.write_render_frame(if opts.fog { Some(0) } else { None }, &mut frame);

    let scene = SceneDesc { map: map.clone(), blueprints, pool, team_colors: setup::TEAM_COLORS };
    let mut renderer = Renderer::new(Target::Headless { width: shot.width, height: shot.height }, scene).map_err(|e| e.to_string())?;
    let size = map.info().size_metres().to_f32();
    let mut camera = Camera::new(glam::Vec2::from(size), glam::Vec2::new(shot.width as f32, shot.height as f32));
    if let Some([x, y, distance, yaw]) = shot.camera {
        camera.focus = glam::Vec3::new(x, y, renderer.ground_height(glam::Vec2::new(x, y)));
        camera.distance = distance.clamp(mc_render::camera::MIN_DISTANCE, camera.max_distance());
        camera.yaw = yaw.to_radians();
    }

    // The same HUD the game draws, with the commander selected so the build menu shows.
    let mut overlay = Overlay::default();
    let mut view = crate::game::View {
        local: 0,
        status: crate::sim_thread::status_of(&world, world.timings.total_ns),
        index_of: frame.units.iter().enumerate().map(|(i, u)| (u.unit_id, i)).collect(),
        selection: Vec::new(),
        mode: crate::game::Mode::Normal,
        fps: 0.0,
        cpu_ms: 0.0,
        show_profiler: true,
        menu_open: false,
        frame: frame.clone(),
    };
    if let Some(player) = world.state.players.first() {
        if world.state.units.row(player.commander).is_some() {
            view.selection.push(player.commander.0);
        }
    }
    let marks: Vec<mc_render::Mark> = view.selection.iter().filter_map(|id| view.index_of.get(id)).map(|&i| mc_render::Mark { unit_index: i as u32, kind: 0 }).collect();
    let mut hud = crate::hud::Hud::default();
    let started = Instant::now();
    // A few frames so streamed terrain tiles arrive before the one we keep.
    for i in 0..40 {
        overlay.clear();
        hud.draw(&mut overlay, &view, &world.blueprints, camera.viewport, &renderer.stats);
        let input = FrameInput { camera: &camera, time: 10.0 + i as f32 * 0.016, alpha: 1.0, sim: (i == 0).then_some(&frame), ghosts: &[], marks: &marks, overlay: &overlay, build_grid: false };
        renderer.render(&input).map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let pixels = renderer.read_pixels().ok_or("no pixels from a headless target")?;
    println!("rendered on {} in {:.1} s; gpu passes: {:?}", renderer.device_name(), started.elapsed().as_secs_f32(), renderer.stats.gpu_passes);
    write_png(Path::new(&shot.path), shot.width, shot.height, &pixels)
}

/// Draws a front-end screen over its backdrop battle, the way the game would
/// after the screen has been up for a couple of seconds.
pub fn ui_screenshot(screen: crate::ui::front::Screen, blueprints: Arc<Blueprints>, pool: Arc<Pool>, ticks: u32, shot: &Shot, cursor: Option<[f32; 2]>) -> Result<(), String> {
    use crate::ui::{self, backdrop::Director, front::Front, menu};
    let path = setup::backdrop_map().ok_or("no maps found")?;
    let map = Arc::new(MapFile::open(&path).map_err(|e| format!("{}: {e}", path.display()))?);
    let opts = Options { map: path, scene: setup::Scene::Backdrop, players: 2, seed: 7, army: 0, fog: false };
    let world = run_sim(&opts, &map, &blueprints, &pool, ticks.max(2), false)?;
    let mut frame = RenderFrame::default();
    world.write_render_frame(None, &mut frame);
    let status = crate::sim_thread::status_of(&world, 0);

    let scene = SceneDesc { map: map.clone(), blueprints, pool, team_colors: setup::TEAM_COLORS };
    let mut renderer = Renderer::new(Target::Headless { width: shot.width, height: shot.height }, scene).map_err(|e| e.to_string())?;
    let viewport = glam::Vec2::new(shot.width as f32, shot.height as f32);
    let mut camera = Camera::new(glam::Vec2::from(map.info().size_metres().to_f32()), viewport);

    let mut settings = crate::settings::Settings::default();
    let mut front = Front::new(Director::new(&map, true));
    front.show(screen, &settings);
    let audio = crate::audio::Audio::silent();
    let (mut overlay, mut memory) = (Overlay::default(), ui::Memory::default());
    overlay.set_image(menu::PREVIEW_SLOT, ui::preview::SIZE, ui::preview::SIZE, &ui::preview::render(&map));
    let input = ui::Input { cursor: cursor.map_or(glam::Vec2::splat(-100.0), glam::Vec2::from), ..Default::default() };
    for i in 0..90 {
        let (time, dt) = (20.0 + i as f32 / 30.0, 1.0 / 30.0);
        front.director.apply(&mut camera);
        camera.focus.z = renderer.ground_height(camera.focus.truncate()).max(map.info().water_level.to_f32());
        overlay.clear();
        memory.begin_frame();
        let mut ui = ui::Ui::new(&mut overlay, &input, &mut memory, &audio, viewport, settings.ui_scale, time, dt);
        let telemetry = menu::Telemetry { map_name: map.name(), tick: status.tick, units: status.units, camera: camera.focus.truncate(), altitude: camera.eye().z - camera.focus.z, preview: true };
        front.frame(&mut ui, &mut settings, &telemetry);
        memory.end_frame(&input);
        let input = FrameInput { camera: &camera, time, alpha: 1.0, sim: (i == 0).then_some(&frame), ghosts: &[], marks: &[], overlay: &overlay, build_grid: false };
        renderer.render(&input).map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(3));
    }
    let pixels = renderer.read_pixels().ok_or("no pixels from a headless target")?;
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
