//! Survival's marks and small helpers, shared by the set-up screen, the siege
//! chart and the rules: domain colours and glyphs, the Progenitor's mark,
//! tooltips, and numbers as the screens print them.

use crate::hud::style::{AIR, LAND, NAVY};
use crate::survival::ENGINE_COLOR;
use crate::ui::{ink, palette, rgb, type_scale, Color, Rect, Style, Ui};
use glam::Vec2;
use mc_data::survival::Domain;
use mc_data::IconKind;
use std::f32::consts::{FRAC_PI_2, TAU};

pub(super) fn domain_index(d: Domain) -> usize {
    match d {
        Domain::Land => 0,
        Domain::Air => 1,
        Domain::Naval => 2,
    }
}

pub(super) fn domain_hex(d: Domain) -> u32 {
    match d {
        Domain::Land => LAND,
        Domain::Air => AIR,
        Domain::Naval => NAVY,
    }
}

/// A domain's colour. Navy is lifted a little on the chart, where it sits on dark sea.
pub(super) fn dcol(d: Domain, a: f32) -> Color {
    rgb(
        if d == Domain::Naval {
            0x4F86FF
        } else {
            domain_hex(d)
        },
        a,
    )
}

pub(super) fn engine(a: f32) -> Color {
    [ENGINE_COLOR[0], ENGINE_COLOR[1], ENGINE_COLOR[2], a]
}

pub(super) fn domain_icon(d: Domain) -> IconKind {
    match d {
        Domain::Land => IconKind::Tank,
        Domain::Air => IconKind::Fighter,
        Domain::Naval => IconKind::Ship,
    }
}

pub(super) fn domain_glyph(ui: &mut Ui, d: Domain, c: Vec2, r: f32, color: Color) {
    crate::hud::icons::strategic(ui, domain_icon(d), 0, c, r, color, ink(0.9));
}

pub(super) fn hexagon(c: Vec2, r: f32, turn: f32) -> [Vec2; 6] {
    std::array::from_fn(|k| c + Vec2::from_angle(-FRAC_PI_2 + turn + k as f32 * TAU / 6.0) * r)
}

pub(super) fn fill_hex(ui: &mut Ui, c: Vec2, r: f32, turn: f32, color: Color) {
    let p = hexagon(c, r, turn);
    for k in 0..6 {
        ui.triangle(c, p[k], p[(k + 1) % 6], color);
    }
}

pub(super) fn outline_hex(ui: &mut Ui, c: Vec2, r: f32, turn: f32, t: f32, color: Color) {
    let p = hexagon(c, r, turn);
    for k in 0..6 {
        ui.stroke(p[k], p[(k + 1) % 6], t, color);
    }
}

/// A ring of `n` dashes turned by `turn`.
pub(super) fn dashed_ring(
    ui: &mut Ui,
    c: Vec2,
    r: f32,
    n: usize,
    duty: f32,
    turn: f32,
    t: f32,
    color: Color,
) {
    let step = TAU / n as f32;
    for k in 0..n {
        let a = turn + k as f32 * step;
        ui.arc(c, r, a, a + step * duty, t, color);
    }
}

/// The Replication Engine's mark: a foundry hexagon with a core, and (when
/// `veil`) its slowly turning dashed veil.
pub fn engine_mark(ui: &mut Ui, c: Vec2, r: f32, a: f32, veil: bool) {
    let pulse = 0.5 + 0.5 * (ui.time * 2.2).sin();
    if veil {
        ui.disc(c, r * 2.6, engine(0.05 * a));
        ui.disc(c, r * 1.8, engine(0.07 * a));
        dashed_ring(
            ui,
            c,
            r * 2.1,
            18,
            0.55,
            ui.time * 0.25,
            1.3,
            engine(0.75 * a),
        );
        dashed_ring(
            ui,
            c,
            r * 2.6,
            9,
            0.3,
            -ui.time * 0.15,
            1.0,
            engine(0.35 * a),
        );
    }
    fill_hex(ui, c, r, 0.0, ink(0.9 * a));
    outline_hex(ui, c, r, 0.0, (r * 0.16).max(1.4), engine(a));
    fill_hex(ui, c, r * 0.46, 0.0, engine((0.55 + 0.4 * pulse) * a));
    // Three bays: short spokes from the core to alternate corners.
    let p = hexagon(c, r, 0.0);
    for k in [1, 3, 5] {
        ui.stroke(
            c + (p[k] - c) * 0.5,
            c + (p[k] - c) * 0.82,
            (r * 0.12).max(1.0),
            engine(0.9 * a),
        );
    }
}

