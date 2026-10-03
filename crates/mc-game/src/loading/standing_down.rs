//! Standing down: the loading screen between a match and the front end. No
//! map and no briefing, since nothing is being deployed: the game's M in a
//! sighting ring on the meridian, what was left and for how long, and the
//! loading line. It arrives over the match's last picture and lifts off the
//! front end. Presentation only: every float here ends on the screen.

use super::ease_out;
use super::opening::{falloff, glow_dot, m_height, Monogram, StatusLine};
use crate::ui::{self, palette, rgb, Rect, Ui};
use glam::Vec2;
use mc_render::Face;
use std::f32::consts::{FRAC_PI_2, TAU};

/// The earliest the screen lifts, so its words are there to be read.
pub(super) const LINGER: f32 = 1.6;

// When each part arrives, in seconds after the screen came up. All of it is
// in before the build starts (`SETTLED_IN`), so no stall lands mid-arrival.
const LINES: (f32, f32) = (0.0, 0.65);
const RING: (f32, f32) = (0.1, 0.65);
const TRACE: (f32, f32) = (0.05, 0.45);
const FILL: (f32, f32) = (0.25, 0.6);
const BAR: (f32, f32) = (0.35, 0.68);
const OVERLINE: (f32, f32) = (0.2, 0.55);
const HEADLINE: (f32, f32) = (0.25, 0.62);
const DETAIL: (f32, f32) = (0.32, 0.68);
const STATUS: (f32, f32) = (0.4, 0.7);
/// A ring runs out from the sight this often, on the screen's clock.
const PING_EVERY: f32 = 3.2;
/// The scope's arc goes round once in this many seconds.
const SWEEP_EVERY: f32 = 5.5;

const HEADLINE_TEXT: &str = "Returning to Command";

fn phase(age: f32, (from, to): (f32, f32)) -> f32 {
    ((age - from) / (to - from)).clamp(0.0, 1.0)
}

pub(super) struct StandingDown {
    status: StatusLine,
}

impl StandingDown {
    pub(super) fn new() -> StandingDown {
        StandingDown {
            status: StatusLine::new(),
        }
    }

    /// Draws the screen: `age` seconds since it came up (its arrival keeps
    /// wall time), `clock` the screen's own clock (it rests while the window
    /// stalls), `lift` 0..1 as it goes, `bar` how far the loading has got.
    #[expect(
        clippy::too_many_arguments,
        reason = "the curtain's state, read once a frame"
    )]
    pub(super) fn draw(
        &mut self,
        ui: &mut Ui,
        age: f32,
        clock: f32,
        lift: f32,
        bar: f32,
        step: Option<&'static str>,
        (overline, detail): (&str, &str),
    ) {
        let (w, h) = (ui.size.x, ui.size.y);
        let surge = super::ease_in_out(lift);
        let centre = Vec2::new(w * 0.5, h * 0.4);
        let m_size = (h * 0.085).clamp(56.0, 120.0);
        let ring = m_size * 0.98 * (1.0 + 0.3 * surge);
        ui.fill(Rect::new(0.0, 0.0, w, h), ui::ink(1.0));

        // A low glow behind the sight, the only warmth on the screen.
        let lines = ease_out(phase(age, LINES));
        let glow = falloff(0.09 * lines);
        ui.ribbon_cap(centre, Vec2::new(0.0, -1.0), h * 0.4, &glow);
        ui.ribbon_cap(centre, Vec2::new(0.0, 1.0), h * 0.4, &glow);

        self.cross(ui, centre, ring, lines);
        scope(ui, centre, ring, age, clock);
        let m = Monogram {
            trace: phase(age, TRACE),
            fill: super::ease_in_out(phase(age, FILL)),
            bar: ease_out(phase(age, BAR)),
            breath: 0.85 + 0.15 * (clock * 1.7).sin(),
        };
        let grow = 0.94 + 0.06 * ease_out(phase(age, FILL)) + 0.06 * surge;
        m.draw(ui, centre, m_size * grow / m_height());

        // The words, under the sight.
        let y = centre.y + ring + 62.0 - 18.0 * surge;
        let rise = |p: f32| (1.0 - p) * 10.0;
        let accent = rgb(palette::ACCENT, 1.0);
        let saved = ui.fade;
        let e = ease_out(phase(age, OVERLINE));
        ui.fade = saved * e;
        let caps = ui::style(Face::Bold, 13.5, 3.2);
        let over = overline.to_uppercase();
        let ow = ui.text_width(caps, &over);
        ui.text(centre.x - ow * 0.5, y + rise(e), caps, accent, &over);
        // Short rules either side of the overline, out from it.
        let rule = 26.0 * e;
        for side in [-1.0, 1.0] {
            let from = centre.x + side * (ow * 0.5 + 14.0);
            let x = if side < 0.0 { from - rule } else { from };
            ui.hline(x, y + rise(e), rule, rgb(palette::ACCENT, 0.55));
        }
        let e = ease_out(phase(age, HEADLINE));
        ui.fade = saved * e;
        ui.text_centred(
            centre.x,
            y + 44.0 + rise(e) * 1.4,
            ui::style(Face::Light, 40.0, 0.6),
            rgb(palette::TEXT, 1.0),
            HEADLINE_TEXT,
        );
        let e = ease_out(phase(age, DETAIL));
        ui.fade = saved * e;
        if !detail.is_empty() {
            ui.text_centred(
                centre.x,
                y + 82.0 + rise(e) * 1.8,
                ui::style(Face::Medium, 14.0, 0.8),
                rgb(palette::DIM, 1.0),
                detail,
            );
        }
        let e = ease_out(phase(age, STATUS));
        ui.fade = saved * e;
        self.status
            .draw(ui, Vec2::new(centre.x, h - 96.0), clock, bar, step);
        ui.fade = saved;
    }

    /// The meridian down the screen and the horizon across it, out from the
    /// sight as the screen arrives, each fading to nothing at its ends.
    fn cross(&self, ui: &mut Ui, centre: Vec2, ring: f32, lines: f32) {
        let (w, h) = (ui.size.x, ui.size.y);
        let gap = ring + 10.0;
        let accent = rgb(palette::ACCENT, 1.0);
        let clear = rgb(palette::ACCENT, 0.0);
        let line = rgb(palette::LINE, 0.22);
        let line_clear = rgb(palette::LINE, 0.0);
        // Up from the sight to the top of the screen.
        let up = (centre.y - gap) * lines;
        if up > 0.0 {
            let top = centre.y - gap - up;
            ui.gradient_v(
                Rect::new(centre.x - 0.75, top, 1.5, up),
                clear,
                rgb(palette::ACCENT, 0.7),
            );
        }
        // Down from the sight a short way, ending in a point over the words.
        let stub = 26.0 * lines;
        if stub > 0.0 {
            ui.gradient_v(
                Rect::new(centre.x - 0.75, centre.y + gap, 1.5, stub),
                rgb(palette::ACCENT, 0.7),
                accent,
            );
            ui.disc(Vec2::new(centre.x, centre.y + gap + stub), 2.2, accent);
        }
        // The horizon, both ways.
        let reach = (w * 0.5 - gap) * lines;
        if reach > 0.0 {
            ui.gradient_h(
                Rect::new(centre.x - gap - reach, centre.y, reach, 1.0),
                line_clear,
                line,
            );
            ui.gradient_h(
                Rect::new(centre.x + gap, centre.y, reach, 1.0),
                line,
                line_clear,
            );
        }
        // Range marks along the horizon, faint, closer in brighter.
        let step = (h * 0.09).max(48.0);
        for i in 1..=12 {
            let d = gap + step * i as f32;
            if d - gap > reach {
                break;
            }
            let a = 0.2 * (1.0 - i as f32 / 13.0) * lines;
            let tall = if i % 3 == 0 { 7.0 } else { 3.0 };
            for side in [-1.0, 1.0] {
                ui.vline(
                    centre.x + side * d,
                    centre.y - tall * 0.5,
                    tall,
                    rgb(palette::LINE, a),
                );
            }
        }
    }
}

