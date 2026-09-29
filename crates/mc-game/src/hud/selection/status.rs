//! The single-unit panel under its name, in bands that always come in this order:
//! integrity and shield, what it is doing, what that makes and spends, its mines,
//! and its record. A band with nothing to say is left out; figures that never
//! change (vision, damage, range...) are on the details card.

use super::*;
use crate::hud::{economy, mine};

/// Construction amber, for work under way.
const BUILDING: u32 = 0xFFA030;

/// `on_strip`: what it makes shows on the queue strip, so its progress is left there.
pub(super) fn status_page(
    ui: &mut Ui,
    s: &Scene,
    u: &UnitInstance,
    bp: &UnitBlueprint,
    r: Rect,
    on_strip: bool,
) {
    let (x, cw) = (r.x, r.w);
    let mut y = vitals(ui, s, u, bp, x, r.y, cw);
    y = doing(ui, s, u, bp, x, y, cw, on_strip);
    y = crate::hud::warp::band(ui, s, u, bp, x, y, cw);
    let (mass, energy) = economy::flows(s, u, bp);
    if economy::strip(
        ui,
        mass,
        energy,
        Rect::new(x - 4.0, y - 4.0, cw + 8.0, economy::STRIP_H),
    ) {
        y += economy::STRIP_H + 4.0;
    }
    let mines = mine::views(s, &[u]);
    if !mines.is_empty() {
        mine::panel(ui, s, &mines, Rect::new(x, y, cw, mine::HEIGHT));
    }
    // The record keeps to the foot of the card, whatever sits above it.
    record(ui, s, u, bp, x, r.bottom() - 6.0, cw);
}

/// Integrity and, above it, a thin shield bar; the figures on one line over both.
/// Full integrity is a plain number, and only a hurt unit shows what it is out of.
fn vitals(
    ui: &mut Ui,
    s: &Scene,
    u: &UnitInstance,
    bp: &UnitBlueprint,
    x: f32,
    y: f32,
    cw: f32,
) -> f32 {
    let hp = veterancy_health(bp.health, u.veterancy_level()).to_f32();
    let whole_hp = u.health >= 0.999;
    let label_end = ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Integrity");
    ui.text(
        label_end + 8.0,
        y,
        type_scale::VALUE,
        rgb(
            if whole_hp {
                palette::TEXT
            } else {
                health_tone(u.health)
            },
            1.0,
        ),
        &if whole_hp {
            whole(hp)
        } else {
            format!("{} / {}", whole(u.health * hp), whole(hp))
        },
    );
    let shield = s
        .view
        .frame
        .shields
        .iter()
        .find(|sh| sh.unit_id == u.unit_id);
    let mut bars = y + 10.0;
    if let Some(sh) = shield {
        let max = bp.shield.map(|sp| sp.health.to_f32()).unwrap_or(0.0);
        let value = whole(sh.health * max);
        ui.text_right(
            x + cw,
            y,
            type_scale::VALUE,
            rgb(crate::hud::style::AIR, 1.0),
            &value,
        );
        let w = ui.text_width(type_scale::VALUE, &value);
        ui.text_right(
            x + cw - w - 6.0,
            y,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            "Shield",
        );
        bar(
            ui,
            Rect::new(x, bars, cw, 3.0),
            sh.health,
            crate::hud::style::AIR,
        );
        bars += 4.0;
    }
    bar(
        ui,
        Rect::new(x, bars, cw, 6.0),
        u.health,
        health_tone(u.health),
    );
    bars + 24.0
}

