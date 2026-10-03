//! Core mines put down with the mouse: one placed in the reach of a mine the side or
//! an ally has planned is let through, with a notice, since whichever of the two is
//! begun second will be refused (`orders::mine_reach`).

use super::Game;
use crate::ui::palette;
use glam::Vec2;
use mc_core::FxVec2;
use mc_data::BlueprintId;

impl Game {
    /// Says so when any of the mines just placed at `sites` overlaps a planned mine.
    pub(super) fn warn_of_planned_mines(&mut self, blueprint: BlueprintId, sites: &[FxVec2]) {
        let overlap = sites.iter().find_map(|at| {
            crate::orders::mine_reach::planned(
                &self.view,
                &self.blueprints,
                blueprint,
                Vec2::from(at.to_f32()),
                None,
            )
        });
        if let Some(c) = overlap {
            let whose = crate::hud::whose_mine(&self.view, c.owner);
            self.hud.toast(
                format!("Overlaps {whose} planned mine: the second begun is refused"),
                palette::WARN,
            );
        }
    }
}
