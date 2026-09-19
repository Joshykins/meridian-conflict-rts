//! The interface toolkit behind the front end and the in-match menu.
//!
//! Immediate mode, like the HUD: a screen is a function that draws itself into
//! the overlay every frame and reports what was clicked. What outlives a frame
//! is in `Memory`: which control the pointer is on, which one is held, and an
//! eased 0..1 value per control that every hover glow and slide is driven by.
//!
//! Screens are laid out in points on a canvas 1080 points tall; `Ui` scales
//! that to the window, so the interface keeps its proportions from 720p to 4K
//! and type is rasterised at the real pixel size rather than stretched.
//!
//! Sound is part of the toolkit, not of the screens: a control makes its own
//! hover, press and refusal sounds, so nothing interactive can end up mute.

pub mod backdrop;
pub mod front;
pub mod menu;
pub mod options;
pub mod pause;
pub mod preview;
pub mod skirmish;

use crate::audio::{Audio, Sfx};
use glam::Vec2;
use mc_render::{Face, Overlay, Type};
use std::collections::HashMap;

/// Height of the design canvas in points.
pub const CANVAS_H: f32 = 1080.0;

pub type Color = [f32; 4];

/// An sRGB hex colour as the linear RGBA the overlay blends in.
pub fn rgb(hex: u32, alpha: f32) -> Color {
    let channel = |shift: u32| {
        let c = ((hex >> shift) & 0xFF) as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    [channel(16), channel(8), channel(0), alpha]
}

/// The interface's near-black at a perceived opacity. Blending is linear-light,
/// where half-transparent black still looks bright; this bends the alpha so
/// that `ink(0.5)` looks half as bright, the way a design tool would show it.
pub fn ink(opacity: f32) -> Color {
    rgb(palette::INK, 1.0 - (1.0 - opacity.clamp(0.0, 1.0)).powf(2.2))
}

pub mod palette {
    pub const TEXT: u32 = 0xE9F2F6;
    pub const DIM: u32 = 0x8A9CA8;
    pub const FAINT: u32 = 0x53646F;
    pub const ACCENT: u32 = 0x3CDCEC;
    pub const ACCENT_DEEP: u32 = 0x0E8FA3;
    pub const INK: u32 = 0x03070B;
    pub const LINE: u32 = 0x9FD8E6;
    pub const WARN: u32 = 0xF4B860;
    pub const BAD: u32 = 0xFF6B5E;
}

/// A text style in points.
#[derive(Clone, Copy)]
pub struct Style {
    pub face: Face,
    pub size: f32,
    pub tracking: f32,
}

pub const fn style(face: Face, size: f32, tracking: f32) -> Style {
    Style { face, size, tracking }
}

pub mod type_scale {
    use super::{style, Style};
    use mc_render::Face;
    pub const DISPLAY: Style = style(Face::Light, 52.0, 24.0);
    pub const TITLE: Style = style(Face::Light, 36.0, 14.0);
    pub const OVERLINE: Style = style(Face::Medium, 17.0, 13.0);
    pub const ITEM: Style = style(Face::Medium, 19.0, 7.5);
    pub const BUTTON: Style = style(Face::Bold, 16.0, 5.0);
    pub const BODY: Style = style(Face::Medium, 16.0, 1.6);
    pub const VALUE: Style = style(Face::Bold, 15.0, 2.5);
    pub const CAPTION: Style = style(Face::Medium, 12.5, 4.5);
    pub const MICRO: Style = style(Face::Medium, 11.0, 3.5);
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }

    pub fn mid_y(&self) -> f32 {
        self.y + self.h * 0.5
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn inset(&self, by: f32) -> Rect {
        Rect::new(self.x + by, self.y + by, (self.w - 2.0 * by).max(0.0), (self.h - 2.0 * by).max(0.0))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
    Backspace,
}

/// What the player did since the last frame. Positions are window pixels.
#[derive(Clone, Default, Debug)]
pub struct Input {
    pub cursor: Vec2,
    pub down: bool,
    pub pressed: bool,
    pub released: bool,
    pub keys: Vec<Key>,
    pub typed: String,
}

impl Input {
    pub fn key(&self, key: Key) -> bool {
        self.keys.contains(&key)
    }

    /// Forgets the per-frame part once a frame has consumed it.
    pub fn end_frame(&mut self) {
        self.pressed = false;
        self.released = false;
        self.keys.clear();
        self.typed.clear();
    }
}

pub type Id = u64;

/// FNV-1a of a label, optionally salted with an index for rows of like controls.
pub fn id(label: &str, index: usize) -> Id {
    let mut h = 0xCBF2_9CE4_8422_2325u64 ^ index as u64;
    for b in label.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x0100_0000_01B3);
    }
    h
}

