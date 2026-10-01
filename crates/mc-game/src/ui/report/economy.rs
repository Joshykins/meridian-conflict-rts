//! The economy page: income, spending and stores over the match, what each side's
//! materials bought, and a ledger of the totals.

use super::analysis::{short, span, Metric, SPEND_KINDS};
use super::chart::{self, Series};
use super::{block, chips, ease, legend, well, Ctx, Report};
use crate::ui::{id, palette, rgb, type_scale, Color, Rect, Ui};

const METRICS: [Metric; 6] = [
    Metric::MassIncome,
    Metric::EnergyIncome,
    Metric::Collected,
    Metric::Spending,
    Metric::Stored,
    Metric::Efficiency,
];

/// A colour for each of `SPEND_KINDS`.
const SPEND_TONES: [u32; 6] = [0xFF5A24, 0xFFB43C, 0xE9D98A, 0x6FD08C, 0x5AA9FF, 0x9A9AA2];

pub fn draw(report: &mut Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let left = Rect::new(r.x, r.y, r.w * 0.66, r.h);
    let right = Rect::new(
        left.right() + 28.0,
        r.y,
        r.right() - left.right() - 28.0,
        r.h,
    );
    curves(report, ui, ctx, left);
    let split = right.y + right.h * 0.56;
    spending(
        report,
        ui,
        ctx,
        Rect::new(right.x, right.y, right.w, split - right.y - 14.0),
    );
    ledger(
        report,
        ui,
        ctx,
        Rect::new(right.x, split, right.w, right.bottom() - split),
    );
}

fn curves(report: &mut Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let inner = block(ui, r, "Over the Match");
    let names: Vec<&str> = METRICS.iter().map(|m| m.name()).collect();
    let chosen = METRICS
        .iter()
        .position(|&m| m == report.economy)
        .unwrap_or(0);
    if let Some(i) = chips(
        ui,
        "report-econ",
        Rect::new(inner.x, inner.y, inner.w, 30.0),
        &names,
        chosen,
    ) {
        report.economy = METRICS[i];
        // The new curves draw in afresh.
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
            values: report.a.curve(report.economy, i),
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
            id: id("report-econ-chart", 0),
            r: plot,
            times: &report.a.times,
            length: super::analysis::seconds(report.a.length),
            series: &series,
            unit: report.economy.unit(),
            reveal: report.reveal(0.0),
            focus: report.focus,
        },
    );
}

