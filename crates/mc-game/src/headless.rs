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
    /// Focus height in metres (`--camera`'s fifth value), where the free camera held
    /// it; without it the focus sits on the ground (or the sea).
    pub focus_z: Option<f32>,
    pub width: u32,
    pub height: u32,
    /// Match screenshots: select player 0's units whose blueprint key contains
    /// this (the first one, or all of them with a trailing `*`) instead of the commander.
    pub select: Option<String>,
    /// Match screenshots: where the pointer is, in pixels.
    pub cursor: Option<[f32; 2]>,
    /// Match screenshots: show the match paused (the frame round the screen).
    pub paused: bool,
    /// Match screenshots: stage a network match's moment (`net_shot`).
    pub net: Option<String>,
    /// Range screenshot: show the searchable subject catalog.
    pub unit_picker: bool,
    /// Range screenshot: show the map browser.
    pub range_maps: bool,
    /// Match screenshots: the construction panel open on its refit (upgrade) tab.
    pub refit_tab: bool,
    /// Match screenshots: the selected unit's DETAILS card open.
    pub details: bool,
    /// Match screenshots: the battle report over the match, open on this page
    /// (`PAGE[@M:SS]`, `ui::report`).
    pub report: Option<String>,
    /// Range screenshots: the range panel open on this tab (unit, stage, economy, sky, range).
    pub range_tab: Option<String>,
    /// Match screenshots: placing the structure with this blueprint key, at `cursor`.
    pub place: Option<String>,
    /// Match screenshots: the commander has structures planned and a way to walk, and shift is held.
    pub plans: bool,
    /// With `plans`: the order under `cursor` has been dragged to this pixel.
    pub drag: Option<[f32; 2]>,
    /// Play this many more ticks through the renderer before the frame that is
    /// kept, so what builds up over time (smoke, dust, track marks, a shell's
    /// flight) is in the picture; and how far into the last tick that frame is.
    pub follow: u32,
    pub alpha: f32,
    /// Draw the build grid, as while a structure is being placed.
    pub build_grid: bool,
}