#[derive(Default)]
pub struct Memory {
    anims: HashMap<Id, f32>,
    /// The control under the pointer this frame and last frame.
    hot: Option<Id>,
    was_hot: Option<Id>,
    /// The control the button went down on.
    active: Option<Id>,
    /// The text field that has the keyboard.
    pub editing: Option<Id>,
}

impl Memory {
    pub fn begin_frame(&mut self) {
        self.was_hot = self.hot.take();
    }

    pub fn end_frame(&mut self, input: &Input) {
        if !input.down {
            self.active = None;
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct Response {
    pub hovered: bool,
    pub held: bool,
    pub clicked: bool,
    /// Eased hover amount, 0..1.
    pub glow: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Primary,
    Secondary,
}

pub struct Ui<'a> {
    pub o: &'a mut Overlay,
    pub input: &'a Input,
    pub mem: &'a mut Memory,
    pub audio: &'a Audio,
    /// Pixels per point.
    pub s: f32,
    /// Canvas size in points.
    pub size: Vec2,
    pub cursor: Vec2,
    pub time: f32,
    pub dt: f32,
    /// Multiplies every alpha; screens fade with it.
    pub fade: f32,
    /// Added to every position, in points; screens slide with it.
    pub shift: Vec2,
    /// Off while a screen is animating out, so it cannot be clicked twice.
    pub interactive: bool,
}

impl<'a> Ui<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(o: &'a mut Overlay, input: &'a Input, mem: &'a mut Memory, audio: &'a Audio, viewport: Vec2, user_scale: f32, time: f32, dt: f32) -> Ui<'a> {
        let s = (viewport.y / CANVAS_H * user_scale).max(0.4);
        Ui { o, input, mem, audio, s, size: viewport / s, cursor: input.cursor / s, time, dt, fade: 1.0, shift: Vec2::ZERO, interactive: true }
    }

    // -- drawing ----------------------------------------------------------------

    fn c(&self, mut color: Color) -> Color {
        color[3] *= self.fade;
        color
    }

    /// Points to whole pixels, so edges are sharp.
    fn px(&self, v: f32) -> f32 {
        (v * self.s).round()
    }

    fn at(&self, x: f32, y: f32) -> (f32, f32) {
        (self.px(x + self.shift.x), self.px(y + self.shift.y))
    }

    pub fn fill(&mut self, r: Rect, color: Color) {
        let (x0, y0) = self.at(r.x, r.y);
        let (x1, y1) = self.at(r.x + r.w, r.y + r.h);
        let color = self.c(color);
        self.o.rect(x0, y0, x1 - x0, y1 - y0, color);
    }

    /// Corner colours: top-left, top-right, bottom-right, bottom-left.
    pub fn gradient(&mut self, r: Rect, colors: [Color; 4]) {
        let (x0, y0) = self.at(r.x, r.y);
        let (x1, y1) = self.at(r.x + r.w, r.y + r.h);
        let colors = colors.map(|c| self.c(c));
        self.o.gradient(x0, y0, x1 - x0, y1 - y0, colors);
    }

    /// A left-to-right fade between two colours.
    pub fn gradient_h(&mut self, r: Rect, left: Color, right: Color) {
        self.gradient(r, [left, right, right, left]);
    }

    pub fn gradient_v(&mut self, r: Rect, top: Color, bottom: Color) {
        self.gradient(r, [top, top, bottom, bottom]);
    }

    /// A dark scrim whose perceived opacity runs from `from` to `to`, left to
    /// right (`across`) or top to bottom. Stepped, because a single quad would
    /// interpolate the bent alpha and bunch the fade up at one end.
    pub fn scrim(&mut self, r: Rect, from: f32, to: f32, across: bool) {
        const STEPS: usize = 12;
        for i in 0..STEPS {
            let (t0, t1) = (i as f32 / STEPS as f32, (i + 1) as f32 / STEPS as f32);
            let (a0, a1) = (ink(from + (to - from) * t0), ink(from + (to - from) * t1));
            if across {
                self.gradient_h(Rect::new(r.x + r.w * t0, r.y, r.w * (t1 - t0), r.h), a0, a1);
            } else {
                self.gradient_v(Rect::new(r.x, r.y + r.h * t0, r.w, r.h * (t1 - t0)), a0, a1);
            }
        }
    }

    /// A hairline: one pixel (two on dense displays), whatever the scale.
    fn hair(&self) -> f32 {
        (self.s * 0.75).round().max(1.0)
    }

    pub fn hline(&mut self, x: f32, y: f32, w: f32, color: Color) {
        let (x0, y0) = self.at(x, y);
        let x1 = self.px(x + w + self.shift.x);
        let (t, color) = (self.hair(), self.c(color));
        self.o.rect(x0, y0, x1 - x0, t, color);
    }

    pub fn vline(&mut self, x: f32, y: f32, h: f32, color: Color) {
        let (x0, y0) = self.at(x, y);
        let y1 = self.px(y + h + self.shift.y);
        let (t, color) = (self.hair(), self.c(color));
        self.o.rect(x0, y0, t, y1 - y0, color);
    }

    pub fn frame(&mut self, r: Rect, color: Color) {
        let (x0, y0) = self.at(r.x, r.y);
        let (x1, y1) = self.at(r.x + r.w, r.y + r.h);
        let (t, color) = (self.hair(), self.c(color));
        self.o.frame(x0, y0, x1 - x0, y1 - y0, t, color);
    }

    pub fn stroke(&mut self, a: Vec2, b: Vec2, thickness: f32, color: Color) {
        let (a, b) = ((a + self.shift) * self.s, (b + self.shift) * self.s);
        let color = self.c(color);
        self.o.stroke(a.into(), b.into(), (thickness * self.s).max(1.0), color);
    }

    pub fn arc(&mut self, centre: Vec2, radius: f32, from: f32, to: f32, thickness: f32, color: Color) {
        let centre = (centre + self.shift) * self.s;
        let color = self.c(color);
        self.o.arc(centre.into(), radius * self.s, from, to, (thickness * self.s).max(1.0), color);
    }

    pub fn disc(&mut self, centre: Vec2, radius: f32, color: Color) {
        let centre = (centre + self.shift) * self.s;
        let color = self.c(color);
        self.o.disc(centre.into(), radius * self.s, color);
    }

    pub fn triangle(&mut self, a: Vec2, b: Vec2, c: Vec2, color: Color) {
        let p = |v: Vec2| -> [f32; 2] { ((v + self.shift) * self.s).into() };
        let color = self.c(color);
        self.o.quad_fill([p(a), p(b), p(c), p(c)], color);
    }

    pub fn image(&mut self, slot: usize, src: [f32; 4], r: Rect, tint: Color) {
        let (x0, y0) = self.at(r.x, r.y);
        let (x1, y1) = self.at(r.x + r.w, r.y + r.h);
        let tint = self.c(tint);
        self.o.image(slot, src, x0, y0, x1 - x0, y1 - y0, tint);
    }

    fn face(&self, st: Style) -> Type {
        Type::new(st.face, st.size * self.s, st.tracking * self.s)
    }

    pub fn text_width(&mut self, st: Style, text: &str) -> f32 {
        let t = self.face(st);
        self.o.type_width(t, text) / self.s
    }

    /// Left-aligned text, its capitals centred on `y`. Returns the x after it.
    pub fn text(&mut self, x: f32, y: f32, st: Style, color: Color, text: &str) -> f32 {
        let t = self.face(st);
        let cap = self.o.cap_height(t);
        let (px, py) = ((x + self.shift.x) * self.s, (y + self.shift.y) * self.s);
        let color = self.c(color);
        let end = self.o.type_text(px.round(), (py + cap * 0.5).round(), t, color, text);
        end / self.s - self.shift.x
    }

    pub fn text_right(&mut self, right: f32, y: f32, st: Style, color: Color, text: &str) {
        let w = self.text_width(st, text);
        self.text(right - w, y, st, color, text);
    }

    pub fn text_centred(&mut self, centre: f32, y: f32, st: Style, color: Color, text: &str) {
        let w = self.text_width(st, text);
        self.text(centre - w * 0.5, y, st, color, text);
    }

    // -- decoration ---------------------------------------------------------------

    /// The dark glass every panel sits on.
    pub fn panel(&mut self, r: Rect) {
        self.fill(r, ink(0.74));
        self.gradient_v(Rect::new(r.x, r.y, r.w, r.h.min(90.0)), rgb(palette::ACCENT_DEEP, 0.07), rgb(palette::ACCENT_DEEP, 0.0));
        self.frame(r, rgb(palette::LINE, 0.16));
        self.brackets(r, 9.0, rgb(palette::ACCENT, 0.55));
    }

    /// Four corner ticks just outside a rectangle.
    pub fn brackets(&mut self, r: Rect, arm: f32, color: Color) {
        for (cx, cy, dx, dy) in [(r.x, r.y, 1.0, 1.0), (r.right(), r.y, -1.0, 1.0), (r.right(), r.bottom(), -1.0, -1.0), (r.x, r.bottom(), 1.0, -1.0)] {
            let (x, y) = (if dx > 0.0 { cx } else { cx - arm }, if dy > 0.0 { cy } else { cy - arm });
            self.hline(x, if dy > 0.0 { cy } else { cy - 1.0 / self.s }, arm, color);
            self.vline(if dx > 0.0 { cx } else { cx - 1.0 / self.s }, y, arm, color);
        }
    }

    /// A small section heading with a rule running off to the right.
    pub fn section(&mut self, x: f32, y: f32, w: f32, title: &str) {
        self.fill(Rect::new(x, y - 5.0, 3.0, 10.0), rgb(palette::ACCENT, 0.9));
        let end = self.text(x + 12.0, y, type_scale::CAPTION, rgb(palette::DIM, 1.0), title);
        self.gradient_h(Rect::new(end + 10.0, y, (x + w - end - 10.0).max(0.0), 1.0 / self.s), rgb(palette::LINE, 0.28), rgb(palette::LINE, 0.0));
    }

    /// The reticle that stands in for a logo: ring, cross hairs, centre dot.
    pub fn reticle(&mut self, centre: Vec2, radius: f32, color: Color) {
        self.arc(centre, radius, 0.0, std::f32::consts::TAU, 1.4, color);
        for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            let d = Vec2::new(dx, dy);
            self.stroke(centre + d * radius * 0.45, centre + d * radius * 1.35, 1.2, color);
        }
        self.disc(centre, 1.8, color);
    }

