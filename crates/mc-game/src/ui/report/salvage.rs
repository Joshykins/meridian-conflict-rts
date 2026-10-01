//! Reclaim on the economy page: where each side's materials came from (mined
//! against reclaimed), where on the map it salvaged (the battles ringed over it,
//! whose fields those were), and the richest wrecks it hauled in.

use super::analysis::{clock, region, short};
use super::tones::{MATERIALS as MINED, SALVAGE};
use super::{block, ease, Ctx, Report};
use crate::ui::{ink, palette, preview, rgb, type_scale, Rect, Ui};
use glam::Vec2;

/// A side's line in the sources block: room for its figures under the bar when
/// there are few sides.
pub(super) fn source_row(sides: usize) -> f32 {
    if sides <= 4 {
        54.0
    } else {
        30.0
    }
}

/// Each side's materials, mined against reclaimed, with what reclaim came to.
pub(super) fn sources(report: &Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let inner = block(ui, r, "Where the Materials Came From");
    let mut x = inner.x;
    for (label, tone) in [("Mined", MINED), ("Reclaimed", SALVAGE)] {
        ui.fill(Rect::new(x, inner.y + 6.0, 9.0, 9.0), rgb(tone, 1.0));
        x = ui.text(
            x + 14.0,
            inner.y + 10.5,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            label,
        ) + 16.0;
    }
    let a = &report.a;
    let most = a.sides.iter().map(|s| s.collected).fold(1.0f32, f32::max);
    let top = inner.y + 26.0;
    let row_h = source_row(a.sides.len()).min((inner.bottom() - top) / a.sides.len().max(1) as f32);
    let detailed = row_h > 44.0;
    for (i, s) in a.sides.iter().enumerate() {
        let k = ease((report.tab_age - 0.1 - 0.06 * i as f32) / 0.6);
        let y = top + i as f32 * row_h;
        let c = report.color(ctx, i);
        let share = if s.collected > 0.0 {
            s.reclaimed / s.collected
        } else {
            0.0
        };
        // The side and how much of its income was reclaim, over its bar.
        let head = y + 8.0;
        ui.fill(Rect::new(inner.x, head - 4.0, 8.0, 8.0), c);
        let (st, name) = ui.fitted(type_scale::CAPTION, &s.name, inner.w * 0.45);
        ui.text(inner.x + 14.0, head, st, rgb(palette::TEXT, 0.9), &name);
        let pct = format!("{:.0}% reclaimed", share * 100.0 * k);
        ui.text_right(
            inner.right(),
            head,
            type_scale::VALUE,
            rgb(SALVAGE, k),
            &pct,
        );
        let bar = Rect::new(
            inner.x,
            y + 17.0,
            inner.w,
            if detailed { 10.0 } else { 6.0 },
        );
        ui.fill(bar, rgb(palette::LINE, 0.04));
        let mined_w = bar.w * s.mined / most * k;
        let salvage_w = bar.w * s.reclaimed / most * k;
        ui.fill(Rect::new(bar.x, bar.y, mined_w, bar.h), rgb(MINED, 0.7));
        ui.fill(
            Rect::new(bar.x + mined_w, bar.y, salvage_w, bar.h),
            rgb(SALVAGE, 0.95),
        );
        // The reclaimed part glows: it is easy to miss beside the mines.
        ui.fill(
            Rect::new(bar.x + mined_w, bar.y - 2.0, salvage_w.max(2.0), 2.0),
            rgb(0xFFFFFF, 0.5 * k),
        );
        if detailed {
            let line = format!(
                "{} reclaimed  \u{b7}  {} mined  \u{b7}  best {}/s  \u{b7}  {} wrecks",
                short(s.reclaimed * k),
                short(s.mined * k),
                short(s.peak_reclaim),
                s.wrecks_cleared
            );
            let (st, line) = ui.fitted(type_scale::MICRO, &line, inner.w);
            ui.text(
                inner.x,
                bar.bottom() + 12.0,
                st,
                rgb(palette::DIM, k),
                &line,
            );
        }
    }
}

