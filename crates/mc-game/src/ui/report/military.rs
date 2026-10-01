//! The military page: the armies over the match, how hard the fighting was at
//! each point of it, who destroyed whom, the deadliest units and where each
//! side's losses fell.

use super::analysis::{clock, seconds, short, Metric, BIN_TICKS, DOMAINS};
use super::chart::{self, Series};
use super::{block, chips, ease, legend, Ctx, Report};
use crate::ui::{id, ink, palette, rgb, type_scale, Color, Rect, Ui};
use mc_data::BlueprintId;

const METRICS: [Metric; 4] = [
    Metric::ArmyValue,
    Metric::ArmySize,
    Metric::Destroyed,
    Metric::Lost,
];

const DOMAIN_TONES: [u32; 4] = [0x7FCF6A, 0x7CC6FF, 0x3F7BFF, 0x9A9AA2];

pub fn draw(report: &mut Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let left = Rect::new(r.x, r.y, r.w * 0.62, r.h);
    let right = Rect::new(
        left.right() + 28.0,
        r.y,
        r.right() - left.right() - 28.0,
        r.h,
    );
    let split = left.y + left.h * 0.62;
    armies(
        report,
        ui,
        ctx,
        Rect::new(left.x, left.y, left.w, split - left.y),
    );
    intensity(
        report,
        ui,
        ctx,
        Rect::new(left.x, split + 10.0, left.w, left.bottom() - split - 10.0),
    );
    let (a, b) = (right.y + right.h * 0.36, right.y + right.h * 0.74);
    matrix(
        report,
        ui,
        ctx,
        Rect::new(right.x, right.y, right.w, a - right.y - 12.0),
    );
    deadliest(
        report,
        ui,
        ctx,
        Rect::new(right.x, a, right.w, b - a - 12.0),
    );
    domains(
        report,
        ui,
        ctx,
        Rect::new(right.x, b, right.w, right.bottom() - b),
    );
}

fn armies(report: &mut Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let inner = block(ui, r, "Forces");
    let names: Vec<&str> = METRICS.iter().map(|m| m.name()).collect();
    let chosen = METRICS
        .iter()
        .position(|&m| m == report.military)
        .unwrap_or(0);
    if let Some(i) = chips(
        ui,
        "report-mil",
        Rect::new(inner.x, inner.y, inner.w, 30.0),
        &names,
        chosen,
    ) {
        report.military = METRICS[i];
        report.tab_age = report.tab_age.min(0.35);
    }
    legend(
        report,
        ui,
        ctx,
        Rect::new(inner.x + 50.0, inner.y + 44.0, inner.w - 50.0, 20.0),
    );
    let colors: Vec<Color> = (0..report.a.sides.len())
        .map(|i| report.color(ctx, i))
        .collect();
    let names: Vec<String> = report.a.sides.iter().map(|s| s.name.clone()).collect();
    let series: Vec<Series> = (0..report.a.sides.len())
        .map(|i| Series {
            values: report.a.curve(report.military, i),
            color: colors[i],
            label: &names[i],
        })
        .collect();
    let plot = Rect::new(
        inner.x + 50.0,
        inner.y + 76.0,
        inner.w - 50.0,
        inner.h - 102.0,
    );
    chart::lines(
        ui,
        &chart::Lines {
            id: id("report-mil-chart", 0),
            r: plot,
            times: &report.a.times,
            length: seconds(report.a.length),
            series: &series,
            unit: report.military.unit(),
            reveal: report.reveal(0.0),
            focus: report.focus,
        },
    );
}

