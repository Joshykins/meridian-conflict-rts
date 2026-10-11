//! Prefabs on the HUD (`crate::prefabs`): the construction panel turned to prefabs
//! (P), the card for the prefab hovered, the box that names a prefab being saved or
//! edited, and the card by the pointer while one is in hand.

use super::style::Role;
use super::{Hud, Scene};
use crate::game::View;
use crate::prefabs::{Category, Prefab, PrefabId, PrefabSite, Turn};
use crate::ui::{palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_data::{BlueprintId, Blueprints, FactionId};

mod dialog;
mod save;
mod tab;
pub(super) use save::{can_save, save_button, WIDE as SAVE_W};
pub(super) use tab::{hover_card, mode_frame, tab, toggle_button};

/// The prefabs' share of the HUD: whether the construction panel shows prefabs, the
/// save box, and a delete waiting to be confirmed.
#[derive(Default)]
pub struct PrefabUi {
    /// The construction panel shows the player's prefabs in place of what the builder
    /// raises (P): the same tiers and shelves. Kept until P is pressed again.
    pub(super) on: bool,
    /// The prefab being named (or edited), before it is kept.
    saving: Option<dialog::Draft>,
    /// A prefab right-clicked once, and when (UI time): a second right-click soon
    /// after deletes it.
    doomed: Option<(PrefabId, f32)>,
    /// The first prefab card shown, when they do not all fit.
    first: usize,
    /// The shelf the item keys pick from, once a shelf key picked it.
    shelf: Option<Category>,
    /// Turn the construction panel to prefabs next frame.
    pub(super) want_on: bool,
}

impl PrefabUi {
    /// Opens the save box on `draft`, its suggested name ready to type over.
    pub fn offer(&mut self, draft: Prefab) {
        self.saving = Some(dialog::Draft::new(draft, false));
    }

    /// Opens the box on a prefab of the library, to rename it, shelve it or give it
    /// another icon.
    pub(super) fn edit(&mut self, prefab: Prefab) {
        self.saving = Some(dialog::Draft::new(prefab, true));
    }

    /// Turns the construction panel to prefabs, when the selection places them.
    pub fn open_tab(&mut self) {
        self.want_on = true;
    }

    /// The save box is up: it has the keyboard, and keys are not orders.
    pub fn typing(&self) -> bool {
        self.saving.is_some()
    }
}

/// What the selected builders (the local player's engineers and engineering stations)
/// can start, counting the tiers and refits they have queued, as the sim does for a
/// queued order.
pub fn selection_builds(view: &View, blueprints: &Blueprints) -> Vec<BlueprintId> {
    let mut out: Vec<BlueprintId> = Vec::new();
    for id in view.selection.iter() {
        let Some(&i) = view.index_of.get(id) else {
            continue;
        };
        let u = &view.frame.units[i];
        if (u.owner_flags & 0xFF) as u8 != view.local {
            continue;
        }
        let bp = blueprints.unit(BlueprintId(u.blueprint as u16));
        if !bp.works_in_field() {
            continue;
        }
        let queue = crate::sim_thread::queue_of(&view.status.queues, u.unit_id);
        let later = super::build::loadout_after_queue(blueprints, queue, bp);
        for b in [bp, later].iter().filter_map(|b| b.builder.as_ref()) {
            for b in &b.builds {
                if !out.contains(b) {
                    out.push(*b);
                }
            }
        }
    }
    out
}

/// The tiers `faction`'s prefabs are on, lowest first.
pub(super) fn tiers(s: &Scene, faction: FactionId) -> Vec<u8> {
    let mut out: Vec<u8> = s
        .prefabs
        .iter()
        .filter(|p| p.faction(s.blueprints) == Some(faction))
        .map(|p| p.tech(s.blueprints))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The construction panel's shelf a prefab category is: its name and key.
pub(super) fn shelf(c: Category) -> super::style::Purpose {
    use super::style::Purpose;
    match c {
        Category::Factories => Purpose::Builders,
        Category::Economy => Purpose::Economy,
        Category::Defense => Purpose::Combat,
        Category::AntiAir => Purpose::AntiAir,
        Category::Artillery => Purpose::Artillery,
        Category::Intel => Purpose::Support,
    }
}

/// What the selection would save as a prefab: the local player's selected units (the
/// structures among them count, `prefabs::capture`) in the order they were begun
/// (`order`), then the structures the selected builders have planned, in the order
/// they will be built; each where it stands and the way it faces.
pub fn selection_structures(
    view: &View,
    order: &crate::prefabs::BuildOrder,
) -> Vec<crate::prefabs::Laid> {
    use crate::prefabs::Laid;
    use mc_core::{Angle, Fx, FxVec2};
    use mc_sim::mirror::{KIND_GHOST, KIND_PROP, KIND_WRECK};
    let mut standing: Vec<&mc_sim::mirror::UnitInstance> = view
        .selection
        .iter()
        .filter_map(|id| view.index_of.get(id))
        .map(|&i| &view.frame.units[i])
        .filter(|u| {
            (u.owner_flags & 0xFF) as u8 == view.local
                && u.owner_flags & (KIND_WRECK | KIND_GHOST | KIND_PROP) == 0
        })
        .collect();
    standing.sort_by_key(|u| (order.rank(u.unit_id), u.unit_id));
    let mut out: Vec<Laid> = standing
        .into_iter()
        .map(|u| Laid {
            blueprint: BlueprintId(u.blueprint as u16),
            at: FxVec2::new(Fx::from_f32(u.pos[0]), Fx::from_f32(u.pos[1])),
            heading: Angle::from_radians_f32(u.heading),
        })
        .collect();
    out.extend(
        view.status
            .plans
            .iter()
            .filter(|p| view.selection.contains(&p.unit_id))
            .map(|p| Laid {
                blueprint: p.blueprint,
                at: p.at,
                heading: Angle::from_radians_f32(p.heading),
            }),
    );
    out
}

/// A prefab's plan from above in `r`, north up: each lot in its structure's role
/// colour, those `can` says no to (when given) dimmed and struck through.
pub(super) fn plan(
    ui: &mut Ui,
    blueprints: &Blueprints,
    prefab: &Prefab,
    r: Rect,
    can: Option<&[BlueprintId]>,
    glow: f32,
) {
    let (members, _) = prefab.resolve(blueprints);
    let cell = mc_map::BUILD_CELL_M as f32;
    let lots: Vec<(BlueprintId, [f32; 4])> = members
        .iter()
        .map(|m| {
            let f = blueprints
                .unit(m.blueprint)
                .footprint_at(m.heading(blueprints));
            let (hw, hh) = (f.0 as f32 * cell * 0.5, f.1 as f32 * cell * 0.5);
            let (x, y) = (m.x as f32, m.y as f32);
            (m.blueprint, [x - hw, y - hh, x + hw, y + hh])
        })
        .collect();
    let Some(bounds) = lots.iter().map(|(_, l)| *l).reduce(|a, b| {
        [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].max(b[2]),
            a[3].max(b[3]),
        ]
    }) else {
        return;
    };
    let (w, h) = (bounds[2] - bounds[0], bounds[3] - bounds[1]);
    // One scale for both axes, so the plan keeps its shape; small plans are not blown up
    // past 1.6 points a metre.
    let k = ((r.w - 8.0) / w).min((r.h - 8.0) / h).min(1.6);
    let (ox, oy) = (r.x + (r.w - w * k) * 0.5, r.y + (r.h - h * k) * 0.5);
    // The lot's corners on screen: north (larger y) up.
    let place = |l: [f32; 4]| {
        Rect::new(
            ox + (l[0] - bounds[0]) * k,
            oy + (bounds[3] - l[3]) * k,
            (l[2] - l[0]) * k,
            (l[3] - l[1]) * k,
        )
    };
    for (b, l) in &lots {
        let lot = place(*l);
        let inner = Rect::new(
            lot.x + 0.5,
            lot.y + 0.5,
            (lot.w - 1.0).max(1.0),
            (lot.h - 1.0).max(1.0),
        );
        let tone = Role::of(blueprints.unit(*b)).tone();
        let able = can.is_none_or(|c| c.contains(b));
        let a = if able { 1.0 } else { 0.3 };
        ui.fill(inner, rgb(tone, (0.32 + 0.12 * glow) * a));
        ui.frame(inner, rgb(tone, (0.8 + 0.2 * glow) * a));
        if !able && inner.w > 5.0 {
            ui.stroke(
                Vec2::new(inner.x + 1.0, inner.bottom() - 1.0),
                Vec2::new(inner.right() - 1.0, inner.y + 1.0),
                1.0,
                rgb(palette::BAD, 0.8),
            );
        }
    }
}

/// The card by the pointer with a prefab in hand: its name, how much of it will go
/// down and why the rest will not, and the keys.
pub fn cursor_hint(ui: &mut Ui, prefab: &Prefab, sites: &[PrefabSite], turn: Turn) {
    let total = sites.len();
    let going = sites.iter().filter(|s| s.goes_down()).count();
    let (text, tone) = if total == 0 {
        (
            format!("{}  \u{b7}  Off the map", prefab.name),
            palette::BAD,
        )
    } else if going == 0 {
        (
            format!(
                "{}  \u{b7}  {}",
                prefab.name,
                crate::prefabs::left_out_reason(sites)
            ),
            palette::BAD,
        )
    } else if going < total {
        (
            format!(
                "Place {going} of {total}  \u{b7}  {}  \u{b7}  {}",
                prefab.name,
                crate::prefabs::left_out_reason(sites)
            ),
            palette::WARN,
        )
    } else {
        (
            format!("Place {}  \u{b7}  {total} structures", prefab.name),
            palette::TEXT,
        )
    };
    let turned = match turn.quarters % 4 {
        0 => "",
        1 => "  \u{b7}  Turned 90\u{b0} left",
        2 => "  \u{b7}  Turned 180\u{b0}",
        _ => "  \u{b7}  Turned 90\u{b0} right",
    };
    let mirrored = if turn.mirrored {
        "  \u{b7}  Mirrored"
    } else {
        ""
    };
    let hint = format!(
        "Click to place  \u{b7}  R turns, Shift+R back  \u{b7}  F mirrors  \u{b7}  Shift keeps placing  \u{b7}  RMB cancels{turned}{mirrored}"
    );
    let p = ui.cursor + Vec2::new(20.0, 22.0);
    let w = ui
        .text_width(type_scale::CAPTION, &text)
        .max(ui.text_width(type_scale::MICRO, &hint))
        + 26.0;
    let r = Rect::new(
        p.x.min(ui.size.x - w - 4.0),
        p.y.min(ui.size.y - 50.0),
        w,
        44.0,
    );
    ui.frost(r, 0.74);
    ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(tone, 1.0));
    ui.text(
        r.x + 13.0,
        r.y + 14.0,
        type_scale::CAPTION,
        rgb(tone, 1.0),
        &text,
    );
    ui.text(
        r.x + 13.0,
        r.y + 31.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &hint,
    );
}

impl Hud {
    /// The save box, over everything, when one is open.
    pub(super) fn prefab_dialog(&mut self, ui: &mut Ui, s: &Scene) {
        dialog::draw(self, ui, s);
    }
}