/// The sighting ring round the M: drawn round from the top as the screen
/// arrives, with marks at the quarters, an outer scale, an arc sweeping it
/// and rings running out from it.
fn scope(ui: &mut Ui, centre: Vec2, r: f32, age: f32, clock: f32) {
    let drawn = super::ease_in_out(phase(age, RING));
    if drawn <= 0.0 {
        return;
    }
    let top = -FRAC_PI_2;
    let line = |a: f32| rgb(palette::LINE, a);
    ui.arc(centre, r, top, top + TAU * drawn, 1.0, line(0.3));
    // The outer scale, a mark every ten degrees.
    let outer = r + 9.0;
    for i in 0..36 {
        let a = top + i as f32 / 36.0 * TAU;
        if a - top > TAU * drawn {
            break;
        }
        let quarter = i % 9 == 0;
        let (from, to) = if quarter {
            (r - 5.0, outer + 7.0)
        } else {
            (outer, outer + 3.0)
        };
        let dir = Vec2::new(a.cos(), a.sin());
        let ink = if quarter { line(0.55) } else { line(0.18) };
        ui.stroke(centre + dir * from, centre + dir * to, 1.0, ink);
    }
    // The sweep: a bright arc with a fading tail, round on the screen's clock.
    let head = top + (clock / SWEEP_EVERY).fract() * TAU;
    let tail = 1.1;
    for k in 0..8 {
        let a0 = head - tail * (k + 1) as f32 / 8.0;
        let a1 = head - tail * k as f32 / 8.0;
        let fade = 1.0 - k as f32 / 8.0;
        ui.arc(
            centre,
            r,
            a0,
            a1,
            1.8,
            rgb(palette::ACCENT, 0.75 * fade * fade * drawn),
        );
    }
    let tip = centre + Vec2::new(head.cos(), head.sin()) * r;
    glow_dot(ui, tip, 10.0, 0.6 * drawn);
    // Rings out from the sight, slow and faint.
    let u = (clock / PING_EVERY).fract();
    let reach = r + ease_out(u) * ui.size.y * 0.32;
    let a = 0.09 * (1.0 - u).powi(2) * drawn;
    if a > 0.002 {
        ui.arc(centre, reach, 0.0, TAU, 1.0, rgb(palette::ACCENT, a));
    }
}