/// Worth lost in each half minute, by the side that lost it, the battles bracketed.
fn intensity(report: &Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let inner = block(ui, r, "Intensity");
    let colors: Vec<Color> = (0..report.a.sides.len())
        .map(|i| report.color(ctx, i))
        .collect();
    let bars = Rect::new(
        inner.x + 50.0,
        inner.y + 6.0,
        inner.w - 50.0,
        inner.h - 34.0,
    );
    let a = &report.a;
    let marks: Vec<(f32, f32)> = a
        .battles
        .iter()
        .map(|b| ((b.from / BIN_TICKS) as f32, (b.to / BIN_TICKS) as f32))
        .collect();
    chart::stacked_bars(ui, bars, &a.intensity, &colors, report.reveal(0.15), &marks);
    // The axis runs over whole bins, so it ends at the last bin's end.
    let span = seconds(a.intensity.len() as u32 * BIN_TICKS);
    chart::time_axis(ui, bars, span);
    let n = a.intensity.len().max(1) as f32;
    for (i, b) in a.battles.iter().enumerate() {
        let x = bars.x + bars.w * ((b.from / BIN_TICKS) as f32 + 0.5) / n;
        ui.text_centred(
            x,
            bars.y + 14.0,
            type_scale::MICRO,
            rgb(palette::ACCENT, 1.0),
            &format!("{}", i + 1),
        );
    }
    // The bar under the pointer, read out.
    let res = ui.interact_with(id("report-intensity", 0), bars, true, false);
    if res.hovered {
        let bin = (((ui.cursor.x - ui.shift.x - bars.x) / bars.w * n) as usize)
            .min(a.intensity.len().saturating_sub(1));
        let Some(lost) = a.intensity.get(bin) else {
            return;
        };
        let x0 = bars.x + bars.w * bin as f32 / n;
        ui.fill(
            Rect::new(x0, bars.y, bars.w / n, bars.h),
            rgb(0xFFFFFF, 0.06),
        );
        let from = bin as u32 * BIN_TICKS;
        let mut lines = vec![format!("{} to {}", clock(from), clock(from + BIN_TICKS))];
        for (side, &v) in lost.iter().enumerate().filter(|(_, v)| **v > 0.0) {
            lines.push(format!("{} lost {}", a.sides[side].name, short(v)));
        }
        if lines.len() == 1 {
            lines.push("No losses".into());
        }
        let w = lines
            .iter()
            .map(|l| ui.text_width(type_scale::CAPTION, l))
            .fold(0.0f32, f32::max)
            + 24.0;
        let h = 12.0 + lines.len() as f32 * 18.0;
        let cx = (ui.cursor.x - ui.shift.x + 14.0).min(bars.right() - w);
        let card = Rect::new(cx, bars.y - h - 6.0, w, h);
        ui.frost_cut(card, 5.0, 0.92);
        ui.bevel(card, 5.0, 0.6);
        for (k, l) in lines.iter().enumerate() {
            ui.text(
                card.x + 12.0,
                card.y + 15.0 + k as f32 * 18.0,
                type_scale::CAPTION,
                rgb(if k == 0 { palette::DIM } else { palette::TEXT }, 1.0),
                l,
            );
        }
    }
}

