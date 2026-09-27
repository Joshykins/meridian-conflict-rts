//! The order card's Pause button, for builders, factories, anything that upgrades, and
//! shields, radar and sonar (pausing those powers them down).

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
    // A selection of nothing but powered kit: pausing switches it off and saves its upkeep.
    let power = units
        .iter()
        .map(|u| s.bp(u))
        .filter(|bp| mc_sim::pause::pausable(s.blueprints, bp))
        .all(mc_sim::pause::powers_down);
    let order = if split.all() {
        Order {
            glyph: Glyph::Play,
            label: "Resume",
            key: "Z",
            hint: if power {
                "Power up: the shield, radar or sonar comes back on and draws its upkeep again."
            } else {
                "Resume work: building, production and upgrades carry on from where they stopped."
            },
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
            hint: if power {
                "Power down: the shield, radar or sonar goes off and stops drawing energy. Z again to power up."
            } else {
                "Pause work: keep every order in the queue but spend nothing. A site stays half built and a factory holds its product, and whoever assists them waits too. A shield, radar or sonar powers down. Z again to resume."
            },
            action: HudAction::PauseWork(true),
            lit: false,
        }
    };
    Some(order)
}
