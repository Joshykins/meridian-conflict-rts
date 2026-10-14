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
