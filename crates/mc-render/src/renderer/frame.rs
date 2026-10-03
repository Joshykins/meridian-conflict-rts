//! Frame recording and submission.

use super::*;

impl Renderer {
    /// Renders one frame. Returns `false` if the swapchain had to be rebuilt and the frame was skipped.
    pub fn render(&mut self, input: &FrameInput) -> Result<bool, GpuError> {
        let result = self.render_frame(input);
        if matches!(result, Err(GpuError::Vk(vk::Result::ERROR_DEVICE_LOST))) {
            self.timers.lost();
            log::error!("{}", self.loss_context(input));
            match self.gpu.fault_report() {
                Some(report) => log::error!("{report}"),
                None => log::error!("no fault report (VK_EXT_device_fault is not available)"),
            }
        }
        result
    }

    /// What was on screen when the device was lost: the counts every pass draws by.
    fn loss_context(&self, input: &FrameInput) -> String {
        let c = input.camera;
        format!(
            "scene at the loss: output {}x{}, scene {}x{}, time {:.2} s, camera focus {:?} \
             distance {:.0} yaw {:.2} tilt {:.2}\n  units {} (dynamic {}, static {}), draw slots {}, stains {}, pads {}, \
             ore tiles {}, tracks {}, prints {}, missiles {}, shields {} (hull {}), \
             fresh sim frame {}",
            self.width,
            self.height,
            self.scene_width,
            self.scene_height,
            input.time,
            c.focus,
            c.distance,
            c.yaw,
            c.tilt,
            self.sim_units,
            self.dynamic_count,
            self.static_count,
            self.slot_count,
            self.stain_count(),
            self.pad_count,
            self.deposit_count,
            self.track_count,
            self.prints.count,
            self.projectile_count,
            self.shield_count,
            self.hull_shield_count,
            input.sim.is_some(),
        )
    }

