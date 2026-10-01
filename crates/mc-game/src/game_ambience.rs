//! The match's side of the ambient sound (`ambience.rs`): what the camera, the
//! sky and the last tick give it.

use super::Game;
use crate::ambience::Cues;
use crate::audio::Audio;
use mc_render::Renderer;

impl Game {
    /// Moves the living world's sound on by a frame. The battle it ducks under is
    /// handed over as it is played (`battle_sounds`, `nuke_sounds`).
    pub(super) fn ambience_frame(&mut self, renderer: &Renderer, audio: &Audio, dt: f32) {
        let camera = &self.camera;
        let (darkness, wind, climate) = renderer.ambience_cues(camera.focus.truncate());
        let cues = Cues {
            focus: camera.focus,
            distance: camera.distance,
            yaw: camera.yaw,
            darkness,
            wind,
            rain: self.rain_here,
            tropical: climate == mc_data::weather::Climate::Tropical,
            desert: climate == mc_data::weather::Climate::Desert,
            sea: self.map.info().water_level.to_f32(),
            clock: renderer.time(),
        };
        let ground = |xy| renderer.ground_height(xy);
        self.ambience.frame(&self.map, &cues, &ground, audio, dt);
    }
}
