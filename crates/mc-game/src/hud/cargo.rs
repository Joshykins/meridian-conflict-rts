//! A lift ship's hold, in a panel of its own right of the order card (where a
//! builder's construction panel goes; a lift ship builds nothing). What the ship is
//! doing (a status line: in flight, setting down, ramp opening, ready, unloading,
//! taking off), a gauge of the room taken (each kind of unit its own stretch, units
//! still on the ramp after them), and one card per kind of unit riding in it: its
//! picture, how many, its name, the room they take and how hurt the worst of them is.
//!
//! Clicking a card lets one of that kind out (the ship sets down where it is first,
//! if it must); shift-click lets out every one of them; ctrl-click picks them
//! alongside the ship instead, so orders can be given to them for when they have
//! walked off. UNLOAD ALL (title row) lets the whole hold out here. Landing, unloading at a
//! spot and taking off are on the order card.

use super::style::Domain;
use super::{Hud, HudAction, Scene};
use crate::ui::{id, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_data::BlueprintId;
use mc_sim::mirror::UnitInstance;
use mc_sim::mirror::{CargoUnit, CargoView, LiftPhase};

const PAD: f32 = 16.0;
const GAP: f32 = 6.0;
/// Narrowest a card gets; as many as fit go on a row.
const CARD_MIN_W: f32 = 150.0;
const CARD_H: f32 = 44.0;
/// Widest the panel gets, points: four cards to a row.
const MAX_W: f32 = 4.0 * CARD_MIN_W + 3.0 * GAP + 2.0 * PAD;
/// Where the cards start below the top of the panel: the title row, then the gauge.
const CARDS_Y: f32 = 56.0;
/// Room left at the foot for the hint.
const FOOT_H: f32 = 30.0;

/// The first unit of `units` that is a lift ship with a hold to show, and its hold.
pub fn ship_of<'a>(s: &Scene, units: &[&'a UnitInstance]) -> Option<(&'a UnitInstance, CargoView)> {
    let _t = mc_core::perf_span!("ui.cargo_ship_of");
    units.iter().find_map(|u| {
        s.queue_of(u)
            .and_then(|q| q.cargo.clone())
            .map(|view| (*u, view))
    })
}

/// Width the panel takes out of `room` points: none when that is too narrow for a card.
pub fn width(room: f32) -> f32 {
    if room < CARD_MIN_W + 2.0 * PAD {
        0.0
    } else {
        room.min(MAX_W)
    }
}

/// The hold as kinds of unit, in the order the first of each would walk out.
fn kinds(stored: &[CargoUnit]) -> Vec<(BlueprintId, Vec<&CargoUnit>)> {
    let mut out: Vec<(BlueprintId, Vec<&CargoUnit>)> = Vec::new();
    for u in stored {
        match out.iter_mut().find(|(b, _)| *b == u.blueprint) {
            Some((_, units)) => units.push(u),
            None => out.push((u.blueprint, vec![u])),
        }
    }
    out
}

fn health_tone(health: f32) -> u32 {
    if health > 0.6 {
        super::HEALTHY
    } else if health > 0.3 {
        palette::WARN
    } else {
        palette::BAD
    }
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
        LiftPhase::Unloading => (
            format!("Unloading \u{b7} {} left", view.to_unload.max(1)),
            air,
            true,
        ),
        LiftPhase::RampClosing => ("Ramp closing".into(), palette::WARN, true),
        LiftPhase::TakingOff => ("Taking off".into(), air, true),
    }
}

