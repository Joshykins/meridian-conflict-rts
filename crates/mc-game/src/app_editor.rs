use super::*;
impl App {
    pub(super) fn reload_editor(&mut self) -> Result<(), String> {
        if self.stage_is_editor_reload_requested() {
            let target = self.window_target()?;
            if let Stage::Editor(editor) = &mut self.stage {
                editor.begin_reload(target);
            }
        }
        let Stage::Editor(editor) = &mut self.stage else {
            return Ok(());
        };
        let Some(reloaded) = editor.poll_reload() else {
            return Ok(());
        };
        if let Some(renderer) = self.renderer.as_mut() {
            editor.install_reload(reloaded, renderer, &mut self.settings, &self.audio, true)?;
            self.args.blueprints = editor.current_blueprints();
        }
        Ok(())
    }
    fn stage_is_editor_reload_requested(&self) -> bool {
        matches!(&self.stage,Stage::Editor(e) if e.reload_requested())
    }
}

impl App {
    /// Test/Stop keeps the window and Vulkan renderer. Only the local match
    /// changes; the authored document and its undo history stay with the editor.
    pub(super) fn editor_transition(
        &mut self,
        pending: Pending,
    ) -> Result<Option<Pending>, String> {
        match pending {
            Pending::Match(start) if matches!(self.stage, Stage::Editor(_)) => {
                let Stage::Editor(editor) = &mut self.stage else {
                    unreachable!()
                };
                let renderer = self.renderer.as_mut().ok_or("no editor renderer")?;
                editor.prepare_test_renderer(&start.map, renderer)?;
                let camera = editor.camera.clone();
                let mut game = Game::new(
                    *start,
                    self.args.blueprints.clone(),
                    self.args.pool.clone(),
                    self.viewport(),
                    self.settings.show_profiler,
                );
                game.editor_camera(camera);
                let previous = std::mem::replace(&mut self.stage, Stage::Match(Box::new(game)));
                if let Stage::Editor(editor) = previous {
                    self.editor_return = Some(editor);
                }
                self.stage_since = Instant::now();
                self.reveal = 1.0;
                self.curtain = None;
                self.memory = Default::default();
                Ok(None)
            }
            Pending::Editor(mut editor) if matches!(self.stage, Stage::Match(_)) => {
                self.audio.release_world();
                editor.camera.viewport = self.viewport();
                editor.restore(self.renderer.as_mut().ok_or("no playtest renderer")?)?;
                self.stage = Stage::Editor(editor);
                self.stage_since = Instant::now();
                self.reveal = 1.0;
                self.curtain = None;
                self.memory = Default::default();
                Ok(None)
            }
            pending => Ok(Some(pending)),
        }
    }
}