pub(super) fn diamond(ui: &mut Ui, c: Vec2, r: f32, fill: Color, edge: Color) {
    let (n, e, s, w) = (
        c - Vec2::Y * r,
        c + Vec2::X * r,
        c + Vec2::Y * r,
        c - Vec2::X * r,
    );
    ui.triangle(n, e, s, fill);
    ui.triangle(n, s, w, fill);
    for (a, b) in [(n, e), (e, s), (s, w), (w, n)] {
        ui.stroke(a, b, 1.2, edge);
    }
}

pub(super) fn anchor(ui: &mut Ui, c: Vec2, r: f32, color: Color) {
    let t = (r * 0.2).max(1.3);
    ui.arc(c - Vec2::Y * r * 0.72, r * 0.2, 0.0, TAU, t, color);
    ui.stroke(c - Vec2::Y * r * 0.52, c + Vec2::Y * r * 0.85, t, color);
    ui.stroke(
        c + Vec2::new(-r * 0.45, -r * 0.25),
        c + Vec2::new(r * 0.45, -r * 0.25),
        t,
        color,
    );
    ui.arc(
        c + Vec2::Y * r * 0.1,
        r * 0.75,
        0.35,
        std::f32::consts::PI - 0.35,
        t,
        color,
    );
}

/// `text` cut short with an ellipsis to fit `width`, in `st` as given.
pub(super) fn clip(ui: &mut Ui, st: Style, text: &str, width: f32) -> String {
    if ui.text_width(st, text) <= width {
        return text.to_owned();
    }
    let mut cut = text.to_owned();
    while cut.pop().is_some() {
        let candidate = format!("{}\u{2026}", cut.trim_end());
        if ui.text_width(st, &candidate) <= width {
            return candidate;
        }
    }
    String::new()
}

/// A tooltip card: a title in `accent`, then lines of text.
pub(super) struct Tip {
    pub(super) at: Vec2,
    pub(super) title: String,
    pub(super) accent: Color,
    pub(super) lines: Vec<String>,
    pub(super) glow: f32,
}

pub(super) fn draw_tip(ui: &mut Ui, tip: &Tip, bounds: Rect) {
    const W: f32 = 270.0;
    let mut lines = Vec::new();
    for l in &tip.lines {
        lines.extend(ui.wrap(type_scale::MICRO, l, W - 24.0));
    }
    let tw = ui.text_width(type_scale::VALUE, &tip.title);
    let lw = lines
        .iter()
        .map(|l| ui.text_width(type_scale::MICRO, l))
        .fold(tw, f32::max);
    let w = (lw + 24.0).min(W);
    let h = 34.0 + lines.len() as f32 * 16.0;
    let mut y = tip.at.y + 22.0;
    if y + h > bounds.bottom() - 4.0 {
        y = tip.at.y - 22.0 - h;
    }
    let r = Rect::new(
        (tip.at.x - w * 0.5).clamp(bounds.x + 4.0, bounds.right() - w - 4.0),
        y,
        w,
        h,
    );
    let g = tip.glow;
    ui.fill(Rect::new(r.x + 3.0, r.y + 4.0, r.w, r.h), ink(0.4 * g));
    ui.fill(r, rgb(0x0B0C0E, 0.94 * g));
    ui.frame(r, rgb(palette::LINE, 0.22 * g));
    ui.fill(
        Rect::new(r.x, r.y, 3.0, r.h),
        [tip.accent[0], tip.accent[1], tip.accent[2], g],
    );
    ui.text(
        r.x + 12.0,
        r.y + 16.0,
        type_scale::VALUE,
        [tip.accent[0], tip.accent[1], tip.accent[2], g],
        &tip.title,
    );
    for (i, l) in lines.iter().enumerate() {
        ui.text(
            r.x + 12.0,
            r.y + 36.0 + i as f32 * 16.0,
            type_scale::MICRO,
            rgb(palette::DIM, g),
            l,
        );
    }
}

/// How far along a panel is in arriving: eased, and `delay` later than the screen.
pub(super) fn km(m: f32) -> String {
    format!("{:.1} km", m / 1000.0)
}

pub(super) fn mass(v: i64) -> String {
    if v >= 10_000_000 {
        format!("{:.0}M", v as f32 / 1e6)
    } else if v >= 1_000_000 {
        format!("{:.1}M", v as f32 / 1e6)
    } else if v >= 10_000 {
        format!("{:.0}k", v as f32 / 1000.0)
    } else if v >= 1000 {
        format!("{:.1}k", v as f32 / 1000.0)
    } else {
        format!("{v}")
    }
}

pub(super) fn label_row(ui: &mut Ui, r: Rect, label: &str, hint: &str) {
    ui.hline(r.x, r.bottom(), r.w, rgb(palette::LINE, 0.10));
    let end = ui.text(
        r.x + 16.0,
        r.mid_y(),
        type_scale::BODY,
        rgb(palette::TEXT, 0.82),
        label,
    );
    if !hint.is_empty() {
        ui.text(
            end + 12.0,
            r.mid_y() + 0.5,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            hint,
        );
    }
}