    fn render_frame(&mut self, input: &FrameInput) -> Result<bool, GpuError> {
        let device = self.gpu.device.clone();
        {
            // Time spent waiting for the GPU to finish the frame before this one.
            let _t = mc_core::perf_span!("cpu.gpu_wait");
            self.timers.step("waiting for the last frame's fence");
            // SAFETY: the fence is this device's and was submitted (or created signalled), so
            // the wait ends.
            unsafe {
                device.wait_for_fences(&[self.fence], true, u64::MAX)?;
            }
            self.timers.step("recording");
        }
        for b in self.garbage.drain(..) {
            self.gpu.destroy_buffer(b);
        }
        self.read_timestamps();
        self.capture.collect(&self.gpu);

        let image_index = match &self.output {
            Output::Window(sc) => {
                let swapchain_fn = self.gpu.swapchain_fn.as_ref().expect("window target");
                self.timers.step("acquiring a swapchain image");
                // SAFETY: the chain and semaphore are this device's; the semaphore has no
                // pending signal: every successful acquire is followed by the submit that
                // waits on it (an error in between is fatal to the app), and that submit's
                // fence has just been waited on.
                match unsafe {
                    swapchain_fn.acquire_next_image(
                        sc.swapchain,
                        u64::MAX,
                        self.image_available,
                        vk::Fence::null(),
                    )
                } {
                    Ok((index, _suboptimal)) => index as usize,
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                        self.create_size_dependent()?;
                        return Ok(false);
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            Output::Headless { .. } => 0,
        };
        // SAFETY: the fence was waited on above, so it is signalled and no queue operation
        // still uses it.
        unsafe { device.reset_fences(&[self.fence])? };

        // ---- CPU-side updates -------------------------------------------------
        let camera = input.camera;
        if let Some(frame) = input.sim {
            let _t = mc_core::perf_span!("cpu.upload_sim");
            // A tick's effects are timed from its start, where its units are drawn at
            // alpha 0; the frame that brings it may already be part way through it.
            let tick_start = input.time - input.alpha.clamp(0.0, 1.0) * self.tick_seconds;
            self.upload_sim(frame, tick_start, camera);
            let _t = mc_core::perf_span!("cpu.upload.draws_terrain");
            self.cull
                .draws
                .set_units(&frame.units[..self.sim_units as usize]);
            self.fog_enabled = !frame.fog.is_empty();
            self.fog.set_frame(&self.gpu, frame)?;
            self.precursor_activity = frame.precursor_activity;
            self.tile_cache
                .apply_edits(&frame.terrain_edits, input.time, &mut self.upload_scratch);
            self.foundations.update(
                &frame.terrain_edits,
                &frame.terrain_edit_factions,
                &self.blueprints,
                input.time,
            );
        }
        let ghosts = &input.ghosts[..input.ghosts.len().min(MAX_GHOSTS)];
        self.dynamic.write(
            (self.sim_units as usize * size_of::<UnitInstance>()) as u64,
            bytemuck::cast_slice(ghosts),
        );
        self.dynamic_count = self.sim_units + ghosts.len() as u32;
        let trees: Vec<_> = self
            .burning_trees
            .iter()
            .map(|tree| {
                let mut instance = tree.instance;
                instance.health = (1.0 - (input.time - tree.start) / 9.0).clamp(0.0, 1.0);
                // Last few seconds: the burned crown crumbles into its own smoke.
                let collapse = ((input.time - tree.start - 34.0) / 8.0).clamp(0.0, 1.0);
                instance.packed = ((instance.packed as f32 * (1.0 - collapse)).max(1.0)) as u32;
                instance
            })
            .collect();
        self.dynamic.write(
            (self.dynamic_count as usize * size_of::<UnitInstance>()) as u64,
            bytemuck::cast_slice(&trees),
        );
        self.dynamic_count += trees.len() as u32;
        self.land_fallen_trees(input.time);
        {
            let _t = mc_core::perf_span!("cpu.sea_fx");
            self.upload_sea_fx(input.time, input.alpha, camera);
        }
        let fallen = self.fallen_tree_instances(input.time);
        let fallen = &fallen[..fallen
            .len()
            .min(MAX_DYNAMIC.saturating_sub(self.dynamic_count as usize))];
        self.dynamic.write(
            (self.dynamic_count as usize * size_of::<UnitInstance>()) as u64,
            bytemuck::cast_slice(fallen),
        );
        self.dynamic_count += fallen.len() as u32;
        // Reclaimed wrecks' last hulls burning away (wreck_finish.rs).
        let going = self.wreck_finish.instances(input.time);
        let going = &going[..going
            .len()
            .min(MAX_DYNAMIC.saturating_sub(self.dynamic_count as usize))];
        self.dynamic.write(
            (self.dynamic_count as usize * size_of::<UnitInstance>()) as u64,
            bytemuck::cast_slice(going),
        );
        self.dynamic_count += going.len() as u32;
        self.cull
            .draws
            .update(ghosts.iter().chain(&trees).chain(fallen).chain(going));

        let z_range = (
            self.map_info.min_z.to_f32(),
            self.map_info.min_z.to_f32() + self.map_info.z_step.to_f32() * 65535.0,
        );
        // The ground's real extent: a map may reach far above or below the default range.
        let z_used = self.tile_cache.height_span;
        if !terrain::select_nodes(
            camera,
            z_used,
            &self.tile_cache.cliffs,
            &mut self.node_scratch,
        ) {
            log::error!("terrain node budget of {MAX_NODES} exceeded; distant terrain is missing this frame");
        }
        self.nodes
            .write(0, bytemuck::cast_slice(&self.node_scratch));
        self.tile_cache
            .update(camera, &self.pool, &mut self.upload_scratch);

        // Ringed marks first, then those that only show bars: the ring draw takes the front run.
        let (ringed, bare): (Vec<&Mark>, Vec<&Mark>) = input
            .marks
            .iter()
            // One per entity at most (`MAX_MARKS`): never cut.
            .take(MAX_MARKS)
            .filter(|m| m.unit_index < self.sim_units)
            .partition(|m| m.kind & Mark::BARS_ONLY == 0);
        let ringed_count = ringed.len() as u32;
        let marks: Vec<Mark> = ringed
            .into_iter()
            .chain(bare)
            .map(|m| Mark {
                unit_index: m.unit_index | 0x8000_0000,
                ..*m
            })
            .collect();
        self.marks.write(0, bytemuck::cast_slice(&marks));
        // Long rings first, then short ones, then those that only mask: each run is one draw.
        let all = &input.ranges[..input.ranges.len().min(MAX_RANGES)];
        let (shown, masks) = all.split_at(input.ranges_drawn.min(all.len()));
        let mut ranges: Vec<RangeRing> = shown
            .iter()
            .filter(|r| r.outer >= RANGE_LONG)
            .copied()
            .collect();
        let ranges_long = ranges.len() as u32;
        ranges.extend(shown.iter().filter(|r| r.outer < RANGE_LONG));
        let ranges_short = ranges.len() as u32 - ranges_long;
        ranges.extend_from_slice(masks);
        self.ranges.write(0, bytemuck::cast_slice(&ranges));
        let overlay =
            &input.overlay.vertices[..input.overlay.vertices.len().min(MAX_OVERLAY_VERTICES)];
        self.overlay_vb.write(0, bytemuck::cast_slice(overlay));
        self.upload_overlay_atlas(input.overlay)?;
        let glass = input.overlay.has_glass();

        // Sun, and a shadow box around what the camera is looking at.
        let sun = self.sky.light_direction();
        let selected: Vec<u32> = input
            .marks
            .iter()
            .filter(|m| m.kind & (crate::gpu_consts::mark::HOVER | Mark::BARS_ONLY) == 0)
            .map(|m| m.unit_index)
            .collect();
        // Explosions and weapon flashes light the clouds over them.
        let glows =
            self.lights
                .cloud_glows(input.time, camera.focus, camera.distance * 2.5 + 3000.0);
        self.sky.set_glows(&glows);
        self.sky.update(&crate::sky::SkyFrame {
            camera,
            time: input.time,
            alpha: input.alpha.clamp(0.0, 1.0),
            tick_seconds: self.tick_seconds,
            selected: &selected,
        });
        let shadow_strength = 1.0 - ((camera.distance - 3000.0) / 5000.0).clamp(0.0, 1.0);
        let cascades = shadow_cascades::fit(camera, sun, z_range, SHADOW_SIZE);
        let shadow_view_proj: Mat4 = cascades[0].view_proj;

        let view_proj = camera.view_proj();
        let eye = camera.eye();
        let size = self.map_info.size_metres().to_f32();
        let (tw, th) = self.tile_cache.tiles();
        // Below this projected radius a unit is drawn as its strategic icon;
        // with icons off, never.
        let icon_px = if input.icons {
            self.height as f32 * 0.0055
        } else {
            0.0
        };
        let (tree_blast_count, tree_blasts) =
            self.tree_blasts.upload(input.time, &camera.frustum());
        let (nukes, strategic, nuke_view) =
            self.nuke_frame(input.time, input.alpha.clamp(0.0, 1.0), camera, view_proj);
        let mut settling = [[0.0; 4]; settle::SLOTS as usize * 2];
        let settling_count = self.tile_cache.settling(input.time, &mut settling);
        let climate = self.climate_globals();
        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            shadow_view_proj: shadow_view_proj.to_cols_array_2d(),
            camera: [eye.x, eye.y, eye.z, input.time],
            sun: [sun.x, sun.y, sun.z, input.alpha.clamp(0.0, 1.0)],
            viewport: [
                self.width as f32,
                self.height as f32,
                1.0 / self.width as f32,
                1.0 / self.height as f32,
            ],
            frustum: camera.frustum().map(|p| p.to_array()),
            map: [
                size[0],
                size[1],
                self.map_info.water_level.to_f32(),
                shadow_strength,
            ],
            height: [z_range.0, z_range.1 - z_range.0, tw as f32, th as f32],
            // Projected radii, not diameters: keep full detail down to about
            // 72 px across, and reserve the coarse silhouettes for under 24 px.
            lod: [camera.projection_scale(), icon_px, 36.0, 12.0],
            counts: [
                self.dynamic_count,
                self.static_count,
                self.slot_count,
                self.fog_enabled as u32 | (input.build_grid as u32) << 1,
            ],
            plating: self.palette[0],
            accent: self.palette[1],
            glow: self.palette[2],
            team_colors: self.team_colors,
            build_cursor: self.build_cursor,
            scene: [
                self.scene_width as f32,
                self.scene_height as f32,
                self.render_scale,
                self.sky.clear_strength(),
            ],
            build_blocked: self.build_blocked,
            tree_wind: [
                tree_blast_count as f32,
                camera.focus.x,
                camera.focus.y,
                self.precursor_activity,
            ],

            tree_blasts,
            shadow_cascades: cascades.each_ref().map(|c| c.view_proj.to_cols_array_2d()),
            shadow_info: cascades.each_ref().map(|c| c.info),
            shield: self.palette[3],
            nukes,
            nuke_view,
            strategic,
            climate: [
                climate.climate,
                self.grass.enabled as u32 as f32,
                grass::reach(camera.projection_scale()),
                // How many sim ticks this frame covers (game time is sim time), so a
                // treads' links blur when they move too far a frame to read (entity.wgsl).
                ((input.time - self.last_time) * mc_core::TICKS_PER_SECOND as f32).clamp(0.0, 4.0),
            ],
            detail: [
                self.quality.prop_detail[0],
                self.quality.prop_detail[1],
                self.quality.prop_detail[2],
                self.quality.simple_shading as u32 as f32,
            ],
            settling,
            settle: [settling_count as f32, 0.0, 0.0, 0.0],
            region_climate: climate.region_climate,
            map_look: climate.map_look,
        };
        self.globals.write(0, bytemuck::bytes_of(&globals));
        self.last_time = input.time;
        {
            let _t = mc_core::perf_span!("cpu.craters_lights");
            self.upload_craters(input.time, camera);
            self.ground_melt.step(input.time);
            self.ground_melt.upload();
            self.heat_haze
                .upload(input.time, input.alpha.clamp(0.0, 1.0), camera);
            self.upload_wake_shells(input.time);
            self.upload_lights(input.time, input.alpha.clamp(0.0, 1.0), camera);
        }
        // Recording, submitting and presenting, to the end of the frame.
        let _record = mc_core::perf_span!("cpu.record_submit");

        // ---- Record -----------------------------------------------------------
        let cmd = self.cmd;
        // SAFETY: the frame fence was waited on at the top of `render`, so the GPU is done with
        // `cmd`, and its pool allows resetting one buffer.
        unsafe {
            device.reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty())?;
            device.begin_command_buffer(
                cmd,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
        }
        self.timers.reset(&device, cmd);
        self.timers.scope(&device, cmd, "uploads");
        self.record_uploads(cmd, input.sim)?;
        self.record_light_copy(cmd);
        self.timers.end(&device, cmd);
        self.timers.scope(&device, cmd, "fog");
        self.fog
            .record(&self.gpu, cmd, self.fog_enabled, input.time, input.alpha);
        self.timers.end(&device, cmd);
        self.timers.draws(&device, cmd, "clouds.sim");
        self.sky.record_sim(&self.gpu, cmd);
        self.sky
            .record_shade(&self.gpu, cmd, self.scene_set, self.quality.simple_shading);
        self.timers.end(&device, cmd);
        // GPU culling and draw generation.
        self.timers.draws(&device, cmd, "cull");
        // SAFETY: `cmd` is recording and outside a render pass; `cull_set` is live and made
        // for `layouts.cull`, the layout of the cull pipelines.
        unsafe {
            self.cull.record(
                &device,
                cmd,
                &self.pipelines,
                self.layouts.cull,
                self.cull_set,
                self.static_count,
                self.dynamic_count,
            )
        };
        self.timers.end(&device, cmd);

        // The grass round the eye, grown for this frame (grass.rs).
        let eye = camera.eye();
        let grass_frame = grass::GrassFrame {
            eye,
            projection_scale: camera.projection_scale(),
            ground: self.ground_height(eye.truncate()),
            dynamic_count: self.dynamic_count,
            static_count: self.static_count,
            scorch_count: self.stain_count(),
            lot_count: self.pad_count,
            track_count: self.prints.gather_count(self.track_count),
            clad_count: self.foundations.count(),
            time: input.time,
        };
        self.timers.draws(&device, cmd, "grass.grow");
        self.grass
            .record(&self.gpu, cmd, self.scene_set, &grass_frame);
        self.timers.end(&device, cmd);

        let node_count = self.node_scratch.len() as u32;
        let model_slots = self.slot_count - 1;
        mc_core::perf_count!("draw.model_slots.total", model_slots);
        mc_core::perf_count!(
            "draw.model_slots.active",
            self.cull
                .draws
                .ranges
                .iter()
                .chain(&self.cull.draws.prop_ranges)
                .map(|r| r.end - r.start)
                .sum::<u32>()
        );
        let gfx = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
        // SAFETY: the closure is called only in `render` while `cmd` is recording, and every
        // pipeline it serves declares viewport and scissor as dynamic state.
        let set_viewport = |w: u32, h: u32| unsafe {
            device.cmd_set_viewport(
                cmd,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: w as f32,
                    height: h as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            device.cmd_set_scissor(
                cmd,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D::default(),
                    extent: vk::Extent2D {
                        width: w,
                        height: h,
                    },
                }],
            );
        };
        // SAFETY: the closure is called only in `render` while `cmd` is recording, and `set` is
        // a live set made for set 1 of `layouts.scene`.
        let bind_pass_set = |set: vk::DescriptorSet| unsafe {
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.layouts.scene,
                1,
                &[set],
                &[],
            );
        };
        // SAFETY: the closure is called only in `render` while `cmd` is recording; the 8 bytes
        // fit the layout's 16-byte vertex+fragment push range.
        let push = |a: u32, b: u32| unsafe {
            device.cmd_push_constants(cmd, self.layouts.scene, gfx, 0, bytemuck::bytes_of(&[a, b]))
        };
        let terrain = terrain_lit::TerrainDraw {
            gpu: &self.gpu,
            cmd,
            nodes_set: self.nodes_set,
            grid_vb: &self.grid_vb,
            grid_ib: &self.grid_ib,
            index_count: self.grid_index_count,
            node_count,
            build_grid: input.build_grid,
        };
        let draw_terrain =
            |pipeline, pass_kind| terrain.draw(pipeline, self.layouts.scene, pass_kind, None);
        let draw_entities = |pipelines, pass_kind, list| {
            self.draw_entities(cmd, pipelines, pass_kind, list);
        };
        // The hull passes: only the draw slots of models wearing a hull field.
        // SAFETY: the closure is called only inside a hull render pass of `render` while `cmd`
        // is recording; every slot in `hull_draws` is a model draw slot below `slot_count`, so
        // each 20-byte command read lies inside `commands`.
        let draw_hull_slots = || unsafe {
            if self.hull_draws.is_empty() {
                device.cmd_draw_indexed_indirect(
                    cmd,
                    self.cull.commands.buffer,
                    0,
                    model_slots,
                    20,
                );
            }
            for &slot in &self.hull_draws {
                device.cmd_draw_indexed_indirect(
                    cmd,
                    self.cull.commands.buffer,
                    slot as u64 * 20,
                    1,
                    20,
                );
            }
        };
        // SAFETY: the closure is called only inside a render pass of `render` while `cmd` is
        // recording, after set 0 is bound; the quad buffers are live and hold the 6 indices
        // drawn.
        let draw_quads = |pipeline: vk::Pipeline, set: vk::DescriptorSet, instances: u32| unsafe {
            if instances == 0 {
                return;
            }
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
            bind_pass_set(set);
            device.cmd_bind_vertex_buffers(cmd, 0, &[self.quad_vb.buffer], &[0]);
            device.cmd_bind_index_buffer(cmd, self.quad_ib.buffer, 0, vk::IndexType::UINT32);
            device.cmd_draw_indexed(cmd, 6, instances, 0, 0, 0);
        };