/// Builds the world for `opts` and runs it for `ticks`; `chronicle` keeps the
/// record the battle report reads.
pub fn run_sim(
    opts: &Options,
    map: &Arc<MapFile>,
    blueprints: &Arc<Blueprints>,
    pool: &Arc<Pool>,
    ticks: u32,
    report: bool,
    mut chronicle: Option<&mut crate::chronicle::Chronicle>,
) -> Result<World, String> {
    let mut playback = opts
        .replay
        .as_deref()
        .map(crate::replay::Playback::open)
        .transpose()?;
    let config = match &playback {
        Some(p) => p.config.clone(),
        None => setup::match_config(opts, map),
    };
    let mut world =
        World::new(map, blueprints.clone(), pool.clone(), &config).map_err(|e| e.to_string())?;
    let survival = match &playback {
        Some(p) => p.survival.clone(),
        None if opts.scene == setup::Scene::Survival => {
            Some(crate::survival::scene_match(opts, map)?.1)
        }
        None => None,
    };
    if let Some(survival) = survival {
        world.begin_survival(survival).map_err(|e| e.to_string())?;
    }
    let opening = match playback {
        Some(_) => Vec::new(),
        None => setup::opening_commands(opts, map, blueprints, &config),
    };
    // A replay plays as far as it was recorded.
    let ticks = match &playback {
        Some(p) if p.ticks() < ticks => {
            log::warn!("the replay ends at tick {}; playing that far", p.ticks());
            p.ticks()
        }
        _ => ticks,
    };
    let mut worst = 0u64;
    let mut total = 0u64;
    let mut phase_totals: Vec<(&'static str, u64)> = Vec::new();
    // MERIDIAN_BENCH_WINDOW=N prints the timings of every N ticks, to see a long match age.
    let window: Option<u32> = std::env::var("MERIDIAN_BENCH_WINDOW")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&w| w > 0);
    let mut win: (u64, u64, Vec<(&'static str, u64)>) = (0, 0, Vec::new());
    let mut nav0 = world.nav.stats();
    let mut perf = crate::perf_out::enabled().then(|| {
        let mut r = mc_core::perf::Report::new(format!("{:?} on {}", opts.scene, map.name()));
        r.note("ticks", ticks.to_string());
        r
    });
    for t in 0..ticks {
        if let Some(p) = &mut playback {
            p.step(&mut world, t)?;
        } else {
            let commands = match t {
                0 => opening.clone(),
                1 => setup::scene_orders(opts, map, blueprints, &world),
                _ => setup::late_orders(opts, &world, t),
            };
            world.tick(&commands).map_err(|e| e.to_string())?;
        }
        if let Some(c) = chronicle.as_deref_mut() {
            c.record(&world);
        }
        if opts.scene == setup::Scene::Survival && report {
            crate::survival::log_tick(&world, t + 1);
        }
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
        if let Some(r) = &mut perf {
            r.add(&world.perf, "sim.tick");
        }
        worst = worst.max(world.timings.total_ns);
        total += world.timings.total_ns;
        if let Some(w) = window {
            win.0 += world.timings.total_ns;
            win.1 = win.1.max(world.timings.total_ns);
            for (i, (name, ns)) in world.timings.phases.iter().enumerate() {
                if win.2.len() <= i {
                    win.2.push((name, 0));
                }
                win.2[i].1 += ns;
            }
            if (t + 1) % w == 0 {
                let s = &world.state;
                let mut top = win.2.clone();
                top.sort_by_key(|p| std::cmp::Reverse(p.1));
                let top: Vec<String> = top
                    .iter()
                    .take(5)
                    .map(|(n, ns)| format!("{n} {:.1}", *ns as f64 / w as f64 / 1e6))
                    .collect();
                println!(
                    "t={:>5} ({:>4.1} min) mean {:>6.2} worst {:>7.2} ms | {} units {} proj {} wrecks | {}",
                    t + 1, (t + 1) as f32 / 600.0, win.0 as f64 / w as f64 / 1e6, win.1 as f64 / 1e6,
                    s.units.slots.live(), s.projectiles.len(), s.wrecks.slots.live(), top.join(", ")
                );
                let n = world.nav.stats();
                println!(
                    "        nav: {} builds ({} repairs, {} extends), {} late joins, {}k nodes, {}k tiles built, {} graphs, {} fields ({} held), {} tiles live",
                    n.builds_scheduled - nav0.builds_scheduled, n.repairs_scheduled - nav0.repairs_scheduled,
                    n.extends_scheduled - nav0.extends_scheduled, n.late_joins - nav0.late_joins,
                    (n.search_nodes - nav0.search_nodes) / 1000, (n.tiles_built - nav0.tiles_built) / 1000,
                    n.graphs_built - nav0.graphs_built, n.live_fields, n.held_fields, n.total_tiles
                );
                nav0 = n;
                win = (0, 0, Vec::new());
            }
        }
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
        for (i, ai) in s.ai.iter().enumerate() {
            if s.players[i].controller == mc_sim::tables::Controller::Ai {
                println!(
                    "  AI {i}: {:?}/{:?} {} built={} lost={} killed={}",
                    ai.config.difficulty,
                    ai.config.doctrine,
                    ai.summary(),
                    s.players[i].units_built,
                    s.players[i].units_lost,
                    s.players[i].units_killed
                );
                let pl = &s.players[i];
                println!("    mass={:.0}/{:.0} income={:.1} energy={:.0}/{:.0} income={:.1} efficiency={:.2}",
                    pl.mass.to_f32(), pl.mass_capacity.to_f32(), pl.mass_income.to_f32(),
                    pl.energy.to_f32(), pl.energy_capacity.to_f32(), pl.energy_income.to_f32(), pl.efficiency.to_f32());
                let mut roster = std::collections::BTreeMap::<&str, usize>::new();
                for row in s
                    .units
                    .slots
                    .iter()
                    .filter(|&r| s.units.owner[r] as usize == i)
                {
                    *roster.entry(world.bp(row).key.as_str()).or_default() += 1;
                }
                println!("    roster: {roster:?}");
            }
        }
        println!("  winner: {:?}", s.winner);
        let nav = world.nav.stats();
        println!(
            "  paths: {} live fields, {} tiles, {} late joins",
            nav.live_fields, nav.total_tiles, nav.late_joins
        );
    }
    if let Some(r) = &perf {
        crate::perf_out::save(r, "sim");
    }
    Ok(world)
}

/// `MERIDIAN_VISION=N` with `--observe`: the screenshot looks through slot N's eyes.
fn observed_vision() -> Option<u8> {
    std::env::var("MERIDIAN_VISION").ok()?.parse().ok()
}

/// Whose eyes a match screenshot is drawn through, as the sim thread would choose.
fn shot_eyes(opts: &Options) -> Option<u8> {
    match (opts.fog, opts.observe) {
        (false, _) => None,
        (true, true) => observed_vision(),
        (true, false) => Some(0),
    }
}

pub fn screenshot(
    opts: &Options,
    map: Arc<MapFile>,
    blueprints: Arc<Blueprints>,
    pool: Arc<Pool>,
    ticks: u32,
    shot: &Shot,
) -> Result<(), String> {
    let mut chronicle =
        crate::chronicle::Chronicle::new(glam::Vec2::from(map.info().size_metres().to_f32()));
    let mut world = run_sim(
        opts,
        &map,
        &blueprints,
        &pool,
        ticks,
        true,
        shot.report.is_some().then_some(&mut chronicle),
    )?;
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
    // MERIDIAN_ARM=n: every nuclear silo of player 0's holds n warheads (for aiming shots).
    if let Some(n) = std::env::var("MERIDIAN_ARM")
        .ok()
        .and_then(|v| v.trim().parse::<u8>().ok())
    {
        let u = &world.state.units;
        let silos: Vec<_> = u
            .slots
            .iter()
            .filter(|&r| {
                u.owner[r] == 0
                    && world
                        .bp(r)
                        .strategic
                        .as_ref()
                        .is_some_and(|s| s.kind == mc_data::strategic::StrategicKind::Nuke)
            })
            .map(|r| u.id(r))
            .collect();
        for id in silos {
            world.state.strategic.launchers.entry(id).or_default().stock = n;
        }
        world.tick(&[]).map_err(|e| e.to_string())?;
    }
    // MERIDIAN_NUKE=x,y[,ticks[,x2,y2...]]: player 0's first nuclear silo is given a
    // warhead a mark and launches at x,y (then x2,y2 and on, in turn, up to its stock);
    // `ticks` more ticks run before the shot (docs/NUKES.md).
    if let Ok(v) = std::env::var("MERIDIAN_NUKE") {
        let v: Vec<f32> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        let u = &world.state.units;
        let silo = u.slots.iter().find(|&r| {
            u.owner[r] == 0
                && world
                    .bp(r)
                    .strategic
                    .as_ref()
                    .is_some_and(|s| s.kind == mc_data::strategic::StrategicKind::Nuke)
        });
        if let (Some(row), [x, y, ..]) = (silo, v.as_slice()) {
            let id = world.state.units.id(row);
            let marks: Vec<(f32, f32)> = std::iter::once((*x, *y))
                .chain(
                    v.get(3..)
                        .unwrap_or(&[])
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|p| (p[0], p[1])),
                )
                .collect();
            world.state.strategic.launchers.entry(id).or_default().stock =
                marks.len().min(255) as u8;
            let commands: Vec<_> = marks
                .iter()
                .map(|&(x, y)| mc_sim::PlayerCommand {
                    player: 0,
                    command: mc_sim::Command::LaunchNuke {
                        units: vec![id],
                        pos: mc_core::FxVec2::new(
                            mc_core::Fx::from_f32(x),
                            mc_core::Fx::from_f32(y),
                        ),
                    },
                })
                .collect();
            world.tick(&commands).map_err(|e| e.to_string())?;
            for _ in 0..v.get(2).copied().unwrap_or(0.0) as u32 {
                world.tick(&[]).map_err(|e| e.to_string())?;
            }
        }
    }
    let mut frame = RenderFrame::default();
    world.write_render_frame(shot_eyes(opts), &mut frame);

    let scene = SceneDesc {
        map: map.clone(),
        blueprints: blueprints.clone(),
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
    // The map's own palette (`MERIDIAN_CLIMATE` overrides it).
    renderer.set_map_look(&setup::map_config(&map).look());
    // A recorded test range: the weather it showed at this tick.
    if let Some(sky) = opts
        .replay
        .as_deref()
        .and_then(|p| crate::replay::range_sky_at(p, world.tick_count()))
    {
        sky.show(&mut renderer, &map);
    }
    let size = map.info().size_metres().to_f32();
    let mut camera = Camera::new(
        glam::Vec2::from(size),
        glam::Vec2::new(shot.width as f32, shot.height as f32),
    );
    if let Some([x, y, distance, yaw]) = shot.camera {
        // Over the sea the camera looks at the surface, as in the game, not at the seabed.
        let ground = renderer.ground_height(glam::Vec2::new(x, y));
        let z = shot
            .focus_z
            .unwrap_or_else(|| ground.max(map.info().water_level.to_f32()));
        camera.focus = glam::Vec3::new(x, y, z);
        camera.distance = distance.clamp(mc_render::camera::MIN_DISTANCE, camera.max_distance());
        camera.yaw = yaw.to_radians();
        // `MERIDIAN_TILT` (radians): the extra tilt Alt-orbit gives, for low side shots.
        if let Some(tilt) = std::env::var("MERIDIAN_TILT")
            .ok()
            .and_then(|t| t.parse::<f32>().ok())
        {
            camera.tilt = tilt;
        }
        // `MERIDIAN_PITCH` (radians, negative looks up) and `MERIDIAN_FOV` (radians):
        // the free camera's own pitch and lens, the eye kept where the pitch puts it.
        let env = |k: &str| std::env::var(k).ok().and_then(|t| t.parse::<f32>().ok());
        if let Some(pitch) = env("MERIDIAN_PITCH") {
            camera.pitch_free = Some(pitch);
            // Looking up: lift the look so the eye stays above the ground.
            let eye = camera.eye();
            let floor = renderer.ground_height(eye.truncate()) + 15.0;
            if eye.z < floor {
                camera.focus.z += floor - eye.z;
            }
        }
        if let Some(fov) = env("MERIDIAN_FOV") {
            camera.fov = fov;
        }
    } else if matches!(
        opts.scene,
        setup::Scene::Formations | setup::Scene::Aircraft | setup::Scene::AircraftCrash
    ) {
        let at = (setup::range_pad(&map) + mc_core::FxVec2::from_ints(100, 30)).to_f32();
        camera.focus = glam::Vec3::new(at[0], at[1], renderer.ground_height(glam::Vec2::from(at)));
        camera.distance = 560.0;
        camera.yaw = -0.6;
        if matches!(
            opts.scene,
            setup::Scene::Aircraft | setup::Scene::AircraftCrash
        ) {
            let at = setup::range_pad(&map).to_f32();
            camera.focus = glam::Vec3::new(
                at[0],
                at[1],
                renderer.ground_height(glam::Vec2::from(at)) + 110.0,
            );
            camera.distance = 650.0;
            if opts.scene == setup::Scene::AircraftCrash {
                camera.distance = 240.0;
                camera.focus.z = renderer.ground_height(glam::Vec2::from(at)) + 70.0;
            }
        }
    }
    if shot.camera.is_none()
        && matches!(
            opts.scene,
            setup::Scene::AircraftDitch | setup::Scene::OffshoreMine
        )
    {
        let at = setup::ditch_point(&map).to_f32();
        let ground = renderer.ground_height(glam::Vec2::from(at));
        camera.focus = glam::Vec3::new(
            at[0],
            at[1],
            ground.max(map.info().water_level.to_f32()) + 4.0,
        );
        camera.distance = 130.0;
        camera.yaw = 0.5;
    }

    if shot.camera.is_none() && opts.scene == setup::Scene::Wreckage {
        // The yard from the south-west, the spacecraft's pieces beyond it.
        let (pad, sea) = (setup::range_pad(&map), setup::ditch_point(&map));
        log::info!(
            "wreckage yard at {:?}, ships at {:?}",
            pad.to_f32(),
            sea.to_f32()
        );
        let at = (pad + mc_core::FxVec2::from_ints(40, 90)).to_f32();
        camera.focus = glam::Vec3::new(at[0], at[1], renderer.ground_height(glam::Vec2::from(at)));
        camera.distance = 520.0;
        camera.yaw = -0.6;
    }

    // The same HUD the game draws, with `select` selected (the commander by default) so the panels show.
    let (mut overlay, mut memory) = (Overlay::default(), crate::ui::Memory::default());
    overlay.set_image(
        crate::hud::MINIMAP_SLOT,
        crate::ui::preview::SIZE,
        crate::ui::preview::SIZE,
        &crate::ui::preview::render(&map, &crate::setup::map_config(&map).look()),
    );
    let mut view = crate::game::View::new(
        0,
        setup::TEAM_COLORS,
        opts.scene != setup::Scene::Formations
            && std::env::var_os("MERIDIAN_NO_PROFILER").is_none(),
    );
    view.formation_panel = opts.scene == setup::Scene::Formations;
    view.observing = opts.observe;
    match mc_sim::placement::SiteMap::for_map(&map) {
        Ok(sites) => {
            let _ = view.sites.set(sites);
        }
        Err(e) => log::warn!("no placement sites for this map: {e}"),
    }
    view.perspective = observed_vision().filter(|_| opts.observe);
    view.status = crate::sim_thread::status_of(&world, world.timings.total_ns);
    view.status.owns_clock = true;
    view.index_of = frame
        .units
        .iter()
        .enumerate()
        .filter(|(_, u)| u.owner_flags & mc_sim::mirror::KIND_WRECK == 0)
        .map(|(i, u)| (u.unit_id, i))
        .collect();
    // `MERIDIAN_STORM_HERE=1` parks a raging storm over what the camera looks
    // at (the range panel's "Storm Overhead"), for rain and lightning shots.
    if std::env::var("MERIDIAN_STORM_HERE").is_ok() {
        renderer.park_storm(Some(camera.focus.truncate()));
    }
    view.frame = frame.clone();
    view.paused = shot.paused;
    let (link, net_notices) = match shot.net.as_deref() {
        Some(state) => {
            let (link, notices) = net_shot(state, &view)?;
            view.status.owns_clock = false;
            view.paused = link.paused_by.is_some();
            if state == "desync" {
                view.status.error = Some("The machines fell out of step at 6:41.".into());
            }
            (Some(link), notices)
        }
        None => (None, Vec::new()),
    };
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
    view.groups.set(1, &view.selection);
    // `MERIDIAN_GROUPS=KEY,KEY,...`: control groups 2, 3, ... hold player 0's
    // units whose blueprint key contains each KEY.
    if let Ok(keys) = std::env::var("MERIDIAN_GROUPS") {
        for (n, key) in (2..10).zip(keys.split(',')) {
            let members: Vec<u32> = frame
                .units
                .iter()
                .filter(|u| {
                    u.owner_flags & 0xFF == 0
                        && u.owner_flags & mc_sim::mirror::KIND_WRECK == 0
                        && world
                            .blueprints
                            .unit(mc_data::BlueprintId(u.blueprint as u16))
                            .key
                            .contains(key)
                })
                .map(|u| u.unit_id)
                .collect();
            view.groups.set(n, &members);
        }
    }
    // MERIDIAN_AIM: a launch being aimed, the pointer at `--cursor` (docs/NUKES.md).
    // MERIDIAN_AIM=ground: fire on the ground being aimed instead (a titan's strike preview).
    // MERIDIAN_AIM=reclaim: the Reclaim order being given, what is under `--cursor` ringed.
    // MERIDIAN_AIM=warp: the selection's warp order being aimed at `--cursor` (`warp_marks.rs`).
    // MERIDIAN_AIM=formation:DEG[:SHAPE]: a move held on the right button at `--cursor`,
    // the selection's formation showing, turned to face DEG degrees and shaped SHAPE
    // wheel notches wider (negative: longer) (`formation_drag.rs`).
    let aim = std::env::var("MERIDIAN_AIM").ok();
    let formation_aim = aim
        .as_deref()
        .and_then(|a| a.strip_prefix("formation"))
        .map(|rest| {
            let mut parts = rest.trim_start_matches(':').split(':');
            let deg = parts.next().and_then(|d| d.parse::<i32>().ok());
            if let Some(shape) = parts.next().and_then(|s| s.parse::<i8>().ok()) {
                view.formation_shape = crate::formation_drag::step_shape(shape, 0);
            }
            deg
        });
    if let Some(aim) = aim.as_ref().filter(|_| formation_aim.is_none()) {
        view.mode = crate::game::Mode::Target(match aim.as_str() {
            "ground" => crate::game::Targeting::Strike,
            "reclaim" => crate::game::Targeting::Reclaim,
            "warp" => crate::game::Targeting::Warp,
            _ => crate::game::Targeting::Nuke,
        });
    }
    if opts.scene == setup::Scene::Formations && shot.camera.is_none() && !view.selection.is_empty()
    {
        let mut center = glam::Vec3::ZERO;
        for id in &view.selection {
            center += glam::Vec3::from(frame.units[view.index_of[id]].pos);
        }
        camera.focus = center / view.selection.len() as f32;
        camera.distance = 420.0;
    }

    if opts.scene == setup::Scene::Aircraft
        && shot.camera.is_none()
        && shot.select.is_some()
        && !view.selection.is_empty()
    {
        let at = view.selection[0];
        camera.focus = glam::Vec3::from(frame.units[view.index_of[&at]].pos);
        camera.distance = 55.0;
    }

    // Range captures should inspect the subject, including its airborne height.
    // `--camera 0,0,DIST,YAW` frames it the same way from that distance and bearing;
    // a positive fifth figure raises the focus that many metres up the subject.
    let on_subject = shot.camera.is_none_or(|c| c[0] == 0.0 && c[1] == 0.0);
    if opts.scene == setup::Scene::Range && on_subject {
        if let Some(subject) = frame.units.iter().find(|u| {
            world
                .blueprints
                .unit(mc_data::BlueprintId(u.blueprint as u16))
                .key
                == opts.subject
                && u.owner_flags & (mc_sim::mirror::KIND_WRECK | mc_sim::mirror::KIND_GHOST) == 0
        }) {
            camera.focus = glam::Vec3::from(subject.pos);
            match shot.camera {
                Some([_, _, distance, yaw]) => {
                    camera.distance =
                        distance.clamp(mc_render::camera::MIN_DISTANCE, camera.max_distance());
                    camera.yaw = yaw.to_radians();
                    camera.focus.z += subject.radius;
                }
                None => {
                    camera.distance = (subject.radius * 5.0).max(65.0);
                    camera.yaw = -0.6;
                }
            }
        }
    }
    view.shift = shot.plans;
    // As the game asks: the whole side's queues, so every group's badge shows.
    world.write_orders(Some(0), &view.selection, Some(0), &mut view.status.queues);
    crate::sim_thread::sort_queues(&mut view.status.queues);
    world.write_plans(0, &mut view.status.plans);
    let mut marks: Vec<mc_render::Mark> = view
        .selection
        .iter()
        .filter_map(|id| view.index_of.get(id).copied())
        .map(|i| {
            let u = &frame.units[i];
            mc_render::Mark {
                unit_index: i as u32,
                kind: 0,
                work: crate::game::unit_bar_work(u, &view.status.queues),
                shield: crate::game::unit_bar_shield(u.unit_id, &frame.shields),
            }
        })
        .collect();
    crate::game::work::bar_marks(&view, |o| o == 0, &mut marks);
    if aim.as_deref() == Some("reclaim") {
        let target = shot.cursor.and_then(|c| {
            crate::pick::unit_at(
                &frame.units,
                &world.blueprints,
                &camera,
                glam::Vec2::from(c),
            )
        });
        if let Some(i) = target {
            marks.retain(|m| m.unit_index != i as u32);
            marks.push(mc_render::Mark {
                unit_index: i as u32,
                kind: mc_render::gpu_consts::mark::HOVER | mc_render::gpu_consts::mark::RECLAIM,
                work: -1.0,
                shield: -1.0,
            });
        }
    }
    let mut rings = crate::rings::Rings::new(&world.blueprints);
    let (mut ranges, mut ranges_drawn) = rings.collect(
        view.selection
            .iter()
            .filter_map(|id| view.index_of.get(id))
            .map(|&i| &frame.units[i]),
        1.0,
        true,
        &|p| renderer.ground_height(glam::Vec2::from(p)),
    );
    view.reaches = crate::rings::Rings::key(&ranges);
    let mut cover_network = false;
    // Titan strikes and storms under way, read off the world (`titan_marks::seed`).
    crate::titan_marks::seed(&mut view, &world, &world.blueprints.clone());
    let mut hud = crate::hud::Hud::default();
    hud.thumbs
        .bake(&mut overlay, &blueprints, setup::TEAM_COLORS[0]);
    if shot.net.as_deref() == Some("chat") {
        hud.net.stage_draft("on my way, hold the ridge", true);
    }
    if shot.unit_picker {
        hud.browse_range_subject();
    }
    if shot.range_maps {
        hud.browse_range_maps(&map);
    }
    hud.details_open = shot.details;
    // `MERIDIAN_ISSUE_NOTE=TEXT`: the report card's note, typed.
    if let Ok(note) = std::env::var("MERIDIAN_ISSUE_NOTE") {
        hud.issues.stage_note(&note);
    }
    // `MERIDIAN_FREE_CAMERA=guide|pill`: the panels folded away (Ctrl+Alt), with
    // the key guide open or folded to its pill.
    if let Ok(v) = std::env::var("MERIDIAN_FREE_CAMERA") {
        hud.free.snap(v != "pill");
    }
    if let Some(tab) = &shot.range_tab {
        hud.open_range_tab(tab);
    }
    if shot.refit_tab {
        hud.open_refit_tab();
    }
    let mut report = shot
        .report
        .as_deref()
        .map(|page| {
            let local = (!opts.observe).then_some(0);
            crate::ui::report::Report::staged(&chronicle, &world.blueprints, local, page)
        })
        .transpose()?;
    let audio = crate::audio::Audio::silent();
    let input = crate::ui::Input {
        cursor: shot
            .cursor
            .map_or(glam::Vec2::splat(-100.0), glam::Vec2::from),
        ..Default::default()
    };
    let (mut order_map, mut ghosts) = (crate::orders::OrderMap::default(), Vec::new());
    let started = Instant::now();
    // `--perf`: every followed frame's GPU scopes (with triangle and fragment
    // counts) and CPU spans, plus the sim tick that frame showed.
    let mut perf_frames = crate::perf_out::enabled().then(|| {
        renderer.set_gpu_stats(true);
        let mut r =
            mc_core::perf::Report::new(format!("{:?} on {} frames", opts.scene, map.name()));
        r.note("size", format!("{}x{}", shot.width, shot.height));
        r.note("camera", format!("{:?}", shot.camera));
        r.note("ticks", ticks.to_string());
        r.note("follow", shot.follow.to_string());
        r.note("device", renderer.device_name().to_string());
        r
    });
    // A few frames so streamed terrain tiles arrive (and hover glows settle) before the one we keep.
    // The unit browser and aircraft scenes have no animated UI to settle. Four warmup frames
    // still allow terrain uploads without spending forty frames on a large dome.
    let warmup_frames = if shot.unit_picker {
        2
    } else if matches!(
        opts.scene,
        setup::Scene::Aircraft | setup::Scene::AircraftCrash
    ) {
        4
    } else {
        40
    };
    let place = shot
        .place
        .as_deref()
        .map(|key| {
            world
                .blueprints
                .id_of(key)
                .ok_or(format!("no blueprint {key}"))
        })
        .transpose()?;
    if let Some(bp) = place {
        view.mode = crate::game::Mode::Place(bp);
    }
    // What the interface costs a frame (orders, marks, the HUD, each by its `ui.*` span)
    // over the warm-up frames: `--perf` writes it as FILE.ui.{json,txt}.
    let mut ui_report =
        mc_core::perf::Report::new(format!("interface, {:?} on {}", opts.scene, map.name()));
    for i in 0..warmup_frames {
        let ui_scope = mc_core::perf::Scope::begin();
        let ui_started = Instant::now();
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
        // The structure being placed, under the pointer, as the game shows it.
        if let Some((at, fit)) = place.and_then(|bp| {
            let ground = crate::orders::surface_under(&field, input.cursor)?;
            crate::orders::site_verdict(&field, bp, ground, None, &[])
        }) {
            let bp = place.expect("a site comes from a blueprint");
            let xy = at.to_f32();
            let p = [
                xy[0],
                xy[1],
                crate::orders::surface_height(&field, glam::Vec2::from(xy)),
            ];
            let heading = field.blueprints.unit(bp).build_heading().to_radians_f32();
            ghosts.push(mc_sim::mirror::UnitInstance {
                prev_pos: p,
                prev_heading: heading,
                pos: p,
                heading,
                blueprint: bp.0 as u32,
                owner_flags: view.local as u32 | mc_sim::mirror::KIND_GHOST,
                health: if fit.is_ok() { 1.0 } else { 0.0 },
                build: 1.0,
                turret_yaw: 0.0,
                radius: world.blueprints.unit(bp).radius.to_f32(),
                unit_id: u32::MAX,
                packed: 0,
                gait: [0.0; 3],
                upgrade: 0.0,
                arm_pitch: [0.0; 4],
                prev_turret_yaw: 0.0,
                weld: [0.0; 3],
                recoil: 0.0,
                prev_recoil: 0.0,
                weld_first: 0,
                weld_count: 0,
                deploy: 0.0,
                prev_deploy: 0.0,
                _pad2: [0.0; 2],
                refit_modules: 0,
                status: [0; 3],
                mount: [0.0; 4],
                spin_recoil: [0.0; 4],
                fx: [0.0; 4],
                drive_swing: [0.0; 2],
                _pad3: [0.0; 2],
            });
        }
        let outlined = order_map.ghosts(&field, &mut ghosts);
        order_map.draw(&mut ui, &field, 1.0);
        crate::game::work::draw_tags(&mut ui, &field, 1.0, |o| o == 0);
        let pointer = crate::orders::surface_under(&field, input.cursor);
        let site = place.and_then(|bp| {
            let (at, _) = crate::orders::site(&field, bp, pointer?, None)?;
            Some((bp, glam::Vec2::from(at.to_f32())))
        });
        crate::nuke_marks::draw(&mut ui, &field, 1.0, pointer, site);
        crate::cover_marks::draw(&mut ui, &field, site);
        crate::titan_marks::draw(&mut ui, &field, 1.0, pointer);
        crate::warp_marks::draw(&mut ui, &field, 1.0, pointer);
        crate::destruct_marks::draw(&mut ui, &field, 1.0);
        if let (Some(deg), Some(at)) = (formation_aim, pointer) {
            let command = mc_sim::Command::Move {
                units: view
                    .selection
                    .iter()
                    .map(|&id| mc_sim::Handle(id))
                    .collect(),
                target: mc_core::FxVec2::new(
                    mc_core::Fx::from_f32(at.x),
                    mc_core::Fx::from_f32(at.y),
                ),
                queue: false,
            };
            if let Ok(drag) = crate::formation_drag::FormationDrag::new(command, input.cursor) {
                let facing = deg.map(mc_core::Angle::from_degrees);
                drag.ghosts(&field, facing, &mut ghosts);
                drag.draw(&mut ui, &field, facing);
            }
        }
        crate::orders::ghost_footprints(&mut ui, &field, &ghosts[..outlined]);
        let build_grid = shot.build_grid || order_map.dragging_plan();
        let grid_focus = build_grid
            .then(|| {
                crate::orders::build_grid_focus(&field, input.cursor, order_map.plan_in_hand())
            })
            .flatten();
        let placing = place.and_then(|bp| {
            let ground = crate::orders::surface_under(&field, input.cursor)?;
            crate::orders::site(&field, bp, ground, None).map(|(at, _)| at)
        });
        // The staged news arrives once, with the first frame.
        let news = if i == 0 { net_notices.as_slice() } else { &[] };
        let scene = crate::hud::Scene {
            net: link.as_ref(),
            net_notices: news,
            view: &view,
            blueprints: &world.blueprints,
            map: &map,
            camera: &camera,
            gpu: &renderer.stats,
            hover: None,
            // MERIDIAN_RECLAIM=1: the reclaim survey, as if Control were held.
            show_reclaim: std::env::var("MERIDIAN_RECLAIM").is_ok_and(|v| v == "1"),
            placing,
        };
        // The report, like the in-match menu, has the pointer to itself.
        ui.interactive = report.is_none();
        hud.draw(&mut ui, &scene, 0.016);
        crate::hud::cursor_hint(&mut ui, &view, &world.blueprints, &[]);
        crate::warp_marks::cursor_card(&mut ui, &field, pointer);
        if let Some(report) = &mut report {
            ui.interactive = true;
            let ctx = crate::ui::report::Ctx {
                blueprints: &world.blueprints,
                colors: &view.colors,
                local: (!opts.observe).then_some(0),
                map_name: map.name(),
                thumbs: &hud.thumbs,
                chart: crate::hud::MINIMAP_SLOT,
                surrender: false,
            };
            report.draw(&mut ui, &ctx, 1.0);
        }
        memory.end_frame(&input);
        // A weapon card under `--cursor` lights its ring on the ground, as in a match.
        let focus = hud.reach_focus.take();
        if focus != rings.focus {
            rings.focus = focus;
            (ranges, ranges_drawn) = rings.collect(
                view.selection
                    .iter()
                    .filter_map(|id| view.index_of.get(id))
                    .map(|&i| &frame.units[i]),
                1.0,
                true,
                &|p| renderer.ground_height(glam::Vec2::from(p)),
            );
            cover_network = false;
        }
        // Placing a post: our posts' rings and the site's, merged (no ghost draws its ring here).
        if let (false, Some((bp, at))) = (cover_network, site) {
            let network = crate::rings::cover_network(
                &world.blueprints,
                bp,
                Some(at.into()),
                view.local,
                frame.units.iter(),
            );
            crate::rings::prepend(&mut ranges, &mut ranges_drawn, network);
            cover_network = true;
        }
        if let Some((centre, radius, lots)) = grid_focus {
            renderer.set_build_grid(centre, radius, &lots);
        }
        let mut f = ui_scope.end();
        f.push("ui", 1, Some(ui_started.elapsed().as_nanos() as u64));
        ui_report.add(&f, "ui");
        renderer.set_ore_highlight(
            if place.is_some_and(|bp| world.blueprints.unit(bp).mine.is_some()) {
                1.0
            } else {
                0.0
            },
        );
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
            build_grid,
        };
        renderer.render(&input).map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    // The followed ticks, at the pace of a live match: two frames a tick.
    let mut time = 10.0 + warmup_frames as f32 * 0.016;
    // MERIDIAN_GPU_MEDIAN=1: each pass's median over the followed frames, which
    // a GPU shared with other work cannot skew the way one frame's time can.
    let median = std::env::var("MERIDIAN_GPU_MEDIAN").is_ok_and(|v| v == "1");
    let mut pass_ms: Vec<(&str, Vec<f32>)> = Vec::new();
    for k in 0..shot.follow {
        // With `--ticks 1` the scene's orders are still owed: given here, what they
        // set off (a self-destruct, say) happens where the renderer sees it.
        let owed = if k == 0 && ticks == 1 {
            setup::scene_orders(opts, &map, &world.blueprints.clone(), &world)
        } else {
            Vec::new()
        };
        world.tick(&owed).map_err(|e| e.to_string())?;
        let mirror = Instant::now();
        world.write_render_frame(shot_eyes(opts), &mut frame);
        let mirror_ns = mirror.elapsed().as_nanos() as u64;
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
                build_grid: shot.build_grid,
            };
            let scope = mc_core::perf::Scope::begin();
            let cpu = Instant::now();
            renderer.render(&input).map_err(|e| e.to_string())?;
            if let Some(r) = &mut perf_frames {
                let mut f = scope.end();
                f.push("cpu.render", 1, Some(cpu.elapsed().as_nanos() as u64));
                if i == 0 {
                    f.merge(&world.perf);
                    // The sim-to-render mirror, built once a tick (on the sim thread in a match).
                    f.push("cpu.mirror", 1, Some(mirror_ns));
                }
                // Scopes of the frame before this one: queries read after its fence.
                mc_render::gpu_scopes_to_perf(&renderer.stats.gpu_scopes, &mut f);
                f.push(
                    "draw.dynamic_entities",
                    renderer.stats.dynamic_entities as u64,
                    None,
                );
                f.push(
                    "draw.static_entities",
                    renderer.stats.static_entities as u64,
                    None,
                );
                f.push(
                    "draw.terrain_nodes",
                    renderer.stats.terrain_nodes as u64,
                    None,
                );
                r.add(&f, "gpu");
            }
            if median {
                for &(name, ms) in &renderer.stats.gpu_passes {
                    match pass_ms.iter_mut().find(|p| p.0 == name) {
                        Some(p) => p.1.push(ms),
                        None => pass_ms.push((name, vec![ms])),
                    }
                }
            }
        }
        time += 1.0 / mc_core::TICKS_PER_SECOND as f32;
    }
    if let Some(r) = &perf_frames {
        crate::perf_out::save(&ui_report, "ui");
        crate::perf_out::save(r, "frames");
    }
    for (name, mut ms) in pass_ms {
        ms.sort_by(f32::total_cmp);
        println!(
            "{name} pass median {:.2} ms over {} frames",
            ms[ms.len() / 2],
            ms.len()
        );
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
            world.blueprints.unit(*b).has(category) && world.blueprints.unit(*b).mine.is_none()
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
    let world = run_sim(&opts, &map, &blueprints, &pool, ticks.max(2), false, None)?;
    let mut frame = RenderFrame::default();
    world.write_render_frame(None, &mut frame);
    let status = crate::sim_thread::status_of(&world, 0);

    let blueprint_hash = blueprints.content_hash();
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
    settings
        .apply_graphics(&mut renderer)
        .map_err(|e| e.to_string())?;
    // `MERIDIAN_SKIRMISH_MAP=stem`: the set-up screen opens on that map.
    if let Ok(stem) = std::env::var("MERIDIAN_SKIRMISH_MAP") {
        settings.skirmish_map = stem;
    }
    // `MERIDIAN_MP_SERVER=host:port`: the multiplayer screen connects to that server.
    if let Ok(addr) = std::env::var("MERIDIAN_MP_SERVER") {
        settings.server = addr;
    }
    // `MERIDIAN_SURVIVAL=...` (see `survival::env_rules`): the survival set-up opens on those rules.
    settings.survival_rules = crate::survival::env_rules();
    let mut front = Front::new(
        Director::new(&map, true),
        blueprint_hash,
        ui::lineup::ReadAhead::start(),
    );
    front.show(screen, &settings);
    let audio = crate::audio::Audio::silent();
    let (mut overlay, mut memory) = (Overlay::default(), ui::Memory::default());
    overlay.set_image(
        menu::PREVIEW_SLOT,
        ui::preview::SIZE,
        ui::preview::SIZE,
        &ui::preview::render(&map, &crate::setup::map_config(&map).look()),
    );
    let still = ui::Input {
        cursor: cursor.map_or(glam::Vec2::splat(-100.0), glam::Vec2::from),
        ..Default::default()
    };
    // `MERIDIAN_CLICK=1`: click at `--cursor` part way through (opens a dropdown's list).
    let click = std::env::var("MERIDIAN_CLICK").is_ok();
    // `MERIDIAN_CLICKS=x,y;x,y`: click each point in turn, ten frames apart.
    let clicks: Vec<glam::Vec2> = std::env::var("MERIDIAN_CLICKS")
        .unwrap_or_default()
        .split(';')
        .filter_map(|p| {
            let v: Vec<f32> = p.split(',').filter_map(|n| n.trim().parse().ok()).collect();
            (v.len() == 2).then(|| glam::Vec2::new(v[0], v[1]))
        })
        .collect();
    // `MERIDIAN_UI_QUICK=1`: the interface runs every frame, the 3D scene is drawn
    // only on the first and last few (for a loaded CPU rasteriser).
    let quick = std::env::var("MERIDIAN_UI_QUICK").is_ok();
    for i in 0..90 {
        let mut input = still.clone();
        if click && i == 40 {
            input.down = true;
            input.pressed = true;
        } else if click && i == 41 {
            input.released = true;
        }
        let k = (i as usize).wrapping_sub(10);
        if let Some(at) = clicks.get(k / 10) {
            input.cursor = *at;
            match k % 10 {
                1 => (input.down, input.pressed) = (true, true),
                2 => input.released = true,
                _ => {}
            }
        }
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
        let outcome = front.frame(&mut ui, &mut settings, &telemetry);
        if outcome.display_changed {
            settings
                .apply_graphics(&mut renderer)
                .map_err(|e| e.to_string())?;
        }
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
        if !quick || i == 0 || i >= 88 {
            renderer.render(&input).map_err(|e| e.to_string())?;
        }
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

/// A network match's link and news for a screenshot (`--net-shot STATE`): `play`
/// (link readout, lagging and dropped seats, chat), `chat` (the chat line open),
/// `paused`, `waiting` (loading), `rejoin` (this machine reconnecting), `desync`.
fn net_shot(
    state: &str,
    view: &crate::game::View,
) -> Result<(crate::netplay::NetLink, Vec<crate::netplay::NetNotice>), String> {
    use crate::netplay::{DesyncReport, NetLink, NetNotice, Rejoining};
    use mc_net::{Link, PeerStat};
    let seats = view.status.players.len() as u8;
    let stats = (0..seats)
        .map(|slot| PeerStat {
            slot: mc_core::PlayerId(slot),
            rtt_ms: 38 + 27 * slot as u16,
            link: match (state, slot) {
                ("play", 2) => Link::Lagging,
                ("play", 3) => Link::Dropped,
                ("waiting", 1) | ("waiting", 3) => Link::Loading,
                _ => Link::Connected,
            },
        })
        .collect();
    let chat = |from: u8, text: &str, private: bool| NetNotice::Chat {
        from: Some(from),
        name: view
            .status
            .players
            .get(from as usize)
            .map_or_else(String::new, |p| p.name.clone()),
        private,
        text: text.into(),
    };
    let mut link = NetLink {
        stats,
        input_delay: 2,
        ..NetLink::default()
    };
    let mut notices = Vec::new();
    match state {
        "play" | "chat" => {
            notices.push(chat(1, "gl hf", false));
            notices.push(chat(0, "moving the tanks up the east road", true));
            notices.push(NetNotice::Chat {
                from: None,
                name: "Spectre".into(),
                private: false,
                text: "that artillery line is brutal".into(),
            });
            notices.push(chat(1, "bring it", false));
            if state == "play" {
                notices.push(NetNotice::Dropped(3.min(seats.saturating_sub(1))));
            }
        }
        "paused" => {
            link.paused_by = Some(Some(1.min(seats.saturating_sub(1))));
            notices.push(NetNotice::Paused(Some(1.min(seats.saturating_sub(1)))));
        }
        "waiting" => link.loading = Some(0b0101),
        "rejoin" => {
            link.rejoining = Some(Rejoining {
                attempts: 3,
                since: std::time::Instant::now() - std::time::Duration::from_secs(12),
            })
        }
        "desync" => {
            let good = 0x6d1f_42a9_0e33_c871;
            link.desync = Some(DesyncReport {
                tick: 4019,
                hashes: (0..seats)
                    .map(|s| (s, if s == 1 { good ^ 0x5a5a } else { good }))
                    .collect(),
                local: Some(0),
                ours: Some([1; mc_sim::state_hash::SECTION_COUNT]),
                theirs: vec![(1, {
                    let mut v = vec![1u64; mc_sim::state_hash::SECTION_COUNT];
                    v[2] = 2;
                    v[12] = 3;
                    v
                })],
                dump: crate::settings::config_dir()
                    .map(|d| d.join("desync").join("desync-1790454313-t4019-p0.mcsnap")),
            });
        }
        other => {
            return Err(format!(
                "--net-shot takes play, chat, paused, waiting, rejoin or desync, not {other}"
            ))
        }
    }
    Ok((link, notices))
}
