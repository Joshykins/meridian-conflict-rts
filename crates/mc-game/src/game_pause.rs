//! The selection's work switches: Pause (Z) holds its spending, and Priority says when
//! its work is paid in a stall (`mc_sim::focus`). The buttons are the HUD's
//! (`hud/selection/pause_order.rs`, `hud/build/queue.rs`, `hud/priority.rs`).

use super::Game;
use crate::hud;
use mc_data::BlueprintId;
use mc_sim::focus::Priority;
use mc_sim::tables::flag;
use mc_sim::{Command, Handle};

impl Game {
    /// Pauses (or resumes) the work of whatever in the selection has work to pause.
    pub(super) fn set_paused(&mut self, paused: bool) {
        let units: Vec<Handle> = self
            .selected_units()
            .filter(|u| {
                mc_sim::pause::pausable(
                    &self.blueprints,
                    self.blueprints.unit(BlueprintId(u.blueprint as u16)),
                )
            })
            .map(|u| Handle(u.unit_id))
            .collect();
        if !units.is_empty() {
            self.send(Command::SetPaused { units, paused });
            self.hud.toast(
                if paused {
                    "Work paused"
                } else {
                    "Work resumed"
                },
                hud::style::Family::Engineering.tone(),
            );
        }
    }

    /// Z: resumes the selection's work if all of it is paused, pauses all of it otherwise
    /// (the Pause buttons say which).
    pub(super) fn toggle_paused(&mut self) {
        let (mut paused, mut workers) = (0, 0);
        for u in self.selected_units() {
            if mc_sim::pause::pausable(
                &self.blueprints,
                self.blueprints.unit(BlueprintId(u.blueprint as u16)),
            ) {
                workers += 1;
                paused += usize::from(u.paused());
            }
        }
        if workers > 0 {
            self.set_paused(paused < workers);
        }
    }

    /// Gives the selection's units that can take one this priority in a stall: anything
    /// with work to order, and sites still going up.
    pub(super) fn set_priority(&mut self, priority: Priority) {
        let units: Vec<Handle> = self
            .selected_units()
            .filter(|u| {
                hud::has_flag(u, flag::UNDER_CONSTRUCTION)
                    || mc_sim::focus::prioritizable(
                        &self.blueprints,
                        self.blueprints.unit(BlueprintId(u.blueprint as u16)),
                    )
            })
            .map(|u| Handle(u.unit_id))
            .collect();
        if units.is_empty() {
            return;
        }
        self.send(Command::SetPriority { units, priority });
        let (text, tone) = match priority {
            Priority::First => (
                "Priority First: paid in full first in a stall",
                hud::style::Family::Stance.tone(),
            ),
            Priority::Last => (
                "Priority Last: built from what is left over",
                hud::style::Family::Control.tone(),
            ),
            Priority::Even => (
                "Priority Auto: follows the Mines/Power row",
                hud::style::Family::Control.tone(),
            ),
        };
        self.hud.toast(text, tone);
    }
}
