//! A gravitic engine's panel (the Regency's The Fallen, mc-sim `gravitic.rs`), in the titan
//! panel's place right of the order card: its tier, its work (idle, gathering, the rift
//! running, the well holding or faltering) with a bar that fills as it goes, what its well
//! does, and the fracture button (J): pick a point anywhere and it tears a rift there
//! (`Command::Fracture`). Aiming again lets the old well go; a stop lets it go too.

use super::{Hud, HudAction, Scene};
use crate::audio::Sfx;
use crate::game::{Mode, Targeting};
use crate::gravitic_marks::WELL;
use crate::titan_marks::{metres, TITAN};
use crate::ui::{id, ink, palette, rgb, style, type_scale, Color, Rect, Ui};
use glam::Vec2;
use mc_render::Face;
use mc_sim::mirror::{UnitInstance, WellStage};

const TPS: f32 = mc_core::TICKS_PER_SECOND as f32;
const HOT: u32 = 0xF0E6FF;

/// The first of the local side's finished gravitic engines among `units`, and how many.
pub fn engine_of<'a>(s: &Scene, units: &[&'a UnitInstance]) -> Option<(&'a UnitInstance, usize)> {
    let ours: Vec<&&UnitInstance> = units
        .iter()
        .filter(|u| {
            (u.owner_flags & 0xFF) as u8 == s.view.local
                && !super::has_flag(u, mc_sim::tables::flag::UNDER_CONSTRUCTION)
                && s.bp(u).gravitic.is_some()
        })
        .collect();
    ours.first().map(|u| (**u, ours.len()))
}

/// The engine's panel over `r`.
pub fn panel(hud: &mut Hud, ui: &mut Ui, s: &Scene, u: &UnitInstance, count: usize, r: Rect) {
    let bp = s.bp(u);
    let Some(g) = bp.gravitic.as_ref() else {
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

    // Its work, and where it stands.
    let work = s.view.frame.wells.iter().find(|w| w.unit_id == u.unit_id);
    ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Well");
    let (label, tone, fill, shimmer) = match work {
        None => ("Idle".to_owned(), palette::DIM, 0.0, 0.0),
        Some(w) if !w.powered => (
            "Faltering: no power".to_owned(),
            palette::WARN,
            w.progress,
            0.3 * (t * 9.0).sin().abs(),
        ),
        Some(w) => match w.stage {
            WellStage::Gathering => (
                format!("Gathering  {:.0} s", w.left.ceil()),
                HOT,
                w.progress,
                0.15 + 0.3 * (t * (4.0 + 16.0 * w.progress)).sin().abs(),
            ),
            WellStage::Running => (
                format!("Rift running  {:.1} s", w.left),
                HOT,
                w.progress,
                0.45 + 0.35 * (t * 25.0).sin(),
            ),
            WellStage::Holding => (
                format!("Holding  {}", clock(w.seconds)),
                WELL,
                1.0,
                0.1 + 0.12 * (t * 1.6).sin(),
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
    let facts = [
        ("Well", metres(g.radius.to_f32()), WELL),
        (
            "Crush",
            format!("{:.0}/s", g.damage.to_f32()),
            palette::TEXT,
        ),
        (
            "Slows to",
            format!("{:.0}%", g.slow.to_f32() * 100.0),
            palette::TEXT,
        ),
        (
            "Gather",
            format!("{:.0} s", g.gather_ticks as f32 / TPS),
            palette::TEXT,
        ),
        (
            "Rift",
            format!("{:.0} m/s", g.step.to_f32() * TPS),
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

    // The fracture button: pick a point anywhere.
    let foot = r.bottom() - 30.0;
    let h = (foot - y).clamp(40.0, 58.0);
    let button = Rect::new(x, foot - h, cw, h);
    let aiming = matches!(s.view.mode, Mode::Target(Targeting::Fracture));
    let res = ui.interact(id("titan-fracture", 0), button, true);
    hud.claim(ui, button);
    let hot = if aiming { 1.0 } else { res.glow };
    ui.fill_cut(button, 7.0, ink(0.66));
    ui.gradient_h(button, rgb(WELL, 0.12 + 0.24 * hot), rgb(WELL, 0.02));
    ui.outline_cut(
        button,
        7.0,
        rgb(WELL, 0.35 + 0.45 * hot),
        rgb(0xFFFFFF, 0.3 + 0.4 * hot),
    );
    glyph(
        ui,
        Vec2::new(button.x + 28.0, button.mid_y()),
        11.0,
        rgb(WELL, 0.95),
        aiming.then_some(t),
    );
    let big = style(Face::Bold, 19.0, 1.0);
    let title = if aiming {
        "Choose where"
    } else {
        "Fracture  [ J ]"
    };
    ui.text(
        button.x + 52.0,
        button.mid_y() - 5.0,
        big,
        rgb(0xFFFFFF, 0.85 + 0.15 * hot),
        title,
    );
    let sub = if aiming {
        "Click anywhere on the map  \u{b7}  RMB cancels"
    } else if work.is_some() {
        "Aim again to move the well  \u{b7}  Stop lets it go"
    } else {
        "Tear a rift to any point and hold a well there"
    };
    ui.text_fit_left(
        button.x + 52.0,
        button.mid_y() + 13.0,
        button.w - 90.0,
        type_scale::MICRO,
        rgb(WELL, 0.9),
        sub,
    );
    if res.clicked {
        ui.audio.play(Sfx::Select);
        hud.actions.push(HudAction::Target(Targeting::Fracture));
    }
    ui.text_fit_left(
        x,
        r.bottom() - 14.0,
        cw,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        "Tears aircraft down  \u{b7}  a shield dome takes the crush for what is under it",
    );
}

/// `s` seconds as m:ss.
fn clock(s: f32) -> String {
    let s = s.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The fracture's mark: a ring with a jagged crack run out of it, the crack flickering
/// while a point is being chosen (`flicker` the time).
pub fn glyph(ui: &mut Ui, c: Vec2, r: f32, color: Color, flicker: Option<f32>) {
    ui.arc(c, r * 0.55, 0.0, std::f32::consts::TAU, 1.6, color);
    ui.disc(c, r * 0.18, color);
    let crack = [
        (0.55, 0.0),
        (0.85, -0.25),
        (1.05, 0.12),
        (1.35, -0.1),
        (1.6, 0.18),
    ];
    let on = flicker.is_none_or(|t| (t * 12.0).sin() > -0.6);
    if on {
        for pair in crack.windows(2) {
            let a = c + Vec2::new(pair[0].0, pair[0].1) * r;
            let b = c + Vec2::new(pair[1].0, pair[1].1) * r;
            ui.stroke(a, b, 2.0, color);
        }
    }
}
