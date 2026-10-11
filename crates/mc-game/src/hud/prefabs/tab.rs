//! The construction panel's Prefabs tab: a card for each of the player's prefabs that
//! the builder's faction raises, most recently used first, each a plan of its lots
//! over its name, size and price. A click (or its item key) takes it in hand; a
//! right-click arms its delete and a second one does it. With none saved, the tab
//! says how to make one.

use super::plan;
use crate::audio::Sfx;
use crate::game::Mode;
use crate::hud::build::key_cap;
use crate::hud::style::ITEM_KEYS;
use crate::hud::{whole, Hud, HudAction, Scene, ENERGY, MATERIALS};
use crate::prefabs::{Prefab, PrefabId};
use crate::ui::{id, ink, palette, rgb, type_scale, Rect, Ui};
use mc_data::{BlueprintId, FactionId};

const CARD_W: f32 = 150.0;
const CARD_GAP: f32 = 6.0;
/// A first right-click arms a delete for this long, seconds.
const DOOM_FOR: f32 = 3.0;

/// The tab's cards in `grid`, for a builder of `faction` that can start `builds`.
/// Returns the card hovered.
pub(in crate::hud) fn tab(
    hud: &mut Hud,
    ui: &mut Ui,
    s: &Scene,
    faction: FactionId,
    builds: &[BlueprintId],
    grid: Rect,
) -> Option<(PrefabId, Rect)> {
    let shown: Vec<&Prefab> = s
        .prefabs
        .iter()
        .filter(|p| p.faction(s.blueprints) == Some(faction))
        .collect();
    let others = s.prefabs.len() - shown.len();
    if shown.is_empty() {
        hud.build_key = None;
        empty(ui, grid, others);
        return None;
    }
    let fits = (((grid.w + CARD_GAP) / (CARD_W + CARD_GAP)).floor() as usize).max(1);
    let last_first = shown.len().saturating_sub(fits);
    let ui_prefabs = &mut hud.prefabs;
    // The wheel runs the cards along, one a notch.
    if grid.contains(ui.cursor - ui.shift) && ui.input.scroll != 0.0 {
        let was = ui_prefabs.first;
        ui_prefabs.first = if ui.input.scroll > 0.0 {
            was.saturating_sub(1)
        } else {
            (was + 1).min(last_first)
        };
        if ui_prefabs.first != was {
            ui.audio.play(Sfx::Tick);
        }
    }
    ui_prefabs.first = ui_prefabs.first.min(last_first);
    let first = ui_prefabs.first;
    if ui_prefabs
        .doomed
        .is_some_and(|(_, at)| ui.time - at > DOOM_FOR)
    {
        ui_prefabs.doomed = None;
    }

    // An item key takes the card in that place.
    if let Some(key) = hud.build_key.take() {
        match ITEM_KEYS
            .iter()
            .position(|k| *k == key)
            .and_then(|n| shown.get(first + n))
        {
            Some(p) => {
                ui.audio.play(Sfx::Select);
                hud.actions.push(HudAction::HoldPrefab(p.id));
            }
            None => ui.audio.play(Sfx::Deny),
        }
    }

    let holding = match s.view.mode {
        Mode::Prefab(id) => Some(id),
        _ => None,
    };
    let mut hovered = None;
    let h = grid.h - 4.0;
    for (n, p) in shown.iter().enumerate().skip(first).take(fits) {
        let slot = n - first;
        let cr = Rect::new(
            grid.x + slot as f32 * (CARD_W + CARD_GAP),
            grid.y + 2.0,
            CARD_W,
            h,
        );
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
            able > 0,
        );
        let doomed = hud.prefabs.doomed.is_some_and(|(d, _)| d == p.id);
        card(ui, s, p, cr, t.glow, builds, able, members.len(), doomed);
        if let Some(&key) = ITEM_KEYS.get(slot) {
            key_cap(ui, cr.x + 6.0, cr.y + 6.0, key, hud.build_keys);
        }
        if t.clicked && able > 0 {
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
        (first, grid.x - 10.0, "\u{2039}"),
        (
            shown.len() - (first + fits).min(shown.len()),
            grid.x + fits as f32 * (CARD_W + CARD_GAP) - CARD_GAP + 4.0,
            "\u{203a}",
        ),
    ];
    for (count, x, mark) in more {
        if count > 0 {
            ui.text(
                x,
                grid.mid_y(),
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
        "engineers that planned them) and press Ctrl+S, or Save Prefab on the order card.",
        "Name it, and it waits here, in every match, to be put down in one click.",
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
    let w = 340.0;
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
        &format!("Prefab  \u{b7}  {count} structures"),
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
        "Click to place  \u{b7}  R turns it  \u{b7}  Right-click twice to delete",
    );
}
