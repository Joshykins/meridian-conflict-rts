//! The construction panel turned to prefabs (P): the player's prefabs for the
//! builder's faction on the open tier (a prefab's tier is its highest structure's),
//! on the shelves the construction panel has (Factories, Economy, Defense, ...), each
//! a card with a plan of its lots over its name, size and price. A prefab is on the
//! shelf of its costliest structure unless the player shelved it elsewhere. A click
//! (or its item key) takes it in hand; Ctrl+click opens it to rename, shelve or give it
//! another icon; a right-click arms its delete and a second one does it. With none
//! saved, the panel says how to make one.

use super::{plan, shelf};
use crate::audio::Sfx;
use crate::game::Mode;
use crate::hud::build::{key_cap, SHELF_H, STRIP_DROP};
use crate::hud::icons;
use crate::hud::style::ITEM_KEYS;
use crate::hud::{whole, Hud, HudAction, Scene, ENERGY, MATERIALS};
use crate::prefabs::{Category, Prefab, PrefabId};
use crate::ui::{id, ink, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_data::{BlueprintId, FactionId};

const CARD_W: f32 = 150.0;
const CARD_GAP: f32 = 6.0;
/// Room between the last card of one shelf and the first of the next, where the
/// divider stands.
const SHELF_GAP: f32 = 22.0;
/// A first right-click arms a delete for this long, seconds.
const DOOM_FOR: f32 = 3.0;

/// A card's place: its prefab, its shelf, and its place on the shelf (its item key).
struct Slot<'a> {
    prefab: &'a Prefab,
    shelf: Category,
    n: usize,
}

/// The Prefabs toggle at `x` on the tab row, filled with the accent while the panel
/// shows prefabs, so the mode cannot pass for a tier tab. Returns its width and
/// whether it was clicked.
pub(in crate::hud) fn toggle_button(
    hud: &mut Hud,
    ui: &mut Ui,
    x: f32,
    y: f32,
    on: bool,
) -> (f32, bool) {
    let tr = Rect::new(x, y, 104.0, 26.0);
    let t = hud.tile(ui, id("prefab-toggle", 0), tr, on, true);
    let k = ui.ease(id("prefab-toggle", 1), if on { 1.0 } else { 0.0 }, 16.0);
    if k > 0.01 {
        ui.fill_cut(tr, 5.0, rgb(palette::ACCENT_DEEP, 0.85 * k));
        ui.fill_cut(tr, 5.0, rgb(palette::ACCENT, 0.25 * k + 0.1 * t.glow * k));
        ui.outline_cut(tr, 5.0, rgb(palette::ACCENT, k), rgb(palette::ACCENT, k));
    }
    let tone = if on {
        rgb(0xFFFFFF, 1.0)
    } else {
        rgb(palette::TEXT, 0.6 + 0.4 * t.glow)
    };
    key_cap(ui, tr.x + 5.0, tr.y + 5.5, 'P', hud.build_keys);
    icons::glyph(
        ui,
        icons::Glyph::Prefab,
        Vec2::new(tr.x + 33.0, tr.mid_y()),
        6.0,
        tone,
    );
    ui.text_fit_left(
        tr.x + 44.0,
        tr.mid_y(),
        tr.w - 48.0,
        type_scale::BUTTON,
        tone,
        "Prefabs",
    );
    if t.clicked {
        ui.audio.play(Sfx::Tick);
    }
    (tr.w, t.clicked)
}

/// The construction panel `r` marked as turned to prefabs: an accent edge round it
/// and a band of accent along its top, behind the tab row.
pub(in crate::hud) fn mode_frame(ui: &mut Ui, r: Rect) {
    // The band stays inside the panel's cut corners (10 across).
    let (top, clear) = (rgb(palette::ACCENT, 0.16), rgb(palette::ACCENT, 0.0));
    ui.fill(Rect::new(r.x + 10.0, r.y + 1.0, r.w - 20.0, 9.0), top);
    ui.gradient_v(
        Rect::new(r.x + 1.0, r.y + 10.0, r.w - 2.0, 34.0),
        top,
        clear,
    );
    let edge = rgb(palette::ACCENT, 0.8);
    ui.outline_cut(r, 10.0, edge, rgb(palette::ACCENT, 1.0));
    let inner = rgb(palette::ACCENT, 0.3);
    ui.outline_cut(r.inset(1.0), 9.6, inner, inner);
}