/// Worth each side destroyed of each other side: killers down, victims across.
fn matrix(report: &Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let inner = block(ui, r, "Who Destroyed Whom");
    let a = &report.a;
    let n = a.sides.len();
    if n == 0 {
        return;
    }
    let head = 24.0;
    let side_w = 30.0;
    let cell = ((inner.w - side_w) / n as f32)
        .min((inner.h - head) / n as f32)
        .min(90.0);
    let max = a.matrix.iter().flatten().copied().fold(1.0f32, f32::max);
    let grid_x = inner.x + side_w;
    // Victims across the top, killers down the side, as colour marks.
    for i in 0..n {
        let c = report.color(ctx, i);
        ui.fill(
            Rect::new(
                grid_x + i as f32 * cell + 4.0,
                inner.y + head - 8.0,
                cell - 8.0,
                3.0,
            ),
            c,
        );
        ui.fill(
            Rect::new(
                inner.x + 10.0,
                inner.y + head + i as f32 * cell + 4.0,
                3.0,
                cell - 8.0,
            ),
            c,
        );
    }
    ui.text(
        grid_x,
        inner.y + 4.0,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        "Killers down, losers across",
    );
    let k = report.reveal(0.2);
    for by in 0..n {
        for victim in 0..n {
            let rr = Rect::new(
                grid_x + victim as f32 * cell + 2.0,
                inner.y + head + by as f32 * cell + 2.0,
                cell - 4.0,
                cell - 4.0,
            );
            if by == victim || a.sides[by].team == a.sides[victim].team {
                ui.fill(rr, rgb(palette::LINE, 0.02));
                continue;
            }
            let v = a.matrix[by][victim];
            let heat = (v / max).sqrt() * k;
            let c = report.color(ctx, by);
            ui.fill(rr, ink(0.4));
            ui.fill(rr, [c[0], c[1], c[2], 0.06 + 0.42 * heat]);
            ui.frame(rr, [c[0], c[1], c[2], 0.25 + 0.5 * heat]);
            let res = ui.interact_with(id("report-matrix", by * 16 + victim), rr, true, false);
            if res.glow > 0.0 {
                ui.frame(rr, rgb(0xFFFFFF, 0.6 * res.glow));
            }
            if cell > 34.0 {
                ui.text_centred(
                    rr.x + rr.w * 0.5,
                    rr.mid_y(),
                    if cell > 60.0 {
                        type_scale::VALUE
                    } else {
                        type_scale::MICRO
                    },
                    rgb(palette::TEXT, if v > 0.0 { 1.0 } else { 0.3 }),
                    &short(v * k),
                );
            }
            if res.hovered {
                let text = format!(
                    "{} destroyed {} of {}",
                    a.sides[by].name,
                    short(v),
                    a.sides[victim].name
                );
                let w = ui.text_width(type_scale::CAPTION, &text) + 24.0;
                let card = Rect::new(
                    (rr.x - w * 0.5 + rr.w * 0.5).min(r.right() - w),
                    rr.y - 32.0,
                    w,
                    26.0,
                );
                ui.frost_cut(card, 5.0, 0.92);
                ui.text(
                    card.x + 12.0,
                    card.mid_y(),
                    type_scale::CAPTION,
                    rgb(palette::TEXT, 1.0),
                    &text,
                );
            }
        }
    }
}