/// What each side's materials bought, as one bar a side split by kind.
fn spending(report: &Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let inner = block(ui, r, "Where the Materials Went");
    // The key.
    let mut x = inner.x;
    for (kind, tone) in SPEND_KINDS.iter().zip(SPEND_TONES) {
        let w = ui.text_width(type_scale::MICRO, kind) + 24.0;
        if x + w > inner.right() {
            break;
        }
        ui.fill(Rect::new(x, inner.y + 6.0, 9.0, 9.0), rgb(tone, 1.0));
        ui.text(
            x + 14.0,
            inner.y + 10.5,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            kind,
        );
        x += w;
    }
    let sides = &report.a.sides;
    let most = sides
        .iter()
        .map(|s| s.spend.iter().sum::<f32>())
        .fold(1.0f32, f32::max);
    let top = inner.y + 30.0;
    let row_h = ((inner.bottom() - top) / sides.len().max(1) as f32).clamp(26.0, 46.0);
    let mut hover: Option<(usize, usize)> = None;
    for (i, s) in sides.iter().enumerate() {
        let k = ease((report.tab_age - 0.1 - 0.06 * i as f32) / 0.6);
        let y = top + i as f32 * row_h;
        let name_w = 120.0;
        let (st, name) = ui.fitted(type_scale::CAPTION, &s.name, name_w - 22.0);
        let c = report.color(ctx, i);
        ui.fill(Rect::new(inner.x, y + row_h * 0.5 - 4.0, 8.0, 8.0), c);
        ui.text(
            inner.x + 14.0,
            y + row_h * 0.5,
            st,
            rgb(palette::TEXT, 0.9),
            &name,
        );
        let total: f32 = s.spend.iter().sum();
        let full = inner.w - name_w - 54.0;
        let bar = Rect::new(inner.x + name_w, y + row_h * 0.5 - 7.0, full, 14.0);
        ui.fill(bar, rgb(palette::LINE, 0.04));
        let mut bx = bar.x;
        for (kind, &v) in s.spend.iter().enumerate() {
            let w = full * v / most * k;
            if w <= 0.0 {
                continue;
            }
            let seg = Rect::new(bx, bar.y, w, bar.h);
            let res = ui.interact_with(id("report-spend", i * 8 + kind), seg, true, false);
            let tone = SPEND_TONES[kind];
            ui.fill(
                Rect::new(seg.x, seg.y, (seg.w - 1.0).max(1.0), seg.h),
                rgb(tone, 0.75 + 0.25 * res.glow),
            );
            if res.hovered {
                hover = Some((i, kind));
            }
            bx += w;
        }
        ui.text(
            bar.right() + 8.0,
            bar.mid_y(),
            type_scale::VALUE,
            rgb(palette::TEXT, k),
            &short(total * k),
        );
    }
    if let Some((i, kind)) = hover {
        let s = &sides[i];
        let total: f32 = s.spend.iter().sum::<f32>().max(1.0);
        let text = format!(
            "{}: {} materials on {}, {:.0}%",
            s.name,
            short(s.spend[kind]),
            SPEND_KINDS[kind].to_lowercase(),
            s.spend[kind] / total * 100.0
        );
        let w = ui.text_width(type_scale::CAPTION, &text) + 24.0;
        let at = Rect::new(
            (ui.cursor.x - w * 0.5).clamp(r.x, r.right() - w),
            ui.cursor.y - 40.0,
            w,
            26.0,
        );
        ui.frost_cut(at, 5.0, 0.92);
        ui.bevel(at, 5.0, 0.6);
        ui.text(
            at.x + 12.0,
            at.mid_y(),
            type_scale::CAPTION,
            rgb(palette::TEXT, 1.0),
            &text,
        );
    }
}

/// A ledger heading, and how a side's figure under it reads.
type LedgerColumn = (&'static str, fn(&super::analysis::SideReport) -> String);

/// The totals: a line a side.
fn ledger(report: &Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let inner = block(ui, r, "Ledger");
    well(ui, inner);
    let cols: [LedgerColumn; 6] = [
        ("Collected", |s| short(s.collected)),
        ("Reclaimed", |s| short(s.reclaimed)),
        ("Spent", |s| short(s.spent)),
        ("Energy", |s| short(s.energy_collected)),
        ("Best /s", |s| short(s.peak_income)),
        ("Stalled", |s| {
            span((s.stalled * mc_core::TICKS_PER_SECOND as f32) as u32)
        }),
    ];
    let name_w = 120.0;
    let col_w = (inner.w - name_w - 12.0) / cols.len() as f32;
    let head = inner.y + 16.0;
    for (c, (title, _)) in cols.iter().enumerate() {
        ui.text_right(
            inner.x + name_w + (c + 1) as f32 * col_w,
            head,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            title,
        );
    }
    ui.hline(
        inner.x + 8.0,
        head + 12.0,
        inner.w - 16.0,
        rgb(palette::LINE, 0.12),
    );
    let sides = &report.a.sides;
    let row_h = ((inner.bottom() - head - 20.0) / sides.len().max(1) as f32).min(30.0);
    for (i, s) in sides.iter().enumerate() {
        let y = head + 14.0 + (i as f32 + 0.5) * row_h;
        if i % 2 == 1 {
            ui.fill(
                Rect::new(inner.x + 4.0, y - row_h * 0.5, inner.w - 8.0, row_h),
                rgb(palette::LINE, 0.025),
            );
        }
        let c = report.color(ctx, i);
        ui.fill(Rect::new(inner.x + 12.0, y - 4.0, 8.0, 8.0), c);
        let (st, name) = ui.fitted(type_scale::CAPTION, &s.name, name_w - 30.0);
        ui.text(inner.x + 26.0, y, st, rgb(palette::TEXT, 0.9), &name);
        for (c, (_, value)) in cols.iter().enumerate() {
            ui.text_right(
                inner.x + name_w + (c + 1) as f32 * col_w,
                y,
                type_scale::VALUE,
                rgb(palette::TEXT, 1.0),
                &value(s),
            );
        }
    }
}
