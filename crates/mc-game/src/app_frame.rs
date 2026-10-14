use super::*;
impl App {
    pub(super) fn run_frame(
        &mut self,
        event_loop: &ActiveEventLoop,
        now: Instant,
        dt: f32,
        time: f32,
    ) -> Result<(), String> {
        let viewport = self.viewport();
        self.memory.begin_frame();
        self.reveal = (self.reveal + dt / 0.6).min(1.0);
        let veil = (1.0 - self.reveal).powi(2);
        // Nothing under the loading screen hears a click or a key.
        if self.curtain.as_ref().is_some_and(|c| c.covering()) {
            self.input.end_frame();
        }
        self.start_job()?;

        let mut next: Option<(Pending, &str, String)> = None;
        let mut quit = false;
        let mut settings_changed = (false, false);
        let mut ready: Option<(Pending, loading::Ready)> = None;
        match &mut self.stage {
            Stage::Loading {
                pending,
                job,
                built,
                eased,
                camera,
            } => {
                // Its first frame here the old stage's last picture is still on
                // screen: a stall to rebuild at a lower scale goes unseen, and
                // this frame draws nothing so the screen's arrival starts after it.
                let mut drawn = true;
                if let (Some(r), false) = (&mut self.renderer, *eased) {
                    *eased = true;
                    drawn = false;
                    r.set_render_quality(0.5, mc_render::Antialiasing::Off)
                        .map_err(|e| format!("could not change the render scale: {e}"))?;
                }
                if let Some(job) = job {
                    if let Some(chart) = job.take_chart() {
                        self.overlay.set_image(
                            crate::hud::MINIMAP_SLOT,
                            ui::preview::SIZE,
                            ui::preview::SIZE,
                            &chart,
                        );
                        if let Some(c) = &mut self.curtain {
                            c.chart_ready();
                        }
                    }
                    let (step, done) = job.progress();
                    if let Some(c) = &mut self.curtain {
                        c.building(step, done);
                    }
                    if built.is_none() {
                        if let Some(result) = job.finished() {
                            *built = Some(Box::new(result?));
                            if let Some(c) = &mut self.curtain {
                                c.built();
                            }
                        }
                    }
                }
                // The window changes hands while the loading screen is at rest.
                // With nothing to draw it (the splash could not start) the
                // curtain never moves, so it is not waited for.
                let unseen = self.renderer.is_none() && self.splash.is_none();
                if built.is_some() && (unseen || self.curtain.as_ref().is_none_or(|c| c.still())) {
                    let pending = pending.take().ok_or("nothing was loading")?;
                    ready = built.take().map(|b| (pending, *b));
                }
                // The outgoing renderer draws the loading screen while the new
                // one is built; the first load of a run has the splash instead.
                if drawn && (self.renderer.is_some() || self.splash.is_some()) {
                    self.overlay.clear();
                    let mut ui = Ui::new(
                        &mut self.overlay,
                        &self.input,
                        &mut self.memory,
                        &self.audio,
                        viewport,
                        self.settings.ui_scale,
                        time,
                        dt,
                    );
                    match &mut self.curtain {
                        Some(c) => c.draw(&mut ui),
                        None => {
                            ui.fill(ui::Rect::new(0.0, 0.0, ui.size.x, ui.size.y), ui::ink(1.0))
                        }
                    }
                    if let Some(r) = &mut self.renderer {
                        let camera = match camera {
                            Some(c) => {
                                c.viewport = viewport;
                                c.clone()
                            }
                            None => Camera::new(Vec2::splat(1000.0), viewport),
                        };
                        r.render(&FrameInput {
                            camera: &camera,
                            time,
                            alpha: 1.0,
                            sim: None,
                            latest: None,
                            ghosts: &[],
                            marks: &[],
                            ranges: &[],
                            ranges_drawn: 0,
                            overlay: &self.overlay,
                            build_grid: false,
                            icons: true,
                        })
                        .map_err(|e| e.to_string())?;
                    } else if let Some(s) = &mut self.splash {
                        s.render(&self.overlay)
                            .map_err(|e| format!("the splash failed: {e}"))?;
                    }
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(4));
                }
            }
            Stage::Front(stage) => {
                let FrontStage {
                    front,
                    map,
                    sim,
                    serial,
                    published_at,
                    interp_span,
                    frame,
                    status,
                    camera,
                    quiet,
                } = &mut **stage;
                let fresh = match sim.pull(serial, frame, status) {
                    Some(at) => {
                        if at != *published_at {
                            let gap = at.duration_since(*published_at).as_secs_f32();
                            *interp_span = gap.clamp(TICK_SECONDS, TICK_SECONDS * 8.0);
                            *published_at = at;
                        }
                        true
                    }
                    None => false,
                };
                // Restage the battle when the director cuts back to it, or when it has burnt out.
                *quiet = if frame.projectiles.is_empty() && status.tick > 300 {
                    *quiet + dt
                } else {
                    0.0
                };
                if *quiet > 12.0 {
                    front.director.restart();
                    *quiet = 0.0;
                }
                if front.director.restage {
                    *sim = FrontStage::stage_battle(&self.args, map)?;
                    *serial = 0;
                }
                let renderer = self.renderer.as_mut().ok_or("no renderer")?;
                front.director.apply(camera);
                camera.focus.z = renderer
                    .ground_height(camera.focus.truncate())
                    .max(map.info().water_level.to_f32());

                self.overlay.clear();
                let mut ui = Ui::new(
                    &mut self.overlay,
                    &self.input,
                    &mut self.memory,
                    &self.audio,
                    viewport,
                    self.settings.ui_scale,
                    time,
                    dt,
                );
                let telemetry = Telemetry {
                    map_name: map.name(),
                    tick: status.tick,
                    units: status.units,
                    camera: camera.focus.truncate(),
                    altitude: camera.eye().z - camera.focus.z,
                    preview: true,
                };
                let out = front.frame(&mut ui, &mut self.settings, &telemetry);
                ui.fill(
                    ui::Rect::new(0.0, 0.0, ui.size.x, ui.size.y),
                    ui::rgb(0x000000, veil),
                );
                if let Some(c) = &mut self.curtain {
                    c.draw(&mut ui);
                }
                settings_changed = (out.settings_changed, out.display_changed);
                match out.event {
                    Some(FrontEvent::Launch(request)) => {
                        let detail = match &request.survival {
                            Some(s) if s.rules.rounds == 0 => {
                                "Survival   \u{b7}   Endless".to_owned()
                            }
                            Some(s) => format!("Survival   \u{b7}   {} rounds", s.rules.rounds),
                            None => format!(
                                "Skirmish   \u{b7}   {} commanders",
                                request.config.players.len()
                            ),
                        };
                        next = Some((
                            Pending::Match(Box::new(local_start(
                                *request,
                                &self.args.blueprints,
                                true,
                            )?)),
                            "Deploying",
                            detail,
                        ));
                    }
                    Some(FrontEvent::LaunchNet(launch)) => {
                        let options = &launch.options;
                        let detail = match &options.survival {
                            Some(s) => format!(
                                "{}   \u{b7}   Co-op Survival   \u{b7}   {}",
                                options.map,
                                crate::ui::survival::rules::rounds_label(&s.rules)
                            ),
                            None => format!(
                                "{}   \u{b7}   {}",
                                options.map,
                                crate::ui::teams::matchup(
                                    &options
                                        .config
                                        .players
                                        .iter()
                                        .map(|p| p.team)
                                        .collect::<Vec<_>>()
                                )
                            ),
                        };
                        next = Some((
                            Pending::Match(Box::new(net_start(*launch))),
                            "Deploying",
                            detail,
                        ));
                    }
                    Some(FrontEvent::Campaign(resume)) => {
                        match crate::campaign::start(&self.args.blueprints, resume) {
                            Ok(start) => {
                                next = Some((
                                    Pending::Match(Box::new(start)),
                                    "Deploying",
                                    "Vela • The Contract".into(),
                                ))
                            }
                            Err(error) => front.complain(error),
                        }
                    }
                    Some(FrontEvent::Editor) => {
                        let path = setup::find_map(Some("vela"))?;
                        let editor = crate::editor::Editor::open(
                            &path,
                            self.args.blueprints.clone(),
                            self.args.pool.clone(),
                            viewport,
                        )?;
                        next = Some((
                            Pending::Editor(Box::new(editor)),
                            "Map Editor",
                            "Vela: The Contract".into(),
                        ));
                    }
                    Some(FrontEvent::Range) => {
                        // The map last picked on the range, while it is still there.
                        let path = Some(self.settings.range_map.as_str())
                            .filter(|stem| !stem.is_empty())
                            .and_then(|stem| setup::find_map(Some(stem)).ok())
                            .map_or_else(|| setup::find_map(None), Ok)?;
                        let map = Arc::new(
                            MapFile::open(&path).map_err(|e| format!("{}: {e}", path.display()))?,
                        );
                        let start = range_start(
                            &map,
                            &self.args.blueprints,
                            crate::range::DEFAULT_SUBJECT,
                            None,
                        )?;
                        next = Some((
                            Pending::Match(Box::new(start)),
                            "Test Range",
                            "Weapons and units on the pad".to_owned(),
                        ));
                    }
                    Some(FrontEvent::Replay(path, at)) => {
                        let playback = crate::replay::Playback::open(&path)?;
                        let map_path = crate::replay::find_map(playback.start())?;
                        let map = Arc::new(
                            MapFile::open(&map_path)
                                .map_err(|e| format!("{}: {e}", map_path.display()))?,
                        );
                        let detail = format!(
                            "Replay   \u{b7}   {}",
                            crate::hud::replay_clock(playback.ticks())
                        );
                        next = Some((
                            Pending::Match(Box::new(crate::replay::game_start(playback, map, at))),
                            "Replay",
                            detail,
                        ));
                    }
                    Some(FrontEvent::Quit) => quit = true,
                    None => {}
                }
                let alpha = ((now - *published_at).as_secs_f32() / *interp_span).clamp(0.0, 1.0);
                renderer
                    .render(&FrameInput {
                        camera,
                        time,
                        alpha,
                        sim: fresh.then_some(&*frame),
                        latest: Some(&*frame),
                        ghosts: &[],
                        marks: &[],
                        ranges: &[],
                        ranges_drawn: 0,
                        overlay: &self.overlay,
                        build_grid: false,
                        icons: true,
                    })
                    .map_err(|e| format!("rendering failed: {e}"))?;
            }
            Stage::Editor(editor) => {
                let renderer = self.renderer.as_mut().ok_or("no renderer")?;
                let ctx = FrameCtx {
                    renderer,
                    overlay: &mut self.overlay,
                    input: &self.input,
                    memory: &mut self.memory,
                    audio: &self.audio,
                    settings: &mut self.settings,
                    settings_changed: &mut settings_changed.0,
                    display_changed: &mut settings_changed.1,
                    now,
                    dt,
                    time,
                    cover: self.curtain.as_mut(),
                };
                match editor.frame(ctx)? {
                    Some(crate::editor::Event::Open(editor)) => {
                        next = Some((
                            Pending::Editor(editor),
                            "Map Editor",
                            "Opening authored map".into(),
                        ));
                    }
                    Some(crate::editor::Event::Leave) => {
                        next = Some((Pending::Front, "Standing Down", "Map Editor".into()))
                    }
                    Some(crate::editor::Event::Test(start)) => {
                        next = Some((
                            Pending::Match(start),
                            "Playtest",
                            "Escape: leave match to return to the editor".into(),
                        ))
                    }
                    None | Some(crate::editor::Event::StopTest) => {}
                }
            }
            Stage::Match(game) => {
                let renderer = self.renderer.as_mut().ok_or("no renderer")?;
                let ctx = FrameCtx {
                    renderer,
                    overlay: &mut self.overlay,
                    input: &self.input,
                    memory: &mut self.memory,
                    audio: &self.audio,
                    settings: &mut self.settings,
                    settings_changed: &mut settings_changed.0,
                    display_changed: &mut settings_changed.1,
                    now,
                    dt,
                    time,
                    cover: self.curtain.as_mut(),
                };
                let game_event = game.frame(ctx)?;
                let stop_test = self
                    .editor_return
                    .as_mut()
                    .is_some_and(|editor| editor.poll_test(game, renderer));
                match if stop_test {
                    Some(GameEvent::Leave)
                } else {
                    game_event
                } {
                    Some(GameEvent::Leave) => {
                        // What was left, and how long it was played.
                        let detail = format!(
                            "{}   \u{b7}   {}",
                            game.map().name(),
                            crate::hud::replay_clock(game.tick())
                        );
                        next = Some(match self.editor_return.take() {
                            Some(editor) => (
                                Pending::Editor(editor),
                                "Map Editor",
                                "Restoring authored map".into(),
                            ),
                            None => (Pending::Front, "Standing Down", detail),
                        });
                    }
                    Some(GameEvent::Quit) => quit = true,
                    Some(GameEvent::RangeMap { stem, subject }) => {
                        let opened = setup::find_map(Some(&stem)).and_then(|path| {
                            MapFile::open(&path).map_err(|e| format!("{}: {e}", path.display()))
                        });
                        match opened {
                            Ok(map) => {
                                let map = Arc::new(map);
                                let start =
                                    range_start(&map, &self.args.blueprints, &subject, None)?;
                                self.settings.range_map = stem;
                                settings_changed.0 = true;
                                next = Some((
                                    Pending::Match(Box::new(start)),
                                    "Test Range",
                                    map.name().to_owned(),
                                ));
                            }
                            Err(e) => {
                                log::error!("range map: {e}");
                                game.complain(&format!("That map did not open: {e}"));
                            }
                        }
                    }
                    // Stats, weapons and costs are data: read them again and stand the range back up.
                    Some(GameEvent::ReloadRange(subject)) => {
                        // Units, and the sounds they name: both are read again, and have to agree.
                        let loaded = Blueprints::locate_data_dir()
                            .ok_or("the data/ directory is gone".to_owned())
                            .and_then(|dir| {
                                let blueprints =
                                    Blueprints::load(&dir).map_err(|e| e.to_string())?;
                                let sounds =
                                    mc_data::SoundLibrary::load(&dir).map_err(|e| e.to_string())?;
                                sounds.check(&blueprints).map_err(|e| e.to_string())?;
                                Ok((blueprints, sounds))
                            });
                        match loaded {
                            Ok((blueprints, sounds)) => {
                                self.audio.set_library(sounds);
                                self.args.blueprints = Arc::new(blueprints);
                                let start =
                                    range_start(game.map(), &self.args.blueprints, &subject, None)?;
                                next = Some((
                                    Pending::Match(Box::new(start)),
                                    "Reloading",
                                    "Data/ Read Again".into(),
                                ));
                            }
                            Err(e) => {
                                log::error!("reload failed: {e}");
                                game.complain(&format!("data/ did not load: {e}"));
                            }
                        }
                    }
                    None => {}
                }
            }
        }
        self.reload_editor()?;
        // The pointer says what a click would do; outside a match it is the plain arrow.
        let pointer = match &self.stage {
            Stage::Match(game) => game.pointer(),
            _ => crate::pointer::Pointer::Arrow,
        };
        if let Some(window) = &self.window {
            self.cursors.show(
                event_loop,
                window,
                pointer,
                viewport.y / ui::CANVAS_H * self.settings.ui_scale,
            );
            // The free camera hides the pointer from a still frame, and holds it for mouse-look.
            let mode = match &self.stage {
                Stage::Match(game) => game.cursor_mode(),
                _ => crate::game::CursorMode::Normal,
            };
            if mode != self.cursor_mode {
                use crate::game::CursorMode;
                use winit::window::CursorGrabMode;
                if mode == CursorMode::Grabbed {
                    self.grabbed_at = Some(self.input.cursor);
                    if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                        let _ = window.set_cursor_grab(CursorGrabMode::Confined);
                    }
                } else if self.cursor_mode == CursorMode::Grabbed {
                    let _ = window.set_cursor_grab(CursorGrabMode::None);
                    if let Some(at) = self.grabbed_at.take() {
                        let _ = window.set_cursor_position(winit::dpi::PhysicalPosition::new(
                            at.x as f64,
                            at.y as f64,
                        ));
                    }
                }
                window.set_cursor_visible(mode == CursorMode::Normal);
                self.cursor_mode = mode;
            }
        }
        self.memory.end_frame(&self.input);
        if self.args.smoke && next.is_none() {
            let waited = self.stage_since.elapsed().as_secs_f32();
            match (&self.stage, self.smoke_step) {
                (Stage::Front(_), 0) if waited > 4.0 => {
                    let request = ui::setup::default_request(&self.settings)
                        .ok_or("smoke: the default skirmish is not startable")?;
                    log::info!(
                        "smoke: front end is up; starting a skirmish on {:?}",
                        request.map.name()
                    );
                    next = Some((
                        Pending::Match(Box::new(local_start(
                            request,
                            &self.args.blueprints,
                            false,
                        )?)),
                        "Deploying",
                        "Smoke Test".into(),
                    ));
                    self.smoke_step = 1;
                }
                (Stage::Match(game), 1) if game.tick() > 50 => {
                    log::info!("smoke: match reached tick {}; leaving", game.tick());
                    next = Some((Pending::Front, "Standing Down", "Smoke Test".into()));
                    self.smoke_step = 2;
                }
                (Stage::Match(_), 1) if waited > 120.0 => {
                    return Err("smoke: the match never started ticking".into())
                }
                (Stage::Front(f), 2) if waited > 4.0 && f.status.tick > 10 => {
                    log::info!(
                        "smoke: back in the front end, backdrop at tick {}; passed",
                        f.status.tick
                    );
                    quit = true;
                }
                _ => {}
            }
        }
        if let Some(c) = &mut self.curtain {
            let running = match &self.stage {
                Stage::Match(game) => game.tick() >= 1,
                Stage::Front(f) => f.status.tick >= 1,
                Stage::Editor(_) => true,
                Stage::Loading { .. } => false,
            };
            c.settle(dt, running);
            if c.gone() {
                self.curtain = None;
            }
        }
        if let Some((pending, loaded)) = ready {
            self.enter(pending, loaded)?;
        }

        if settings_changed.0 || settings_changed.1 {
            self.apply_settings(settings_changed.1)?;
        }
        if let Some((pending, title, detail)) = next {
            if let Some(pending) = self.editor_transition(pending)? {
                self.load(pending, title, &detail);
            }
        }
        if quit {
            event_loop.exit();
        }
        Ok(())
    }
}
