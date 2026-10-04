//! The HUD's segmented switch (Last / Even / First, `focus.rs` and `priority.rs`): one
//! bevelled button body like a tile's, with a cut-corner pill that slides to the chosen
//! segment, outlined and barred along its foot as a lit tile is. The caller does the
//! interaction and draws what is on each segment.

use crate::ui::{ink, palette, rgb, Color, Rect, Ui};

/// The body's corner cut: a tile's, smaller on a slim switch.
fn cut(r: Rect) -> f32 {
    (r.h * 0.3).min(5.0)
}

/// Segment `n` of `count` across `r`.
pub(in crate::hud) fn segment(r: Rect, n: usize, count: usize) -> Rect {
    let w = r.w / count as f32;
    Rect::new(r.x + n as f32 * w, r.y, w, r.h)
}

/// How far a segment's pill and hover sit in from its edges: clear of the body's bevel.
fn inner(r: Rect, seg: Rect) -> Rect {
    seg.inset(if r.h < 20.0 { 2.0 } else { 3.0 })
}

/// The body: a tile's dark cut-corner glass and bevelled edges, brighter while `glow`
/// (the pointer is on a segment).
pub(in crate::hud) fn body(ui: &mut Ui, r: Rect, glow: f32) {
    let c = cut(r);
    ui.fill_cut(r, c, ink(0.62));
    ui.fill_cut(r, c, rgb(0xFFFFFF, 0.04 * glow));
    ui.bevel(r, c, (0.45 + 0.4 * glow).min(1.0));
}

/// The pointer's wash on segment `n` of `count`.
pub(in crate::hud) fn hover(ui: &mut Ui, r: Rect, n: usize, count: usize, glow: f32) {
    if glow > 0.01 {
        let p = inner(r, segment(r, n, count));
        ui.fill_cut(p, cut(p), rgb(palette::TEXT, 0.06 * glow));
    }
}

/// A faint divider before segment `n` (not the first), left out beside the pill resting
/// at `at`, whose own outline parts it.
pub(in crate::hud) fn divider(ui: &mut Ui, r: Rect, n: usize, count: usize, at: Option<f32>) {
    if n == 0 || at.is_some_and(|a| (a - n as f32).abs() < 0.5 || (a + 1.0 - n as f32).abs() < 0.5)
    {
        return;
    }
    let s = segment(r, n, count);
    let inset = (r.h * 0.28).max(4.0);
    ui.vline(
        s.x,
        s.y + inset,
        s.h - 2.0 * inset,
        rgb(palette::LINE, 0.12),
    );
}

/// The pill at `at` segments from the left (eased, so it slides), in `color`, `lit`
/// zero to one: a soft cut-corner wash and an outline in its colour, as a lit tile has.
pub(in crate::hud) fn pill(ui: &mut Ui, r: Rect, at: f32, count: usize, color: Color, lit: f32) {
    if lit <= 0.01 {
        return;
    }
    let w = r.w / count as f32;
    let p = inner(r, Rect::new(r.x + at * w, r.y, w, r.h));
    let c = cut(p);
    ui.fill_cut(p, c, with_alpha(color, 0.13 * lit));
    let edge = with_alpha(color, 0.75 * lit);
    ui.outline_cut(p, c, edge, edge);
}

/// Where segment `n`'s inner area is, for a caller's own marks (a split gauge, an alarm).
pub(in crate::hud) fn inner_segment(r: Rect, n: usize, count: usize) -> Rect {
    inner(r, segment(r, n, count))
}

pub(in crate::hud) fn with_alpha(mut c: Color, a: f32) -> Color {
    c[3] = a;
    c
}
