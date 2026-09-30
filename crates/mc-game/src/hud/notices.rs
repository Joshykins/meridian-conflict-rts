//! The notice stack across the top of the screen: one place for every toast, launch
//! warning and "this just happened" line, which folds repeats into one card.
//!
//! Two kinds of card share it. A *live* card is rebuilt every frame from what is true
//! now (warheads in flight, grouped by whose they are), so it can never go stale or
//! double up. A *note* is an event that fades on its own; the same note again while it
//! is up bumps a count on the card already there ("Warhead intercepted ×3") instead of
//! stacking a second one.

use super::{Hud, HudAction};
use crate::audio::Sfx;
use crate::ui::{id, ink, palette, rgb, style, type_scale, Rect, Ui};
use glam::Vec2;
use mc_render::Face;

/// How long a note stays up after its last repeat, in seconds.
const NOTE_LIFE: f32 = 4.5;
/// Notes shown at once; the oldest go first.
const MAX_NOTES: usize = 4;
/// A live card's width.
const LIVE_W: f32 = 380.0;

/// The mark on a card's left.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    /// A plain bar: an ordinary message.
    Bar,
    /// A turning trefoil: a warhead.
    Trefoil,
    /// A ring with a cross: an interception.
    Intercept,
    /// A tier-5 titan's figure (`hud/titan.rs`).
    Titan,
    /// A lightning bolt in a turning ring: a titan's great bore.
    Storm,
}

/// Something going on right now, grouped: built fresh every frame.
pub struct Live {
    pub key: &'static str,
    pub title: String,
    pub sub: String,
    pub tone: u32,
    pub glyph: Glyph,
    /// Loud: an enemy's; the card breathes and blinks faster as the time runs down.
    pub loud: bool,
    /// Big figure on the right (seconds to the soonest), if any.
    pub figure: Option<String>,
    /// How soon each member lands (seconds) and where: drawn as ticks on a time strip,
    /// and a click looks at them in turn, soonest first.
    pub marks: Vec<(f32, Vec2)>,
}

struct Note {
    key: String,
    title: String,
    tone: u32,
    glyph: Glyph,
    count: u32,
    /// Seconds since the last repeat.
    age: f32,
    /// Seconds since the card first appeared (for the slide-in).
    shown: f32,
    /// Flash on a repeat, 1 → 0.
    flash: f32,
    /// Where it happened, newest last; a click looks at the newest.
    at: Vec<Vec2>,
}

#[derive(Default)]
pub struct Notices {
    notes: Vec<Note>,
    live: Vec<Live>,
    /// Which member a live card's next click looks at, by card key.
    cycle: Vec<(&'static str, usize)>,
}

impl Notices {
    /// A note; the same key while its card is still up counts on that card instead.
    pub fn note(
        &mut self,
        key: impl Into<String>,
        title: impl Into<String>,
        tone: u32,
        glyph: Glyph,
        at: Option<Vec2>,
    ) {
        let key = key.into();
        if let Some(n) = self.notes.iter_mut().find(|n| n.key == key) {
            n.count += 1;
            n.age = 0.0;
            n.flash = 1.0;
            n.title = title.into();
            n.tone = tone;
            n.at.extend(at);
            return;
        }
        self.notes.push(Note {
            key,
            title: title.into(),
            tone,
            glyph,
            count: 1,
            age: 0.0,
            shown: 0.0,
            flash: 0.0,
            at: at.into_iter().collect(),
        });
        if self.notes.len() > MAX_NOTES {
            self.notes.remove(0);
        }
    }

