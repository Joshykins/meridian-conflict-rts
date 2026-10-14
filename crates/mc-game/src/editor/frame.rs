use super::*;
impl Editor {
    pub fn frame(&mut self, ctx: FrameCtx) -> Result<Option<Event>, String> {
        let FrameCtx {
            renderer,
            overlay,
            input,
            memory,
            audio,
            settings,
            dt,
            time,
            cover,
            ..
        } = ctx;
        let mut event = self.api_frame(renderer);
        if memory.editing.is_none() {
            let axis =
                |a, b| (self.held.contains(&a) as i32 - self.held.contains(&b) as i32) as f32;
            self.camera.pan(
                Vec2::new(
                    axis(KeyCode::KeyD, KeyCode::KeyA),
                    axis(KeyCode::KeyS, KeyCode::KeyW),
                ) * dt
                    * 600.0,
            );
            self.camera
                .orbit(axis(KeyCode::KeyE, KeyCode::KeyQ) * dt, 0.0);
        }
        overlay.clear();
        let mut ui = Ui::new(
            overlay,
            input,
            memory,
            audio,
            self.camera.viewport,
            settings.ui_scale,
            time,
            dt,
        );
        let browsing = self.browser.as_ref().is_some_and(|b| b.browser.is_open());
        ui.interactive = !browsing && !self.confirm_exit && !self.reloading();
        let bottom = ui.size.y - (144.0 + Self::tool_rows(ui.size.x) as f32 * 44.0);
        self.top_bar(&mut ui, renderer, &mut event);
        self.tools_panel(&mut ui, bottom);
        let card = self.properties(&mut ui, renderer);
        let outliner = self.outliner(&mut ui, bottom);
        if !browsing && !self.reloading() {
            self.world_input(&mut ui, renderer, bottom, card, outliner);
        }
        self.draw_regions(&mut ui, renderer);
        ui.interactive = true;
        self.exit_dialog(&mut ui, &mut event);
        if let Some(open) = self.browser_frame(&mut ui) {
            event = Some(open);
        }
        if let Some(c) = cover {
            c.draw(&mut ui);
        }
        renderer
            .render(&FrameInput {
                camera: &self.camera,
                time,
                alpha: 1.0,
                sim: self.fresh.then_some(&self.frame),
                ghosts: &[],
                marks: &[],
                ranges: &[],
                ranges_drawn: 0,
                overlay,
                build_grid: true,
                icons: true,
            })
            .map_err(|e| e.to_string())?;
        self.fresh = false;
        Ok(event)
    }
}
