//! A lift ship in the unit panel: its hold. What the ship is doing (a status line:
//! in flight, setting down, ramp opening, ready, unloading, taking off), how much room is
//! taken, and a tile for each unit riding in it, as wide as the room it takes, with the
//! free room after them.
//!
//! Clicking a tile lets that one unit out (the ship sets down where it is first, if it
//! must); shift-click lets out every unit of that kind; ctrl-click picks the unit
//! alongside the ship instead, so orders can be given to it for when it has walked off.
//! Landing, unloading at a spot or here, and taking off are on the order card.

use super::{Hud, HudAction, Scene};
use crate::ui::{id, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_sim::mirror::{CargoView, LiftPhase};

const CELL_GAP: f32 = 4.0;
/// Room shown on a row of the hold.
const PER_ROW: usize = 12;

/// Height the block takes for a hold of `capacity`, points.
pub fn height(capacity: u16) -> f32 {
    let rows = (capacity as usize).div_ceil(PER_ROW).max(1) as f32;
    24.0 + rows * 32.0 + 16.0
}

/// The status line: what the ship is doing, its tone, and whether it is under way
/// (the lamp beside it pulses).
pub fn status(view: &CargoView) -> (String, u32, bool) {
    let air = super::style::AIR;
    match view.phase {
        LiftPhase::InFlight => ("In flight".into(), palette::DIM, false),
        LiftPhase::Descending => ("Setting down".into(), air, true),
        LiftPhase::RampOpening => ("Ramp opening".into(), palette::WARN, true),
        LiftPhase::Ready => ("Ready \u{b7} ramp down".into(), super::HEALTHY, false),
        LiftPhase::Unloading => (format!("Unloading \u{b7} {} left", view.to_unload.max(1)), air, true),
        LiftPhase::RampClosing => ("Ramp closing".into(), palette::WARN, true),
        LiftPhase::TakingOff => ("Taking off".into(), air, true),
    }
}

pub fn panel(hud: &mut Hud, ui: &mut Ui, s: &Scene, ship: u32, view: &CargoView, r: Rect) {
    let (x, cw) = (r.x, r.w);
    ui.text(x, r.y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Hold");
    let mut tally = format!("{} / {}", view.used, view.capacity);
    if view.boarding > 0 {
        tally += &format!("  \u{b7}  {} boarding", view.boarding);
    }
    let label_w = ui.text_width(type_scale::MICRO, "Hold") + 10.0;
    ui.text(x + label_w, r.y, type_scale::VALUE, rgb(super::style::AIR, 1.0), &tally);

    // The status line, right-aligned: a lamp, pulsing while something is under way.
    let (line, tone, busy) = status(view);
    let line_w = ui.text_width(type_scale::VALUE, &line);
    let lx = x + cw - line_w;
    ui.text(lx, r.y, type_scale::VALUE, rgb(tone, 1.0), &line);
    let glow = if busy { 0.55 + 0.45 * (ui.time * 5.0).sin().abs() } else { 1.0 };
    ui.disc(Vec2::new(lx - 9.0, r.y), 3.0, rgb(tone, glow));

    // The hold, room by room: each unit a tile as wide as the room it takes.
    let cell = ((cw - CELL_GAP * (PER_ROW as f32 - 1.0)) / PER_ROW as f32).min(30.0);
    let at_room = |i: usize, span: usize| {
        let col = i % PER_ROW;
        let span = span.min(PER_ROW - col).max(1);
        Rect::new(
            x + col as f32 * (cell + CELL_GAP),
            r.y + 22.0 + (i / PER_ROW) as f32 * (cell + CELL_GAP),
            span as f32 * cell + (span - 1) as f32 * CELL_GAP,
            cell,
        )
    };
    let mut hint = None;
    let mut room = 0usize;
    for u in &view.stored {
        let span = u.room.max(1) as usize;
        // A wide unit does not break across rows.
        if room % PER_ROW + span > PER_ROW {
            room += PER_ROW - room % PER_ROW;
        }
        let at = at_room(room, span);
        room += span;
        let picked = s.view.selection.contains(&u.unit_id);
        let t = hud.tile(ui, id("cargo-unit", u.unit_id as usize), at, picked, true);
        let thumb = at.h - 7.0;
        hud.thumbs.draw(ui, u.blueprint, Rect::new(at.x + (at.w - thumb) / 2.0, at.y + 1.0, thumb, thumb), 1.0);
        let tone = if u.health > 0.6 {
            super::HEALTHY
        } else if u.health > 0.3 {
            palette::WARN
        } else {
            palette::BAD
        };
        ui.fill(Rect::new(at.x + 3.0, at.bottom() - 5.0, (at.w - 6.0) * u.health, 2.0), rgb(tone, 0.95));
        if t.hovered {
            let name = &s.blueprints.unit(u.blueprint).name;
            hint = Some(format!(
                "{name} {:.0}%  \u{b7}  Click lets it out  \u{b7}  Shift: every {name}  \u{b7}  Ctrl: pick it",
                u.health * 100.0
            ));
        }
        if t.clicked {
            if s.view.ctrl {
                // Pick it alongside the ship: its orders are carried out once it is off.
                ui.audio.play(crate::audio::Sfx::Select);
                let mut units: Vec<u32> = s.view.selection.clone();
                if picked {
                    units.retain(|&v| v != u.unit_id);
                } else {
                    units.push(u.unit_id);
                }
                if !units.contains(&ship) {
                    units.insert(0, ship);
                }
                hud.actions.push(HudAction::Select { units, focus: false });
            } else {
                ui.audio.play(crate::audio::Sfx::Order);
                let units = if s.view.shift {
                    view.stored.iter().filter(|o| o.blueprint == u.blueprint).map(|o| o.unit_id).collect()
                } else {
                    vec![u.unit_id]
                };
                hud.actions.push(HudAction::UnloadUnits(units));
            }
        }
    }
    // The free room: faint outlines.
    for i in (view.used as usize).max(room)..view.capacity as usize {
        let at = at_room(i, 1);
        ui.fill(Rect::new(at.x, at.bottom() - 2.0, at.w, 2.0), rgb(palette::LINE, 0.18));
        ui.fill(Rect::new(at.x, at.y, 2.0, at.h), rgb(palette::LINE, 0.1));
        ui.fill(Rect::new(at.right() - 2.0, at.y, 2.0, at.h), rgb(palette::LINE, 0.1));
    }
    let text = hint.unwrap_or_else(|| {
        if view.stored.is_empty() {
            match view.phase {
                LiftPhase::Ready => "Ramp down. Right-click the ship with land units to board them.",
                _ => "Empty. Right-click it with land units to board them; it comes down for them.",
            }
            .to_owned()
        } else {
            "Click a unit to let it out  \u{b7}  U unloads at a spot  \u{b7}  Shift+U here".to_owned()
        }
    });
    ui.text_fit_left(x, r.bottom() - 10.0, cw, type_scale::MICRO, rgb(palette::FAINT, 1.0), &text);
}

/// Near the pointer, over one of the player's own lift ships while land units are
/// selected: that a right-click boards them, how much room they take, and which will
/// not fit and why. `free` is how much of the hold is free, when it is known.
pub fn board_hint(
    ui: &mut Ui,
    blueprints: &mc_data::Blueprints,
    ship: &mc_data::UnitBlueprint,
    riders: &[mc_data::BlueprintId],
    free: Option<u16>,
) {
    let Some(t) = ship.transport else {
        return;
    };
    let mut need = 0u32;
    let (mut fit, mut big) = (0usize, 0usize);
    for &b in riders {
        let bp = blueprints.unit(b);
        match bp.cargo_room() {
            Some(room) if room <= t.capacity && bp.radius * 2 <= t.width && bp.height <= t.clearance => {
                fit += 1;
                need += room as u32;
            }
            Some(_) => big += 1,
            None => {}
        }
    }
    if fit + big == 0 {
        return;
    }
    let free = free.unwrap_or(t.capacity) as u32;
    let (title, tone) = if fit == 0 {
        ("Won't fit".to_owned(), palette::BAD)
    } else if need > free {
        (format!("Board {}  \u{b7}  hold too small", ship.name), palette::WARN)
    } else {
        (format!("Board {}", ship.name), super::style::AIR)
    };
    let mut detail = format!("{need} of {free} room");
    if need > free {
        detail = format!("{need} room, {free} free: the rest wait");
    }
    if big > 0 {
        detail += &format!("  \u{b7}  {big} too big for the hold");
    }
    let p = ui.cursor + Vec2::new(20.0, 22.0);
    let w = ui
        .text_width(type_scale::CAPTION, &title)
        .max(ui.text_width(type_scale::MICRO, &detail))
        + 26.0;
    let r = Rect::new(p.x.min(ui.size.x - w - 4.0), p.y.min(ui.size.y - 50.0), w, 44.0);
    ui.frost(r, 0.74);
    ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(tone, 1.0));
    ui.text(r.x + 13.0, r.y + 14.0, type_scale::CAPTION, rgb(tone, 1.0), &title);
    ui.text(r.x + 13.0, r.y + 31.0, type_scale::MICRO, rgb(palette::DIM, 1.0), &detail);
}
