//! A dasher's panel (the Regency Seraph, mc-sim `dash.rs`), in the titan panel's place
//! right of the order card: what the Behemoth's great bore has there (`titan.rs`), its
//! dash has here. Its tier, the dash charging, gone, recharging or ready with a bar that
//! fills as it goes, what it does (its reach, the wave it lands with, how long it charges
//! and is gone, its cycle), and the dash button (J): pick a point and it dashes there by
//! hand (`Command::Dash`), as well as on its own.

use super::{Hud, HudAction, Scene};
use crate::audio::Sfx;
use crate::dash_marks::DASH;
use crate::game::{Mode, Targeting};
use crate::titan_marks::{metres, TITAN};
use crate::ui::{id, ink, palette, rgb, style, type_scale, Color, Rect, Ui};
use glam::Vec2;
use mc_data::UnitBlueprint;
use mc_render::Face;
use mc_sim::mirror::{DashStage, UnitInstance};

const TPS: f32 = mc_core::TICKS_PER_SECOND as f32;

/// Where a dash stands for the panel: its share through it (0 to 1) and seconds left.
#[derive(Clone, Copy, PartialEq)]
enum State {
    Charging(f32, f32),
    Gone(f32, f32),
    Recharging(f32, f32),
    Ready,
}

impl State {
    fn of(s: &Scene, u: &UnitInstance, bp: &UnitBlueprint) -> State {
        // Read by the tick: the panel's bar steps a tick at a time, as the titan's does.
        if let Some(d) = s.view.frame.dashes.iter().find(|d| d.unit_id == u.unit_id) {
            let len = d.length.max(1) as f32;
            let k = (d.ticks as f32 / len).clamp(0.0, 1.0);
            let left = (len - d.ticks as f32).max(0.0) / TPS;
            match d.stage {
                DashStage::Charging => return State::Charging(k, left),
                DashStage::Gone => return State::Gone(k, left),
                DashStage::Landed => {}
            }
        }
        let left = s.queue_of(u).map_or(0.0, |q| q.dash_recharge);
        let full = bp
            .dash
            .map_or(1.0, |d| d.cooldown_ticks.max(1) as f32 / TPS);
        if left > 0.0 {
            State::Recharging((1.0 - left / full).clamp(0.0, 1.0), left)
        } else {
            State::Ready
        }
    }
}