        // Shadow pass, once per cascade. Always begun so every layer ends up in its
        // sampled layout.
        self.timers.scope(&device, cmd, "shadow");
        for (cascade, &shadow_fb) in self.shadow_fbs.iter().enumerate() {
            let clear = [vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            }];
            let begin = vk::RenderPassBeginInfo::default()
                .render_pass(self.passes.shadow)
                .framebuffer(shadow_fb)
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D::default(),
                    extent: vk::Extent2D {
                        width: SHADOW_SIZE,
                        height: SHADOW_SIZE,
                    },
                })
                .clear_values(&clear);
            // SAFETY: `cmd` is recording and outside a render pass; the shadow framebuffer was
            // made for `passes.shadow` at `SHADOW_SIZE`, the render area, and `begin` and
            // `clear` live to the end of the call.
            unsafe { device.cmd_begin_render_pass(cmd, &begin, vk::SubpassContents::INLINE) };
            self.timers.draws(
                &device,
                cmd,
                SHADOW_SCOPES[cascade.min(SHADOW_SCOPES.len() - 1)],
            );
            if shadow_strength > 0.0 {
                // Shaders read the cascade from above the pass kind's low byte.
                let kind = pass::SHADOW | (cascade as u32) << pass::CASCADE_SHIFT;
                set_viewport(SHADOW_SIZE, SHADOW_SIZE);
                // SAFETY: `cmd` is recording inside the shadow pass; `scene_set` is live and
                // made for set 0 of `layouts.scene`.
                unsafe {
                    device.cmd_bind_descriptor_sets(
                        cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.layouts.scene,
                        0,
                        &[self.scene_set],
                        &[],
                    )
                };
                draw_terrain(self.pipelines.terrain_shadow, kind);
                self.foundations
                    .record(&self.gpu, cmd, self.layouts.scene, kind);
                draw_entities(
                    [self.pipelines.entity_shadow, self.pipelines.prop[2]],
                    kind,
                    cull_list::SHADOW + cascade as u32,
                );
            }
            self.timers.end(&device, cmd);
            // SAFETY: `cmd` is recording inside the shadow pass begun above in this loop turn.
            unsafe { device.cmd_end_render_pass(cmd) };
        }
        self.timers.end(&device, cmd);

        // Depth pre-pass: terrain and entities into the scene's depth, nothing
        // shaded. The scene pass then shades each pixel once (its test passes only
        // the surface in front), and GTAO reads the depth before it.
        // MERIDIAN_PREPASS=0 leaves it cleared, for A/B timings.
        self.timers.draws(&device, cmd, "depth_prepass");
        // SAFETY: `cmd` is recording and outside a render pass; `prepass_fb` was made for
        // `passes.shadow` at the scene size, which is the render area, and the draws are made
        // inside the pass it begins and ends.
        unsafe {
            let clear = [vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 0.0,
                    stencil: 0,
                },
            }];
            device.cmd_begin_render_pass(
                cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.passes.shadow)
                    .framebuffer(self.prepass_fb)
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D::default(),
                        extent: vk::Extent2D {
                            width: self.scene_width,
                            height: self.scene_height,
                        },
                    })
                    .clear_values(&clear),
                vk::SubpassContents::INLINE,
            );
            if self.prepass {
                set_viewport(self.scene_width, self.scene_height);
                device.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.layouts.scene,
                    0,
                    &[self.scene_set],
                    &[],
                );
                draw_terrain(self.pipelines.terrain_prepass, pass::MAIN);
                self.foundations
                    .record(&self.gpu, cmd, self.layouts.scene, pass::PREPASS);
                draw_entities(
                    [self.pipelines.entity_prepass, self.pipelines.prop[3]],
                    pass::PREPASS,
                    cull_list::PREPASS,
                );
            }
            device.cmd_end_render_pass(cmd);
        }
        self.timers.end(&device, cmd);
        self.timers.scope(&device, cmd, "gtao");
        self.gtao.record(&self.gpu, cmd);
        self.timers.end(&device, cmd);
        self.record_terrain_lit(&terrain);

        // Scene pass.
        self.timers.scope(&device, cmd, "scene");
        // SAFETY: `cmd` is recording and outside a render pass; `scene_fb` was made for
        // `passes.scene` at the scene size, the render area; every pipeline, set and buffer
        // bound below is this device's and live, and the pass is ended at the bottom of this
        // block.
        unsafe {
            let clear = [
                vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [0.55, 0.66, 0.8, 1.0],
                    },
                },
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 0.0,
                        stencil: 0,
                    },
                },
            ];
            let begin = vk::RenderPassBeginInfo::default()
                .render_pass(self.passes.scene)
                .framebuffer(self.scene_fb)
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D::default(),
                    extent: vk::Extent2D {
                        width: self.scene_width,
                        height: self.scene_height,
                    },
                })
                .clear_values(&clear);
            device.cmd_begin_render_pass(cmd, &begin, vk::SubpassContents::INLINE);
            set_viewport(self.scene_width, self.scene_height);
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.layouts.scene,
                0,
                &[self.scene_set],
                &[],
            );
            self.timers.draws(&device, cmd, "scene.terrain");
            self.draw_scene_terrain(&terrain);
            self.foundations
                .record(&self.gpu, cmd, self.layouts.scene, pass::MAIN);
            self.timers.end(&device, cmd);

            self.timers.draws(&device, cmd, "scene.decals");
            if self.stain_count() + self.pad_count + self.deposit_count > 0 {
                bind_pass_set(self.stains_set);
                device.cmd_bind_vertex_buffers(cmd, 0, &[self.patch_vb.buffer], &[0]);
                device.cmd_bind_index_buffer(cmd, self.patch_ib.buffer, 0, vk::IndexType::UINT32);
                // Ore fields are ground, under the foundations and the scorch marks.
                if self.deposit_count > 0 {
                    self.timers
                        .crumb(cmd, || format!("ore fields x{}", self.deposit_count));
                    device.cmd_bind_pipeline(
                        cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.pipelines.deposit,
                    );
                    device.cmd_draw_indexed(
                        cmd,
                        self.patch_index_count,
                        self.deposit_count,
                        0,
                        0,
                        self.deposit_first,
                    );
                }
                if self.pad_count > 0 {
                    self.timers
                        .crumb(cmd, || format!("pads x{}", self.pad_count));
                    device.cmd_bind_pipeline(
                        cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.pipelines.pad,
                    );
                    device.cmd_draw_indexed(
                        cmd,
                        self.patch_index_count,
                        self.pad_count,
                        0,
                        0,
                        self.stain_count(),
                    );
                }
                if self.stain_count() > 0 {
                    device.cmd_bind_pipeline(
                        cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.pipelines.stain,
                    );
                    let mut first = 0;
                    for (count, kind) in self.stain_runs.into_iter().zip(["scorch", "craters"]) {
                        if count > 0 {
                            self.timers
                                .crumb(cmd, || format!("stains: {kind} x{count}"));
                            device.cmd_draw_indexed(
                                cmd,
                                self.patch_index_count,
                                count,
                                0,
                                0,
                                first,
                            );
                        }
                        first += count;
                    }
                }
            }
            if self.track_count > 0 {
                self.timers
                    .crumb(cmd, || format!("tracks x{}", self.track_count));
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipelines.track,
                );
                bind_pass_set(self.stains_set);
                device.cmd_bind_vertex_buffers(cmd, 0, &[self.quad_vb.buffer], &[0]);
                device.cmd_bind_index_buffer(cmd, self.quad_ib.buffer, 0, vk::IndexType::UINT32);
                device.cmd_draw_indexed(cmd, 6, self.track_count, 0, 0, 0);
            }
            if self.prints.count > 0 {
                self.timers
                    .crumb(cmd, || format!("prints x{}", self.prints.count));
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipelines.print,
                );
                bind_pass_set(self.stains_set);
                device.cmd_draw(
                    cmd,
                    titan_fx::PRINT_VERTICES,
                    self.prints.count,
                    0,
                    MAX_TRACK_MARKS as u32,
                );
            }
            self.timers.end(&device, cmd);
            self.timers.draws(&device, cmd, "scene.grass");
            self.grass.draw(&self.gpu, cmd, self.scene_set, |band| {
                self.timers.crumb(cmd, || format!("grass band {band}"))
            });
            self.timers.end(&device, cmd);
            self.timers.draws(&device, cmd, "scene.entities");
            if self.prepass {
                // What the pre-pass drew is shaded only where it is the nearest; the
                // rest writes its own depth.
                draw_entities(
                    [self.pipelines.entity_over_prepass, self.pipelines.prop[1]],
                    pass::MAIN,
                    cull_list::PREPASS,
                );
                draw_entities(
                    [self.pipelines.entity, self.pipelines.prop[0]],
                    pass::MAIN,
                    cull_list::REST,
                );
            } else {
                draw_entities(
                    [self.pipelines.entity, self.pipelines.prop[0]],
                    pass::MAIN,
                    cull_list::MAIN,
                );
            }
            self.timers.end(&device, cmd);
            self.timers.draws(&device, cmd, "scene.missiles");
            self.timers
                .crumb(cmd, || format!("missiles x{}", self.projectile_count));
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.missile);
            bind_pass_set(self.sprites_set);
            // Eight-sided casing, nose, rear cap, four fins, and a cruise missile's wings.
            device.cmd_draw(cmd, 132, self.projectile_count, 0, 0);
            // Strategic missiles: a lathed body and four fins each, or a Regency body and its
            // pieces (nuke.wgsl `vs_strategic`, nova.wgsl).
            let (nuke_count, strategic_count) = (nuke_view[2] as u32, nuke_view[3] as u32);
            if strategic_count > 0 {
                self.timers
                    .crumb(cmd, || format!("strategic missiles x{strategic_count}"));
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipelines.nuke_missile,
                );
                device.cmd_draw(
                    cmd,
                    crate::gpu_consts::missile::VERTS,
                    strategic_count,
                    0,
                    0,
                );
            }

            self.timers.end(&device, cmd);

            // The sky wherever nothing else was drawn.
            self.timers.draws(&device, cmd, "scene.sky");
            self.sky.draw_sky(&self.gpu, cmd);
            self.timers.end(&device, cmd);

            // The sea reads what is under and around it: end the scene here,
            // copy it, and carry on drawing into the same targets.
            device.cmd_end_render_pass(cmd);
            self.timers.draws(&device, cmd, "scene.hull_depth");
            if self.hull_shield_count > 0 {
                // The hull fields' outermost skin, into their own depth target.
                let clear = [vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 0.0,
                        stencil: 0,
                    },
                }];
                device.cmd_begin_render_pass(
                    cmd,
                    &vk::RenderPassBeginInfo::default()
                        .render_pass(self.passes.shadow)
                        .framebuffer(self.hull_depth_fb)
                        .render_area(vk::Rect2D {
                            offset: vk::Offset2D::default(),
                            extent: vk::Extent2D {
                                width: self.scene_width,
                                height: self.scene_height,
                            },
                        })
                        .clear_values(&clear),
                    vk::SubpassContents::INLINE,
                );
                set_viewport(self.scene_width, self.scene_height);
                device.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.layouts.scene,
                    0,
                    &[self.scene_set],
                    &[],
                );
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipelines.hull_shield_depth,
                );
                bind_pass_set(self.shields_set);
                push(pass::HULL, self.shield_count);
                device.cmd_bind_vertex_buffers(cmd, 0, &[self.mesh_vb.buffer], &[0]);
                device.cmd_bind_index_buffer(cmd, self.mesh_ib.buffer, 0, vk::IndexType::UINT32);
                draw_hull_slots();
                device.cmd_end_render_pass(cmd);
            }
            self.timers.end(&device, cmd);
            // The clouds, at half size, stopped by the scene's depth.
            self.timers.draws(&device, cmd, "clouds.march");
            self.sky.record_march(&self.gpu, cmd, self.scene_set);
            self.timers.end(&device, cmd);
            self.timers.draws(&device, cmd, "shafts.march");
            self.shafts.record_march(&self.gpu, cmd, self.scene_set);
            self.timers.end(&device, cmd);
            self.timers.draws(&device, cmd, "nuke.march");
            if nuke_view[2] > 0.0 {
                self.nuke_volume.record_march(
                    &self.gpu,
                    cmd,
                    self.scene_set,
                    self.sky.history_index(),
                );
            }
            self.timers.end(&device, cmd);
            self.timers.draws(&device, cmd, "scene.refract_copy");
            set_viewport(self.scene_width, self.scene_height);
            let area = vk::Rect2D {
                offset: vk::Offset2D::default(),
                extent: vk::Extent2D {
                    width: self.scene_width,
                    height: self.scene_height,
                },
            };
            device.cmd_begin_render_pass(
                cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.passes.bloom_down)
                    .framebuffer(self.refract_fb)
                    .render_area(area),
                vk::SubpassContents::INLINE,
            );
            device.cmd_bind_pipeline(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipelines.refract_copy,
            );
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.layouts.screen,
                0,
                &[self.hdr_set],
                &[],
            );
            device.cmd_draw(cmd, 3, 1, 0, 0);
            device.cmd_end_render_pass(cmd);
            self.timers.end(&device, cmd);
            device.cmd_begin_render_pass(
                cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.passes.scene_over)
                    .framebuffer(self.scene_fb)
                    .render_area(area),
                vk::SubpassContents::INLINE,
            );
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.layouts.water,
                0,
                &[self.scene_set, self.water_fx.set, self.water_set],
                &[],
            );
            // The copy's layout left push constants undefined; restore the entities' values.
            push(0, self.shield_count);
            self.timers.draws(&device, cmd, "scene.water");
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.water);
            device.cmd_draw(cmd, 6, 1, 0, 0);
            self.timers.end(&device, cmd);

            // Shockwaves after the sea: drawn before it, the surface painted over
            // every front that crossed open water.
            self.timers.draws(&device, cmd, "scene.shockwaves");
            device.cmd_bind_pipeline(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipelines.shockwave,
            );
            bind_pass_set(self.shockwaves_set);
            device.cmd_draw(
                cmd,
                SHOCKWAVE_LAT * SHOCKWAVE_LON * 6,
                MAX_SHOCKWAVES as u32,
                0,
                0,
            );
            self.timers.end(&device, cmd);

            // Cone weapons' wakes, shells of light added onto the scene (wake_shell.wgsl).
            self.timers.draws(&device, cmd, "scene.wake_shells");
            self.wake_shells.draw(&device, cmd, self.layouts.scene);
            self.timers.end(&device, cmd);

            // The climate walls' curtain (curtain.wgsl): a quad a segment of wall.
            let segments = self.look.walls().segments().len() as u32;
            if segments > 0 {
                self.timers.draws(&device, cmd, "scene.curtain");
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipelines.curtain,
                );
                device.cmd_draw(cmd, segments * 6, 1, 0, 0);
                self.timers.end(&device, cmd);
            }

            self.timers.draws(&device, cmd, "scene.rings");
            if ranges_long + ranges_short > 0 {
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipelines.range,
                );
                bind_pass_set(self.ranges_set);
                device.cmd_bind_index_buffer(cmd, self.range_ib.buffer, 0, vk::IndexType::UINT32);
                // Four instances a ring: its reach, its dead zone, and a part ring's two edges.
                for (first, count, segments) in [
                    (0, ranges_long, RANGE_SEGMENTS[0]),
                    (ranges_long, ranges_short, RANGE_SEGMENTS[1]),
                ] {
                    if count > 0 {
                        push(ranges.len() as u32, segments);
                        device.cmd_draw_indexed(cmd, segments * 6, count * 4, 0, 0, first * 4);
                    }
                }
            }
            // A grid of cells a mark, so it lies over the ground (icons.wgsl `vs_ring`).
            let ring_cells = crate::gpu_consts::ring::GRID * crate::gpu_consts::ring::GRID;
            draw_quads(
                self.pipelines.ring,
                self.marks_set,
                ringed_count * ring_cells,
            );
            self.timers.end(&device, cmd);
            self.timers.draws(&device, cmd, "scene.shields");
            if self.shield_count > self.hull_shield_count {
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipelines.shield,
                );
                bind_pass_set(self.shields_set);
                // One fullscreen triangle traces every dome. Instancing a cube
                // per bubble re-solved the same union on overlapping pixels.
                push(self.shield_count, 0);
                device.cmd_draw(cmd, 3, 1, 0, 0);
            }
            self.timers.end(&device, cmd);
            self.timers.draws(&device, cmd, "scene.puffs");
            draw_quads(self.pipelines.puff, self.puffs_set, MAX_PUFFS as u32);
            self.timers.end(&device, cmd);
            self.timers.draws(&device, cmd, "scene.beams");
            draw_quads(
                self.pipelines.beam,
                self.puffs_set,
                self.work_beams.count * crate::gpu_consts::beam::QUADS,
            );
            self.timers.end(&device, cmd);
            self.timers.draws(&device, cmd, "scene.shots");
            push(sprite_layer::UNDER_CLOUD, 0);
            draw_quads(
                self.pipelines.projectile,
                self.sprites_set,
                self.projectile_count,
            );
            draw_quads(self.pipelines.effect, self.sprites_set, MAX_EFFECTS as u32);
            // After the glow, so a shot's dot is not washed out by its own tracer.
            draw_quads(self.pipelines.shot, self.sprites_set, self.projectile_count);
            if strategic_count > 0 {
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipelines.nuke_plume,
                );
                // nuke.wgsl PLUME_VERTS: a lathed hull round each plume.
                device.cmd_draw(cmd, 10 * 16 * 6 + 16 * 3, strategic_count, 0, 0);
            }
            self.timers.end(&device, cmd);
            // Hull fields last, so their glow lies over the effects inside them.
            self.timers.draws(&device, cmd, "scene.hull_shields");
            if self.hull_shield_count > 0 {
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipelines.hull_shield,
                );
                device.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.layouts.water,
                    1,
                    &[self.shields_set, self.hull_set],
                    &[],
                );
                push(pass::HULL, self.shield_count);
                device.cmd_bind_vertex_buffers(cmd, 0, &[self.mesh_vb.buffer], &[0]);
                device.cmd_bind_index_buffer(cmd, self.mesh_ib.buffer, 0, vk::IndexType::UINT32);
                draw_hull_slots();
            }

            self.timers.end(&device, cmd);
            // Ore veins glow through the ground, and through trees and units,
            // while the mine survey is up.
            self.timers.draws(&device, cmd, "scene.veins");
            if self.vein_count > 0 && self.ore_highlight > 0.01 {
                // Set 0 (scene) is bound already; the veins read nothing else.
                device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.vein);
                device.cmd_bind_vertex_buffers(cmd, 0, &[self.vein_vb.buffer], &[0]);
                device.cmd_push_constants(
                    cmd,
                    self.layouts.scene,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    bytemuck::bytes_of(&[self.ore_highlight, self.vein_time]),
                );
                device.cmd_draw(cmd, self.vein_count, 1, 0, 0);
            }
            self.timers.end(&device, cmd);
            // Light shafts: the haze's sunlight taken back where the air is shadowed
            // (shafts.wgsl), before the clouds cover the far view.
            if self.shafts.enabled {
                self.timers.draws(&device, cmd, "scene.shafts");
                self.shafts.draw_composite(&self.gpu, cmd, self.scene_set);
                self.timers.end(&device, cmd);
            }
            // The clouds over everything in the world, then the rain close up: from under the
            // deck the drops are nearer than any cloud (fs_rain lets the clouds cover them
            // only seen from over the deck). Icons and bars stay on top.
            self.timers.draws(&device, cmd, "clouds.composite");
            self.sky.draw_composite(&self.gpu, cmd, self.scene_set);
            self.timers.end(&device, cmd);
            self.timers.draws(&device, cmd, "scene.rain");
            self.sky.draw_rain(&self.gpu, cmd, self.scene_set);
            self.timers.end(&device, cmd);
            self.timers.draws(&device, cmd, "nuke.composite");
            // Nuclear blasts over the clouds they tear through, marched at half size
            // against the scene's depth (nuke_volume.rs, nuke.wgsl).
            if nuke_count > 0 {
                self.nuke_volume
                    .draw_composite(&self.gpu, cmd, self.scene_set);
            }

            self.timers.end(&device, cmd);

            // The shots again over the clouds: from strategic height they are yellow
            // markers like the icons, and a cloud deck would hide the whole fight's fire.
            // The cloud and nuke composites bound set 0 under their own layouts.
            self.timers.draws(&device, cmd, "scene.shot_marks");
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.layouts.scene,
                0,
                &[self.scene_set],
                &[],
            );
            push(sprite_layer::OVER_CLOUD, 0);
            draw_quads(
                self.pipelines.projectile,
                self.sprites_set,
                self.projectile_count,
            );
            draw_quads(self.pipelines.shot, self.sprites_set, self.projectile_count);
            self.timers.end(&device, cmd);

            // Strategic icons: the cull pass's last draw slot, as quads.
            self.timers.draws(&device, cmd, "scene.icons");
            if input.icons {
                device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.icon);
                device.cmd_bind_vertex_buffers(cmd, 0, &[self.quad_vb.buffer], &[0]);
                device.cmd_bind_index_buffer(cmd, self.mesh_ib.buffer, 0, vk::IndexType::UINT32);
                device.cmd_draw_indexed_indirect(
                    cmd,
                    self.cull.commands.buffer,
                    model_slots as u64 * 20,
                    1,
                    20,
                );
            }
            draw_quads(self.pipelines.bar, self.marks_set, marks.len() as u32);
            self.timers.end(&device, cmd);
            device.cmd_end_render_pass(cmd);
        }
        self.timers.end(&device, cmd);

        // Bloom: down the chain from the scene, then back up it, each level added onto the next larger one.
        self.timers.draws(&device, cmd, "bloom");
        // SAFETY: `cmd` is recording and outside a render pass; each bloom framebuffer was made
        // for the bloom passes at its image's size, which is the render area, and each pass
        // begun here is ended in `level_pass`.
        unsafe {
            device.cmd_bind_pipeline(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipelines.bloom_down,
            );
            let level_pass = |pass: vk::RenderPass,
                              level: usize,
                              source: vk::DescriptorSet,
                              a: [f32; 2]| {
                let image = &self.bloom[level];
                let begin = vk::RenderPassBeginInfo::default()
                    .render_pass(pass)
                    .framebuffer(self.bloom_fbs[level])
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D::default(),
                        extent: vk::Extent2D {
                            width: image.width,
                            height: image.height,
                        },
                    });
                device.cmd_begin_render_pass(cmd, &begin, vk::SubpassContents::INLINE);
                set_viewport(image.width, image.height);
                device.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.layouts.screen,
                    0,
                    &[source],
                    &[],
                );
                device.cmd_push_constants(cmd, self.layouts.screen, gfx, 0, bytemuck::bytes_of(&a));
                device.cmd_draw(cmd, 3, 1, 0, 0);
                device.cmd_end_render_pass(cmd);
            };
            for level in 0..BLOOM_LEVELS {
                let source = if level == 0 {
                    self.hdr_set
                } else {
                    self.bloom_sets[level - 1]
                };
                level_pass(
                    self.passes.bloom_down,
                    level,
                    source,
                    [(level == 0) as u32 as f32, 0.0],
                );
            }
            device.cmd_bind_pipeline(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipelines.bloom_up,
            );
            for level in (1..BLOOM_LEVELS).rev() {
                level_pass(
                    self.passes.bloom_up,
                    level - 1,
                    self.bloom_sets[level],
                    [1.0, 0.85],
                );
            }
        }

        self.timers.end(&device, cmd);

        // Overlay glass: the tone-mapped picture at quarter size, blurred across, then down.
        self.timers.draws(&device, cmd, "glass");
        if glass {
            // SAFETY: `cmd` is recording and outside a render pass; the glass framebuffers were
            // made for `passes.bloom_down` at `w`x`h`, and each pass begun in `glass_pass` is
            // ended there.
            unsafe {
                let (w, h) = (self.glass[0].width, self.glass[0].height);
                let glass_pass = |pipeline: vk::Pipeline,
                                  target: usize,
                                  source: vk::DescriptorSet,
                                  a: [f32; 2]| {
                    let begin = vk::RenderPassBeginInfo::default()
                        .render_pass(self.passes.bloom_down)
                        .framebuffer(self.glass_fbs[target])
                        .render_area(vk::Rect2D {
                            offset: vk::Offset2D::default(),
                            extent: vk::Extent2D {
                                width: w,
                                height: h,
                            },
                        });
                    device.cmd_begin_render_pass(cmd, &begin, vk::SubpassContents::INLINE);
                    set_viewport(w, h);
                    device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
                    device.cmd_bind_descriptor_sets(
                        cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.layouts.screen,
                        0,
                        &[source],
                        &[],
                    );
                    device.cmd_push_constants(
                        cmd,
                        self.layouts.screen,
                        gfx,
                        0,
                        bytemuck::bytes_of(&a),
                    );
                    device.cmd_draw(cmd, 3, 1, 0, 0);
                    device.cmd_end_render_pass(cmd);
                };
                glass_pass(self.pipelines.glass_source, 0, self.screen_set, TONEMAP);
                glass_pass(self.pipelines.glass_blur, 1, self.glass_sets[0], [1.0, 0.0]);
                glass_pass(self.pipelines.glass_blur, 0, self.glass_sets[1], [0.0, 1.0]);
            }
        }
        self.timers.end(&device, cmd);

        // The tone map's exposure and vignette, then which shockwave slots are live
        // (bits, as two words): `wave_bend` visits only those.
        let live_waves = self
            .shockwave_ends
            .iter()
            .enumerate()
            .filter(|(_, &end)| input.time < end)
            .fold(0u64, |mask, (i, _)| mask | 1 << i);
        let tonemap_push = [
            TONEMAP[0],
            TONEMAP[1],
            f32::from_bits(live_waves as u32),
            f32::from_bits((live_waves >> 32) as u32),
        ];
        // With SMAA or FSR, the tone map writes an image of its own and they
        // work on that (post.rs); the swapchain pass then draws their result.
        if self.post.active() {
            self.timers.scope(&device, cmd, "post");
            self.post.record(
                &self.gpu,
                cmd,
                self.layouts.screen,
                self.screen_set,
                tonemap_push,
            );
            self.timers.end(&device, cmd);
        }

        // Tone map to the output, then the UI on top.
        self.timers.scope(&device, cmd, "present");
        // SAFETY: `cmd` is recording and outside a render pass; `image_index` came from this
        // frame's acquire (or is 0 headless), so it indexes `present_fbs`, made for
        // `passes.present` at the output size; the pass is ended at the bottom of this block.
        unsafe {
            let begin = vk::RenderPassBeginInfo::default()
                .render_pass(self.passes.present)
                .framebuffer(self.present_fbs[image_index])
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D::default(),
                    extent: vk::Extent2D {
                        width: self.width,
                        height: self.height,
                    },
                });
            device.cmd_begin_render_pass(cmd, &begin, vk::SubpassContents::INLINE);
            set_viewport(self.width, self.height);
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.layouts.screen,
                0,
                &[self.screen_set],
                &[],
            );
            self.timers.draws(&device, cmd, "present.tonemap");
            if self.post.active() {
                self.post.draw_present(&self.gpu, cmd);
                // The overlay reads the screen set again (the post layout replaced it).
                device.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.layouts.screen,
                    0,
                    &[self.screen_set],
                    &[],
                );
            } else {
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipelines.tonemap,
                );
                device.cmd_push_constants(
                    cmd,
                    self.layouts.screen,
                    gfx,
                    0,
                    bytemuck::bytes_of(&tonemap_push),
                );
                device.cmd_draw(cmd, 3, 1, 0, 0);
            }
            self.timers.end(&device, cmd);
            self.timers.draws(&device, cmd, "present.ui");
            if !overlay.is_empty() {
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipelines.overlay,
                );
                if glass {
                    // The blurred picture where the tone mapper had the scene; same atlas.
                    device.cmd_bind_descriptor_sets(
                        cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.layouts.screen,
                        0,
                        &[self.glass_sets[0]],
                        &[],
                    );
                }
                device.cmd_push_constants(
                    cmd,
                    self.layouts.screen,
                    gfx,
                    0,
                    bytemuck::bytes_of(&[2.0 / self.width as f32, 2.0 / self.height as f32]),
                );
                device.cmd_bind_vertex_buffers(cmd, 0, &[self.overlay_vb.buffer], &[0]);
                device.cmd_draw(cmd, overlay.len() as u32, 1, 0, 0);
            }
            self.timers.end(&device, cmd);
            device.cmd_end_render_pass(cmd);
        }
        self.timers.end(&device, cmd);

        if let Output::Headless { image, readback } = &self.output {
            // SAFETY: `cmd` is recording and outside a render pass; the present pass leaves the
            // headless image in TRANSFER_SRC_OPTIMAL (its final layout), the image has
            // TRANSFER_SRC usage, and `readback` (TRANSFER_DST) holds `width * height * 4`
            // bytes.
            unsafe {
                let copy = [vk::BufferImageCopy::default()
                    .image_subresource(vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        mip_level: 0,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .image_extent(vk::Extent3D {
                        width: self.width,
                        height: self.height,
                        depth: 1,
                    })];
                device.cmd_copy_image_to_buffer(
                    cmd,
                    image.image,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    readback.buffer,
                    &copy,
                );
            }
        }

        if let (Output::Window(sc), true) = (&self.output, self.capture.wanted()) {
            let swapchain_fn = self.gpu.swapchain_fn.as_ref().expect("window target");
            // SAFETY: the surface and chain are this device's and alive.
            let (caps, images) = unsafe {
                (
                    self.gpu
                        .surface_fn
                        .get_physical_device_surface_capabilities(self.gpu.physical, sc.surface)?,
                    swapchain_fn.get_swapchain_images(sc.swapchain)?,
                )
            };
            // `swapchain::rebuild` asked for TRANSFER_SRC wherever the surface offers it.
            if !caps
                .supported_usage_flags
                .contains(vk::ImageUsageFlags::TRANSFER_SRC)
            {
                self.capture.refuse();
            }
            self.capture.record(
                &self.gpu,
                cmd,
                images[image_index],
                (self.width, self.height),
                self.present_format,
            )?;
        }

        // ---- Submit -----------------------------------------------------------
        // SAFETY: `cmd` holds a complete recording; the fence was reset above and is not in
        // use, the semaphores are this device's (waited/signalled only for a window, where the
        // acquire signalled `image_available`), and the queue is used only from this thread.
        unsafe {
            device.end_command_buffer(cmd)?;
            let cmds = [cmd];
            let wait = [self.image_available];
            let signal = [self.render_finished];
            let stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let mut submit = vk::SubmitInfo::default().command_buffers(&cmds);
            if matches!(self.output, Output::Window(_)) {
                submit = submit
                    .wait_semaphores(&wait)
                    .wait_dst_stage_mask(&stages)
                    .signal_semaphores(&signal);
            }
            self.timers.step("submitting the frame");
            device.queue_submit(self.gpu.queue, &[submit], self.fence)?;
        }
        self.timers.submitted();

        if let Output::Window(sc) = &self.output {
            let swapchain_fn = self.gpu.swapchain_fn.as_ref().expect("window target");
            let wait = [self.render_finished];
            let swapchains = [sc.swapchain];
            let indices = [image_index as u32];
            let present = vk::PresentInfoKHR::default()
                .wait_semaphores(&wait)
                .swapchains(&swapchains)
                .image_indices(&indices);
            self.timers.step("presenting");
            // SAFETY: `image_index` was acquired from this chain this frame, and
            // `render_finished` is signalled by the submit just made; the arrays in `present`
            // live to the end of the call.
            match unsafe { swapchain_fn.queue_present(self.gpu.queue, &present) } {
                Ok(false) => {}
                Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    self.create_size_dependent()?
                }
                Err(e) => return Err(e.into()),
            }
        }

        self.stats.terrain_nodes = self.node_scratch.len();
        self.stats.dynamic_entities = self.dynamic_count as usize;
        self.stats.static_entities = self.static_count as usize;
        Ok(true)
    }
}