    // -- interaction ----------------------------------------------------------------

    /// Eases the value remembered under `id` toward `target`; `rate` is per second.
    pub fn ease(&mut self, id: Id, target: f32, rate: f32) -> f32 {
        let v = self.mem.anims.entry(id).or_insert(0.0);
        *v += (target - *v) * (1.0 - (-rate * self.dt).exp());
        if (*v - target).abs() < 0.002 {
            *v = target;
        }
        *v
    }

    /// Pointer handling for one control. Plays the hover sound, and the refusal
    /// sound when a disabled control is clicked; the caller plays its own on `clicked`.
    pub fn interact(&mut self, id: Id, r: Rect, enabled: bool) -> Response {
        let over = self.interactive && r.contains(self.cursor - self.shift) && self.mem.hot.is_none();
        let mut out = Response::default();
        if over {
            self.mem.hot = Some(id);
            if self.mem.was_hot != Some(id) && enabled {
                self.audio.play(Sfx::Hover);
            }
            if self.input.pressed {
                if enabled {
                    self.mem.active = Some(id);
                } else {
                    self.audio.play(Sfx::Deny);
                }
            }
        }
        out.hovered = over && enabled;
        out.held = self.mem.active == Some(id) && self.input.down;
        out.clicked = over && enabled && self.input.released && self.mem.active == Some(id);
        out.glow = self.ease(id, if out.hovered || out.held { 1.0 } else { 0.0 }, 14.0);
        out
    }