/// What it is doing, always one line: going up, paused, working (with how far along
/// when the queue strip is not showing it), on the move, or idle.
#[expect(
    clippy::too_many_arguments,
    reason = "one band of the card: its unit, place and the strip flag"
)]
fn doing(
    ui: &mut Ui,
    s: &Scene,
    u: &UnitInstance,
    bp: &UnitBlueprint,
    x: f32,
    y: f32,
    cw: f32,
    on_strip: bool,
) -> f32 {
    let queue = s.queue_of(u);
    // A stun or a jump under way says so, with its own figure (seconds, or the charge).
    let warp = crate::hud::warp::activity(s, u);
    let (label, progress, tone) = if let Some(w) = &warp {
        (w.label.clone(), w.progress, w.tone)
    } else if has_flag(u, flag::UNDER_CONSTRUCTION) {
        let label = if u.paused() {
            "Paused  \u{b7}  Under Construction"
        } else {
            "Under Construction"
        };
        let tone = if u.paused() { PAUSED } else { BUILDING };
        (label.to_owned(), Some(u.build.clamp(0.0, 1.0)), tone)
    } else if let Some(front) = queue.and_then(|q| q.orders.first()) {
        let making = matches!(
            front.kind,
            OrderKind::Build | OrderKind::Produce | OrderKind::Upgrade
        );
        let label = if making {
            format!(
                "{} {}",
                activity(front.kind),
                s.blueprints.unit(front.blueprint).name
            )
        } else {
            match front.formation_phase {
                1 => "Forming on Move",
                2 => "In Formation",
                3 => "Crossing Obstacle",
                _ => activity(front.kind),
            }
            .to_owned()
        };
        let progress = queue
            .map(|q| q.progress)
            .filter(|&p| making && !on_strip && p > 0.0);
        if u.paused() {
            (format!("Paused  \u{b7}  {label}"), progress, PAUSED)
        } else {
            (label, progress, palette::TEXT)
        }
    } else if u.paused() {
        ("Paused  \u{b7}  Z Resumes".to_owned(), None, PAUSED)
    } else if bp.motion.is_none() && bp.builder.is_none() {
        ("Online".to_owned(), None, palette::DIM)
    } else {
        ("Idle".to_owned(), None, palette::DIM)
    };
    // A dot in the line's colour leads, lit while there is work under way.
    let busy = tone != palette::DIM;
    let dot = Vec2::new(x + 3.0, y);
    if busy {
        ui.disc(dot, 3.0, rgb(tone, 0.6 + 0.4 * (ui.time * 3.0).sin().abs()));
    } else {
        ui.disc(dot, 3.0, rgb(tone, 0.5));
    }
    ui.text_fit_left(
        x + 12.0,
        y,
        cw - 60.0,
        type_scale::CAPTION,
        rgb(tone, 1.0),
        &label,
    );
    if let Some(w) = warp
        .as_ref()
        .filter(|w| progress.is_none() && !w.value.is_empty())
    {
        ui.text_right(x + cw, y, type_scale::VALUE, rgb(tone, 1.0), &w.value);
    }
    match progress {
        Some(p) => {
            let value = warp.map_or_else(|| format!("{:.0}%", p * 100.0), |w| w.value);
            ui.text_right(x + cw, y, type_scale::VALUE, rgb(tone, 1.0), &value);
            bar(ui, Rect::new(x, y + 10.0, cw, 5.0), p, tone);
            y + 30.0
        }
        None => y + 22.0,
    }
}

/// The unit's record at the foot of the card, only once it has one: its rank,
/// kills and the way to the next rank, and the materials it has reclaimed.
fn record(ui: &mut Ui, s: &Scene, u: &UnitInstance, bp: &UnitBlueprint, x: f32, y: f32, cw: f32) {
    let level = u.veterancy_level().min(VETERANCY_MAX);
    let kills = u.kill_count();
    let reclaimed = if bp.sends_reclaimers() {
        s.queue_of(u).map_or(0.0, |q| q.reclaimed)
    } else {
        0.0
    };
    let mut at = x;
    if level > 0 {
        chevrons(ui, Vec2::new(at + 5.0, y), level);
        at += VETERANCY_MAX as f32 * 11.0 + 4.0;
        at = ui.text(
            at,
            y,
            type_scale::CAPTION,
            rgb(VETERANCY, 1.0),
            RANKS[level as usize],
        ) + 10.0;
    }
    if kills > 0 {
        let mut text = format!("{kills} kill{}", if kills == 1 { "" } else { "s" });
        if level < VETERANCY_MAX {
            text += &format!(
                "  \u{b7}  {:.0}% to {}",
                u.veterancy_progress() * 100.0,
                RANKS[level as usize + 1]
            );
        }
        at = ui.text(at, y, type_scale::MICRO, rgb(VETERANCY, 0.85), &text);
    }
    if reclaimed >= 0.5 {
        // Right of the rank and kills, or on the line over them when they leave no room.
        let text = format!("{} materials reclaimed", whole(reclaimed));
        let w = ui.text_width(type_scale::MICRO, &text);
        let row = if at + 12.0 > x + cw - w { y - 16.0 } else { y };
        ui.text_right(
            x + cw,
            row,
            type_scale::MICRO,
            rgb(crate::hud::MASS, 1.0),
            &text,
        );
    }
}
