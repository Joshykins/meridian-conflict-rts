//! The order card's Repeat button, for a half-built unit on a lot of its own (an
//! experimental raised in the field, `mc_sim::site_repeat`): once it is done and has
//! driven off, the lot is laid again with the same unit.

use super::Order;
use crate::hud::build::Split;
use crate::hud::icons::Glyph;
use crate::hud::{has_flag, HudAction, Scene};
use mc_sim::mirror::UnitInstance;
use mc_sim::tables::flag;

/// A half-built unit on its own lot, which Repeat raises there again once it is done.
pub fn repeat_site(bp: &mc_data::UnitBlueprint, u: &UnitInstance) -> bool {
    bp.is_site_built_unit()
        && has_flag(u, flag::UNDER_CONSTRUCTION)
        && !has_flag(u, flag::IN_FACTORY)
}

/// Lit while every such site in the selection repeats; a click sets them all to the
/// other way, or all on when they are split.
pub(super) fn repeat_order(s: &Scene, units: &[&UnitInstance]) -> Option<Order> {
    let split = Split::count(
        units
            .iter()
            .filter(|u| repeat_site(s.bp(u), u))
            .map(|u| has_flag(u, flag::REPEAT)),
    );
    if split.of == 0 {
        return None;
    }
    Some(Order {
        glyph: Glyph::Repeat,
        label: if split.mixed() { "Repeat All" } else { "Repeat" },
        key: "L",
        hint: "Repeat (L): once it is built and has driven off, its lot is laid again with the same unit. Builders that had only it queued start on the next; with none, the site waits for builders. Goes round until switched off, or until an enemy kills the site.",
        action: HudAction::Repeat(split.next()),
        lit: split.all(),
    })
}
