//! The Save Prefab button in the heading of the card of a selection of many. It
//! offers the selection's structures (and its builders' plans) for naming.

use crate::hud::build::tip;
use crate::hud::icons::{self, Glyph};
use crate::hud::{Hud, HudAction, Scene};
use crate::ui::{id, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_sim::mirror::UnitInstance;

/// The button's width.
pub(in crate::hud) const WIDE: f32 = 122.0;
const TIP: &str = "Save Prefab  \u{b7}  keep this layout, to lay it again with B, then P";

/// Whether the selection has anything to save: a structure of the player's own,
/// or a builder with structures planned.
pub(in crate::hud) fn can_save(s: &Scene, units: &[&UnitInstance]) -> bool {
    let structures = units.iter().any(|u| {
        (u.owner_flags & 0xFF) as u8 == s.view.local
            && s.bp(u).built_on_site()
            && !s.bp(u).is_mobile()
    });
    if structures || s.view.status.plans.is_empty() {
        return structures;
    }
    // Who holds a plan, looked up by each unit: not every unit for each plan.
    let mut planners: Vec<u32> = s.view.status.plans.iter().map(|p| p.unit_id).collect();
    planners.sort_unstable();
    units
        .iter()
        .any(|u| planners.binary_search(&u.unit_id).is_ok())
}

/// The button in `r`, filled with the accent so it stands out from the card.
pub(in crate::hud) fn save_button(hud: &mut Hud, ui: &mut Ui, r: Rect) {
    let t = hud.tile(ui, id("save-prefab", 0), r, false, true);
    ui.fill_cut(r, 5.0, rgb(palette::ACCENT_DEEP, 0.7 + 0.2 * t.glow));
    ui.fill_cut(r, 5.0, rgb(palette::ACCENT, 0.15 + 0.2 * t.glow));
    let edge = rgb(palette::ACCENT, 0.8 + 0.2 * t.glow);
    ui.outline_cut(r, 5.0, edge, edge);
    let tone = rgb(0xFFFFFF, 1.0);
    icons::glyph(
        ui,
        Glyph::Prefab,
        Vec2::new(r.x + 14.0, r.mid_y()),
        6.0,
        tone,
    );
    ui.text_fit_left(
        r.x + 26.0,
        r.mid_y(),
        r.w - 30.0,
        type_scale::BUTTON,
        tone,
        "Save Prefab",
    );
    if t.hovered {
        let w = ui.text_width(type_scale::MICRO, TIP) + 24.0;
        tip(ui, r.right() - w, r.y - 32.0, TIP);
    }
    if t.clicked {
        hud.actions.push(HudAction::OfferPrefab);
    }
}