    // -- controls -----------------------------------------------------------------

    pub fn button(&mut self, id: Id, r: Rect, label: &str, kind: ButtonKind, enabled: bool) -> bool {
        let res = self.interact(id, r, enabled);
        let press = self.ease(id ^ 1, if res.held { 1.0 } else { 0.0 }, 30.0);
        let live = if enabled { 1.0 } else { 0.35 };
        let r = Rect::new(r.x, r.y + press * 1.5, r.w, r.h);
        match kind {
            ButtonKind::Primary => {
                self.fill(r, rgb(palette::ACCENT_DEEP, (0.30 + 0.35 * res.glow) * live));
                self.gradient_h(r, rgb(palette::ACCENT, (0.35 + 0.30 * res.glow) * live), rgb(palette::ACCENT, 0.04 * live));
                self.frame(r, rgb(palette::ACCENT, (0.65 + 0.35 * res.glow) * live));
                // A sheen that sweeps across while hovered.
                if res.glow > 0.01 {
                    let k = (self.time * 0.9).fract();
                    let x = r.x + (r.w + 120.0) * k - 120.0;
                    let (x0, x1) = (x.max(r.x), (x + 120.0).min(r.right()));
                    if x1 > x0 {
                        let a = 0.16 * res.glow;
                        self.gradient_h(Rect::new(x0, r.y, (x1 - x0) * 0.5, r.h), rgb(0xFFFFFF, 0.0), rgb(0xFFFFFF, a));
                        self.gradient_h(Rect::new(x0 + (x1 - x0) * 0.5, r.y, (x1 - x0) * 0.5, r.h), rgb(0xFFFFFF, a), rgb(0xFFFFFF, 0.0));
                    }
                }
            }
            ButtonKind::Secondary => {
                self.fill(r, ink(0.55));
                self.gradient_h(r, rgb(palette::ACCENT, 0.18 * res.glow), rgb(palette::ACCENT, 0.0));
                self.frame(r, rgb(palette::LINE, (0.22 + 0.5 * res.glow) * live));
            }
        }
        self.fill(Rect::new(r.x, r.y, 3.0, r.h), rgb(palette::ACCENT, (0.35 + 0.65 * res.glow) * live));
        let tone = if kind == ButtonKind::Primary { rgb(0xFFFFFF, live) } else { rgb(palette::TEXT, (0.78 + 0.22 * res.glow) * live) };
        let w = self.text_width(type_scale::BUTTON, label);
        // Chevrons march in from the right of a primary button's label.
        let chevrons = if kind == ButtonKind::Primary { 34.0 } else { 0.0 };
        let x = r.x + (r.w - w - chevrons) * 0.5 + res.glow * 2.0;
        self.text(x, r.mid_y(), type_scale::BUTTON, tone, label);
        if kind == ButtonKind::Primary {
            for i in 0..3 {
                let phase = ((self.time * 2.2 - i as f32 * 0.22).fract() + 1.0).fract();
                let a = (0.25 + 0.75 * (1.0 - phase) * res.glow.max(0.25)) * live;
                let cx = x + w + 14.0 + i as f32 * 9.0;
                let (top, mid, bottom) = (Vec2::new(cx, r.mid_y() - 5.0), Vec2::new(cx + 5.0, r.mid_y()), Vec2::new(cx, r.mid_y() + 5.0));
                self.stroke(top, mid, 1.6, rgb(0xFFFFFF, a));
                self.stroke(mid, bottom, 1.6, rgb(0xFFFFFF, a));
            }
        }
        res.clicked
    }

