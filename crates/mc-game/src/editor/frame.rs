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
        if !input.down && self.doc.stroking() {
            if let Err(error) = self.finish_stroke(renderer) {
                self.message = error;
            }
        }
        let mut event = self.api_frame(renderer, None);
        if self.catalogue.is_none() && memory.editing == Some(id("editor_catalogue_search", 0)) {
            memory.editing = None;
        }
        if memory.editing.is_none() && self.catalogue.is_none() && !self.confirm_exit {
            self.navigation.update(&mut self.camera, &self.held, dt);
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
        self.draw_regions(&mut ui, renderer);
        self.draw_gizmo(&mut ui, renderer);
        let browsing = self.browser.as_ref().is_some_and(|b| b.browser.is_open());
        ui.interactive =
            !browsing && self.catalogue.is_none() && !self.confirm_exit && !self.reloading();
        let bottom = ui.size.y - (144.0 + Self::tool_rows(ui.size.x) as f32 * 44.0);
        self.top_bar(&mut ui, renderer, &mut event);
        self.tools_panel(&mut ui, bottom);
        let card = self.properties(&mut ui, renderer);
        let outliner = self.outliner(&mut ui, bottom);
        if !browsing && self.catalogue.is_none() && !self.reloading() {
            self.world_input(&mut ui, renderer, bottom, card, outliner);
        }
        ui.interactive = true;
        self.catalogue_frame(&mut ui);
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
