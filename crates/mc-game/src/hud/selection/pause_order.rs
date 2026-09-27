//! The order card's Pause button, for builders, factories and anything that upgrades.

use super::Order;
use crate::hud::build::Split;
use crate::hud::icons::Glyph;
use crate::hud::{HudAction, Scene};
use mc_sim::mirror::UnitInstance;

/// Lit Resume while all of the selection's work is paused; Pause All while only some of
/// it is, since a click pauses the rest (the queue strip shows how it is split).
pub(super) fn pause_order(s: &Scene, units: &[&UnitInstance]) -> Option<Order> {
    let split = Split::count(
        units
            .iter()
            .filter(|u| mc_sim::pause::pausable(s.blueprints, s.bp(u)))
            .map(|u| u.paused()),
    );
    if split.of == 0 {
        return None;
    }
    let order = if split.all() {
        Order {
            glyph: Glyph::Play,
            label: "Resume",
            key: "Z",
            hint:
                "Resume work: building, production and upgrades carry on from where they stopped.",
            action: HudAction::PauseWork(false),
            lit: true,
        }
    } else if split.mixed() {
        Order {
            glyph: Glyph::Pause,
            label: "Pause All",
            key: "Z",
            hint: "Some of the selection is paused and some is working. Pause all of it; Z again resumes all.",
            action: HudAction::PauseWork(true),
            lit: false,
        }
    } else {
        Order {
            glyph: Glyph::Pause,
            label: "Pause",
            key: "Z",
            hint: "Pause work: keep every order in the queue but spend nothing. A site stays half built and a factory holds its product, and whoever assists them waits too. Z again to resume.",
            action: HudAction::PauseWork(true),
            lit: false,
        }
    };
    Some(order)
}