    /// A labelled row with an on/off switch at its right end. Returns true when flipped.
    pub fn toggle(&mut self, id: Id, r: Rect, label: &str, hint: &str, value: &mut bool) -> bool {
        let res = self.interact(id, r, true);
        if res.clicked {
            *value = !*value;
            self.audio.play(if *value { Sfx::ToggleOn } else { Sfx::ToggleOff });
        }
        self.row(r, label, hint, res.glow);
        let on = self.ease(id ^ 2, if *value { 1.0 } else { 0.0 }, 16.0);
        let track = Rect::new(r.right() - 58.0, r.mid_y() - 10.0, 42.0, 20.0);
        self.fill(track, ink(0.8));
        self.fill(track, rgb(palette::ACCENT_DEEP, 0.55 * on));
        self.frame(track, rgb(if *value { palette::ACCENT } else { palette::LINE }, 0.35 + 0.45 * on));
        let knob = Rect::new(track.x + 3.0 + 22.0 * on, track.y + 3.0, 14.0, 14.0);
        self.fill(knob, rgb(if *value { 0xFFFFFF } else { palette::DIM }, 1.0));
        self.text_right(track.x - 12.0, r.mid_y(), type_scale::VALUE, rgb(if *value { palette::ACCENT } else { palette::FAINT }, 1.0), if *value { "ON" } else { "OFF" });
        res.clicked
    }

