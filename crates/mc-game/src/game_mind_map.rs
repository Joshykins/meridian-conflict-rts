//! An observer's map of a Commander's zones (`mind_map.rs`): on while the AI Mind
//! card's switch says so and an AI side is watched.

use super::Game;
use crate::mind_map::{self, MapMode};
use crate::orders::Field;
use crate::ui::Ui;
use mc_render::Renderer;

impl Game {
    /// The side whose map is drawn, when it is on.
    fn mind_map_side(&self) -> Option<u8> {
        let on = self.hud.mind_view.map != MapMode::Off;
        let who = self.view.perspective?;
        let ai = self
            .view
            .status
            .players
            .get(who as usize)
            .is_some_and(|p| p.mind.is_some());
        (on && self.view.observing && ai).then_some(who)
    }

    /// Whether the sim should publish the zones and their truth.
    pub(super) fn mind_map_on(&self) -> bool {
        self.mind_map_side().is_some()
    }

    pub(super) fn draw_mind_map(&self, ui: &mut Ui, field: &Field, renderer: &Renderer) {
        let Some(who) = self.mind_map_side() else {
            return;
        };
        let mode = self.hud.mind_view.map;
        let cursor = self.ground_under_cursor(renderer);
        if let Some(z) = mind_map::draw(ui, field, mode, who, cursor) {
            mind_map::card(ui, field, mode, who, z, self.cursor / ui.s);
        }
    }
}
