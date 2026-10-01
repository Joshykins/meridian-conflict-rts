//! The battlefield page: the match replayed on the map's chart. Armies as glowing
//! marks that move and swell, bases spreading, each commander's track, losses
//! flaring where they fell and leaving ash, the battles ringed and warheads
//! landing; a scrubber under it with the match's moments pinned along it.

use super::analysis::{clock, region, seconds, short, Analysis, MomentKind};
use super::{block, ease, Ctx, Report};
use crate::chronicle::{cell_centre, Frame, GRID};
use crate::ui::{id, ink, palette, preview, rgb, type_scale, Color, Rect, Ui};
use glam::Vec2;
use mc_core::TICKS_PER_SECOND;

/// Match seconds a second of playback, to pick from.
const SPEEDS: [f32; 4] = [15.0, 30.0, 60.0, 120.0];

/// Seconds a loss flares on the chart before it is ash.
const FLARE: f32 = 25.0;

/// Cells a side of the ash map is cut into.
const ASH: usize = 96;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Layer {
    Armies,
    Bases,
    Commanders,
    Losses,
    Battles,
    Warheads,
    Salvage,
}

const LAYERS: [Layer; 7] = [
    Layer::Armies,
    Layer::Bases,
    Layer::Commanders,
    Layer::Losses,
    Layer::Battles,
    Layer::Warheads,
    Layer::Salvage,
];

impl Layer {
    fn name(self) -> &'static str {
        match self {
            Layer::Armies => "Armies",
            Layer::Bases => "Bases",
            Layer::Commanders => "Commanders",
            Layer::Losses => "Losses",
            Layer::Battles => "Battles",
            Layer::Warheads => "Warheads",
            Layer::Salvage => "Salvage",
        }
    }
}

pub struct Playback {
    /// Where in the match the replay is, in ticks.
    t: f32,
    playing: bool,
    speed: usize,
    shown: [bool; LAYERS.len()],
    length: f32,
}

impl Playback {
    pub fn new(a: &Analysis) -> Playback {
        let length = a.length as f32;
        // About half a minute for the whole match.
        let want = seconds(a.length) / 30.0;
        let speed = SPEEDS
            .iter()
            .position(|&s| s >= want)
            .unwrap_or(SPEEDS.len() - 1);
        Playback {
            t: 0.0,
            playing: true,
            speed,
            shown: [true; LAYERS.len()],
            length,
        }
    }

    /// Stops the replay at `tick`.
    pub fn seek(&mut self, tick: u32) {
        self.t = (tick as f32).min(self.length);
        self.playing = false;
    }

    fn on(&self, layer: Layer) -> bool {
        LAYERS
            .iter()
            .position(|&l| l == layer)
            .is_some_and(|i| self.shown[i])
    }

    fn advance(&mut self, dt: f32) {
        if self.playing {
            self.t += dt * SPEEDS[self.speed] * TICKS_PER_SECOND as f32;
            if self.t >= self.length {
                self.t = self.length;
                self.playing = false;
            }
        }
    }
}

/// Where a point of the map lands on a chart drawn in `chart` (`preview::locate`).
pub(super) fn locate(size: Vec2, chart: Rect, at: Vec2) -> Vec2 {
    let longest = size.x.max(size.y);
    let k = chart.w / longest;
    let pad = Vec2::new(longest - size.x, longest - size.y) * 0.5 * k;
    Vec2::new(chart.x, chart.y) + Vec2::new(at.x * k, (size.y - at.y) * k) + pad
}

pub fn draw(report: &mut Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    report.field.advance(ui.dt);
    let scrub_h = 84.0;
    let side = (r.h - scrub_h - 20.0).min(r.w * 0.6);
    let chart = Rect::new(r.x, r.y, side, side);
    map(report, ui, ctx, chart);
    let panel = Rect::new(
        chart.right() + 28.0,
        r.y,
        r.right() - chart.right() - 28.0,
        side,
    );
    status(report, ui, ctx, panel);
    scrubber(
        report,
        ui,
        ctx,
        Rect::new(r.x, r.bottom() - scrub_h + 8.0, r.w, scrub_h - 8.0),
    );
}

