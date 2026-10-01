//! The timeline page: the match's moments in order down a spine, and a chart that
//! shows where the one under the pointer happened. Picking one opens the
//! battlefield at it.

use super::analysis::{clock, region, MomentKind};
use super::{block, ease, Ctx, Report};
use crate::ui::{id, ink, palette, preview, rgb, type_scale, Rect, Ui};
use glam::Vec2;

#[derive(Default)]
pub struct State {
    /// Rows scrolled past the top.
    scroll: f32,
    /// The moment last under the pointer, which the chart shows.
    shown: Option<usize>,
}

fn kind_name(k: MomentKind) -> &'static str {
    match k {
        MomentKind::Start => "Deployment",
        MomentKind::FirstBlood => "First Blood",
        MomentKind::Tier => "Technology",
        MomentKind::Domain => "New Front",
        MomentKind::Expansion => "Expansion",
        MomentKind::Lead => "Lead Change",
        MomentKind::Salvage => "Salvage",
        MomentKind::Experimental => "Experimental",
        MomentKind::ExperimentalLost => "Experimental Lost",
        MomentKind::Warhead => "Warhead",
        MomentKind::Battle => "Battle",
        MomentKind::Defeat => "Defeat",
        MomentKind::End => "Decided",
    }
}

fn kind_tone(k: MomentKind) -> u32 {
    match k {
        MomentKind::Battle | MomentKind::End | MomentKind::Lead => palette::ACCENT,
        MomentKind::Warhead | MomentKind::Experimental => palette::WARN,
        MomentKind::Defeat | MomentKind::ExperimentalLost => palette::BAD,
        MomentKind::Salvage => super::SALVAGE,
        _ => palette::DIM,
    }
}

/// Returns the tick of a moment picked to see on the battlefield.
pub fn draw(report: &mut Report, ui: &mut Ui, ctx: &Ctx, r: Rect) -> Option<u32> {
    let left = Rect::new(r.x, r.y, r.w * 0.6, r.h);
    let right = Rect::new(
        left.right() + 28.0,
        r.y,
        r.right() - left.right() - 28.0,
        r.h,
    );
    let picked = list(report, ui, ctx, left);
    locator(report, ui, ctx, right);
    picked
}

fn list(report: &mut Report, ui: &mut Ui, ctx: &Ctx, r: Rect) -> Option<u32> {
    let inner = block(ui, r, "Moments of the Match");
    let row_h = 58.0;
    let count = report.a.moments.len();
    let fits = ((inner.h / row_h).floor() as usize).max(1);
    let most = count.saturating_sub(fits) as f32;
    let over = ui.interactive && inner.contains(ui.cursor - ui.shift);
    if over && ui.input.scroll != 0.0 {
        report.timeline.scroll =
            (report.timeline.scroll - ui.input.scroll.signum()).clamp(0.0, most);
    }
    let first = report.timeline.scroll as usize;
    let spine = inner.x + 92.0;
    ui.vline(spine, inner.y, inner.h, rgb(palette::LINE, 0.16));
    let mut picked = None;
    for (row, i) in (first..count).take(fits).enumerate() {
        let m = &report.a.moments[i];
        let k = ease((report.tab_age - 0.04 * row as f32) / 0.4);
        let y = inner.y + row as f32 * row_h;
        let line = Rect::new(inner.x, y, inner.w, row_h - 4.0);
        let res = ui.interact(id("report-moment", i), line, true);
        if res.hovered {
            report.timeline.shown = Some(i);
        }
        let lit = report.timeline.shown == Some(i);
        if res.glow > 0.0 || lit {
            ui.fill_cut(line, 4.0, rgb(0xFFFFFF, 0.025 + 0.04 * res.glow));
        }
        let mid = line.mid_y();
        ui.shift.x += 14.0 * (1.0 - k);
        ui.text_right(
            spine - 16.0,
            mid - 7.0,
            type_scale::VALUE,
            rgb(palette::TEXT, k),
            &clock(m.tick),
        );
        ui.text_right(
            spine - 16.0,
            mid + 10.0,
            type_scale::MICRO,
            rgb(kind_tone(m.kind), 0.9 * k),
            kind_name(m.kind),
        );
        // The node on the spine, in the side's colour.
        let tone = m.side.map_or(rgb(kind_tone(m.kind), 1.0), |s| {
            report.color(ctx, s as usize)
        });
        let c = Vec2::new(spine + 0.5, mid);
        let d = if lit { 7.0 } else { 5.5 };
        ui.dot(c, d + 4.0, ink(0.9 * k));
        ui.triangle(
            c - Vec2::Y * d,
            c + Vec2::X * d,
            c + Vec2::Y * d,
            [tone[0], tone[1], tone[2], k],
        );
        ui.triangle(
            c - Vec2::Y * d,
            c + Vec2::Y * d,
            c - Vec2::X * d,
            [tone[0], tone[1], tone[2], k],
        );
        let tx = spine + 22.0;
        let (st, title) = ui.fitted(type_scale::ITEM, &m.title, line.right() - tx - 120.0);
        ui.text(tx, mid - 8.0, st, rgb(palette::TEXT, k), &title);
        let (st, detail) = ui.fitted(type_scale::BODY, &m.detail, line.right() - tx - 120.0);
        ui.text(tx, mid + 11.0, st, rgb(palette::DIM, k), &detail);
        ui.text_right(
            line.right() - 14.0,
            mid,
            type_scale::MICRO,
            rgb(palette::FAINT, (0.4 + 0.6 * res.glow) * k),
            "See it on the map",
        );
        ui.shift.x -= 14.0 * (1.0 - k);
        if res.clicked {
            picked = Some(m.tick);
        }
    }
    // Where the list is when it runs past the page.
    if most > 0.0 {
        let bar_h = inner.h * fits as f32 / count as f32;
        let y = inner.y + (inner.h - bar_h) * report.timeline.scroll / most;
        ui.fill(
            Rect::new(inner.right() - 3.0, inner.y, 2.0, inner.h),
            rgb(palette::LINE, 0.06),
        );
        ui.fill(
            Rect::new(inner.right() - 3.0, y, 2.0, bar_h),
            rgb(palette::LINE, 0.5),
        );
    }
    picked
}