/// The chart with every side's salvage on it: a glow in the side's colour round a
/// bright salvage core, sized by what came out of the ground there.
pub(super) fn map(report: &Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let inner = block(ui, r, "Where It Was Salvaged");
    let chart = Rect::new(inner.x, inner.y, inner.w, inner.w);
    ui.fill(chart, ink(0.9));
    let px = preview::SIZE as f32;
    ui.image(
        ctx.chart,
        [0.0, 0.0, px, px],
        chart,
        [0.13, 0.14, 0.16, 1.0],
    );
    ui.brackets(chart.inset(-4.0), 14.0, rgb(palette::LINE, 0.5));
    let a = &report.a;
    let at = |p: Vec2| super::battlefield::locate(a.size, chart, p);
    let most = (0..a.sides.len())
        .flat_map(|i| a.salvage_points(i).map(|(_, v)| v))
        .fold(1.0f32, f32::max);
    let k = report.reveal(0.15);
    let scale = (chart.w / 640.0).max(0.6);
    for i in 0..a.sides.len() {
        let c = report.color(ctx, i);
        for (p, v) in a.salvage_points(i) {
            let heat = (v / most).sqrt() * k;
            let rad = (1.8 + 9.0 * heat) * scale;
            let p = at(p);
            ui.dot(p, rad * 2.0, [c[0], c[1], c[2], 0.16 * heat + 0.04]);
            ui.dot(p, rad * 0.75, rgb(SALVAGE, 0.35 + 0.65 * heat));
        }
    }
    // The battles over it: much of what is reclaimed is what they left.
    for (n, b) in a.battles.iter().enumerate() {
        let c = at(b.at);
        let reach = (b.radius * 1.4).max(140.0) * chart.w / a.size.x.max(a.size.y);
        ui.arc(
            c,
            reach,
            0.0,
            std::f32::consts::TAU,
            1.0,
            rgb(palette::ACCENT, 0.45),
        );
        ui.text_centred(
            c.x,
            c.y - reach - 8.0,
            type_scale::MICRO,
            rgb(palette::ACCENT, 0.8),
            &format!("{}", n + 1),
        );
    }
    if a.total_reclaimed <= 0.0 {
        ui.text_centred(
            chart.x + chart.w * 0.5,
            chart.mid_y(),
            type_scale::CAPTION,
            rgb(palette::DIM, 1.0),
            "Nothing was reclaimed",
        );
    }
}

/// The richest wrecks cleared, with who took them, when and where.
pub(super) fn hauls(report: &Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let inner = block(ui, r, "Richest Wrecks");
    let a = &report.a;
    if a.hauls.is_empty() {
        ui.text(
            inner.x,
            inner.y + 18.0,
            type_scale::BODY,
            rgb(palette::FAINT, 1.0),
            "No wreck was cleared",
        );
        return;
    }
    let row_h = 52.0;
    let fits = ((inner.h / row_h).floor() as usize).max(1);
    let top = a.hauls[0].value.max(1.0);
    for (k, h) in a.hauls.iter().take(fits).enumerate() {
        let t = ease((report.tab_age - 0.25 - 0.05 * k as f32) / 0.45);
        let row = Rect::new(inner.x, inner.y + k as f32 * row_h, inner.w, row_h - 6.0);
        ui.fill_cut(row, 4.0, ink(0.35 * t));
        let tone =
            h.by.map_or(rgb(palette::DIM, 1.0), |b| report.color(ctx, b as usize));
        ui.fill(
            Rect::new(row.x, row.y + 4.0, 2.0, row.h - 8.0),
            [tone[0], tone[1], tone[2], t],
        );
        let pic = Rect::new(row.x + 8.0, row.y + 3.0, row.h - 6.0, row.h - 6.0);
        ctx.thumbs.draw(ui, h.blueprint, pic, t);
        let tx = pic.right() + 8.0;
        let name = format!("{} wreck", ctx.blueprints.unit(h.blueprint).name);
        let (st, name) = ui.fitted(type_scale::CAPTION, &name, row.right() - tx - 70.0);
        ui.text(tx, row.y + 14.0, st, rgb(palette::TEXT, t), &name);
        let by =
            h.by.and_then(|b| a.sides.get(b as usize))
                .map_or("", |s| s.name.as_str());
        let line = format!(
            "{by}  \u{b7}  {}  \u{b7}  {}",
            clock(h.tick),
            region(h.pos, a.size)
        );
        let (st, line) = ui.fitted(type_scale::MICRO, &line, row.right() - tx - 70.0);
        ui.text(tx, row.y + 31.0, st, rgb(palette::DIM, t), &line);
        ui.text_right(
            row.right() - 10.0,
            row.y + 14.0,
            type_scale::VALUE,
            rgb(SALVAGE, t),
            &short(h.value),
        );
        let w = (row.w - 12.0) * h.value / top * t;
        ui.fill(
            Rect::new(row.x + 6.0, row.bottom() - 2.0, w, 1.5),
            rgb(SALVAGE, 0.6 * t),
        );
    }
}
