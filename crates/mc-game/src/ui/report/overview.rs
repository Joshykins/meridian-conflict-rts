//! The first page: every side's line on the scoreboard, grouped by team, the
//! armies' strength over the match, and the honours.

use super::analysis::{clock, short, Metric, SideReport};
use super::chart::{self, Series};
use super::{block, ease, legend, well, Ctx, Report};
use crate::ui::{id, palette, rgb, type_scale, Rect, Ui};

/// A scoreboard column: heading, width share, the figure and how it reads.
struct Column {
    title: &'static str,
    value: fn(&SideReport) -> f32,
    text: fn(f32) -> String,
    /// Lower is better (losses).
    low_wins: bool,
}

const COLUMNS: [Column; 8] = [
    Column {
        title: "Collected",
        value: |s| s.collected,
        text: short,
        low_wins: false,
    },
    Column {
        title: "Peak Army",
        value: |s| s.peak_army,
        text: short,
        low_wins: false,
    },
    Column {
        title: "Built",
        value: |s| s.built as f32,
        text: |v| format!("{v:.0}"),
        low_wins: false,
    },
    Column {
        title: "Kills",
        value: |s| s.kills as f32,
        text: |v| format!("{v:.0}"),
        low_wins: false,
    },
    Column {
        title: "Losses",
        value: |s| s.losses as f32,
        text: |v| format!("{v:.0}"),
        low_wins: true,
    },
    Column {
        title: "Destroyed",
        value: |s| s.destroyed,
        text: short,
        low_wins: false,
    },
    Column {
        title: "Lost",
        value: |s| s.lost,
        text: short,
        low_wins: true,
    },
    Column {
        title: "Efficiency",
        value: |s| s.efficiency * 100.0,
        text: |v| format!("{v:.0}%"),
        low_wins: false,
    },
];

pub fn draw(report: &mut Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let n = report.a.sides.len();
    let teams = report.a.teams();
    let grouped = teams.len() < n;
    let heads = if grouped {
        teams.len() as f32 * 30.0
    } else {
        0.0
    };
    let row_h = ((r.h * 0.5 - 34.0 - heads) / n.max(1) as f32).clamp(38.0, 62.0);
    let table_h = 34.0 + heads + row_h * n as f32 + 8.0;
    let table = Rect::new(r.x, r.y, r.w, table_h);
    scoreboard(report, ui, ctx, table, row_h, grouped);

    let rest = Rect::new(
        r.x,
        table.bottom() + 22.0,
        r.w,
        r.bottom() - table.bottom() - 22.0,
    );
    let left = Rect::new(rest.x, rest.y, rest.w * 0.62, rest.h);
    let right = Rect::new(
        left.right() + 28.0,
        rest.y,
        rest.right() - left.right() - 28.0,
        rest.h,
    );
    strength(report, ui, ctx, left);
    honours(report, ui, ctx, right);
}

fn scoreboard(report: &Report, ui: &mut Ui, ctx: &Ctx, r: Rect, row_h: f32, grouped: bool) {
    well(ui, r);
    let name_w = (r.w * 0.24).clamp(220.0, 360.0);
    let col_x0 = r.x + name_w + 150.0;
    let col_w = (r.right() - 16.0 - col_x0) / COLUMNS.len() as f32;
    // Headings.
    let hy = r.y + 18.0;
    ui.text(
        r.x + 18.0,
        hy,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        "Commander",
    );
    ui.text(
        r.x + name_w,
        hy,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        "Result",
    );
    for (c, col) in COLUMNS.iter().enumerate() {
        ui.text_right(
            col_x0 + (c + 1) as f32 * col_w - 12.0,
            hy,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            col.title,
        );
    }
    ui.hline(r.x + 12.0, r.y + 32.0, r.w - 24.0, rgb(palette::LINE, 0.16));
    // Each column's best and its largest value, for the bars.
    let a = &report.a;
    let stats: Vec<(f32, f32)> = COLUMNS
        .iter()
        .map(|col| {
            let vals = a.sides.iter().map(col.value);
            let max = vals.clone().fold(0.0f32, f32::max);
            let best = if col.low_wins {
                vals.fold(f32::MAX, f32::min)
            } else {
                max
            };
            (max, best)
        })
        .collect();

    let grid = Grid {
        name_w,
        col_x0,
        col_w,
        stats,
    };
    let mut y = r.y + 34.0;
    let mut row = 0;
    for team in a.teams() {
        if grouped {
            let won = a.winner == Some(team);
            ui.text(
                r.x + 18.0,
                y + 16.0,
                type_scale::OVERLINE,
                rgb(if won { palette::ACCENT } else { palette::DIM }, 1.0),
                &format!("Team {}", team + 1),
            );
            if won {
                ui.text(
                    r.x + 100.0,
                    y + 16.0,
                    type_scale::MICRO,
                    rgb(palette::ACCENT, 0.9),
                    "Victorious",
                );
            }
            y += 30.0;
        }
        for i in (0..a.sides.len()).filter(|&i| a.sides[i].team == team) {
            let k = ease((report.tab_age - 0.08 * row as f32) / 0.45);
            let line = Rect::new(r.x + 6.0, y, r.w - 12.0, row_h);
            ui.shift.x -= 18.0 * (1.0 - k);
            side_row(report, ui, ctx, &grid, (line, i, k));
            ui.shift.x += 18.0 * (1.0 - k);
            y += row_h;
            row += 1;
        }
    }
}