/// The hold panel over `r` (glass and all).
pub fn panel(hud: &mut Hud, ui: &mut Ui, s: &Scene, ship: u32, view: &CargoView, r: Rect) {
    hud.glass(ui, r);
    let (x, cw) = (r.x + PAD, r.w - 2.0 * PAD);
    let top = r.y + 22.0;
    ui.text(x, top, type_scale::ITEM, rgb(palette::TEXT, 1.0), "Hold");
    let mut after = x + ui.text_width(type_scale::ITEM, "Hold") + 12.0;
    let tally = format!("{} / {}", view.used, view.capacity);
    ui.text(
        after,
        top + 1.0,
        type_scale::VALUE,
        rgb(palette::TEXT, 1.0),
        &tally,
    );
    after += ui.text_width(type_scale::VALUE, &tally) + 6.0;
    ui.text(
        after,
        top + 1.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        "room",
    );
    after += ui.text_width(type_scale::MICRO, "room") + 12.0;
    if view.boarding > 0 {
        let line = format!("+{} boarding", view.boarding);
        ui.text(
            after,
            top + 1.0,
            type_scale::MICRO,
            rgb(super::style::AIR, 1.0),
            &line,
        );
    }

    // UNLOAD ALL on the right of the title row, while there is anything to let out.
    let mut right = x + cw;
    if !view.stored.is_empty() {
        let w = ui.text_width(type_scale::MICRO, "UNLOAD")
            + ui.text_width(type_scale::VALUE, "ALL")
            + 34.0;
        right -= w;
        let (clicked, _) = hud.chip(
            ui,
            id("cargo-unload-all", 0),
            right,
            top - 11.0,
            "UNLOAD",
            "ALL",
            super::style::AIR,
        );
        if clicked {
            ui.audio.play(crate::audio::Sfx::Order);
            hud.actions.push(HudAction::UnloadUnits(
                view.stored.iter().map(|u| u.unit_id).collect(),
            ));
        }
        right -= 16.0;
    }
    // The status line, right-aligned before it: a lamp, pulsing while something is under way.
    let (line, tone, busy) = status(view);
    let line_w = ui.text_width(type_scale::VALUE, &line);
    let lx = right - line_w;
    ui.text(lx, top + 1.0, type_scale::VALUE, rgb(tone, 1.0), &line);
    let glow = if busy {
        0.55 + 0.45 * (ui.time * 5.0).sin().abs()
    } else {
        1.0
    };
    ui.disc(Vec2::new(lx - 9.0, top + 1.0), 3.0, rgb(tone, glow));

    // The gauge: each kind its own stretch in its domain's colour, the ramp's after.
    let groups = kinds(&view.stored);
    let gauge = Rect::new(x, r.y + 42.0, cw, 5.0);
    let per = gauge.w / view.capacity.max(1) as f32;
    ui.fill(gauge, rgb(palette::LINE, 0.08));
    let mut at = gauge.x;
    for (bp, units) in &groups {
        let room: f32 = units.iter().map(|u| u.room.max(1) as f32).sum();
        let (tone, _) = Domain::of(s.blueprints.unit(*bp)).tones();
        let w = (room * per).min(gauge.right() - at);
        ui.fill(
            Rect::new(at, gauge.y, (w - 1.0).max(1.0), gauge.h),
            rgb(tone, 0.9),
        );
        at += w;
    }
    if view.boarding > 0 {
        // Room those on the ramp will take is not known here; a unit's worth each.
        let w = (view.boarding as f32 * per).min(gauge.right() - at);
        let pulse = 0.25 + 0.25 * (ui.time * 4.0).sin().abs();
        ui.fill(
            Rect::new(at, gauge.y, w.max(0.0), gauge.h),
            rgb(super::style::AIR, pulse),
        );
    }

    // One card per kind aboard; past the rows that fit, the last card stands for the rest.
    let row_n = (((cw + GAP) / (CARD_MIN_W + GAP)) as usize).max(1);
    let rows = (((r.h - CARDS_Y - FOOT_H + GAP) / (CARD_H + GAP)) as usize).max(1);
    let card_w = (cw - GAP * (row_n as f32 - 1.0)) / row_n as f32;
    let slots = row_n * rows;
    let card_at = |i: usize| {
        Rect::new(
            x + (i % row_n) as f32 * (card_w + GAP),
            r.y + CARDS_Y + (i / row_n) as f32 * (CARD_H + GAP),
            card_w,
            CARD_H,
        )
    };
    let shown = if groups.len() > slots {
        slots - 1
    } else {
        groups.len()
    };
    let mut hint = None;
    for (i, (bp, units)) in groups.iter().enumerate().take(shown) {
        let card = card_at(i);
        let blueprint = s.blueprints.unit(*bp);
        let picked = units.iter().any(|u| s.view.selection.contains(&u.unit_id));
        let t = hud.tile(ui, id("cargo-kind", bp.0 as usize), card, picked, true);
        let pic = CARD_H - 6.0;
        let pr = Rect::new(card.x + 3.0, card.y + 2.0, pic, pic);
        hud.thumbs
            .stage(ui, pr, rgb(0xFFFFFF, 0.10 + 0.08 * t.glow));
        hud.thumbs.draw(ui, *bp, pr, 0.92 + 0.08 * t.glow);
        // How many, on the picture's corner.
        let count = format!("\u{d7}{}", units.len());
        let count_w = ui.text_width(type_scale::CAPTION, &count) + 8.0;
        let badge = Rect::new(
            pr.right() - count_w + 4.0,
            pr.bottom() - 15.0,
            count_w,
            14.0,
        );
        ui.fill(badge, rgb(palette::INK, 0.72));
        ui.text_centred(
            badge.x + badge.w * 0.5,
            badge.mid_y(),
            type_scale::CAPTION,
            rgb(palette::TEXT, 1.0),
            &count,
        );
        let tx = pr.right() + 8.0;
        ui.text_fit_left(
            tx,
            card.y + 14.0,
            card.right() - 7.0 - tx,
            type_scale::CAPTION,
            rgb(palette::TEXT, 0.85 + 0.15 * t.glow),
            &blueprint.name,
        );
        let room: u32 = units.iter().map(|u| u.room.max(1) as u32).sum();
        let each = units[0].room.max(1);
        let sub = if units.len() > 1 && each > 1 {
            format!("{each} each \u{b7} {room} room")
        } else {
            format!("{room} room")
        };
        ui.text_fit_left(
            tx,
            card.y + 28.0,
            card.right() - 7.0 - tx,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            &sub,
        );
        // How hurt the worst of them is, along the foot.
        let worst = units.iter().map(|u| u.health).fold(1.0f32, f32::min);
        let bar = Rect::new(tx, card.bottom() - 7.0, card.right() - 8.0 - tx, 2.0);
        ui.fill(bar, rgb(palette::LINE, 0.1));
        ui.fill(
            Rect::new(bar.x, bar.y, bar.w * worst, bar.h),
            rgb(health_tone(worst), 0.95),
        );
        if t.hovered {
            let name = &blueprint.name;
            let lowest = if units.len() > 1 { "lowest " } else { "" };
            hint = Some(format!(
                "{name} {lowest}{:.0}%  \u{b7}  Click lets one out  \u{b7}  Shift: all {}  \u{b7}  Ctrl: pick them",
                worst * 100.0,
                units.len()
            ));
        }
        if t.clicked {
            if s.view.ctrl {
                // Picked alongside the ship: their orders are carried out once they are off.
                ui.audio.play(crate::audio::Sfx::Select);
                let mut sel: Vec<u32> = s.view.selection.to_vec();
                if picked {
                    sel.retain(|v| !units.iter().any(|u| u.unit_id == *v));
                } else {
                    sel.extend(units.iter().map(|u| u.unit_id));
                }
                if !sel.contains(&ship) {
                    sel.insert(0, ship);
                }
                hud.actions.push(HudAction::Select {
                    units: sel,
                    focus: false,
                });
            } else {
                ui.audio.play(crate::audio::Sfx::Order);
                let out = if s.view.shift {
                    units.iter().map(|u| u.unit_id).collect()
                } else {
                    // The healthiest walks out first; the first to go out of those as healthy.
                    let best =
                        units
                            .iter()
                            .copied()
                            .reduce(|a, b| if b.health > a.health { b } else { a });
                    vec![best.expect("a kind has units").unit_id]
                };
                hud.actions.push(HudAction::UnloadUnits(out));
            }
        }
    }
    // The kinds that did not get a card: one card for them all.
    if shown < groups.len() {
        let rest = &groups[shown..];
        let card = card_at(shown);
        let t = hud.tile(ui, id("cargo-kind-rest", 0), card, false, true);
        let n: usize = rest.iter().map(|(_, u)| u.len()).sum();
        let title = format!("+{} kinds", rest.len());
        ui.text_fit_left(
            card.x + 12.0,
            card.y + 14.0,
            card.w - 18.0,
            type_scale::CAPTION,
            rgb(palette::TEXT, 0.85 + 0.15 * t.glow),
            &title,
        );
        ui.text_fit_left(
            card.x + 12.0,
            card.y + 28.0,
            card.w - 18.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            &format!("{n} units"),
        );
        if t.hovered {
            let names: Vec<String> = rest
                .iter()
                .map(|(bp, u)| format!("{} \u{d7}{}", s.blueprints.unit(*bp).name, u.len()))
                .collect();
            hint = Some(format!(
                "{}  \u{b7}  Click lets them all out",
                names.join(", ")
            ));
        }
        if t.clicked {
            ui.audio.play(crate::audio::Sfx::Order);
            let out = rest
                .iter()
                .flat_map(|(_, u)| u.iter().map(|u| u.unit_id))
                .collect();
            hud.actions.push(HudAction::UnloadUnits(out));
        }
    }
    if groups.is_empty() {
        // The empty hold: faint outlines where the cards would be.
        for i in 0..row_n {
            let at = card_at(i);
            ui.fill(
                Rect::new(at.x, at.bottom() - 2.0, at.w, 2.0),
                rgb(palette::LINE, 0.12),
            );
            ui.fill(Rect::new(at.x, at.y, 2.0, at.h), rgb(palette::LINE, 0.07));
            ui.fill(
                Rect::new(at.right() - 2.0, at.y, 2.0, at.h),
                rgb(palette::LINE, 0.07),
            );
        }
    }

    // The foot: a hint.
    let foot = r.bottom() - 16.0;
    let text = hint.unwrap_or_else(|| {
        if view.stored.is_empty() {
            match view.phase {
                LiftPhase::Ready => {
                    "Ramp down. Right-click the ship with land units to board them."
                }
                _ => "Empty. Right-click it with land units to board them; it comes down for them.",
            }
            .to_owned()
        } else {
            "Click a kind to let one out  \u{b7}  U unloads at a spot".to_owned()
        }
    });
    ui.text_fit_left(
        x,
        foot,
        cw,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        &text,
    );
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
            Some(room)
                if room <= t.capacity && bp.radius * 2 <= t.width && bp.height <= t.clearance =>
            {
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
        (
            format!("Board {}  \u{b7}  hold too small", ship.name),
            palette::WARN,
        )
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
        &title,
    );
    ui.text(
        r.x + 13.0,
        r.y + 31.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &detail,
    );
}
