//! The report's charts: curves over the match with a crosshair that reads every
//! side's value under the pointer, and the stacked bars of the intensity strip.

use super::analysis::{clock, short};
use crate::ui::{ink, palette, rgb, type_scale, Color, Id, Rect, Ui};
use glam::Vec2;

pub struct Series<'a> {
    pub values: &'a [f32],
    pub color: Color,
    pub label: &'a str,
}

/// A round step that splits `max` into about `count` parts.
pub fn nice_step(max: f32, count: f32) -> f32 {
    let raw = (max / count).max(1e-6);
    let mag = 10f32.powf(raw.log10().floor());
    let k = raw / mag;
    mag * if k <= 1.0 {
        1.0
    } else if k <= 2.0 {
        2.0
    } else if k <= 2.5 {
        2.5
    } else if k <= 5.0 {
        5.0
    } else {
        10.0
    }
}

/// Seconds between time ticks, so a match of `length` seconds gets at most `most`.
pub fn time_step(length: f32, most: f32) -> f32 {
    [
        30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1200.0, 1800.0, 3600.0,
    ]
    .into_iter()
    .find(|s| length / s <= most)
    .unwrap_or(7200.0)
}

/// The time axis under a chart: ticks and labels every `time_step`.
pub fn time_axis(ui: &mut Ui, r: Rect, length: f32) {
    let step = time_step(length, (r.w / 90.0).max(2.0));
    let mut t = 0.0;
    while t <= length + 0.01 {
        let x = r.x + r.w * t / length.max(1.0);
        ui.vline(x, r.bottom(), 4.0, rgb(palette::LINE, 0.3));
        ui.text_centred(
            x,
            r.bottom() + 13.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &clock((t * mc_core::TICKS_PER_SECOND as f32) as u32),
        );
        t += step;
    }
}

/// A line chart of `series` over `times` (seconds).
pub struct Lines<'a> {
    pub id: Id,
    pub r: Rect,
    pub times: &'a [f32],
    /// The match's length in seconds: the time axis runs to it.
    pub length: f32,
    pub series: &'a [Series<'a>],
    pub unit: &'a str,
    /// Draws it in from the left, 0 to 1.
    pub reveal: f32,
    /// Dims every series but this one.
    pub focus: Option<usize>,
}