/// Where the scoreboard's columns are, and each column's (largest, best) value.
struct Grid {
    name_w: f32,
    col_x0: f32,
    col_w: f32,
    stats: Vec<(f32, f32)>,
}

/// One side's line: `line` is its strip, `k` how far it has come in.
fn side_row(
    report: &Report,
    ui: &mut Ui,
    ctx: &Ctx,
    grid: &Grid,
    (line, i, k): (Rect, usize, f32),
) {
    let s = &report.a.sides[i];
    let Grid {
        name_w,
        col_x0,
        col_w,
        ref stats,
    } = *grid;
    let res = ui.interact_with(id("report-row", i), line, true, false);
    let me = ctx.local == Some(i as u8);
    let c = report.color(ctx, i);
    if res.glow > 0.0 || me {
        ui.fill(line, rgb(0xFFFFFF, 0.03 + 0.04 * res.glow));
    }
    ui.gradient_h(
        Rect::new(line.x, line.y + 1.0, 160.0, line.h - 2.0),
        [c[0], c[1], c[2], 0.16 * k],
        [c[0], c[1], c[2], 0.0],
    );
    ui.fill(
        Rect::new(line.x, line.y + 1.0, 3.0, line.h - 2.0),
        [c[0], c[1], c[2], k],
    );
    let mid = line.mid_y();
    let alive = if s.defeated_at.is_some() { 0.6 } else { 1.0 };
    let (st, name) = ui.fitted(type_scale::ITEM, &s.name, name_w - 70.0);
    let end = ui.text(
        line.x + 14.0,
        mid - 7.0,
        st,
        rgb(palette::TEXT, k * alive),
        &name,
    );
    if me {
        ui.text(
            end + 8.0,
            mid - 7.0,
            type_scale::MICRO,
            rgb(palette::ACCENT, k),
            "You",
        );
    }
    let tag = format!("{}{}", s.faction, if s.ai { "  \u{b7}  AI" } else { "" });
    ui.text(
        line.x + 14.0,
        mid + 11.0,
        type_scale::MICRO,
        rgb(palette::FAINT, k),
        &tag,
    );
    let (result, tone) = match (s.victor, s.defeated_at) {
        (true, _) => ("Victor".to_string(), palette::ACCENT),
        (_, Some(t)) => (format!("Fell at {}", clock(t)), palette::BAD),
        (false, None) if report.a.winner.is_some() => ("Defeated".to_string(), palette::BAD),
        _ => ("Standing".to_string(), palette::DIM),
    };
    ui.text(
        line.x - 6.0 + name_w,
        mid,
        type_scale::CAPTION,
        rgb(tone, k),
        &result,
    );
    for (ci, col) in COLUMNS.iter().enumerate() {
        let v = (col.value)(s);
        let (max, best) = stats[ci];
        let right = col_x0 + (ci + 1) as f32 * col_w - 12.0;
        let top = report.a.sides.len() > 1 && v == best && v > 0.0;
        let shown = report.count(v, 0.15 + 0.03 * ci as f32);
        ui.text_right(
            right,
            mid - 4.0,
            type_scale::VALUE,
            rgb(if top { palette::TEXT } else { palette::DIM }, k),
            &(col.text)(shown),
        );
        // A bar under each figure against the column's largest.
        let bw = col_w - 24.0;
        let frac = if max > 0.0 { shown / max } else { 0.0 };
        ui.fill(
            Rect::new(right - bw, mid + 10.0, bw, 2.0),
            rgb(palette::LINE, 0.06 * k),
        );
        ui.fill(
            Rect::new(right - bw * frac, mid + 10.0, bw * frac, 2.0),
            [c[0], c[1], c[2], (if top { 1.0 } else { 0.55 }) * k],
        );
        if top {
            ui.fill(
                Rect::new(right + 4.0, mid - 7.0, 3.0, 6.0),
                rgb(palette::ACCENT, k),
            );
        }
    }
}

