//! The Reclaim order given with the mouse: a click on a wreck or a unit takes it apart,
//! a click on open ground sends the reclaimers there clearing the wrecks on the way,
//! and a press dragged out clears every wreck in the circle (`mc_sim`'s `reclaim_area.rs`).

use super::{Game, Mode, Targeting, DRAG_THRESHOLD};
use crate::audio::{Audio, Sfx};
use glam::Vec2;
use mc_core::{Fx, FxVec2};
use mc_render::Renderer;
use mc_sim::Command;

impl Game {
    /// The left button let go with Reclaim armed, pressed at `from`.
    pub(super) fn reclaim_released(&mut self, from: Vec2, r: &Renderer, audio: &Audio) {
        let centre = self.view.circle_from.take();
        let ground = self.ground_under_cursor(r).map(|g| g.truncate());
        let dragged = from.distance(self.cursor) >= DRAG_THRESHOLD;
        let (Some(centre), Some(edge), true) = (centre, ground, dragged) else {
            let unit = self.unit_at(self.cursor);
            self.targeted_order(Targeting::Reclaim, ground, unit, audio);
            return;
        };
        audio.play(Sfx::Order);
        self.send(Command::ReclaimArea {
            units: self.selected_ids(),
            pos: FxVec2::new(Fx::from_f32(centre.x), Fx::from_f32(centre.y)),
            radius: Fx::from_f32(crate::orders::reclaim_radius(centre.distance(edge))),
            queue: self.shift,
        });
        if !self.shift {
            self.view.mode = Mode::Normal;
        }
    }
}