/// Draws a `Lines` chart. Returns the sample under the pointer, if any.
pub fn lines(ui: &mut Ui, chart: &Lines) -> Option<usize> {
    let Lines {
        id,
        r,
        times,
        length,
        series,
        unit,
        reveal,
        focus,
    } = *chart;
    ui.fill(r, ink(0.35));
    let max = series
        .iter()
        .flat_map(|s| s.values.iter().copied())
        .fold(0.0f32, f32::max)
        .max(1.0);
    let step = nice_step(max, 4.0);
    let top = (max / step).ceil() * step;
    // The grid and the values up the side.
    let mut v = 0.0;
    while v <= top + step * 0.01 {
        let y = r.bottom() - r.h * v / top;
        ui.hline(
            r.x,
            y,
            r.w,
            rgb(palette::LINE, if v == 0.0 { 0.28 } else { 0.07 }),
        );
        ui.text_right(
            r.x - 8.0,
            y,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &short(v),
        );
        v += step;
    }
    time_axis(ui, r, length);
    if times.len() < 2 {
        ui.text_centred(
            r.x + r.w * 0.5,
            r.mid_y(),
            type_scale::CAPTION,
            rgb(palette::FAINT, 1.0),
            "Not enough of the match was recorded",
        );
        return None;
    }
    let length = length.max(1.0);
    let at = |t: f32, v: f32| Vec2::new(r.x + r.w * t / length, r.bottom() - r.h * v / top);
    // Thinned to about one point every two points of width.
    let stride = (times.len() as f32 / (r.w / 2.0)).ceil().max(1.0) as usize;
    let shown = ((times.len() as f32 * reveal.clamp(0.0, 1.0)).ceil() as usize).min(times.len());
    let fill_all = series.len() <= 2;
    for (i, s) in series.iter().enumerate() {
        let dim = focus.is_some_and(|f| f != i);
        let alpha = if dim { 0.18 } else { 1.0 };
        let pts: Vec<Vec2> = (0..shown)
            .step_by(stride)
            .chain((shown > 0).then_some(shown - 1))
            .map(|k| at(times[k], s.values.get(k).copied().unwrap_or(0.0)))
            .collect();
        if pts.len() < 2 {
            continue;
        }
        // A wash under the curve, fading down to the axis.
        if (fill_all && focus.is_none()) || focus == Some(i) {
            let c = s.color;
            for w in pts.windows(2) {
                let x0 = w[0].x;
                let wide = (w[1].x - x0).max(1.0);
                let y = w[0].y.min(w[1].y);
                ui.gradient_v(
                    Rect::new(x0, y, wide, r.bottom() - y),
                    [c[0], c[1], c[2], 0.12],
                    [c[0], c[1], c[2], 0.0],
                );
            }
        }
        let c = s.color;
        ui.polyline(&pts, 5.0, [c[0], c[1], c[2], 0.12 * alpha], false);
        ui.polyline(&pts, 1.8, [c[0], c[1], c[2], alpha], false);
        if let Some(&end) = pts.last() {
            ui.dot(end, 3.0, [c[0], c[1], c[2], alpha]);
        }
    }

    // The crosshair: the sample under the pointer, every side's value at it.
    let res = ui.interact_with(id, r, true, false);
    if !res.hovered || shown == 0 {
        return None;
    }
    let t = (ui.cursor.x - ui.shift.x - r.x) / r.w * length;
    let k = times[..shown].partition_point(|&x| x < t).min(shown - 1);
    let x = r.x + r.w * times[k] / length;
    ui.vline(x, r.y, r.h, rgb(palette::LINE, 0.45));
    let mut rows: Vec<(usize, f32)> = series
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s.values.get(k).copied().unwrap_or(0.0)))
        .collect();
    for &(i, v) in &rows {
        let c = series[i].color;
        ui.dot(at(times[k], v), 4.5, ink(0.9));
        ui.dot(at(times[k], v), 3.0, [c[0], c[1], c[2], 1.0]);
    }
    rows.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let card_w = 210.0;
    let card_h = 34.0 + rows.len() as f32 * 20.0;
    let cx = if x + 16.0 + card_w > r.right() {
        x - 16.0 - card_w
    } else {
        x + 16.0
    };
    let card = Rect::new(cx, (r.y + 10.0).min(r.bottom() - card_h), card_w, card_h);
    ui.frost_cut(card, 6.0, 0.92);
    ui.bevel(card, 6.0, 0.7);
    ui.text(
        card.x + 12.0,
        card.y + 16.0,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        &clock((times[k] * mc_core::TICKS_PER_SECOND as f32) as u32),
    );
    for (row, &(i, v)) in rows.iter().enumerate() {
        let y = card.y + 38.0 + row as f32 * 20.0;
        ui.fill(Rect::new(card.x + 12.0, y - 4.0, 8.0, 8.0), series[i].color);
        let (st, name) = ui.fitted(type_scale::BODY, series[i].label, card_w - 110.0);
        ui.text(card.x + 28.0, y, st, rgb(palette::TEXT, 0.9), &name);
        ui.text_right(
            card.right() - 12.0,
            y,
            type_scale::VALUE,
            rgb(palette::TEXT, 1.0),
            &if unit.is_empty() {
                short(v)
            } else {
                format!("{} {unit}", short(v))
            },
        );
    }
    Some(k)
}

/// Stacked bars: `bins[i][side]`, coloured by side, drawn in from the left by `reveal`.
/// `marks` are spans (from, to, in bins) to bracket over the bars.
pub fn stacked_bars(
    ui: &mut Ui,
    r: Rect,
    bins: &[Vec<f32>],
    colors: &[Color],
    reveal: f32,
    marks: &[(f32, f32)],
) {
    ui.fill(r, ink(0.35));
    ui.hline(r.x, r.bottom(), r.w, rgb(palette::LINE, 0.28));
    let max = bins
        .iter()
        .map(|b| b.iter().sum::<f32>())
        .fold(0.0f32, f32::max)
        .max(1.0);
    let n = bins.len().max(1) as f32;
    let bw = r.w / n;
    let gap = (bw * 0.18).min(2.0);
    for (i, b) in bins.iter().enumerate() {
        let k = ((reveal * n - i as f32) * 2.0).clamp(0.0, 1.0);
        if k <= 0.0 {
            break;
        }
        let mut y = r.bottom();
        for (side, &v) in b.iter().enumerate() {
            if v <= 0.0 {
                continue;
            }
            let h = r.h * 0.94 * v / max * k;
            let c = colors.get(side).copied().unwrap_or([1.0; 4]);
            ui.fill(
                Rect::new(r.x + i as f32 * bw + gap * 0.5, y - h, bw - gap, h),
                [c[0], c[1], c[2], 0.85],
            );
            y -= h;
        }
    }
    for &(from, to) in marks {
        let (x0, x1) = (r.x + from * bw, r.x + (to + 1.0) * bw);
        ui.hline(x0, r.y + 2.0, x1 - x0, rgb(palette::ACCENT, 0.9));
        ui.vline(x0, r.y + 2.0, 6.0, rgb(palette::ACCENT, 0.9));
        ui.vline(x1, r.y + 2.0, 6.0, rgb(palette::ACCENT, 0.9));
    }
}