    /// A live card for this frame.
    pub fn live(&mut self, card: Live) {
        self.live.push(card);
    }
}

/// Draws the stack from `top` down: live cards first, then notes. Clears this frame's
/// live cards.
pub fn draw(hud: &mut Hud, ui: &mut Ui, top: f32, dt: f32) {
    let _t = mc_core::perf_span!("ui.notices");
    let t = ui.time;
    let w = ui.size.x;
    let mut y = top;

    let live = std::mem::take(&mut hud.notices.live);
    hud.notices
        .cycle
        .retain(|(k, _)| live.iter().any(|c| c.key == *k));
    for (i, c) in live.iter().enumerate() {
        let strip = c.marks.len() > 1;
        let h = if strip { 58.0 } else { 50.0 };
        let r = Rect::new((w - LIVE_W) * 0.5, y, LIVE_W, h);
        hud.claim(ui, r);
        let res = ui.interact(id("notice-live", i), r, !c.marks.is_empty());
        let soonest = c.marks.iter().map(|m| m.0).fold(f32::INFINITY, f32::min);
        let urgency = if soonest.is_finite() {
            (1.0 - soonest / 40.0).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let blink = 0.5 + 0.5 * (t * (3.0 + 7.0 * urgency)).sin();
        ui.frost_cut(r, 6.0, 0.9);
        ui.fill_cut(r, 6.0, ink(0.55));
        let wash = if c.loud {
            0.18 + 0.2 * blink
        } else {
            0.18 + 0.08 * blink
        };
        ui.gradient_h(r, rgb(c.tone, wash + 0.1 * res.glow), rgb(c.tone, 0.0));
        ui.fill(
            Rect::new(r.x, r.y, 3.0, r.h),
            rgb(c.tone, 0.7 + 0.3 * blink),
        );
        ui.outline_cut(
            r,
            6.0,
            rgb(c.tone, 0.35 + 0.3 * res.glow),
            rgb(0xFFFFFF, 0.3),
        );
        glyph(
            ui,
            Vec2::new(r.x + 26.0, r.y + 25.0),
            c.glyph,
            c.tone,
            t,
            1.0,
        );
        let title_ink = rgb(0xFFFFFF, if c.loud { 0.75 + 0.25 * blink } else { 1.0 });
        ui.text(
            r.x + 46.0,
            r.y + 17.0,
            style(Face::Bold, 17.0, 0.6),
            title_ink,
            &c.title,
        );
        ui.text_fit_left(
            r.x + 46.0,
            r.y + 36.0,
            r.w - 150.0,
            type_scale::MICRO,
            rgb(c.tone, 1.0),
            &c.sub,
        );
        if let Some(f) = &c.figure {
            ui.text_right(
                r.right() - 12.0,
                r.y + 17.0,
                style(Face::Bold, 22.0, 0.0),
                rgb(c.tone, 1.0),
                f,
            );
        }
        let n = c.marks.len();
        let at = hud
            .notices
            .cycle
            .iter()
            .find(|(k, _)| *k == c.key)
            .map_or(0, |e| e.1)
            % n.max(1);
        if n > 0 {
            let hint = if n == 1 {
                "Click to look".to_owned()
            } else {
                format!("Click to look  {}/{}", at + 1, n)
            };
            ui.text_right(
                r.right() - 12.0,
                r.y + 36.0,
                type_scale::MICRO,
                rgb(palette::DIM, 0.6 + 0.4 * res.glow),
                &hint,
            );
        }
        if strip {
            time_strip(ui, r, c, at, blink);
        }
        if res.clicked && n > 0 {
            ui.audio.play(Sfx::Select);
            let mut order: Vec<_> = c.marks.clone();
            order.sort_by(|a, b| a.0.total_cmp(&b.0));
            hud.actions.push(HudAction::LookAt(order[at].1));
            match hud.notices.cycle.iter_mut().find(|(k, _)| *k == c.key) {
                Some(e) => e.1 = (at + 1) % n,
                None => hud.notices.cycle.push((c.key, (at + 1) % n)),
            }
        }
        y += h + 8.0;
    }

    for n in &mut hud.notices.notes {
        n.age += dt;
        n.shown += dt;
        n.flash = (n.flash - dt * 3.0).max(0.0);
    }
    hud.notices.notes.retain(|n| n.age < NOTE_LIFE);
    let mut clicked = None;
    for (i, n) in hud.notices.notes.iter().enumerate() {
        let k = (n.shown / 0.2).min(1.0) * ((NOTE_LIFE - n.age) / 0.7).clamp(0.0, 1.0);
        let count = (n.count > 1).then(|| format!("\u{d7}{}", n.count));
        let cw = count
            .as_ref()
            .map_or(0.0, |c| ui.text_width(type_scale::VALUE, c) + 14.0);
        let tw = ui.text_width(type_scale::CAPTION, &n.title) + 58.0 + cw;
        let r = Rect::new((w - tw) * 0.5, y - 6.0 * (1.0 - k), tw, 30.0);
        let res = ui.interact(id("notice-note", i), r, !n.at.is_empty());
        if k > 0.5 {
            hud.covered
                .push(Rect::new(r.x * ui.s, r.y * ui.s, r.w * ui.s, r.h * ui.s));
        }
        ui.frost_cut(r, 4.0, 0.85 * k);
        ui.fill_cut(r, 4.0, ink((0.6 + 0.2 * n.flash) * k));
        ui.gradient_h(
            r,
            rgb(n.tone, (0.1 + 0.25 * n.flash + 0.08 * res.glow) * k),
            rgb(n.tone, 0.0),
        );
        ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(n.tone, k));
        glyph(ui, Vec2::new(r.x + 18.0, r.mid_y()), n.glyph, n.tone, t, k);
        ui.text(
            r.x + 34.0,
            r.mid_y(),
            type_scale::CAPTION,
            rgb(n.tone, k),
            &n.title,
        );
        if let Some(c) = &count {
            let cr = Rect::new(r.right() - cw - 8.0, r.y + 6.0, cw, r.h - 12.0);
            ui.fill(cr, rgb(n.tone, (0.2 + 0.5 * n.flash) * k));
            ui.text_centred(
                cr.x + cr.w * 0.5,
                cr.mid_y(),
                type_scale::VALUE,
                rgb(0xFFFFFF, k),
                c,
            );
        }
        if res.clicked {
            clicked = n.at.last().copied();
        }
        y += 36.0;
    }
    if let Some(at) = clicked {
        ui.audio.play(Sfx::Select);
        hud.actions.push(HudAction::LookAt(at));
    }
}

/// Under a grouped card: time runs right to left toward the impact line on the left,
/// one tick per member; the one a click will look at next is bracketed.
fn time_strip(ui: &mut Ui, r: Rect, c: &Live, next: usize, blink: f32) {
    let x0 = r.x + 46.0;
    let x1 = r.right() - 12.0;
    let y = r.bottom() - 9.0;
    let far = c.marks.iter().map(|m| m.0).fold(10.0f32, f32::max);
    // Round the span up to a tidy 10 s so ticks do not creep as the farthest lands.
    let span = (far / 10.0).ceil() * 10.0;
    ui.fill(Rect::new(x0, y, x1 - x0, 1.0), rgb(palette::LINE, 0.25));
    ui.fill(
        Rect::new(x0 - 1.0, y - 4.0, 2.0, 9.0),
        rgb(c.tone, 0.6 + 0.4 * blink),
    );
    let mut order: Vec<_> = c.marks.iter().map(|m| m.0).collect();
    order.sort_by(f32::total_cmp);
    for (i, eta) in order.iter().enumerate() {
        let x = x0 + (x1 - x0) * (eta.max(0.0) / span).min(1.0);
        let hot = i == next;
        let hgt = if hot { 9.0 } else { 7.0 };
        ui.fill(
            Rect::new(x - 1.5, y - hgt * 0.5, 3.0, hgt),
            if hot {
                rgb(0xFFFFFF, 1.0)
            } else {
                rgb(c.tone, 0.8)
            },
        );
        if hot {
            ui.brackets(
                Rect::new(x - 5.0, y - 6.0, 10.0, 12.0),
                3.0,
                rgb(0xFFFFFF, 0.7),
            );
        }
    }
}

fn glyph(ui: &mut Ui, c: Vec2, g: Glyph, tone: u32, t: f32, k: f32) {
    match g {
        Glyph::Bar => {
            ui.fill(
                Rect::new(c.x - 1.0, c.y - 6.0, 2.0, 12.0),
                rgb(tone, 0.8 * k),
            );
        }
        Glyph::Trefoil => {
            for i in 0..3 {
                let a = t * 0.9 + i as f32 * std::f32::consts::TAU / 3.0;
                ui.arc(c, 7.5, a - 0.5, a + 0.5, 7.0, rgb(tone, 0.9 * k));
            }
            ui.disc(c, 2.4, rgb(tone, k));
        }
        Glyph::Titan => super::icons::titan(ui, c, 9.0, rgb(tone, k)),
        Glyph::Storm => super::titan::bolt(ui, c, 7.0, rgb(tone, 0.95 * k), t * 0.9),
        Glyph::Intercept => {
            ui.arc(c, 6.5, 0.0, std::f32::consts::TAU, 1.5, rgb(tone, 0.9 * k));
            ui.fill(
                Rect::new(c.x - 9.0, c.y - 0.5, 18.0, 1.0),
                rgb(tone, 0.7 * k),
            );
            ui.fill(
                Rect::new(c.x - 0.5, c.y - 9.0, 1.0, 18.0),
                rgb(tone, 0.7 * k),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeats_fold_into_one_card_with_a_count() {
        let mut n = Notices::default();
        for _ in 0..3 {
            n.note(
                "intercepted",
                "Warhead intercepted",
                0,
                Glyph::Intercept,
                Some(Vec2::ONE),
            );
        }
        n.note("ready", "Warhead ready", 0, Glyph::Trefoil, None);
        assert_eq!(n.notes.len(), 2);
        assert_eq!(n.notes[0].count, 3);
        assert_eq!(n.notes[0].at.len(), 3);
        for i in 0..10 {
            n.note(format!("k{i}"), "x", 0, Glyph::Bar, None);
        }
        assert_eq!(n.notes.len(), MAX_NOTES);
    }
}