/// The unit types that destroyed the most, across every side.
fn deadliest(report: &Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let inner = block(ui, r, "Deadliest Units");
    let a = &report.a;
    let mut all: Vec<(usize, BlueprintId, u32, f32)> = a
        .sides
        .iter()
        .enumerate()
        .flat_map(|(i, s)| {
            s.deadliest
                .iter()
                .map(move |&(bp, kills, v)| (i, bp, kills, v))
        })
        .filter(|e| e.3 > 0.0)
        .collect();
    all.sort_by(|x, y| y.3.total_cmp(&x.3).then(x.0.cmp(&y.0)).then(x.1.cmp(&y.1)));
    if all.is_empty() {
        ui.text(
            inner.x,
            inner.y + 18.0,
            type_scale::BODY,
            rgb(palette::FAINT, 1.0),
            "Nothing was destroyed",
        );
        return;
    }
    let row_h = 46.0;
    let cols = if inner.w > 520.0 && all.len() > ((inner.h / row_h).floor() as usize).max(1) {
        2
    } else {
        1
    };
    let per_col = ((inner.h / row_h).floor() as usize).max(1);
    let col_w = (inner.w - 12.0 * (cols as f32 - 1.0)) / cols as f32;
    let top = all[0].3.max(1.0);
    for (k, &(side, bp, kills, v)) in all.iter().take(per_col * cols).enumerate() {
        let t = ease((report.tab_age - 0.25 - 0.05 * k as f32) / 0.45);
        let x = inner.x + (k / per_col) as f32 * (col_w + 12.0);
        let y = inner.y + (k % per_col) as f32 * row_h;
        let row = Rect::new(x, y, col_w, row_h - 6.0);
        ui.fill_cut(row, 4.0, ink(0.35 * t));
        let c = report.color(ctx, side);
        ui.fill(
            Rect::new(row.x, row.y + 4.0, 2.0, row.h - 8.0),
            [c[0], c[1], c[2], t],
        );
        let pic = Rect::new(row.x + 8.0, row.y + 2.0, row.h - 4.0, row.h - 4.0);
        ctx.thumbs.draw(ui, bp, pic, t);
        let name = &ctx.blueprints.unit(bp).name;
        let tx = pic.right() + 8.0;
        let (st, name) = ui.fitted(type_scale::CAPTION, name, row.right() - tx - 70.0);
        ui.text(tx, row.y + 13.0, st, rgb(palette::TEXT, t), &name);
        let side_name = &a.sides[side].name;
        let (st, side_name) = ui.fitted(type_scale::MICRO, side_name, row.right() - tx - 70.0);
        ui.text(tx, row.y + 29.0, st, [c[0], c[1], c[2], t], &side_name);
        ui.text_right(
            row.right() - 10.0,
            row.y + 13.0,
            type_scale::VALUE,
            rgb(palette::TEXT, t),
            &short(v),
        );
        ui.text_right(
            row.right() - 10.0,
            row.y + 29.0,
            type_scale::MICRO,
            rgb(palette::FAINT, t),
            &format!("{kills} {}", if kills == 1 { "kill" } else { "kills" }),
        );
        // How it compares with the deadliest.
        let w = (row.w - 12.0) * v / top * t;
        ui.fill(
            Rect::new(row.x + 6.0, row.bottom() - 2.0, w, 1.5),
            [c[0], c[1], c[2], 0.6 * t],
        );
    }
}

/// Each side's losses split by where the unit fought.
fn domains(report: &Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let inner = block(ui, r, "Losses by Domain");
    let mut x = inner.x;
    for (d, tone) in DOMAINS.iter().zip(DOMAIN_TONES) {
        ui.fill(Rect::new(x, inner.y + 6.0, 9.0, 9.0), rgb(tone, 1.0));
        x = ui.text(
            x + 14.0,
            inner.y + 10.5,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            d,
        ) + 14.0;
    }
    let sides = &report.a.sides;
    let most = sides.iter().map(|s| s.lost).fold(1.0f32, f32::max);
    let top = inner.y + 26.0;
    let row_h = ((inner.bottom() - top) / sides.len().max(1) as f32).clamp(16.0, 30.0);
    let k = report.reveal(0.3);
    for (i, s) in sides.iter().enumerate() {
        let y = top + i as f32 * row_h + row_h * 0.5;
        let c = report.color(ctx, i);
        ui.fill(Rect::new(inner.x, y - 4.0, 8.0, 8.0), c);
        let (st, name) = ui.fitted(type_scale::CAPTION, &s.name, 100.0);
        ui.text(inner.x + 14.0, y, st, rgb(palette::TEXT, 0.9), &name);
        let full = inner.w - 180.0;
        let mut bx = inner.x + 124.0;
        ui.fill(Rect::new(bx, y - 5.0, full, 10.0), rgb(palette::LINE, 0.04));
        for (d, &v) in s.lost_by_domain.iter().enumerate() {
            let w = full * v / most * k;
            if w > 0.0 {
                ui.fill(
                    Rect::new(bx, y - 5.0, (w - 1.0).max(1.0), 10.0),
                    rgb(DOMAIN_TONES[d], 0.85),
                );
                bx += w;
            }
        }
        ui.text(
            inner.x + 124.0 + full + 8.0,
            y,
            type_scale::VALUE,
            rgb(palette::TEXT, 1.0),
            &short(s.lost),
        );
    }
}
