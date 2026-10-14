use super::*;
impl Game {
    pub fn frame(&mut self, ctx: FrameCtx) -> Result<Option<GameEvent>, String> {
        let FrameCtx {
            renderer,
            overlay,
            input,
            memory,
            audio,
            settings,
            settings_changed,
            display_changed,
            now,
            dt,
            time,
            cover,
        } = ctx;
        if !self.chart_ready {
            overlay.set_image(
                hud::MINIMAP_SLOT,
                ui::preview::SIZE,
                ui::preview::SIZE,
                &ui::preview::render(&self.map, &crate::setup::map_config(&self.map).look()),
            );
            self.chart_ready = true;
        }
        if !self.hud.thumbs.baked() {
            let team = self.view.colors[self.view.local as usize % mc_core::MAX_PLAYERS];
            self.hud.thumbs.bake(overlay, &self.blueprints, team);
        }
        self.view.fps = self.view.fps * 0.95 + (1.0 / dt.max(1e-4)) * 0.05;
        // F1 toggles the profiler; the choice is remembered.
        if settings.show_profiler != self.view.show_profiler {
            if self.keys.contains(&KeyCode::F1) {
                settings.show_profiler = self.view.show_profiler;
                *settings_changed = true;
            } else {
                self.view.show_profiler = settings.show_profiler;
            }
        }

        // Keyboard camera. Alt owns the view: the usual keys must not pan underneath an orbit.
        let cine = self.hud.free.on;
        self.cine.hand_back_lift(&mut self.camera);
        if self.orbit_saved.is_none()
            && !self.hud.browser_open()
            && !self.hud.net.typing()
            && !cine
            && memory.editing.is_none()
        {
            self.keyboard_camera(dt);
        }
        if !cine {
            self.ease_zoom(dt);
            self.ease_orbit(dt);
        }

        // The range's weather: the default on every visit, applied whenever the
        // panel changes it, and kept in the recording. A recorded range being
        // watched shows the weather it was recorded with.
        let sky = match self.view.range.as_mut() {
            Some(range) => Some(*range.sky.get_or_insert_with(Default::default)),
            None => self.view.status.replay.and_then(|r| r.range_sky),
        };
        if let Some(sky) = sky.filter(|&s| self.view.sky_shown != Some(s)) {
            sky.show(renderer, &self.map);
            self.view.sky_shown = Some(sky);
            if self.view.range.is_some() {
                // The sim thread is gone only after a fatal error, which the HUD already shows.
                let _ = self.sim.notes.send(crate::recorder::Note::RangeSky(sky));
            }
        }
        let fresh = self.pull_sim();
        self.rain_here = self.rain_heard(renderer);
        self.thunder(renderer.take_thunder(), audio);
        self.ambience_frame(renderer, audio, dt);
        let alpha = ((now - self.published_at).as_secs_f32() / self.interp_span).clamp(0.0, 1.0);
        self.cine_alpha = alpha;
        if cine {
            self.cine_frame(renderer, dt, alpha);
            self.focus_eased_at = None;
        } else {
            self.follow_camera(alpha);
            if self.track.is_none() && self.orbit_unit.is_none() {
                self.ease_focus_height(renderer, dt);
            } else {
                self.focus_eased_at = None;
            }
            // The play camera never looks from under the sea: a dived boat it
            // follows is framed from the surface above it.
            let sea = self.map.info().water_level.to_f32();
            self.camera.focus.z = self.camera.focus.z.max(sea);
        }
        self.cine.hand_back_apply(&mut self.camera, dt);
        self.mission.update(
            self.view.status.mission.as_ref(),
            &mut self.camera,
            renderer,
            audio,
            dt,
        );
        if fresh {
            self.note_events(audio);
            // A scenario's builder has arrived: give it the order it was spawned for.
            let owed = self
                .view
                .range
                .as_mut()
                .map(|r| r.resolve_pending(&self.view.frame.units))
                .unwrap_or_default();
            if !owed.is_empty() {
                self.view.local = range::BLUE;
            }
            for command in owed {
                self.send(command);
            }
        }
        self.live_frame(renderer, fresh);

        self.answer_selection(audio, now);

        self.result_frame(dt);

        // Selection and hover marks.
        let over_ui = self.menu.is_some() || self.editor_covers(self.cursor);
        let hover_unit = if self.view.mode == Mode::Normal && !over_ui {
            self.unit_at(self.cursor).and_then(|i| {
                let u = &self.view.frame.units[i];
                (u.owner_flags & KIND_WRECK == 0).then_some(i)
            })
        } else {
            None
        };

        // The sim publishes the order queues of the whole side: every command group wears
        // its count on the map. Only the selection's lines show without shift. The unit
        // under the pointer is watched too, so its card (`hud::selection::hover_card`)
        // shows the sim's own figures, as the selected unit's card does.
        let everyone = true;
        let hovered = hover_unit
            .map(|i| self.view.frame.units[i].unit_id)
            .filter(|id| !self.view.selection.contains(id));
        let watch = self
            .view
            .selection
            .iter()
            .take(MAX_WATCHED)
            .copied()
            .chain(hovered);
        if !watch.clone().eq(self.watched.units.iter().copied())
            || self.watched.side != self.view.local
            || self.watched.everyone != everyone
            || self.watched.perspective != self.view.perspective
            || self.watched.zones != self.mind_map_on()
            || self.watched.reveal != self.editor_tools.as_ref().is_some_and(|t| t.reveal)
        {
            self.watched = Watch {
                units: watch.collect(),
                side: self.view.local,
                everyone,
                perspective: self.view.perspective,
                zones: self.mind_map_on(),
                reveal: self.editor_tools.as_ref().is_some_and(|t| t.reveal),
            };
            self.sim.watch.lock().unwrap().clone_from(&self.watched);
        }
        let mut marks: Vec<Mark> = self
            .view
            .selection
            .iter()
            .filter_map(|id| self.view.index_of.get(id).copied())
            .map(|i| self.unit_mark(i, false))
            .collect();
        if let Some(i) = hover_unit {
            if !self
                .view
                .selection
                .contains(&self.view.frame.units[i].unit_id)
            {
                marks.push(self.unit_mark(i, true));
            }
        }
        if !self.hud.free.on {
            work::bar_marks(&self.view, |o| !self.is_enemy(o), &mut marks);
        }

        // Placement preview: one ghost under the pointer, or a line of them while dragging.
        let mut ghosts: Vec<UnitInstance> = Vec::new();
        let sites = match self.view.mode {
            Mode::Place(blueprint) => self.placing_sites(
                blueprint,
                self.left_down
                    .is_some_and(|from| from.distance(self.cursor) >= DRAG_THRESHOLD),
                renderer,
            ),
            _ => Vec::new(),
        };
        if let Mode::Place(blueprint) = self.view.mode {
            let field = Field {
                view: &self.view,
                blueprints: &self.blueprints,
                map: &self.map,
                camera: &self.camera,
                renderer,
            };
            let heading = self.place_heading(blueprint);
            for &(pos, fit) in &sites {
                ghosts.push(orders::placement_ghost(
                    &field,
                    blueprint,
                    pos,
                    heading,
                    fit.is_ok(),
                ));
            }
        }
        let prefab_sites = match self.view.mode {
            Mode::Prefab(id) if !over_ui => self.prefab_sites(id, renderer),
            _ => Vec::new(),
        };
        self.prefab_ghosts(&prefab_sites, renderer, &mut ghosts);

        if let (Mode::Spawn | Mode::SpawnSubject, Some(range), false) =
            (self.view.mode, &self.view.range, over_ui)
        {
            if let Some(g) = self.ground_under_cursor(renderer) {
                let owner = if range.side == range::Side::Blue {
                    range::BLUE
                } else {
                    range::RED
                };
                let mobile_heading = if owner == range::BLUE {
                    0.0
                } else {
                    (Vec2::from(range.pad.to_f32()) - g.truncate()).to_angle()
                };
                let picked: Vec<&UnitInstance> = self
                    .view
                    .selection
                    .iter()
                    .filter_map(|id| self.view.index_of.get(id))
                    .map(|&i| &self.view.frame.units[i])
                    .filter(|u| u.owner_flags & KIND_WRECK == 0)
                    .collect();
                let n = picked.len().max(1) as f32;
                let cx = picked.iter().map(|u| u.pos[0]).sum::<f32>() / n;
                let cy = picked.iter().map(|u| u.pos[1]).sum::<f32>() / n;
                let ghosts_of: Vec<(u32, f32, [f32; 2])> =
                    if self.view.mode == Mode::SpawnSubject || picked.is_empty() {
                        vec![(
                            range.subject.0 as u32,
                            self.blueprints.unit(range.subject).radius.to_f32(),
                            [0.0, 0.0],
                        )]
                    } else {
                        picked
                            .iter()
                            .map(|u| (u.blueprint, u.radius, [u.pos[0] - cx, u.pos[1] - cy]))
                            .collect()
                    };
                let origin: [f32; 3] = g.into();
                for (blueprint, radius, offset) in ghosts_of {
                    let pos = [origin[0] + offset[0], origin[1] + offset[1], origin[2]];
                    let bp = self.blueprints.unit(BlueprintId(blueprint as u16));
                    let heading = if bp.built_on_site() {
                        bp.build_heading().to_radians_f32()
                    } else {
                        mobile_heading
                    };
                    ghosts.push(UnitInstance {
                        prev_pos: pos,
                        prev_heading: heading,
                        pos,
                        heading,
                        blueprint,
                        owner_flags: owner as u32 | KIND_GHOST,
                        health: 1.0,
                        build: 1.0,
                        turret_yaw: 0.0,
                        radius,
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
                        twin_spin: [0.0; 2],
                    });
                }
            }
        }

        // Behind those: the structures that are planned, and the order the pointer has hold of.
        let placing = ghosts.len();
        let field = Field {
            view: &self.view,
            blueprints: &self.blueprints,
            map: &self.map,
            camera: &self.camera,
            renderer,
        };
        self.orders.update(&field, self.cursor, over_ui);
        let outlined = self.orders.ghosts(&field, &mut ghosts);
        // Walls in the preview join each other and the walls already standing.
        mc_sim::mirror::join_walls(&self.blueprints, &mut ghosts, &self.view.frame.units);
        // A held move: a hologram of each member where it would stand.
        let formation_facing = self
            .formation_drag
            .as_ref()
            .filter(|d| d.showing(self.cursor, DRAG_THRESHOLD))
            .map(|d| {
                let ground = self.ground_under_cursor(renderer).map(|g| g.truncate());
                let facing = d.facing(self.cursor, ground, DRAG_THRESHOLD);
                d.ghosts(&field, facing, &mut ghosts);
                facing
            });
        self.pointer = if self.hud.free.on {
            self.free_camera_pointer()
        } else {
            self.pointer_for(
                renderer,
                over_ui,
                sites.iter().any(|(_, fit)| fit.is_ok())
                    || prefab_sites.iter().any(|s| s.goes_down()),
            )
        };
        // What a click would take apart is ringed in the Materials red-orange, wreck or
        // unit, in place of its selection or hover mark (its bars stay).
        if self.pointer == Pointer::Reclaim {
            if let Some(i) = self.unit_at(self.cursor) {
                marks.retain(|m| m.unit_index != i as u32 || m.kind & mark::BARS_ONLY != 0);
                let mut target = self.unit_mark(i, true);
                target.kind |= mark::RECLAIM;
                marks.push(target);
            }
        }

        // Range rings: what the selection reaches, and what the thing being placed would.
        // Units in a lift ship's hold reach nothing until they are out.
        let selected = self
            .view
            .selection
            .iter()
            .filter_map(|id| self.view.index_of.get(id))
            .map(|&i| &self.view.frame.units[i])
            .filter(|u| !u.stored());
        // A survival Shaper's rings are its level's, not its blueprint's.
        let shaper = crate::rings::shaper(&self.view, self.hud.survival.shaper_focus);
        let selected = selected.filter(|u| shaper.as_ref().is_none_or(|s| s.0 != u.unit_id));
        self.rings.focus = self.hud.reach_focus.take();
        let (mut ranges, mut ranges_drawn) = self.rings.collect(
            ghosts[..placing]
                .iter()
                .chain(self.orders.ghost_in_hand(&ghosts[placing..]))
                .chain(selected),
            alpha,
            fresh,
            &|p| renderer.ground_height(Vec2::from(p)),
        );
        if let Some((_, rings)) = shaper {
            crate::rings::prepend(&mut ranges, &mut ranges_drawn, rings);
        }
        // Placing a radar, sonar or missile defence post: our standing posts' rings too,
        // merged with the new one's.
        if let Mode::Place(bp) = self.view.mode {
            let network = crate::rings::cover_network(
                &self.blueprints,
                bp,
                None,
                self.view.local,
                self.view.frame.units.iter(),
            );
            crate::rings::prepend(&mut ranges, &mut ranges_drawn, network);
        }
        self.view.reaches = Rings::key(&ranges);

        // A single-player match holds its clock under the menu and on the player's
        // pause; a network match plays on.
        let menu_holds = self.menu_holds_clock();
        self.sim.paused.store(
            self.view.status.owns_clock && (menu_holds || self.view.paused),
            Ordering::Relaxed,
        );
        self.music_frame(audio, dt, fresh);

        // HUD.
        overlay.clear();
        self.view.menu_open = self.menu.is_some();
        let viewport = self.camera.viewport;
        let mut ui = Ui::new(
            overlay,
            input,
            memory,
            audio,
            viewport,
            settings.ui_scale,
            time,
            dt,
        );
        ui.interactive = self.menu.is_none();
        // The ground shocks of the giants shake the view (audio/titan.rs). What is drawn on
        // the ground (warhead and AEB marks, orders) shakes with it, or it would slide over
        // the ground; the panels and the minimap's footprint hold still.
        let shaken = self.giant_sounds.shaken(&self.camera);
        let field = Field {
            view: &self.view,
            blueprints: &self.blueprints,
            map: &self.map,
            camera: &shaken,
            renderer,
        };
        self.mission
            .signs(&mut ui, &self.map, &shaken, renderer, &self.view.frame);
        // Marks on the ground grow with the side (every plan's lot, every order's line):
        // half the overlay is kept back for the HUD after them, so a big base cannot
        // crowd the panels off the screen.
        ui.o.hold_back(mc_render::overlay::MAX_OVERLAY_VERTICES / 2);
        // The free camera's picture is clean: no orders, marks or reticles over it.
        if !self.hud.free.on {
            self.orders.draw(&mut ui, &field, alpha);
            crate::line_of_fire::draw_hidden(&mut ui, &field, alpha);
            // Warheads in flight, where they will land, and a launch being aimed (`nuke_marks.rs`).
            let placing = match self.view.mode {
                Mode::Place(bp) => sites
                    .last()
                    .map(|s| (bp, Vec2::new(s.0.x.to_f32(), s.0.y.to_f32()))),
                _ => None,
            };
            crate::nuke_marks::draw(
                &mut ui,
                &field,
                alpha,
                self.ground_under_cursor(renderer),
                placing,
            );
            crate::cover_marks::draw(&mut ui, &field, placing);
            crate::adjacency_marks::draw(
                &mut ui,
                &field,
                hover_unit.map(|i| self.view.frame.units[i].unit_id),
                self.hud.adjacency.partner,
                placing.map(|(bp, at)| (bp, at, self.place_heading(bp))),
            );
            work::draw_tags(&mut ui, &field, alpha, |o| !self.is_enemy(o));
            crate::warp_in_marks::draw(&mut ui, &field);
            crate::titan_marks::draw(&mut ui, &field, alpha, self.ground_under_cursor(renderer));
            crate::dash_marks::draw(&mut ui, &field, alpha, self.ground_under_cursor(renderer));
            crate::gravitic_marks::draw(&mut ui, &field, self.ground_under_cursor(renderer));
            crate::edict_marks::draw(&mut ui, &field, self.ground_under_cursor(renderer));
            crate::warp_marks::draw(&mut ui, &field, alpha, self.ground_under_cursor(renderer));
            crate::destruct_marks::draw(&mut ui, &field, alpha);
            self.draw_mind_map(&mut ui, &field, renderer);
            if self.pointer == Pointer::Attack {
                if let Some(target) = self.unit_at(self.cursor) {
                    crate::line_of_fire::draw_hover(&mut ui, &field, target);
                }
            }
            orders::ghost_footprints(&mut ui, &field, &ghosts[..outlined]);
            self.orders.draw_pending(&mut ui, &field, self.cursor);
            if let (Some(drag), Some(facing)) = (self.formation_drag.as_ref(), formation_facing) {
                drag.draw(&mut ui, &field, facing);
            }
            if let Some(from) = self.left_down {
                if self.view.mode == Mode::Normal && from.distance(self.cursor) >= DRAG_THRESHOLD {
                    hud::drag_box(&mut ui, from, self.cursor);
                }
            }
        }
        let link = self.sim.net.as_ref().map(|n| n.link());
        let net_notices = self.sim.net.as_ref().map_or_else(Vec::new, |n| n.notices());
        if let Some(link) = &link {
            // A network match's pause and speed are the relay's, whoever asked for them.
            self.view.paused = link.paused_by.is_some();
            self.view.speed = link.speed.unwrap_or(100);
        }
        let placing_open = match (self.view.mode, sites.last()) {
            (Mode::Place(bp), Some(&(at, _))) => {
                let field = Field {
                    view: &self.view,
                    blueprints: &self.blueprints,
                    map: &self.map,
                    camera: &self.camera,
                    renderer,
                };
                orders::mine_reach::open_instead(&field, bp, Vec2::from(at.to_f32()))
            }
            _ => None,
        };
        let scene = hud::Scene {
            net: link.as_ref(),
            net_notices: &net_notices,
            prefabs: &self.prefabs.library,
            view: &self.view,
            blueprints: &self.blueprints,
            map: &self.map,
            camera: &self.camera,
            gpu: &renderer.stats,
            show_reclaim: self.ctrl && !self.alt && self.menu.is_none() && !self.hud.free.on,
            placing: sites.last().map(|s| s.0),
            placing_open,
            hover: hover_unit
                .filter(|&i| self.can_inspect(&self.view.frame.units[i]))
                .map(|i| self.view.frame.units[i].unit_id),
        };
        ui.o.release();
        let actions = self.hud.draw(&mut ui, &scene, dt);
        self.mission
            .draw(&mut ui, self.view.status.mission.as_ref());
        if !over_ui && !self.hud.free.on {
            hud::cursor_hint(&mut ui, &self.view, &self.blueprints, &sites);
            if let Mode::Prefab(id) = self.view.mode {
                if let Some(prefab) = self.prefabs.library.get(id) {
                    hud::prefabs::cursor_hint(&mut ui, prefab, &prefab_sites, self.prefabs.turn);
                }
            }
            let field = Field {
                view: &self.view,
                blueprints: &self.blueprints,
                map: &self.map,
                camera: &self.camera,
                renderer,
            };
            crate::warp_marks::cursor_card(&mut ui, &field, self.ground_under_cursor(renderer));
            if self.pointer == Pointer::Board {
                self.board_hint(&mut ui);
            }
        }

        // The in-match menu, over everything; the settings screen over that.
        let mut event = self.editor_panel(&mut ui).then_some(GameEvent::Leave);
        // A match that is already over for this machine (decided, out of step, link lost) is
        // left, not given up.
        let surrender = link
            .as_ref()
            .is_some_and(|l| l.desync.is_none() && l.rejoining.is_none())
            && !self.report_open_to_all();
        if let Some(e) = self.menu_frame(
            &mut ui,
            settings,
            (settings_changed, display_changed),
            surrender,
            dt,
        ) {
            event = Some(e);
        }
        // A surrender goes out with the next turn; the match is left once it has been
        // carried out, so every machine sees this side fall rather than stand idle.
        if link
            .as_ref()
            .and_then(|l| l.surrendering)
            .is_some_and(|since| self.defeated() || since.elapsed() > SURRENDER_WAIT)
        {
            event = Some(GameEvent::Leave);
        }
        if let Some(cover) = cover {
            cover.draw(&mut ui);
        }
        for action in actions {
            if action == HudAction::Leave {
                event = Some(GameEvent::Leave);
                continue;
            }
            self.hud_action(action, audio);
        }

        // Ore shows its veins while a mine is placed or selected, or with Ctrl.
        let placing_mine =
            matches!(self.view.mode, Mode::Place(bp) if self.blueprints.unit(bp).mine.is_some());
        let mine_selected = self.selected_units().any(|u| {
            self.blueprints
                .unit(BlueprintId(u.blueprint as u16))
                .mine
                .is_some()
        });
        let survey = !self.hud.free.on
            && (placing_mine || mine_selected || (self.ctrl && !self.alt && self.menu.is_none()));
        renderer.set_ore_highlight(if survey { 1.0 } else { 0.0 });
        renderer.set_ore_claims(&hud::ore_claims(&self.blueprints, &self.view.frame.units));
        // Adjacency conduits: the selection's and the pointer's lit, and the placing
        // site's would-be links drawn as planned (`hud::adjacency`).
        let lit: Vec<u32> = self
            .view
            .selection
            .iter()
            .copied()
            .chain(hover_unit.map(|i| self.view.frame.units[i].unit_id))
            .chain(self.hud.adjacency.partner)
            .collect();
        let planned = match (self.view.mode, sites.last()) {
            (Mode::Place(bp), Some(site)) => hud::adjacency::planned_links(
                &self.blueprints,
                &self.view.frame.units,
                self.view.local,
                self.blueprints.unit(bp),
                site.0,
                self.place_heading(bp),
            ),
            _ => Vec::new(),
        };
        renderer.set_link_focus(&lit, &planned);
        let build_grid = matches!(self.view.mode, Mode::Place(_) | Mode::Prefab(_))
            || self.orders.dragging_plan();
        if build_grid {
            let field = Field {
                view: &self.view,
                blueprints: &self.blueprints,
                map: &self.map,
                camera: &self.camera,
                renderer,
            };
            let focus = orders::build_grid_focus(&field, self.cursor, self.orders.plan_in_hand());
            let (centre, radius, lots) = focus.unwrap_or((Vec2::ZERO, 0.0, Vec::new()));
            renderer.set_build_grid(centre, radius, &lots);
        }
        let clean = self.hud.free.on;
        // The world is drawn on the match's clock, not the wall's: the moment the units
        // are drawn at (between the last two ticks). Every effect ages by it, so a blast
        // or a beam lasts as long in game time at any game speed, and holds while paused.
        let world_time = ((self.view.frame.tick as f32 - 1.0 + alpha) * TICK_SECONDS).max(0.0);
        let frame = FrameInput {
            camera: &shaken,
            time: world_time,
            alpha,
            sim: fresh.then_some(&self.view.frame),
            ghosts: if clean { &[] } else { &ghosts },
            marks: if clean { &[] } else { &marks },
            ranges: if clean { &[] } else { &ranges },
            ranges_drawn: if clean { 0 } else { ranges_drawn },
            overlay,
            build_grid,
            icons: !clean,
        };
        self.hud.issues.pump_capture(renderer);
        renderer
            .render(&frame)
            .map_err(|e| format!("rendering failed: {e}"))?;
        self.view.cpu_ms = self.view.cpu_ms * 0.9 + now.elapsed().as_secs_f32() * 100.0;
        Ok(event.or(self.event.take()))
    }
}