/// The prefabs of the open tier (`hud.tab`) in `grid`, shelf by shelf, for a builder
/// of `faction` that can start `builds`. Returns the card hovered.
pub(in crate::hud) fn tab(
    hud: &mut Hud,
    ui: &mut Ui,
    s: &Scene,
    faction: FactionId,
    builds: &[BlueprintId],
    grid: Rect,
) -> Option<(PrefabId, Rect)> {
    let theirs: Vec<&Prefab> = s
        .prefabs
        .iter()
        .filter(|p| p.faction(s.blueprints) == Some(faction))
        .collect();
    if theirs.is_empty() {
        hud.build_key = None;
        empty(ui, grid, s.prefabs.len());
        return None;
    }
    // Shelf by shelf, each most recently used first.
    let mut slots: Vec<Slot> = Vec::new();
    for c in Category::ALL {
        let on = theirs
            .iter()
            .filter(|p| p.tech(s.blueprints) == hud.tab && p.category(s.blueprints) == c);
        for (n, prefab) in on.enumerate() {
            slots.push(Slot {
                prefab,
                shelf: c,
                n,
            });
        }
    }
    if slots.is_empty() {
        hud.build_key = None;
        ui.text(
            grid.x,
            grid.y + 20.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            "No prefabs at this tier",
        );
        return None;
    }
    let cards = Rect::new(
        grid.x,
        grid.y + STRIP_DROP,
        grid.w,
        grid.bottom() - grid.y - STRIP_DROP,
    );
    // The last first card from which every card after it fits.
    let fits_from = |first: usize| {
        let mut x = 0.0;
        for (i, t) in slots.iter().enumerate().skip(first) {
            if i > first && t.shelf != slots[i - 1].shelf {
                x += SHELF_GAP - CARD_GAP;
            }
            x += CARD_W + CARD_GAP;
        }
        x - CARD_GAP <= cards.w
    };
    let last_first = (0..slots.len())
        .find(|&i| fits_from(i))
        .unwrap_or(slots.len() - 1);
    let start_of = |c: Category| slots.iter().position(|t| t.shelf == c);
    let pui = &mut hud.prefabs;
    pui.shelf = pui.shelf.filter(|c| start_of(*c).is_some());
    // The wheel runs the cards along, one a notch.
    if grid.contains(ui.cursor - ui.shift) && ui.input.scroll != 0.0 {
        let was = pui.first;
        pui.first = if ui.input.scroll > 0.0 {
            was.saturating_sub(1)
        } else {
            (was + 1).min(last_first)
        };
        if pui.first != was {
            ui.audio.play(Sfx::Tick);
            pui.shelf = None;
        }
    }
    pui.first = pui.first.min(last_first);
    if pui.doomed.is_some_and(|(_, at)| ui.time - at > DOOM_FOR) {
        pui.doomed = None;
    }

    // A shelf key runs the cards to that shelf; an item key takes the card in that place
    // on the shelf picked, or else the shelf in front.
    if let Some(key) = hud.build_key.take() {
        if let Some(c) = Category::ALL.into_iter().find(|c| shelf(*c).key() == key) {
            match start_of(c) {
                Some(i) => {
                    ui.audio.play(Sfx::Tick);
                    hud.prefabs.shelf = Some(c);
                    hud.prefabs.first = i.min(last_first);
                }
                None => ui.audio.play(Sfx::Deny),
            }
        } else if let Some(n) = ITEM_KEYS.iter().position(|k| *k == key) {
            let current = hud.prefabs.shelf.unwrap_or(slots[hud.prefabs.first].shelf);
            match slots.iter().find(|t| t.shelf == current && t.n == n) {
                Some(t) => {
                    ui.audio.play(Sfx::Select);
                    hud.actions.push(HudAction::HoldPrefab(t.prefab.id));
                }
                None => ui.audio.play(Sfx::Deny),
            }
        }
    }
    let first = hud.prefabs.first;
    let current = hud.prefabs.shelf.unwrap_or(slots[first].shelf);
    let live = hud.build_keys;

    // The shelves, as buttons with their keys and counts: a click runs the cards there.
    let mut bx = grid.x;
    for c in Category::ALL {
        let Some(start) = start_of(c) else {
            continue;
        };
        let count = slots.iter().filter(|t| t.shelf == c).count();
        let label = shelf(c).label(true);
        let count_text = count.to_string();
        let w = 30.0
            + ui.text_width(type_scale::MICRO, label)
            + 10.0
            + ui.text_width(type_scale::MICRO, &count_text)
            + 9.0;
        if bx + w > grid.right() {
            break;
        }
        let br = Rect::new(bx, grid.y, w, SHELF_H);
        let lit = current == c;
        let t = hud.tile(ui, id("prefab-shelf", c as usize), br, lit, true);
        key_cap(ui, br.x + 8.0, br.y + 4.5, shelf(c).key(), live);
        ui.text(
            br.x + 30.0,
            br.mid_y(),
            type_scale::MICRO,
            rgb(
                if lit { 0xFFFFFF } else { palette::TEXT },
                if lit { 1.0 } else { 0.7 + 0.3 * t.glow },
            ),
            label,
        );
        ui.text_right(
            br.right() - 9.0,
            br.mid_y(),
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &count_text,
        );
        if t.clicked {
            ui.audio.play(Sfx::Tick);
            hud.prefabs.shelf = Some(c);
            hud.prefabs.first = start.min(last_first);
        }
        bx += w + 4.0;
    }

    let holding = match s.view.mode {
        Mode::Prefab(id) => Some(id),
        _ => None,
    };
    let mut hovered = None;
    let mut x = cards.x;
    let mut last = first;
    for (i, slot) in slots.iter().enumerate().skip(first) {
        let p = slot.prefab;
        if i > first && slot.shelf != slots[i - 1].shelf {
            // The divider between one shelf and the next.
            let dx = x - CARD_GAP + SHELF_GAP * 0.5;
            if x + SHELF_GAP - CARD_GAP + CARD_W > cards.right() + 0.5 {
                break;
            }
            ui.vline(dx, cards.y + 4.0, cards.h - 8.0, rgb(palette::LINE, 0.5));
            x += SHELF_GAP - CARD_GAP;
        }
        if x + CARD_W > cards.right() + 0.5 {
            break;
        }
        last = i;
        let cr = Rect::new(x, cards.y, CARD_W, cards.h);
        x += CARD_W + CARD_GAP;
        let (members, _) = p.resolve(s.blueprints);
        let able = members
            .iter()
            .filter(|m| builds.contains(&m.blueprint))
            .count();
        let t = hud.tile(
            ui,
            id("prefab", p.id.0 as usize),
            cr,
            holding == Some(p.id),
            able > 0 || ui.input.command,
        );
        let doomed = hud.prefabs.doomed.is_some_and(|(d, _)| d == p.id);
        card(ui, s, p, cr, t.glow, builds, able, members.len(), doomed);
        if slot.shelf == current {
            if let Some(&key) = ITEM_KEYS.get(slot.n) {
                key_cap(ui, cr.x + 6.0, cr.y + 6.0, key, live);
            }
        }
        if t.clicked && ui.input.command {
            ui.audio.play(Sfx::Select);
            hud.prefabs.doomed = None;
            hud.prefabs.edit((*p).clone());
        } else if t.clicked && able > 0 {
            ui.audio.play(Sfx::Select);
            hud.prefabs.doomed = None;
            hud.actions.push(HudAction::HoldPrefab(p.id));
        }
        if t.right_clicked {
            if doomed {
                hud.prefabs.doomed = None;
                hud.actions.push(HudAction::DropPrefab(p.id));
            } else {
                ui.audio.play(Sfx::Tick);
                hud.prefabs.doomed = Some((p.id, ui.time));
            }
        }
        if t.hovered {
            hovered = Some((p.id, cr));
        }
    }
    // More to either side: a mark at that edge, and the count beyond it.
    let more = [
        (first, cards.x - 10.0, "\u{2039}"),
        (slots.len() - 1 - last, x - CARD_GAP + 4.0, "\u{203a}"),
    ];
    for (count, x, mark) in more {
        if count > 0 {
            ui.text(
                x,
                cards.mid_y(),
                type_scale::ITEM,
                rgb(palette::TEXT, 0.4 + 0.2 * (ui.time * 3.0).sin().abs()),
                mark,
            );
        }
    }
    hovered
}