/// The dasher's panel over `r`.
pub fn panel(hud: &mut Hud, ui: &mut Ui, s: &Scene, u: &UnitInstance, count: usize, r: Rect) {
    let bp = s.bp(u);
    let Some(dash) = bp.dash else {
        return;
    };
    hud.glass(ui, r);
    let t = ui.time;
    let (x, cw) = (r.x + 16.0, r.w - 32.0);
    let mut y = r.y + 22.0;

    // Header: its name, the tier plate, and how many are picked.
    ui.text(x, y, type_scale::ITEM, rgb(palette::TEXT, 1.0), &bp.name);
    let nx = x + ui.text_width(type_scale::ITEM, &bp.name) + 10.0;
    let tier = format!("Tier {}", bp.tech);
    let tw = ui.text_width(type_scale::MICRO, &tier) + 16.0;
    let plate = Rect::new(nx, y - 10.0, tw, 17.0);
    ui.fill(plate, rgb(TITAN, 0.92));
    ui.brackets(plate.inset(-3.0), 4.0, rgb(TITAN, 0.7));
    ui.text_centred(
        plate.x + plate.w * 0.5,
        plate.mid_y(),
        type_scale::MICRO,
        ink(1.0),
        &tier,
    );
    let right = if count > 1 {
        format!("{count} picked")
    } else {
        bp.role.clone()
    };
    ui.text_right(x + cw, y, type_scale::MICRO, rgb(palette::DIM, 1.0), &right);
    y += 24.0;

    // The dash, and where it stands.
    let state = State::of(s, u, bp);
    ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Dash");
    let (label, tone) = match state {
        State::Charging(_, left) => (format!("Charging  {:.1} s", left), 0xFFE2E8),
        State::Gone(_, left) => (format!("Arriving  {:.1} s", left), 0xFFFFFF),
        State::Recharging(_, left) => (format!("Recharging  {:.0} s", left.ceil()), palette::WARN),
        State::Ready => ("Ready".to_owned(), DASH),
    };
    ui.text_right(x + cw, y, type_scale::VALUE, rgb(tone, 1.0), &label);
    y += 10.0;
    // The bar: white-hot filling through the charge, flickering while it is gone, amber
    // while it recharges, full and breathing red when ready.
    let bar = Rect::new(x, y, cw, 10.0);
    ui.fill_cut(bar, 3.0, ink(0.6));
    let (fill, shimmer) = match state {
        State::Charging(k, _) => (k, 0.2 + 0.3 * (t * (8.0 + 20.0 * k)).sin().abs()),
        State::Gone(k, _) => (1.0 - k, 0.45 + 0.35 * (t * 31.0).sin()),
        State::Recharging(k, _) => (k, 0.0),
        State::Ready => (1.0, 0.1 + 0.12 * (t * 2.2).sin()),
    };
    let inner = Rect::new(bar.x + 2.0, bar.y + 2.0, (bar.w - 4.0) * fill, bar.h - 4.0);
    ui.gradient_h(inner, rgb(tone, 0.5), rgb(tone, 0.95));
    ui.fill(inner, rgb(0xFFFFFF, shimmer.max(0.0)));
    for k in 1..10 {
        let tx = bar.x + bar.w * k as f32 / 10.0;
        ui.vline(tx, bar.y + 2.0, bar.h - 4.0, ink(0.35));
    }
    ui.frame(bar, rgb(tone, 0.4));
    y += 24.0;

    // What it does, a column each: label over value.
    let wave = bp
        .weapons
        .iter()
        .filter(|w| w.dash_only)
        .map(|w| w.range_max.to_f32())
        .reduce(f32::max);
    let secs = |ticks: u16| format!("{:.1} s", ticks as f32 / TPS);
    let facts = [
        (
            "Reach",
            format!(
                "{:.1}\u{2013}{:.1} km",
                dash.min.to_f32() / 1000.0,
                dash.range.to_f32() / 1000.0
            ),
            palette::TEXT,
        ),
        ("Nova", wave.map_or("\u{2014}".to_owned(), metres), DASH),
        ("Charge", secs(dash.windup_ticks), palette::TEXT),
        ("Gone", secs(dash.transit_ticks), palette::TEXT),
        (
            "Cycle",
            format!("{:.0} s", dash.cooldown_ticks as f32 / TPS),
            palette::TEXT,
        ),
    ];
    let widths = [0.26, 0.2, 0.19, 0.17, 0.18];
    let mut fx = x;
    for ((k, v, tone), share) in facts.iter().zip(widths) {
        ui.text(fx, y, type_scale::MICRO, rgb(palette::FAINT, 1.0), k);
        ui.text(fx, y + 17.0, type_scale::VALUE, rgb(*tone, 0.95), v);
        fx += cw * share;
    }
    y += 34.0;

    // The dash button: pick a point and it dashes there once its dash is ready.
    let foot = r.bottom() - 30.0;
    let h = (foot - y).clamp(40.0, 58.0);
    let button = Rect::new(x, foot - h, cw, h);
    let aiming = matches!(s.view.mode, Mode::Target(Targeting::Dash));
    let res = ui.interact(id("titan-dash", 0), button, true);
    hud.claim(ui, button);
    let hot = if aiming { 1.0 } else { res.glow };
    let ready = state == State::Ready;
    let k = if ready { 1.0 } else { 0.6 };
    ui.fill_cut(button, 7.0, ink(0.66));
    ui.gradient_h(button, rgb(DASH, (0.12 + 0.24 * hot) * k), rgb(DASH, 0.02));
    ui.outline_cut(
        button,
        7.0,
        rgb(DASH, (0.35 + 0.45 * hot) * k),
        rgb(0xFFFFFF, 0.3 + 0.4 * hot),
    );
    // A chevron going through a ring on the left, streaking while a point is being chosen.
    let c = Vec2::new(button.x + 28.0, button.mid_y());
    glyph(
        ui,
        c,
        11.0,
        rgb(DASH, 0.95 * k),
        if aiming { t } else { -1.0 },
    );
    let big = style(Face::Bold, 19.0, 1.0);
    ui.text(
        button.x + 52.0,
        button.mid_y() - 5.0,
        big,
        rgb(0xFFFFFF, 0.85 + 0.15 * hot),
        if aiming {
            "Choose where"
        } else {
            "Dash  [ J ]"
        },
    );
    let sub = if aiming {
        "Click the ground  \u{b7}  RMB cancels".to_owned()
    } else if ready {
        format!(
            "Come down anywhere {}\u{2013}{} off",
            metres(dash.min.to_f32()),
            metres(dash.range.to_f32())
        )
    } else {
        "Pick a point now: it goes once it has recharged".to_owned()
    };
    ui.text_fit_left(
        button.x + 52.0,
        button.mid_y() + 13.0,
        button.w - 90.0,
        type_scale::MICRO,
        rgb(DASH, 0.9),
        &sub,
    );
    if res.clicked {
        ui.audio.play(Sfx::Select);
        hud.actions.push(HudAction::Target(Targeting::Dash));
    }
    ui.text_fit_left(
        x,
        r.bottom() - 14.0,
        cw,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        "Dashes on its own at what it sees  \u{b7}  gone, it cannot be hit",
    );
}

/// The dash's mark: a ring with a chevron driven through it, trailing streaks when `streak`
/// is a time (aiming), still when it is below zero.
pub fn glyph(ui: &mut Ui, c: Vec2, r: f32, color: Color, streak: f32) {
    ui.arc(c, r, 0.0, std::f32::consts::TAU, 1.6, color);
    let tip = c + Vec2::new(r * 0.75, 0.0);
    for dy in [-1.0f32, 1.0] {
        ui.stroke(tip, c + Vec2::new(-r * 0.25, dy * r * 0.55), 2.2, color);
    }
    let lines = if streak >= 0.0 { 3 } else { 1 };
    for i in 0..lines {
        let off = if streak >= 0.0 {
            (streak * 3.0 + i as f32 * 0.33).fract()
        } else {
            0.5
        };
        let yy = (i as f32 - (lines - 1) as f32 * 0.5) * r * 0.28;
        let a = c + Vec2::new(-r * (1.5 + off * 0.6), yy);
        let b = c + Vec2::new(-r * (0.7 + off * 0.4), yy);
        ui.stroke(a, b, 1.4, color);
    }
}
