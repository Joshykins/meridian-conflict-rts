//! Apply saved preferences to the live menu or match.

use super::*;

impl App {
    pub(super) fn apply_render_quality(&mut self) -> Result<(), String> {
        if let Some(r) = &mut self.renderer {
            self.settings
                .apply_graphics(r)
                .map_err(|e| format!("could not change graphics quality: {e}"))?;
        }
        Ok(())
    }

    pub(super) fn apply_settings(&mut self, display: bool) -> Result<(), String> {
        self.audio.set_volumes(self.settings.volumes());
        self.audio
            .set_music_volume(self.settings.master_volume * self.settings.music_volume);
        if display {
            if let Some(w) = &self.window {
                w.set_fullscreen(
                    self.settings
                        .fullscreen
                        .then_some(Fullscreen::Borderless(None)),
                );
            }
            self.apply_render_quality()?;
            // Vertical sync is a property of the swapchain: rebuild the renderer around it.
            let vsync = self.settings.vsync && !self.args.force_no_vsync;
            if let (Stage::Front(f), true) = (&self.stage, vsync != self.applied_vsync) {
                let map = f.map.clone();
                self.build_renderer(&map, setup::TEAM_COLORS)?;
                if let Stage::Front(f) = &mut self.stage {
                    f.serial = 0;
                }
            }
        }
        self.settings.save();
        Ok(())
    }
}
