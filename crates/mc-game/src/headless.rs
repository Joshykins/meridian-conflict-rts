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
    /// Range screenshot: show the searchable subject catalog.
    pub unit_picker: bool,
    /// Match screenshots: the construction panel open on its refit (upgrade) tab.
    pub refit_tab: bool,
    /// Match screenshots: the selected unit's DETAILS card open.
    pub details: bool,
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
    if opts.scene == setup::Scene::Survival {
        let (_, survival) = crate::survival::scene_match(opts, map)?;
        world.begin_survival(survival).map_err(|e| e.to_string())?;
    }
    let opening = setup::opening_commands(opts, map, blueprints, &config);
    let mut worst = 0u64;
    let mut total = 0u64;
    let mut phase_totals: Vec<(&'static str, u64)> = Vec::new();
    // MERIDIAN_BENCH_WINDOW=N prints the timings of every N ticks, to see a long match age.
    let window: Option<u32> = std::env::var("MERIDIAN_BENCH_WINDOW").ok().and_then(|v| v.parse().ok()).filter(|&w| w > 0);
    let mut win: (u64, u64, Vec<(&'static str, u64)>) = (0, 0, Vec::new());
    let mut nav0 = world.nav.stats();
    for t in 0..ticks {
        let commands = match t {
            0 => opening.clone(),
            1 => setup::scene_orders(opts, map, blueprints, &world),
            _ => Vec::new(),
        };
        world.tick(&commands).map_err(|e| e.to_string())?;
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
                let top: Vec<String> = top.iter().take(5).map(|(n, ns)| format!("{n} {:.1}", *ns as f64 / w as f64 / 1e6)).collect();
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
                println!("  AI {i}: {:?}/{:?} {} built={} lost={} killed={}",
                    ai.config.difficulty, ai.config.doctrine, ai.summary(),
                    s.players[i].units_built, s.players[i].units_lost, s.players[i].units_killed);
                let pl = &s.players[i];
                println!("    mass={:.0}/{:.0} income={:.1} energy={:.0}/{:.0} income={:.1} efficiency={:.2}",
                    pl.mass.to_f32(), pl.mass_capacity.to_f32(), pl.mass_income.to_f32(),
                    pl.energy.to_f32(), pl.energy_capacity.to_f32(), pl.energy_income.to_f32(), pl.efficiency.to_f32());
                let mut roster = std::collections::BTreeMap::<&str,usize>::new();
                for row in s.units.slots.iter().filter(|&r| s.units.owner[r] as usize == i) {
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
    let size = map.info().size_metres().to_f32();
    let mut camera = Camera::new(
        glam::Vec2::from(size),
        glam::Vec2::new(shot.width as f32, shot.height as f32),
    );
    if let Some([x, y, distance, yaw]) = shot.camera {
        // Over the sea the camera looks at the surface, as in the game, not at the seabed.
        let ground = renderer.ground_height(glam::Vec2::new(x, y));
        camera.focus = glam::Vec3::new(x, y, ground.max(map.info().water_level.to_f32()));
        camera.distance = distance.clamp(mc_render::camera::MIN_DISTANCE, camera.max_distance());
        camera.yaw = yaw.to_radians();
        // `MERIDIAN_TILT` (radians): the extra tilt Alt-orbit gives, for low side shots.
        if let Some(tilt) = std::env::var("MERIDIAN_TILT").ok().and_then(|t| t.parse::<f32>().ok()) {
            camera.tilt = tilt;
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
    if shot.camera.is_none() && matches!(opts.scene, setup::Scene::AircraftDitch | setup::Scene::OffshoreMine) {
        let at = setup::ditch_point(&map).to_f32();
        let ground = renderer.ground_height(glam::Vec2::from(at));
        camera.focus = glam::Vec3::new(at[0], at[1], ground.max(map.info().water_level.to_f32()) + 4.0);
        camera.distance = 130.0;
        camera.yaw = 0.5;
    }

    // The same HUD the game draws, with `select` selected (the commander by default) so the panels show.
    let (mut overlay, mut memory) = (Overlay::default(), crate::ui::Memory::default());
    overlay.set_image(
        crate::hud::MINIMAP_SLOT,
        crate::ui::preview::SIZE,
        crate::ui::preview::SIZE,
        &crate::ui::preview::render(&map),
    );
    let mut view = crate::game::View::new(
        0,
        setup::TEAM_COLORS,
        opts.scene != setup::Scene::Formations,
    );
    view.formation_panel = opts.scene == setup::Scene::Formations;
    view.observing = opts.observe;
    if let Some(sites) = mc_sim::placement::SiteMap::for_map(&map) {
        let _ = view.sites.set(sites);
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
        if let Some(subject) = frame.units.iter().find(|u|
            world.blueprints.unit(mc_data::BlueprintId(u.blueprint as u16)).key == opts.subject
            && u.owner_flags & (mc_sim::mirror::KIND_WRECK | mc_sim::mirror::KIND_GHOST) == 0)
        {
            camera.focus = glam::Vec3::from(subject.pos);
            match shot.camera {
                Some([_, _, distance, yaw]) => {
                    camera.distance = distance.clamp(mc_render::camera::MIN_DISTANCE, camera.max_distance());
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
                shield: crate::game::unit_bar_shield(u.unit_id, &frame.shields),
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
    hud.thumbs.bake(&mut overlay, &blueprints, setup::TEAM_COLORS[0]);
    if shot.unit_picker {
        hud.browse_range_subject();
    }
    hud.details_open = shot.details;
    if let Some(tab) = &shot.range_tab {
        hud.open_range_tab(tab);
    }
    if shot.refit_tab {
        hud.open_refit_tab();
    }
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
        .map(|key| world.blueprints.id_of(key).ok_or(format!("no blueprint {key}")))
        .transpose()?;
    if let Some(bp) = place {
        view.mode = crate::game::Mode::Place(bp);
    }
    for i in 0..warmup_frames {
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
            let p = [xy[0], xy[1], crate::orders::surface_height(&field, glam::Vec2::from(xy))];
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
                _pad: 0,
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
                _pad3: [0; 3],
                mount: [0.0; 4],
                spin_recoil: [0.0; 4],
            });
        }
        let outlined = order_map.ghosts(&field, &mut ghosts);
        order_map.draw(&mut ui, &field, 1.0);
        crate::orders::ghost_footprints(&mut ui, &field, &ghosts[..outlined]);
        let build_grid = shot.build_grid || order_map.dragging_plan();
        let grid_focus = build_grid
            .then(|| crate::orders::build_grid_focus(&field, input.cursor, order_map.plan_in_hand()))
            .flatten();
        let placing = place.and_then(|bp| {
            let ground = crate::orders::surface_under(&field, input.cursor)?;
            crate::orders::site(&field, bp, ground, None).map(|(at, _)| at)
        });
        let scene = crate::hud::Scene {
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
        hud.draw(&mut ui, &scene, 0.016);
        memory.end_frame(&input);
        if let Some((centre, radius, lots)) = grid_focus {
            renderer.set_build_grid(centre, radius, &lots);
        }
        renderer.set_ore_highlight(if place.is_some_and(|bp| world.blueprints.unit(bp).mine.is_some()) { 1.0 } else { 0.0 });
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
    // MERIDIAN_GPU_MEDIAN=1: the scene pass's median over the followed frames, which
    // a GPU shared with other work cannot skew the way one frame's time can.
    let median = std::env::var("MERIDIAN_GPU_MEDIAN").is_ok_and(|v| v == "1");
    let mut scene_ms: Vec<f32> = Vec::new();
    for k in 0..shot.follow {
        // With `--ticks 1` the scene's orders are still owed: given here, what they
        // set off (a self-destruct, say) happens where the renderer sees it.
        let owed = if k == 0 && ticks == 1 {
            setup::scene_orders(opts, &map, &world.blueprints.clone(), &world)
        } else {
            Vec::new()
        };
        world.tick(&owed).map_err(|e| e.to_string())?;
        world.write_render_frame(shot_eyes(opts), &mut frame);
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
            renderer.render(&input).map_err(|e| e.to_string())?;
            if median {
                scene_ms.extend(renderer.stats.gpu_passes.iter().filter(|p| p.0 == "scene").map(|p| p.1));
            }
        }
        time += 1.0 / mc_core::TICKS_PER_SECOND as f32;
    }
    if !scene_ms.is_empty() {
        scene_ms.sort_by(f32::total_cmp);
        println!("scene pass median {:.2} ms over {} frames", scene_ms[scene_ms.len() / 2], scene_ms.len());
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