#[expect(
    clippy::too_many_arguments,
    reason = "the card's prefab, where it goes and what the builder makes of it"
)]
fn card(
    ui: &mut Ui,
    s: &Scene,
    p: &Prefab,
    cr: Rect,
    glow: f32,
    builds: &[BlueprintId],
    able: usize,
    total: usize,
    doomed: bool,
) {
    let well = Rect::new(cr.x + 4.0, cr.y + 4.0, cr.w - 8.0, cr.h - 52.0);
    ui.fill(well, ink(0.45));
    plan(ui, s.blueprints, p, well.inset(4.0), Some(builds), glow);
    let live = if able > 0 { 1.0 } else { 0.45 };
    // Its icon in the well's corner, with its tier's pips.
    icons::strategic(
        ui,
        p.badge(s.blueprints).icon(),
        p.tech(s.blueprints),
        Vec2::new(well.right() - 13.0, well.y + 12.0),
        8.0,
        rgb(0xFFFFFF, (0.75 + 0.25 * glow) * live),
        ink(0.45),
    );
    ui.text_fit_left(
        cr.x + 8.0,
        cr.bottom() - 36.0,
        cr.w - 16.0,
        type_scale::BUTTON,
        rgb(0xFFFFFF, (0.88 + 0.12 * glow) * live),
        &p.name,
    );
    let (materials, energy) = p.cost(s.blueprints);
    let end = ui.text(
        cr.x + 8.0,
        cr.bottom() - 17.0,
        type_scale::MICRO,
        rgb(MATERIALS, live),
        &whole(materials),
    );
    ui.text(
        end + 8.0,
        cr.bottom() - 17.0,
        type_scale::MICRO,
        rgb(ENERGY, live),
        &whole(energy),
    );
    // How much of it this builder raises, when not all.
    let (badge, tone) = if able < total {
        (
            format!("{able}/{total}"),
            if able == 0 {
                palette::BAD
            } else {
                palette::WARN
            },
        )
    } else {
        (total.to_string(), palette::DIM)
    };
    ui.text_right(
        cr.right() - 8.0,
        cr.bottom() - 17.0,
        type_scale::MICRO,
        rgb(tone, 1.0),
        &badge,
    );
    if doomed {
        let k = 0.75 + 0.25 * (ui.time * 6.0).sin().abs();
        ui.fill(well, rgb(palette::BAD, 0.25 * k));
        ui.text_centred(
            well.x + well.w * 0.5,
            well.mid_y() - 8.0,
            type_scale::BUTTON,
            rgb(0xFFFFFF, k),
            "Delete?",
        );
        ui.text_centred(
            well.x + well.w * 0.5,
            well.mid_y() + 10.0,
            type_scale::MICRO,
            rgb(palette::TEXT, k),
            "Right-click again",
        );
    }
}

