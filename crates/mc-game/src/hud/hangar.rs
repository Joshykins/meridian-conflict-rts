//! An airbase in the unit panel: how full it is, and every aircraft below its
//! hatch, one tile each, with the empty berths after them. Clicking a tile picks
//! that aircraft (the base stays selected, so the roster stays up); orders given
//! to picked aircraft fire them out to carry them out. Right-clicking a tile
//! launches that one. ALL picks every aircraft below; a chip switches whether
//! idle aircraft land here by themselves.

use super::{Hud, HudAction, Scene};
use crate::ui::{id, palette, rgb, type_scale, Rect, Ui};
use mc_sim::mirror::HangarView;

const TILE_GAP: f32 = 5.0;
const PER_ROW: usize = 8;

/// Height the block takes for a base holding `capacity`, points.
pub fn height(capacity: u8) -> f32 {
    let rows = (capacity as usize).div_ceil(PER_ROW).max(1) as f32;
    24.0 + rows * 36.0 + 16.0
}

pub fn panel(hud: &mut Hud, ui: &mut Ui, s: &Scene, base: u32, view: &HangarView, r: Rect) {
    let (x, cw) = (r.x, r.w);
    let held = view.stored.len();
    ui.text(x, r.y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Hangar");
    let mut tally = format!("{held} / {}", view.capacity);
    if view.incoming > 0 {
        tally += &format!("  \u{b7}  {} in", view.incoming);
    }
    let label_w = ui.text_width(type_scale::MICRO, "Hangar") + 10.0;
    ui.text(x + label_w, r.y, type_scale::VALUE, rgb(super::style::AIR, 1.0), &tally);

    // Chips on the right: whether idle aircraft land here, and pick them all.
    let chip_w = |ui: &mut Ui, label: &str, value: &str| {
        ui.text_width(type_scale::MICRO, label) + ui.text_width(type_scale::VALUE, value) + 34.0
    };
    let on = view.auto_land;
    let value = if on { "ON" } else { "OFF" };
    let w = chip_w(ui, "AUTO-LAND", value);
    let right = x + cw - w;
    let tone = if on { super::style::AIR } else { palette::DIM };
    let (clicked, _) = hud.chip(ui, id("hangar-auto-land", 0), right, r.y - 8.0, "AUTO-LAND", value, tone);
    if clicked {
        ui.audio.play(crate::audio::Sfx::Tick);
        hud.actions.push(HudAction::AutoLand(!on));
    }
    if held > 0 {
        let w = chip_w(ui, "PICK", "ALL");
        let (clicked, _) = hud.chip(ui, id("hangar-all", 0), right - w - 6.0, r.y - 8.0, "PICK", "ALL", palette::TEXT);
        if clicked {
            ui.audio.play(crate::audio::Sfx::Select);
            let mut units = vec![base];
            units.extend(view.stored.iter().map(|a| a.unit_id));
            hud.actions.push(HudAction::Select { units, focus: false });
        }
    }

    // One tile per berth, the aircraft first, in the order they would be called out.
    let tile = ((cw - TILE_GAP * (PER_ROW as f32 - 1.0)) / PER_ROW as f32).min(34.0);
    let mut hint = None;
    for i in 0..view.capacity as usize {
        let at = Rect::new(
            x + (i % PER_ROW) as f32 * (tile + TILE_GAP),
            r.y + 22.0 + (i / PER_ROW) as f32 * (tile + TILE_GAP),
            tile,
            tile,
        );
        let Some(a) = view.stored.get(i) else {
            // An empty berth: a faint outline.
            ui.fill(Rect::new(at.x, at.bottom() - 2.0, at.w, 2.0), rgb(palette::LINE, 0.18));
            ui.fill(Rect::new(at.x, at.y, 2.0, at.h), rgb(palette::LINE, 0.1));
            ui.fill(Rect::new(at.right() - 2.0, at.y, 2.0, at.h), rgb(palette::LINE, 0.1));
            continue;
        };
        let picked = s.view.selection.contains(&a.unit_id);
        let t = hud.tile(ui, id("hangar-berth", a.unit_id as usize), at, picked || a.called, true);
        hud.thumbs.draw(ui, a.blueprint, Rect::new(at.x + 2.0, at.y + 1.0, at.w - 4.0, at.h - 7.0), 1.0);
        let tone = if a.health > 0.6 {
            super::HEALTHY
        } else if a.health > 0.3 {
            palette::WARN
        } else {
            palette::BAD
        };
        ui.fill(Rect::new(at.x + 3.0, at.bottom() - 5.0, (at.w - 6.0) * a.health, 2.0), rgb(tone, 0.95));
        if t.hovered {
            let name = &s.blueprints.unit(a.blueprint).name;
            hint = Some(if a.called {
                format!("{name}  \u{b7}  {:.0}%  \u{b7}  waiting for a tunnel", a.health * 100.0)
            } else {
                format!("{name}  \u{b7}  {:.0}%  \u{b7}  click to pick, right-click to launch", a.health * 100.0)
            });
        }
        if t.clicked {
            // Picked alongside the base, or put back.
            ui.audio.play(crate::audio::Sfx::Select);
            let mut units: Vec<u32> = s.view.selection.clone();
            if picked {
                units.retain(|&u| u != a.unit_id);
            } else {
                units.push(a.unit_id);
            }
            if !units.contains(&base) {
                units.insert(0, base);
            }
            hud.actions.push(HudAction::Select { units, focus: false });
        } else if t.right_clicked {
            ui.audio.play(crate::audio::Sfx::Order);
            hud.actions.push(HudAction::LaunchUnits(vec![a.unit_id]));
        }
    }
    let text = hint.unwrap_or_else(|| {
        if held == 0 {
            "Empty. Right-click it with aircraft to land them.".to_owned()
        } else {
            "Pick aircraft and give them orders: they are fired out to carry them out.".to_owned()
        }
    });
    ui.text_fit_left(x, r.bottom() - 10.0, cw, type_scale::MICRO, rgb(palette::FAINT, 1.0), &text);
}