    /// A labelled row with a 0..1 slider. Returns true while the value changes.
    pub fn slider(&mut self, id: Id, r: Rect, label: &str, value: &mut f32) -> bool {
        let res = self.interact(id, r, true);
        self.row(r, label, "", res.glow);
        let track = Rect::new(r.right() - 300.0, r.mid_y() - 2.0, 220.0, 4.0);
        let mut changed = false;
        if res.held {
            let v = ((self.cursor.x - self.shift.x - track.x) / track.w).clamp(0.0, 1.0);
            // Snap to 5% steps; each step ticks, so dragging sounds like a detent wheel.
            let v = (v * 20.0).round() / 20.0;
            if (v - *value).abs() > 1e-4 {
                *value = v;
                changed = true;
                self.audio.play(Sfx::Tick);
            }
        }
        self.fill(track, rgb(palette::LINE, 0.16));
        self.fill(Rect::new(track.x, track.y, track.w * *value, track.h), rgb(palette::ACCENT, 0.9));
        for i in 0..=10 {
            let x = track.x + track.w * i as f32 / 10.0;
            self.vline(x, track.bottom() + 5.0, if i % 5 == 0 { 6.0 } else { 3.0 }, rgb(palette::LINE, 0.3));
        }
        let grow = 1.5 * res.glow + if res.held { 1.5 } else { 0.0 };
        let knob = Rect::new(track.x + track.w * *value - 4.0 - grow * 0.5, r.mid_y() - 9.0 - grow * 0.5, 8.0 + grow, 18.0 + grow);
        self.fill(knob, rgb(0xFFFFFF, 1.0));
        self.text_right(r.right() - 16.0, r.mid_y(), type_scale::VALUE, rgb(palette::TEXT, 1.0), &format!("{:.0}", *value * 100.0));
        changed
    }