/// Every side's army worth over the match.
fn strength(report: &mut Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let inner = block(ui, r, "Army Strength");
    legend(
        report,
        ui,
        ctx,
        Rect::new(inner.x + 44.0, inner.y, inner.w - 44.0, 20.0),
    );
    let colors: Vec<_> = (0..report.a.sides.len())
        .map(|i| report.color(ctx, i))
        .collect();
    let names: Vec<String> = report.a.sides.iter().map(|s| s.name.clone()).collect();
    let series: Vec<Series> = (0..report.a.sides.len())
        .map(|i| Series {
            values: report.a.curve(Metric::ArmyValue, i),
            color: colors[i],
            label: &names[i],
        })
        .collect();
    let plot = Rect::new(
        inner.x + 44.0,
        inner.y + 30.0,
        inner.w - 44.0,
        inner.h - 56.0,
    );
    chart::lines(
        ui,
        &chart::Lines {
            id: id("report-strength", 0),
            r: plot,
            times: &report.a.times,
            length: super::analysis::seconds(report.a.length),
            series: &series,
            unit: "",
            reveal: report.reveal(0.2),
            focus: report.focus,
        },
    );
}

fn honours(report: &Report, ui: &mut Ui, ctx: &Ctx, r: Rect) {
    let inner = block(ui, r, "Honours");
    let awards = &report.a.awards;
    if awards.is_empty() {
        ui.text(
            inner.x,
            inner.y + 20.0,
            type_scale::BODY,
            rgb(palette::FAINT, 1.0),
            "Honours go to the best of two or more sides",
        );
        return;
    }
    let cols = 2;
    let rows = awards.len().div_ceil(cols);
    let gap = 10.0;
    let cw = (inner.w - gap) / cols as f32;
    let ch = ((inner.h - gap * (rows as f32 - 1.0)) / rows as f32).min(96.0);
    for (i, aw) in awards.iter().enumerate() {
        let k = ease((report.tab_age - 0.35 - 0.09 * i as f32) / 0.5);
        if k <= 0.0 {
            continue;
        }
        let card = Rect::new(
            inner.x + (i % cols) as f32 * (cw + gap),
            inner.y + (i / cols) as f32 * (ch + gap) + 10.0 * (1.0 - k),
            cw,
            ch,
        );
        let side = aw.side as usize;
        let c = report.color(ctx, side);
        ui.fill_cut(card, 6.0, [0.0, 0.0, 0.0, 0.45 * k]);
        ui.gradient_h(card, [c[0], c[1], c[2], 0.09 * k], [c[0], c[1], c[2], 0.0]);
        ui.bevel(card, 6.0, 0.6 * k);
        // A small medal: a ring with the side's colour in it.
        let medal = glam::Vec2::new(card.x + 26.0, card.y + 28.0);
        ui.arc(
            medal,
            11.0,
            0.0,
            std::f32::consts::TAU,
            1.5,
            rgb(palette::ACCENT, k),
        );
        ui.disc(medal, 6.5, [c[0], c[1], c[2], k]);
        ui.text(
            card.x + 48.0,
            card.y + 22.0,
            type_scale::ITEM,
            rgb(palette::TEXT, k),
            aw.title,
        );
        ui.text(
            card.x + 48.0,
            card.y + 40.0,
            type_scale::MICRO,
            rgb(palette::FAINT, k),
            aw.blurb,
        );
        let name = report.a.sides.get(side).map_or("", |s| s.name.as_str());
        let (st, name) = ui.fitted(type_scale::CAPTION, name, cw * 0.5);
        ui.text(
            card.x + 16.0,
            card.bottom() - 18.0,
            st,
            [c[0], c[1], c[2], k],
            &name,
        );
        ui.text_right(
            card.right() - 14.0,
            card.bottom() - 18.0,
            type_scale::VALUE,
            rgb(palette::TEXT, k),
            &aw.figure,
        );
    }
}