/// No prefabs for this faction yet: how to make one.
fn empty(ui: &mut Ui, grid: Rect, others: usize) {
    let x = grid.x + 4.0;
    ui.text(
        x,
        grid.y + 22.0,
        type_scale::ITEM,
        rgb(palette::TEXT, 1.0),
        "No prefabs yet",
    );
    let lines = [
        "Lay out a group of structures, built or just planned. Select them (or the",
        "engineers that planned them) and press Save Prefab on the selection card.",
        "Name it, and it waits here (P), on its tier and shelf, in every match.",
    ];
    for (i, line) in lines.iter().enumerate() {
        ui.text_fit_left(
            x,
            grid.y + 50.0 + i as f32 * 19.0,
            grid.w - 8.0,
            type_scale::BODY,
            rgb(palette::DIM, 1.0),
            line,
        );
    }
    if others > 0 {
        ui.text(
            x,
            grid.y + 50.0 + 3.0 * 19.0 + 8.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &format!("{others} saved for the other faction"),
        );
    }
}

/// The card over the prefab hovered: what is in it, what it costs, how big it is, and
/// what this builder cannot raise of it.
pub(in crate::hud) fn hover_card(
    ui: &mut Ui,
    s: &Scene,
    id: PrefabId,
    builds: &[BlueprintId],
    tile: Rect,
    bottom: f32,
) {
    let Some(p) = s.prefabs.get(id) else {
        return;
    };
    let census = p.census(s.blueprints);
    let (_, gone) = p.resolve(s.blueprints);
    let missing: Vec<&str> = census
        .iter()
        .filter(|(b, _)| !builds.contains(b))
        .map(|(b, _)| s.blueprints.unit(*b).name.as_str())
        .collect();
    let w = 390.0;
    let rows = census.len().min(8);
    let h = 96.0
        + rows as f32 * 19.0
        + if census.len() > rows { 19.0 } else { 0.0 }
        + if missing.is_empty() { 0.0 } else { 22.0 }
        + if gone > 0 { 22.0 } else { 0.0 }
        + 30.0;
    let r = Rect::new(
        (tile.x + tile.w * 0.5 - w * 0.5).clamp(14.0, ui.size.x - w - 14.0),
        (bottom - h).max(14.0),
        w,
        h,
    );
    ui.panel(r);
    let (x, cw) = (r.x + 18.0, w - 36.0);
    ui.text(x, r.y + 26.0, type_scale::ITEM, rgb(0xFFFFFF, 1.0), &p.name);
    let count: usize = census.iter().map(|(_, n)| n).sum();
    ui.text(
        x,
        r.y + 48.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 1.0),
        &format!(
            "Prefab  \u{b7}  Tech {}  \u{b7}  {}  \u{b7}  {count} structures",
            p.tech(s.blueprints),
            shelf(p.category(s.blueprints)).label(true)
        ),
    );
    let (materials, energy) = p.cost(s.blueprints);
    for (i, (label, value, tone)) in [
        ("Materials", whole(materials), MATERIALS),
        ("Energy", whole(energy), ENERGY),
    ]
    .into_iter()
    .enumerate()
    {
        let cx = x + i as f32 * cw * 0.5;
        ui.fill(Rect::new(cx, r.y + 62.0, 2.0, 24.0), rgb(tone, 0.9));
        ui.text(
            cx + 10.0,
            r.y + 68.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            label,
        );
        ui.text(
            cx + 10.0,
            r.y + 82.0,
            type_scale::VALUE,
            rgb(tone, 1.0),
            &value,
        );
    }
    let mut y = r.y + 110.0;
    for &(b, n) in census.iter().take(rows) {
        let bp = s.blueprints.unit(b);
        let able = builds.contains(&b);
        let tone = crate::hud::style::Role::of(bp).tone();
        ui.fill(
            Rect::new(x, y - 5.0, 10.0, 10.0),
            rgb(tone, if able { 0.85 } else { 0.3 }),
        );
        ui.text_fit_left(
            x + 18.0,
            y,
            cw - 60.0,
            type_scale::VALUE,
            rgb(if able { palette::TEXT } else { palette::FAINT }, 1.0),
            &bp.name,
        );
        ui.text_right(
            x + cw,
            y,
            type_scale::VALUE,
            rgb(palette::DIM, 1.0),
            &format!("\u{d7}{n}"),
        );
        y += 19.0;
    }
    if census.len() > rows {
        let more: usize = census[rows..].iter().map(|(_, n)| n).sum();
        ui.text(
            x + 18.0,
            y,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &format!("and {more} more"),
        );
        y += 19.0;
    }
    if !missing.is_empty() {
        ui.text_fit_left(
            x,
            y + 4.0,
            cw,
            type_scale::MICRO,
            rgb(palette::WARN, 1.0),
            &format!("This builder cannot raise: {}", missing.join(", ")),
        );
        y += 22.0;
    }
    if gone > 0 {
        ui.text_fit_left(
            x,
            y + 4.0,
            cw,
            type_scale::MICRO,
            rgb(palette::WARN, 1.0),
            &format!("{gone} of its structures are no longer in the game"),
        );
    }
    ui.text(
        x,
        r.bottom() - 16.0,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        "Click to place  \u{b7}  Ctrl+click to edit  \u{b7}  Right-click twice to delete",
    );
}