    /// `‹ VALUE ›`: returns -1 or 1 when stepped. Clicking the value itself steps forward.
    pub fn stepper(&mut self, id: Id, r: Rect, value: &str, color: Color, enabled: bool) -> i32 {
        let arrow_w = 26.0;
        let mut step = 0;
        let body = self.interact(id, Rect::new(r.x + arrow_w, r.y, r.w - 2.0 * arrow_w, r.h), enabled);
        let live = if enabled { 1.0 } else { 0.35 };
        self.fill(r, ink(0.5));
        self.frame(r, rgb(palette::LINE, (0.14 + 0.3 * body.glow) * live));
        self.text_centred(r.x + r.w * 0.5, r.mid_y(), type_scale::VALUE, [color[0], color[1], color[2], color[3] * live], value);
        if body.clicked {
            step = 1;
        }
        for (k, dir) in [(3u64, -1.0f32), (4, 1.0)] {
            let zone = Rect::new(if dir < 0.0 { r.x } else { r.right() - arrow_w }, r.y, arrow_w, r.h);
            let res = self.interact(id ^ k, zone, enabled);
            if res.clicked {
                step = dir as i32;
            }
            let c = Vec2::new(zone.x + zone.w * 0.5 + dir * res.glow * 1.5, zone.mid_y());
            let tone = rgb(if res.glow > 0.5 { palette::ACCENT } else { palette::DIM }, (0.6 + 0.4 * res.glow) * live);
            self.stroke(c + Vec2::new(-dir * 3.0, -5.0), c + Vec2::new(dir * 3.0, 0.0), 1.6, tone);
            self.stroke(c + Vec2::new(dir * 3.0, 0.0), c + Vec2::new(-dir * 3.0, 5.0), 1.6, tone);
        }
        if step != 0 {
            self.audio.play(Sfx::Tick);
        }
        step
    }

    /// A single-line text field. Returns true when the text changed.
    pub fn text_field(&mut self, id: Id, r: Rect, text: &mut String, max: usize) -> bool {
        let res = self.interact(id, r, true);
        if res.clicked {
            self.mem.editing = Some(id);
            self.audio.play(Sfx::Select);
        } else if self.input.pressed && !res.hovered && self.mem.editing == Some(id) {
            self.mem.editing = None;
        }
        let editing = self.mem.editing == Some(id);
        let mut changed = false;
        if editing {
            for ch in self.input.typed.chars() {
                if (ch.is_ascii_graphic() || ch == ' ') && text.chars().count() < max {
                    text.push(ch);
                    changed = true;
                }
            }
            if self.input.key(Key::Backspace) && text.pop().is_some() {
                changed = true;
            }
            if self.input.key(Key::Enter) || self.input.key(Key::Escape) {
                self.mem.editing = None;
            }
            if changed {
                self.audio.play(Sfx::Tick);
            }
        }
        let focus = self.ease(id ^ 5, if editing { 1.0 } else { 0.0 }, 16.0);
        self.fill(r, ink(0.55 + 0.2 * focus));
        self.frame(r, rgb(if editing { palette::ACCENT } else { palette::LINE }, 0.2 + 0.3 * res.glow + 0.5 * focus));
        let end = self.text(r.x + 12.0, r.mid_y(), type_scale::VALUE, rgb(palette::TEXT, 1.0), text);
        if editing && (self.time * 1.6).fract() < 0.55 {
            self.fill(Rect::new(end + 2.0, r.mid_y() - 8.0, 2.0, 16.0), rgb(palette::ACCENT, 1.0));
        }
        changed
    }

    /// Background and label shared by the option rows.
    fn row(&mut self, r: Rect, label: &str, hint: &str, glow: f32) {
        self.gradient_h(r, rgb(palette::ACCENT, 0.10 * glow), rgb(palette::ACCENT, 0.0));
        self.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(palette::ACCENT, glow));
        self.hline(r.x, r.bottom(), r.w, rgb(palette::LINE, 0.10));
        let end = self.text(r.x + 16.0 + 4.0 * glow, r.mid_y(), type_scale::BODY, rgb(palette::TEXT, 0.82 + 0.18 * glow), label);
        if !hint.is_empty() {
            self.text(end + 14.0, r.mid_y() + 0.5, type_scale::MICRO, rgb(palette::FAINT, 1.0), hint);
        }
    }
}