/// The two snapshots either side of `t`, and how far from the first to the second.
fn frames_at(frames: &[Frame], t: f32) -> Option<(&Frame, &Frame, f32)> {
    let i = frames.partition_point(|f| f.tick as f32 <= t);
    let a = frames.get(i.checked_sub(1)?)?;
    let b = frames.get(i).unwrap_or(a);
    let k = if b.tick > a.tick {
        ((t - a.tick as f32) / (b.tick - a.tick) as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    Some((a, b, k))
}

fn map(report: &Report, ui: &mut Ui, ctx: &Ctx, chart: Rect) {
    let a = &report.a;
    let p = &report.field;
    let t = p.t;
    let size = a.size;
    let at = |v: Vec2| locate(size, chart, v);
    // Metres to points on the chart.
    let m = chart.w / size.x.max(size.y);
    ui.fill(chart, ink(0.9));
    let px = preview::SIZE as f32;
    ui.image(
        ctx.chart,
        [0.0, 0.0, px, px],
        chart,
        [0.13, 0.14, 0.16, 1.0],
    );
    // A plotting grid with lettered columns and numbered rows.
    for i in 1..8 {
        let f = i as f32 / 8.0;
        ui.vline(
            chart.x + chart.w * f,
            chart.y,
            chart.h,
            rgb(palette::LINE, 0.06),
        );
        ui.hline(
            chart.x,
            chart.y + chart.h * f,
            chart.w,
            rgb(palette::LINE, 0.06),
        );
    }
    for i in 0..8 {
        let f = (i as f32 + 0.5) / 8.0;
        let letter = ((b'A' + i as u8) as char).to_string();
        ui.text_centred(
            chart.x + chart.w * f,
            chart.y - 9.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &letter,
        );
        ui.text_right(
            chart.x - 6.0,
            chart.y + chart.h * f,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &format!("{}", i + 1),
        );
    }
    ui.brackets(chart.inset(-4.0), 14.0, rgb(palette::LINE, 0.5));

    let colors: Vec<Color> = (0..a.sides.len()).map(|i| report.color(ctx, i)).collect();
    let frames = frames_at(&a.frames, t);

    if p.on(Layer::Losses) {
        ash(ui, a, t, &at, &colors, chart);
    }
    if let Some(frames) = frames {
        forces(ui, report, ctx, frames, (size, chart, m));
    }
    if p.on(Layer::Commanders) {
        commanders(ui, a, t, &at, &colors);
    }
    if p.on(Layer::Losses) {
        // Fresh losses flare and fade.
        let from = t - FLARE * TICKS_PER_SECOND as f32;
        let start = a.fallen.partition_point(|f| (f.tick as f32) < from);
        for f in a.fallen[start..].iter().take_while(|f| f.tick as f32 <= t) {
            let age = (t - f.tick as f32) / (FLARE * TICKS_PER_SECOND as f32);
            let c = colors.get(f.owner as usize).copied().unwrap_or([1.0; 4]);
            let big = (f.value.sqrt() * 0.12).clamp(1.5, 9.0);
            let centre = at(f.pos);
            let ring = big * (1.0 + 2.5 * age.sqrt());
            ui.arc(
                centre,
                ring,
                0.0,
                std::f32::consts::TAU,
                1.0,
                [1.0, 0.85, 0.6, 0.7 * (1.0 - age)],
            );
            ui.dot(centre, big * 0.6, [c[0], c[1], c[2], 0.9 * (1.0 - age)]);
        }
    }
    if p.on(Layer::Warheads) {
        for b in a.blasts.iter().filter(|b| b.tick as f32 <= t) {
            let age = (t - b.tick as f32) / (12.0 * TICKS_PER_SECOND as f32);
            let centre = at(b.pos);
            let reach = 520.0 * m;
            if age < 1.0 {
                ui.disc(
                    centre,
                    reach * ease(age * 3.0),
                    [1.0, 0.7, 0.35, 0.35 * (1.0 - age)],
                );
                ui.arc(
                    centre,
                    reach * (0.4 + 0.8 * age),
                    0.0,
                    std::f32::consts::TAU,
                    2.0,
                    [1.0, 0.95, 0.85, 1.0 - age],
                );
            }
            ui.arc(
                centre,
                reach * 0.5,
                0.0,
                std::f32::consts::TAU,
                1.0,
                rgb(palette::WARN, 0.5),
            );
        }
    }
    if p.on(Layer::Salvage) {
        salvage(ui, a, t, &at, &colors, chart);
    }
    if p.on(Layer::Battles) {
        battles(ui, a, t, &at, m, chart);
    }
    // The hour in the corner of the chart.
    ui.text(
        chart.x + 12.0,
        chart.bottom() - 16.0,
        type_scale::VALUE,
        rgb(palette::TEXT, 0.9),
        &clock(t as u32),
    );
}

/// Bases and armies at the snapshot (or faded between two): the map's size, the chart
/// and points a metre come in a tuple.
fn forces(
    ui: &mut Ui,
    report: &Report,
    ctx: &Ctx,
    frames: (&Frame, &Frame, f32),
    (size, chart, m): (Vec2, Rect, f32),
) {
    let p = &report.field;
    let at = |v: Vec2| locate(size, chart, v);
    let colors: Vec<Color> = (0..report.a.sides.len())
        .map(|i| report.color(ctx, i))
        .collect();
    let cell_pt = size.x / GRID as f32 * m;
    let (f0, f1, k) = frames;
    if p.on(Layer::Bases) {
        for (side, sf) in f0.sides.iter().enumerate() {
            let c = colors[side];
            for &(cell, count) in &sf.bases {
                let centre = at(cell_centre(size, cell));
                let s = cell_pt * (0.45 + 0.1 * (count as f32).min(5.0));
                let r = Rect::new(centre.x - s * 0.5, centre.y - s * 0.5, s, s);
                ui.fill(r, [c[0], c[1], c[2], 0.4]);
            }
        }
    }
    if p.on(Layer::Armies) {
        // Each snapshot faded across into the next, so the armies glide.
        for (frame, alpha) in [(f0, 1.0 - k), (f1, k)] {
            if alpha <= 0.01 {
                continue;
            }
            for (side, sf) in frame.sides.iter().enumerate() {
                let c = colors[side];
                for &(cell, value) in &sf.army {
                    let centre = at(cell_centre(size, cell));
                    let rad = (2.4 + value.sqrt() * 0.1).min(11.0) * (chart.w / 640.0).max(0.6);
                    ui.dot(centre, rad * 2.4, [c[0], c[1], c[2], 0.10 * alpha]);
                    ui.dot(centre, rad, [c[0], c[1], c[2], 0.9 * alpha]);
                }
            }
        }
    }
}

/// Seconds a patch of reclaim glints on the chart after the work there.
const GLINT: f32 = 20.0;

/// Where each side was reclaiming lately: bright salvage glints in a halo of the
/// side's colour, fading as the work there gets older.
fn salvage(
    ui: &mut Ui,
    a: &Analysis,
    t: f32,
    at: &dyn Fn(Vec2) -> Vec2,
    colors: &[Color],
    chart: Rect,
) {
    let tps = TICKS_PER_SECOND as f32;
    let from = t - GLINT * tps;
    let first = a.frames.partition_point(|f| (f.tick as f32) < from);
    let scale = (chart.w / 640.0).max(0.6);
    for f in a.frames[first..].iter().take_while(|f| f.tick as f32 <= t) {
        let age = ((t - f.tick as f32) / (GLINT * tps)).clamp(0.0, 1.0);
        for (side, sf) in f.sides.iter().enumerate() {
            let c = colors.get(side).copied().unwrap_or([1.0; 4]);
            for &(cell, mass) in &sf.salvage {
                let p = at(cell_centre(a.size, cell));
                let rad = (1.5 + mass.sqrt() * 0.35).min(9.0) * scale;
                let fade = 1.0 - age;
                ui.dot(p, rad * 2.2, [c[0], c[1], c[2], 0.12 * fade]);
                ui.dot(p, rad * 0.8, rgb(super::SALVAGE, 0.9 * fade));
                // A glint that turns as it fades.
                let spin = Vec2::from_angle(ui.time * 1.5 + cell as f32) * rad * 1.6 * fade;
                ui.stroke(p - spin, p + spin, 1.0, rgb(0xFFFFFF, 0.6 * fade));
            }
        }
    }
}

/// Where units have fallen so far: a faint wash of ash in the losers' colours.
fn ash(
    ui: &mut Ui,
    a: &Analysis,
    t: f32,
    at: &dyn Fn(Vec2) -> Vec2,
    colors: &[Color],
    chart: Rect,
) {
    let mut cells = vec![(0.0f32, 0u8); ASH * ASH];
    let end = a.fallen.partition_point(|f| f.tick as f32 <= t);
    for f in &a.fallen[..end] {
        let k = (f.pos / a.size * ASH as f32).clamp(Vec2::ZERO, Vec2::splat(ASH as f32 - 1.0));
        let c = &mut cells[k.y as usize * ASH + k.x as usize];
        if f.value >= c.0 {
            c.1 = f.owner;
        }
        c.0 += f.value.max(5.0);
    }
    let s = chart.w / ASH as f32;
    for (i, &(v, owner)) in cells.iter().enumerate().filter(|(_, c)| c.0 > 0.0) {
        let centre = (Vec2::new((i % ASH) as f32, (i / ASH) as f32) + 0.5) / ASH as f32 * a.size;
        let p = at(centre);
        let c = colors.get(owner as usize).copied().unwrap_or([1.0; 4]);
        let heat = (v / 600.0).sqrt().min(1.0);
        ui.dot(
            p,
            s * (0.5 + 0.6 * heat),
            [c[0] * 0.6, c[1] * 0.4, c[2] * 0.3, 0.12 + 0.3 * heat],
        );
    }
}

/// Each commander's track so far, and where it is (or where it fell).
fn commanders(ui: &mut Ui, a: &Analysis, t: f32, at: &dyn Fn(Vec2) -> Vec2, colors: &[Color]) {
    for (side, c) in colors.iter().enumerate() {
        let track: Vec<Vec2> = a
            .frames
            .iter()
            .take_while(|f| f.tick as f32 <= t)
            .filter_map(|f| f.sides.get(side).and_then(|s| s.commander))
            .map(at)
            .collect();
        if track.len() >= 2 {
            ui.polyline(&track, 1.4, [c[0], c[1], c[2], 0.55], false);
        }
        let fell = a.sides[side].defeated_at.filter(|&d| d as f32 <= t);
        let Some(&head) = track.last() else {
            continue;
        };
        if fell.is_some() {
            let d = 6.0;
            ui.stroke(
                head - Vec2::splat(d),
                head + Vec2::splat(d),
                2.0,
                rgb(palette::BAD, 1.0),
            );
            ui.stroke(
                head + Vec2::new(-d, d),
                head + Vec2::new(d, -d),
                2.0,
                rgb(palette::BAD, 1.0),
            );
            continue;
        }
        // A diamond with a slow pulse.
        let pulse = 0.5 + 0.5 * (ui.time * 3.0 + side as f32).sin();
        ui.arc(
            head,
            9.0 + 3.0 * pulse,
            0.0,
            std::f32::consts::TAU,
            1.2,
            [c[0], c[1], c[2], 0.5 * (1.0 - pulse) + 0.2],
        );
        let d = 6.0;
        let (n, e, s, w) = (
            head - Vec2::Y * d,
            head + Vec2::X * d,
            head + Vec2::Y * d,
            head - Vec2::X * d,
        );
        ui.triangle(n, e, s, [1.0, 1.0, 1.0, 0.95]);
        ui.triangle(n, s, w, [1.0, 1.0, 1.0, 0.95]);
        ui.triangle(n, e, s, [c[0], c[1], c[2], 0.6]);
        ui.triangle(n, s, w, [c[0], c[1], c[2], 0.6]);
    }
}

/// The battles: ringed while they are fought, numbered after.
fn battles(ui: &mut Ui, a: &Analysis, t: f32, at: &dyn Fn(Vec2) -> Vec2, m: f32, chart: Rect) {
    let tps = TICKS_PER_SECOND as f32;
    for (i, b) in a.battles.iter().enumerate() {
        let from = b.from as f32 - 5.0 * tps;
        if t < from {
            continue;
        }
        let centre = at(b.at);
        let reach = (b.radius * 1.4).max(140.0) * m;
        let live = t <= b.to as f32 + 10.0 * tps;
        if live {
            let pulse = (ui.time * 2.0).fract();
            ui.arc(
                centre,
                reach,
                0.0,
                std::f32::consts::TAU,
                1.6,
                rgb(palette::ACCENT, 0.95),
            );
            ui.arc(
                centre,
                reach * (1.0 + 0.5 * pulse),
                0.0,
                std::f32::consts::TAU,
                1.0,
                rgb(palette::ACCENT, 0.6 * (1.0 - pulse)),
            );
            let label = format!("{}  {}", i + 1, short(b.value));
            let w = ui.text_width(type_scale::CAPTION, &label) + 16.0;
            let x = (centre.x + reach * 0.7).min(chart.right() - w - 4.0);
            let y = (centre.y - reach * 0.7 - 12.0).max(chart.y + 4.0);
            let tag = Rect::new(x, y, w, 20.0);
            ui.fill(tag, ink(0.75));
            ui.fill(
                Rect::new(tag.x, tag.y, 2.0, tag.h),
                rgb(palette::ACCENT, 1.0),
            );
            ui.text(
                tag.x + 9.0,
                tag.mid_y(),
                type_scale::CAPTION,
                rgb(palette::TEXT, 1.0),
                &label,
            );
        } else {
            ui.arc(
                centre,
                reach,
                0.0,
                std::f32::consts::TAU,
                1.0,
                rgb(palette::ACCENT, 0.35),
            );
            ui.text_centred(
                centre.x,
                centre.y,
                type_scale::CAPTION,
                rgb(palette::ACCENT, 0.8),
                &format!("{}", i + 1),
            );
        }
    }
}

/// The right-hand panel: the hour, each side's strength at it, what just happened,
/// and the layers.
fn status(report: &mut Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let t = report.field.t;
    let a = &report.a;
    let inner = block(ui, r, "At This Point");
    ui.text(
        inner.x,
        inner.y + 26.0,
        crate::ui::style(mc_render::Face::Light, 40.0, 0.5),
        rgb(palette::TEXT, 1.0),
        &clock(t as u32),
    );
    ui.text(
        inner.x + 130.0,
        inner.y + 32.0,
        type_scale::CAPTION,
        rgb(palette::FAINT, 1.0),
        &format!("of {}", clock(a.length)),
    );
    // Each side's army worth now, against the most any side fielded.
    let k = a
        .times
        .partition_point(|&s| s * TICKS_PER_SECOND as f32 <= t)
        .saturating_sub(1);
    let most = a.sides.iter().map(|s| s.peak_army).fold(1.0f32, f32::max);
    let mut y = inner.y + 66.0;
    let row_h = ((inner.h * 0.42 - 66.0) / a.sides.len().max(1) as f32).clamp(18.0, 34.0);
    for (i, s) in a.sides.iter().enumerate() {
        let v = a
            .curve(super::analysis::Metric::ArmyValue, i)
            .get(k)
            .copied()
            .unwrap_or(0.0);
        let fell = s.defeated_at.is_some_and(|d| d as f32 <= t);
        let c = report.color(ctx, i);
        ui.fill(Rect::new(inner.x, y - 4.0, 8.0, 8.0), c);
        let (st, name) = ui.fitted(type_scale::CAPTION, &s.name, 140.0);
        ui.text(
            inner.x + 14.0,
            y,
            st,
            rgb(palette::TEXT, if fell { 0.4 } else { 0.9 }),
            &name,
        );
        let bar = Rect::new(inner.x + 170.0, y - 3.0, inner.w - 240.0, 6.0);
        ui.fill(bar, rgb(palette::LINE, 0.05));
        ui.fill(
            Rect::new(bar.x, bar.y, bar.w * v / most, bar.h),
            [c[0], c[1], c[2], 0.85],
        );
        ui.text_right(
            inner.right(),
            y,
            type_scale::VALUE,
            rgb(if fell { palette::BAD } else { palette::TEXT }, 1.0),
            &if fell { "Fallen".to_string() } else { short(v) },
        );
        y += row_h;
    }
    // What happened last.
    y += 10.0;
    ui.section(inner.x, y, inner.w, "Latest");
    y += 24.0;
    let recent: Vec<_> = a
        .moments
        .iter()
        .filter(|m| m.tick as f32 <= t && m.kind != MomentKind::Start)
        .collect();
    let latest_top = y;
    for m in recent.iter().rev().take(4) {
        let fade = (1.0 - (t - m.tick as f32) / (240.0 * TICKS_PER_SECOND as f32)).clamp(0.35, 1.0);
        let tone = m
            .side
            .map_or(rgb(palette::ACCENT, 1.0), |s| report.color(ctx, s as usize));
        ui.fill(
            Rect::new(inner.x, y - 6.0, 2.0, 30.0),
            [tone[0], tone[1], tone[2], fade],
        );
        ui.text(
            inner.x + 10.0,
            y,
            type_scale::MICRO,
            rgb(palette::FAINT, fade),
            &clock(m.tick),
        );
        let (st, title) = ui.fitted(type_scale::CAPTION, &m.title, inner.w - 70.0);
        ui.text(inner.x + 56.0, y, st, rgb(palette::TEXT, fade), &title);
        let (st, detail) = ui.fitted(type_scale::MICRO, &m.detail, inner.w - 20.0);
        ui.text(
            inner.x + 10.0,
            y + 15.0,
            st,
            rgb(palette::DIM, fade),
            &detail,
        );
        y += 38.0;
    }
    // The tide of the fighting: every side's army over the match, the replay's hour on it.
    let ly = inner.bottom() - 2.0 * 34.0;
    let top = latest_top + 4.0 * 38.0 + 6.0;
    if ly - 18.0 - top > 90.0 {
        tide(
            report,
            ui,
            ctx,
            Rect::new(inner.x, top, inner.w, ly - 18.0 - top),
        );
    }
    layers(report, ui, Rect::new(inner.x, ly, inner.w, 2.0 * 34.0));
}

/// The layers of the chart, as switches.
fn layers(report: &mut Report, ui: &mut Ui, r: Rect) {
    let (inner, ly) = (r, r.y);
    let lw = (inner.w - 15.0) / 4.0;
    for (i, layer) in LAYERS.iter().enumerate() {
        let cell = Rect::new(
            inner.x + (i % 4) as f32 * (lw + 5.0),
            ly + (i / 4) as f32 * 34.0,
            lw,
            28.0,
        );
        let on = report.field.shown[i];
        let res = ui.tile(id("report-layer", i), cell, false, true);
        ui.fill(
            Rect::new(cell.x + 10.0, cell.mid_y() - 4.0, 8.0, 8.0),
            rgb(if on { palette::ACCENT } else { palette::FAINT }, 1.0),
        );
        ui.text(
            cell.x + 26.0,
            cell.mid_y() - 1.0,
            type_scale::CAPTION,
            rgb(palette::TEXT, if on { 0.95 } else { 0.5 + 0.3 * res.glow }),
            layer.name(),
        );
        if res.clicked {
            ui.audio.play(crate::audio::Sfx::Tick);
            report.field.shown[i] = !on;
        }
    }
}

/// Every side's army worth over the match, with the replay's hour marked and what is
/// still to come dimmed.
fn tide(report: &Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let inner = block(ui, r, "Tide of Battle");
    let a = &report.a;
    let colors: Vec<Color> = (0..a.sides.len()).map(|i| report.color(ctx, i)).collect();
    let series: Vec<super::chart::Series> = (0..a.sides.len())
        .map(|i| super::chart::Series {
            values: a.curve(super::analysis::Metric::ArmyValue, i),
            color: colors[i],
            label: &a.sides[i].name,
        })
        .collect();
    let plot = Rect::new(
        inner.x + 40.0,
        inner.y + 6.0,
        inner.w - 40.0,
        inner.h - 30.0,
    );
    super::chart::lines(
        ui,
        &super::chart::Lines {
            id: id("report-tide", 0),
            r: plot,
            times: &a.times,
            length: seconds(a.length),
            series: &series,
            unit: "",
            reveal: 1.0,
            focus: None,
        },
    );
    let x = plot.x + plot.w * (report.field.t / a.length.max(1) as f32).clamp(0.0, 1.0);
    ui.fill(Rect::new(x, plot.y, plot.right() - x, plot.h), ink(0.55));
    ui.fill(
        Rect::new(x - 1.0, plot.y, 2.0, plot.h),
        rgb(palette::ACCENT, 1.0),
    );
}

/// Play and pause, the speed, and the match's length as a track to drag along, with
/// the intensity of the fighting under it and the moments pinned over it.
fn scrubber(report: &mut Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let a = &report.a;
    let length = a.length.max(1) as f32;
    // Play / pause.
    let play = Rect::new(r.x, r.y + 16.0, 52.0, 44.0);
    let res = ui.tile(id("report-play", 0), play, report.field.playing, true);
    let c = Vec2::new(play.x + play.w * 0.5, play.mid_y());
    if report.field.playing {
        ui.fill(
            Rect::new(c.x - 7.0, c.y - 8.0, 5.0, 16.0),
            rgb(palette::TEXT, 1.0),
        );
        ui.fill(
            Rect::new(c.x + 2.0, c.y - 8.0, 5.0, 16.0),
            rgb(palette::TEXT, 1.0),
        );
    } else {
        ui.triangle(
            c + Vec2::new(-6.0, -9.0),
            c + Vec2::new(10.0, 0.0),
            c + Vec2::new(-6.0, 9.0),
            rgb(palette::TEXT, 0.8 + 0.2 * res.glow),
        );
    }
    if res.clicked {
        ui.audio.play(crate::audio::Sfx::Tick);
        let f = &mut report.field;
        if !f.playing && f.t >= f.length {
            f.t = 0.0;
        }
        f.playing = !f.playing;
    }
    // Speeds.
    let mut sx = r.right() - SPEEDS.len() as f32 * 52.0;
    for (i, s) in SPEEDS.iter().enumerate() {
        let chip = Rect::new(sx, r.y + 24.0, 46.0, 28.0);
        let res = ui.tile(id("report-speed", i), chip, report.field.speed == i, true);
        ui.text_centred(
            chip.x + chip.w * 0.5,
            chip.mid_y() - 1.0,
            type_scale::CAPTION,
            rgb(
                palette::TEXT,
                if report.field.speed == i {
                    1.0
                } else {
                    0.55 + 0.3 * res.glow
                },
            ),
            &format!("\u{d7}{s:.0}"),
        );
        if res.clicked {
            ui.audio.play(crate::audio::Sfx::Tick);
            report.field.speed = i;
        }
        sx += 52.0;
    }
    let track = Rect::new(
        play.right() + 24.0,
        r.y + 22.0,
        r.right() - SPEEDS.len() as f32 * 52.0 - 24.0 - play.right() - 24.0,
        32.0,
    );
    // The fighting under the track, faint.
    let colors: Vec<Color> = (0..a.sides.len()).map(|i| report.color(ctx, i)).collect();
    let n = a.intensity.len().max(1) as f32;
    let most = a
        .intensity
        .iter()
        .map(|b| b.iter().sum::<f32>())
        .fold(1.0f32, f32::max);
    ui.fill(track, ink(0.4));
    for (i, b) in a.intensity.iter().enumerate() {
        let mut y = track.bottom();
        for (side, &v) in b.iter().enumerate().filter(|(_, v)| **v > 0.0) {
            let h = track.h * 0.9 * v / most;
            let c = colors[side];
            ui.fill(
                Rect::new(
                    track.x + track.w * i as f32 / n,
                    y - h,
                    (track.w / n - 1.0).max(1.0),
                    h,
                ),
                [c[0], c[1], c[2], 0.35],
            );
            y -= h;
        }
    }
    super::chart::time_axis(
        ui,
        Rect::new(track.x, track.y, track.w, track.h),
        seconds(a.length),
    );
    // The played part, and the head.
    let x = track.x + track.w * report.field.t / length;
    ui.fill(
        Rect::new(track.x, track.y, x - track.x, 2.0),
        rgb(palette::ACCENT, 0.9),
    );
    ui.fill(
        Rect::new(x - 1.0, track.y - 6.0, 2.0, track.h + 12.0),
        rgb(palette::ACCENT, 1.0),
    );
    ui.triangle(
        Vec2::new(x - 6.0, track.y - 12.0),
        Vec2::new(x + 6.0, track.y - 12.0),
        Vec2::new(x, track.y - 5.0),
        rgb(palette::ACCENT, 1.0),
    );
    // The moments, pinned over the track.
    let mut hover = None;
    for (i, m) in a.moments.iter().enumerate() {
        if m.kind == MomentKind::Start {
            continue;
        }
        let mx = track.x + track.w * m.tick as f32 / length;
        let pin = Rect::new(mx - 5.0, track.y - 5.0 - 13.0, 10.0, 10.0);
        let tone = m
            .side
            .map_or(rgb(palette::ACCENT, 1.0), |s| report.color(ctx, s as usize));
        let res = ui.interact_with(id("report-pin", i), pin.inset(-3.0), true, false);
        let big = 1.0 + 0.5 * res.glow;
        let c = Vec2::new(mx, pin.mid_y());
        let d = 4.0 * big;
        ui.triangle(c - Vec2::Y * d, c + Vec2::X * d, c + Vec2::Y * d, tone);
        ui.triangle(c - Vec2::Y * d, c + Vec2::Y * d, c - Vec2::X * d, tone);
        if res.hovered {
            hover = Some(i);
        }
        if res.clicked {
            ui.audio.play(crate::audio::Sfx::Tick);
            report.field.seek(m.tick);
        }
    }
    // Dragging along the track scrubs.
    let res = ui.interact_with(id("report-track", 0), track.inset(-4.0), true, false);
    if res.held {
        let f = ((ui.cursor.x - ui.shift.x - track.x) / track.w).clamp(0.0, 1.0);
        report.field.t = f * length;
        report.field.playing = false;
    }
    if let Some(i) = hover {
        let m = &a.moments[i];
        let place =
            m.at.map_or(String::new(), |p| format!(" \u{b7} {}", region(p, a.size)));
        let title = format!("{}  {}{}", clock(m.tick), m.title, place);
        let w = ui
            .text_width(type_scale::CAPTION, &title)
            .max(ui.text_width(type_scale::MICRO, &m.detail))
            + 24.0;
        let mx = track.x + track.w * m.tick as f32 / length;
        let card = Rect::new(
            (mx - w * 0.5).clamp(r.x, r.right() - w),
            track.y - 70.0,
            w,
            46.0,
        );
        ui.frost_cut(card, 5.0, 0.94);
        ui.bevel(card, 5.0, 0.6);
        ui.text(
            card.x + 12.0,
            card.y + 15.0,
            type_scale::CAPTION,
            rgb(palette::TEXT, 1.0),
            &title,
        );
        ui.text(
            card.x + 12.0,
            card.y + 32.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            &m.detail,
        );
    }
}
