//! A ship's Edict panel (the Regent's main gun, mc-sim `edict.rs`), in the titan panel's
//! place right of the order card: its tier, the Edict's state (ready, gathering, stalled,
//! the wave burning, reloading) with a bar that fills as it goes, what it does, and the
//! Edict button (J): pick a point and the ship closes to its reach and fires there
//! (`Command::Edict`). A stop lets a gather go.

use super::{Hud, HudAction, Scene};
use crate::audio::Sfx;
use crate::edict_marks::EDICT;
use crate::game::{Mode, Targeting};
use crate::titan_marks::{metres, TITAN};
use crate::ui::{id, ink, palette, rgb, style, type_scale, Color, Rect, Ui};
use glam::Vec2;
use mc_render::Face;
use mc_sim::mirror::{EdictStage, UnitInstance};

const TPS: f32 = mc_core::TICKS_PER_SECOND as f32;
const HOT: u32 = 0xFFF1E6;

/// The panel over `r` for `u`, a ship with an Edict.
pub fn panel(hud: &mut Hud, ui: &mut Ui, s: &Scene, u: &UnitInstance, count: usize, r: Rect) {
    let bp = s.bp(u);
    let Some(e) = bp.edict.as_ref() else {
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

    // The Edict's state.
    let work = s.view.frame.edicts.iter().find(|w| w.unit_id == u.unit_id);
    let reload = e.reload_ticks as f32 / TPS;
    ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Edict");
    let (label, tone, fill, shimmer) = match work {
        None => (
            "Ready".to_owned(),
            EDICT,
            1.0,
            0.08 + 0.08 * (t * 1.4).sin(),
        ),
        Some(w) => match w.stage {
            EdictStage::Gathering if !w.powered => (
                "Stalled: no power".to_owned(),
                palette::WARN,
                w.progress,
                0.3 * (t * 9.0).sin().abs(),
            ),
            EdictStage::Gathering => (
                format!("Gathering  {:.0} s", w.left.ceil()),
                HOT,
                w.progress,
                0.15 + 0.35 * (t * (4.0 + 18.0 * w.progress)).sin().abs(),
            ),
            EdictStage::Fired if w.seconds < w.wave_life => {
                ("Burning".to_owned(), HOT, 0.0, 0.5 + 0.3 * (t * 20.0).sin())
            }
            EdictStage::Fired => (
                format!("Reloading  {}", clock(w.left)),
                palette::DIM,
                1.0 - w.left / reload.max(1.0),
                0.0,
            ),
        },
    };
    ui.text_right(x + cw, y, type_scale::VALUE, rgb(tone, 1.0), &label);
    y += 10.0;
    let bar = Rect::new(x, y, cw, 10.0);
    ui.fill_cut(bar, 3.0, ink(0.6));
    let inner = Rect::new(bar.x + 2.0, bar.y + 2.0, (bar.w - 4.0) * fill, bar.h - 4.0);
    ui.gradient_h(inner, rgb(tone, 0.5), rgb(tone, 0.95));
    ui.fill(inner, rgb(0xFFFFFF, shimmer.max(0.0)));
    for k in 1..10 {
        ui.vline(
            bar.x + bar.w * k as f32 / 10.0,
            bar.y + 2.0,
            bar.h - 4.0,
            ink(0.35),
        );
    }
    ui.frame(bar, rgb(tone, 0.4));
    y += 24.0;

    // What it does, a column each: label over value.
    let gather = e.gather_ticks as f32 / TPS;
    let facts = [
        ("Blast", metres(e.blast.radius.to_f32()), EDICT),
        ("Kills to", metres(e.blast.core.to_f32()), palette::TEXT),
        ("Reach", metres(e.range.to_f32()), palette::TEXT),
        ("Gather", format!("{gather:.0} s"), palette::TEXT),
        (
            "Draws",
            format!("{:.0}k", e.upkeep.to_f32() * gather / 1000.0),
            palette::TEXT,
        ),
    ];
    let widths = [0.2, 0.2, 0.2, 0.19, 0.21];
    let mut fx = x;
    for ((k, v, tone), share) in facts.iter().zip(widths) {
        ui.text(fx, y, type_scale::MICRO, rgb(palette::FAINT, 1.0), k);
        ui.text(fx, y + 17.0, type_scale::VALUE, rgb(*tone, 0.95), v);
        fx += cw * share;
    }
    y += 34.0;

    // The Edict button: pick a point.
    let foot = r.bottom() - 30.0;
    let h = (foot - y).clamp(40.0, 58.0);
    let button = Rect::new(x, foot - h, cw, h);
    let aiming = matches!(s.view.mode, Mode::Target(Targeting::Edict));
    let res = ui.interact(id("titan-edict", 0), button, true);
    hud.claim(ui, button);
    let hot = if aiming { 1.0 } else { res.glow };
    ui.fill_cut(button, 7.0, ink(0.66));
    ui.gradient_h(button, rgb(EDICT, 0.12 + 0.24 * hot), rgb(EDICT, 0.02));
    ui.outline_cut(
        button,
        7.0,
        rgb(EDICT, 0.35 + 0.45 * hot),
        rgb(0xFFFFFF, 0.3 + 0.4 * hot),
    );
    glyph(
        ui,
        Vec2::new(button.x + 28.0, button.mid_y()),
        11.0,
        rgb(EDICT, 0.95),
        aiming.then_some(t),
    );
    let big = style(Face::Bold, 19.0, 1.0);
    let title = if aiming {
        "Choose where"
    } else {
        "Edict  [ J ]"
    };
    ui.text(
        button.x + 52.0,
        button.mid_y() - 5.0,
        big,
        rgb(0xFFFFFF, 0.85 + 0.15 * hot),
        title,
    );
    let sub = if aiming {
        "Click a point  \u{b7}  shift queues  \u{b7}  RMB cancels"
    } else if work.is_some_and(|w| w.stage == EdictStage::Gathering) {
        "Aim again to turn it  \u{b7}  Stop lets it go"
    } else {
        "Closes to its reach, gathers and fires"
    };
    ui.text_fit_left(
        button.x + 52.0,
        button.mid_y() + 13.0,
        button.w - 90.0,
        type_scale::MICRO,
        rgb(EDICT, 0.9),
        sub,
    );
    if res.clicked {
        ui.audio.play(Sfx::Select);
        hud.actions.push(HudAction::Target(Targeting::Edict));
    }
    ui.text_fit_left(
        x,
        r.bottom() - 14.0,
        cw,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        "Every other gun falls silent while it gathers  \u{b7}  it burns every side's units",
    );
}

/// `s` seconds as m:ss.
fn clock(s: f32) -> String {
    let s = s.max(0.0).ceil() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The Edict's mark: a column striking down into a ring, the column flickering while a
/// point is being chosen (`flicker` the time).
fn glyph(ui: &mut Ui, c: Vec2, r: f32, color: Color, flicker: Option<f32>) {
    let ground = c + Vec2::new(0.0, r * 0.7);
    ui.arc(ground, r * 0.9, 0.0, std::f32::consts::TAU, 1.6, color);
    ui.disc(ground, r * 0.22, color);
    let on = flicker.is_none_or(|t| (t * 12.0).sin() > -0.6);
    if on {
        ui.stroke(c - Vec2::new(0.0, r * 1.2), ground, 2.6, color);
        ui.stroke(
            c - Vec2::new(r * 0.5, r * 1.2),
            c - Vec2::new(-r * 0.5, r * 1.2),
            1.6,
            color,
        );
    }
}