/// The chart, with the moment under the pointer marked on it.
fn locator(report: &Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let inner = block(ui, r, "Where");
    let side = inner.w.min(inner.h - 120.0);
    let chart = Rect::new(inner.x, inner.y, side, side);
    ui.fill(chart, ink(0.9));
    let px = preview::SIZE as f32;
    ui.image(
        ctx.chart,
        [0.0, 0.0, px, px],
        chart,
        [0.16, 0.17, 0.19, 1.0],
    );
    ui.brackets(chart.inset(-4.0), 14.0, rgb(palette::LINE, 0.5));
    let a = &report.a;
    let size = a.size;
    let at = |p: Vec2| super::battlefield::locate(size, chart, p);
    // Every placed moment, faint; the one in hand bright and ringed.
    for (i, m) in a.moments.iter().enumerate() {
        let Some(p) = m.at else { continue };
        if report.timeline.shown == Some(i) {
            continue;
        }
        let tone = rgb(kind_tone(m.kind), 0.45);
        ui.dot(at(p), 3.0, tone);
    }
    let Some(m) = report.timeline.shown.and_then(|i| a.moments.get(i)) else {
        ui.text(
            inner.x,
            chart.bottom() + 24.0,
            type_scale::BODY,
            rgb(palette::FAINT, 1.0),
            "Point at a moment to see where it happened",
        );
        return;
    };
    if let Some(p) = m.at {
        let c = at(p);
        let pulse = (ui.time * 1.5).fract();
        let tone = kind_tone(m.kind);
        ui.arc(
            c,
            10.0 + 26.0 * pulse,
            0.0,
            std::f32::consts::TAU,
            1.5,
            rgb(tone, 1.0 - pulse),
        );
        ui.arc(c, 8.0, 0.0, std::f32::consts::TAU, 2.0, rgb(tone, 1.0));
        ui.dot(c, 3.0, rgb(0xFFFFFF, 1.0));
        ui.hline(chart.x, c.y, chart.w, rgb(tone, 0.18));
        ui.vline(c.x, chart.y, chart.h, rgb(tone, 0.18));
    }
    let y = chart.bottom() + 24.0;
    ui.text(
        inner.x,
        y,
        type_scale::ITEM,
        rgb(palette::TEXT, 1.0),
        &m.title,
    );
    ui.text(
        inner.x,
        y + 22.0,
        type_scale::BODY,
        rgb(palette::DIM, 1.0),
        &m.detail,
    );
    let place = m.at.map_or("Across the map".to_string(), |p| {
        format!("In {}", region(p, size))
    });
    ui.text(
        inner.x,
        y + 44.0,
        type_scale::CAPTION,
        rgb(kind_tone(m.kind), 1.0),
        &format!("{}  \u{b7}  {}", clock(m.tick), place),
    );
}
