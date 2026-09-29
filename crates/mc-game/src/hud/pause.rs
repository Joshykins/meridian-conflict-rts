//! The pause frame. A paused match is still played: the camera moves, orders and
//! spawns are carried out at once (the sim applies them without the clock moving),
//! so nothing is laid over the battlefield. The edges of the screen say time is
//! held, and the top bar's play button lets it go.

use super::Hud;
use crate::ui::{id, palette, rgb, Rect, Ui};

impl Hud {
    /// The battlefield held still: the edges darken, a hairline and corner brackets
    /// frame the view, and a light runs slowly round it. Drawn under every panel.
    pub(super) fn pause_frame(&mut self, ui: &mut Ui, paused: bool) {
        // A photo taken with the free camera keeps a clean frame.
        let on = paused && !self.free.on;
        let k = ui.ease(id("pause-frame", 0), if on { 1.0 } else { 0.0 }, 7.0);
        if k < 0.01 {
            return;
        }
        let (w, h) = (ui.size.x, ui.size.y);
        let breath = 0.85 + 0.15 * (ui.time * 1.7).sin();

        // The view darkens toward every edge, as a held frame does.
        let d = (w.min(h) * 0.16).clamp(80.0, 220.0);
        let dark = 0.62 * k;
        ui.scrim(Rect::new(0.0, 0.0, w, d), dark, 0.0, false);
        ui.scrim(Rect::new(0.0, h - d, w, d), 0.0, dark, false);
        ui.scrim(Rect::new(0.0, 0.0, d, h), dark, 0.0, true);
        ui.scrim(Rect::new(w - d, 0.0, d, h), 0.0, dark, true);
        // A glow just inside the edge, under the hairline.
        let glow = rgb(palette::LINE, 0.2 * k * breath);
        let clear = rgb(palette::LINE, 0.0);
        let g = 22.0;
        ui.gradient_v(Rect::new(0.0, 0.0, w, g), glow, clear);
        ui.gradient_v(Rect::new(0.0, h - g, w, g), clear, glow);
        ui.gradient_h(Rect::new(0.0, 0.0, g, h), glow, clear);
        ui.gradient_h(Rect::new(w - g, 0.0, g, h), clear, glow);

        // The hairline round the whole screen.
        let line = rgb(palette::LINE, 0.7 * k * breath);
        let t = 3.0;
        ui.fill(Rect::new(0.0, 0.0, w, t), line);
        ui.fill(Rect::new(0.0, h - t, w, t), line);
        ui.fill(Rect::new(0.0, 0.0, t, h), line);
        ui.fill(Rect::new(w - t, 0.0, t, h), line);

        // Corner brackets, a little inside the hairline: the frame is held.
        let (inset, arm, thick) = (12.0, 96.0, 4.0);
        let tone = rgb(palette::LINE, 0.9 * k);
        for (cx, cy, dx, dy) in [
            (inset, inset, 1.0, 1.0),
            (w - inset, inset, -1.0, 1.0),
            (w - inset, h - inset, -1.0, -1.0),
            (inset, h - inset, 1.0, -1.0),
        ] {
            let x = if dx > 0.0 { cx } else { cx - arm };
            let y = if dy > 0.0 { cy } else { cy - thick };
            ui.fill(Rect::new(x, y, arm, thick), tone);
            let x = if dx > 0.0 { cx } else { cx - thick };
            let y = if dy > 0.0 { cy } else { cy - arm };
            ui.fill(Rect::new(x, y, thick, arm), tone);
        }

        // Lights running round the hairline, clockwise, a lap every seven seconds;
        // two, half a lap apart, so one is always in sight.
        let lap = 2.0 * (w + h);
        let head = (ui.time * lap / 7.0) % lap;
        for i in 0..2 {
            sweep(ui, (head + i as f32 * lap * 0.5) % lap, 220.0, (w, h), t, k);
        }
    }
}

/// One running light: bright at `head`, fading out over `len` behind it. Distances
/// run clockwise round the screen from the top-left corner.
fn sweep(ui: &mut Ui, head: f32, len: f32, (w, h): (f32, f32), t: f32, k: f32) {
    let lap = 2.0 * (w + h);
    let mut start = 0.0;
    for (e, edge) in [w, h, w, h].into_iter().enumerate() {
        // The tail may reach back past the top-left corner, onto the left edge.
        for wrap in [0.0, lap] {
            let (a, b) = (head - len + wrap - start, head + wrap - start);
            let (a0, b0) = (a.max(0.0), b.min(edge));
            if b0 <= a0 {
                continue;
            }
            let tail = rgb(palette::LINE, 0.95 * k * (a0 - a) / len);
            let lead = rgb(palette::LINE, 0.95 * k * (b0 - a) / len);
            match e {
                0 => ui.gradient_h(Rect::new(a0, 0.0, b0 - a0, t), tail, lead),
                1 => ui.gradient_v(Rect::new(w - t, a0, t, b0 - a0), tail, lead),
                2 => ui.gradient_h(Rect::new(w - b0, h - t, b0 - a0, t), lead, tail),
                _ => ui.gradient_v(Rect::new(0.0, h - b0, t, b0 - a0), lead, tail),
            }
        }
        start += edge;
    }
}
