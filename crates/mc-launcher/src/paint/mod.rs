//! Drawing the launcher on the CPU, in the game's look (mc-game `ui`): black
//! glass panels with cut corners and machined edges, Barlow type, one
//! red-orange accent. Shapes and pictures go through tiny-skia, type through
//! fontdue (`text.rs`). Everything is placed in points; `s` turns them into
//! pixels, so a high-DPI screen gets a sharper frame of the same layout.
//!
//! No GPU: the launcher is what repairs an install whose game will not start,
//! so it must draw on a machine whose driver is the trouble.

mod backdrop;
mod text;

pub(crate) use backdrop::Backdrop;
pub(crate) use text::{type_scale, Fonts, Style};

use tiny_skia::{
    FillRule, GradientStop, LinearGradient, Mask, Paint, PathBuilder, Pixmap, Point, SpreadMode,
    Stroke, Transform,
};

/// The game's colours (mc-game `ui::palette`), sRGB.
pub(crate) mod palette {
    pub(crate) const TEXT: u32 = 0xF2F2F0;
    pub(crate) const DIM: u32 = 0xA3A3A0;
    pub(crate) const FAINT: u32 = 0x66666A;
    pub(crate) const ACCENT: u32 = 0xFF5A24;
    pub(crate) const ACCENT_DEEP: u32 = 0xA8300F;
    pub(crate) const INK: u32 = 0x000000;
    pub(crate) const LINE: u32 = 0xFFFFFF;
    pub(crate) const GOOD: u32 = 0x5BD08A;
    pub(crate) const WARN: u32 = 0xFFB43C;
}

/// Straight-alpha sRGB, 0..1.
pub(crate) type Color = [f32; 4];

pub(crate) fn rgb(hex: u32, a: f32) -> Color {
    [
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
        a.clamp(0.0, 1.0),
    ]
}

/// Black at `a`.
pub(crate) fn ink(a: f32) -> Color {
    rgb(palette::INK, a)
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub(crate) struct Rect {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) h: f32,
}

impl Rect {
    pub(crate) const fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    pub(crate) fn right(&self) -> f32 {
        self.x + self.w
    }

    pub(crate) fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub(crate) fn mid_x(&self) -> f32 {
        self.x + self.w * 0.5
    }

    pub(crate) fn mid_y(&self) -> f32 {
        self.y + self.h * 0.5
    }

    pub(crate) fn inset(&self, d: f32) -> Rect {
        Rect::new(self.x + d, self.y + d, self.w - 2.0 * d, self.h - 2.0 * d)
    }

    pub(crate) fn contains(&self, p: (f32, f32)) -> bool {
        p.0 >= self.x && p.0 < self.right() && p.1 >= self.y && p.1 < self.bottom()
    }

    /// The part of `self` inside `other`.
    pub(crate) fn clip(&self, other: Rect) -> Rect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        Rect::new(
            x,
            y,
            (self.right().min(other.right()) - x).max(0.0),
            (self.bottom().min(other.bottom()) - y).max(0.0),
        )
    }
}

/// A frame being drawn: the pixels, the type, and what lies behind the glass.
pub(crate) struct Painter {
    pub(crate) px: Pixmap,
    /// Pixels per point.
    pub(crate) s: f32,
    pub(crate) fonts: Fonts,
    pub(crate) backdrop: Backdrop,
    /// Drawing outside this is cut away (a scrolled list); `None` draws anywhere.
    clip: Option<(Rect, Mask)>,
}

impl Painter {
    pub(crate) fn new(width: u32, height: u32, s: f32, backdrop: Backdrop) -> Painter {
        Painter {
            px: Pixmap::new(width.max(1), height.max(1)).expect("a pixmap of a window's size"),
            s,
            fonts: Fonts::new(),
            backdrop,
            clip: None,
        }
    }

    /// Starts a frame `width` x `height` pixels at `s` pixels per point, with
    /// the backdrop already down.
    pub(crate) fn begin(&mut self, width: u32, height: u32, s: f32) {
        let (width, height) = (width.max(1), height.max(1));
        if self.px.width() != width || self.px.height() != height {
            self.px = Pixmap::new(width, height).expect("a pixmap of a window's size");
        }
        self.s = s;
        self.clip = None;
        self.backdrop.draw(&mut self.px);
    }

    /// The frame's size in points.
    pub(crate) fn size(&self) -> (f32, f32) {
        (
            self.px.width() as f32 / self.s,
            self.px.height() as f32 / self.s,
        )
    }

    /// Cuts drawing to `r` until [`Painter::unclip`].
    pub(crate) fn clip_to(&mut self, r: Rect) {
        let mut mask = Mask::new(self.px.width(), self.px.height()).expect("a mask");
        if let Some(rect) = self.pixels(r).to_skia() {
            let path = PathBuilder::from_rect(rect);
            mask.fill_path(&path, FillRule::Winding, false, Transform::identity());
        }
        self.clip = Some((r, mask));
    }

    pub(crate) fn unclip(&mut self) {
        self.clip = None;
    }

    pub(crate) fn clip_rect(&self) -> Option<Rect> {
        self.clip.as_ref().map(|c| c.0)
    }

    /// `r` in pixels, snapped so edges fall on whole pixels.
    fn pixels(&self, r: Rect) -> PxRect {
        let x0 = (r.x * self.s).round();
        let y0 = (r.y * self.s).round();
        let x1 = (r.right() * self.s).round();
        let y1 = (r.bottom() * self.s).round();
        PxRect { x0, y0, x1, y1 }
    }

    fn paint(&self, c: Color) -> Paint<'static> {
        let mut p = Paint::default();
        p.set_color_rgba8(
            (c[0] * 255.0) as u8,
            (c[1] * 255.0) as u8,
            (c[2] * 255.0) as u8,
            (c[3] * 255.0).round() as u8,
        );
        p.anti_alias = true;
        p
    }

    pub(crate) fn fill(&mut self, r: Rect, c: Color) {
        if c[3] <= 0.0 || r.w <= 0.0 || r.h <= 0.0 {
            return;
        }
        let pr = self.pixels(r);
        if let Some(rect) = pr.to_skia() {
            let paint = self.paint(c);
            let mask = self.clip.take();
            self.px.fill_rect(
                rect,
                &paint,
                Transform::identity(),
                mask.as_ref().map(|m| &m.1),
            );
            self.clip = mask;
        }
    }

    fn cut_path(&self, r: Rect, cut: f32) -> Option<tiny_skia::Path> {
        let pr = self.pixels(r);
        let c = (cut * self.s)
            .round()
            .min((pr.x1 - pr.x0) * 0.5)
            .min((pr.y1 - pr.y0) * 0.5)
            .max(0.0);
        let mut pb = PathBuilder::new();
        pb.move_to(pr.x0 + c, pr.y0);
        pb.line_to(pr.x1 - c, pr.y0);
        pb.line_to(pr.x1, pr.y0 + c);
        pb.line_to(pr.x1, pr.y1 - c);
        pb.line_to(pr.x1 - c, pr.y1);
        pb.line_to(pr.x0 + c, pr.y1);
        pb.line_to(pr.x0, pr.y1 - c);
        pb.line_to(pr.x0, pr.y0 + c);
        pb.close();
        pb.finish()
    }

    fn fill_path(&mut self, path: &tiny_skia::Path, paint: &Paint) {
        let mask = self.clip.take();
        self.px.fill_path(
            path,
            paint,
            FillRule::Winding,
            Transform::identity(),
            mask.as_ref().map(|m| &m.1),
        );
        self.clip = mask;
    }

    /// Left `c0` to right `c1`.
    pub(crate) fn gradient_h(&mut self, r: Rect, c0: Color, c1: Color) {
        self.gradient(r, c0, c1, true);
    }

    fn gradient(&mut self, r: Rect, c0: Color, c1: Color, across: bool) {
        if (c0[3] <= 0.0 && c1[3] <= 0.0) || r.w <= 0.0 || r.h <= 0.0 {
            return;
        }
        let pr = self.pixels(r);
        let Some(rect) = pr.to_skia() else {
            return;
        };
        let (start, end) = if across {
            (Point::from_xy(pr.x0, 0.0), Point::from_xy(pr.x1, 0.0))
        } else {
            (Point::from_xy(0.0, pr.y0), Point::from_xy(0.0, pr.y1))
        };
        let color = |c: Color| {
            tiny_skia::Color::from_rgba(c[0], c[1], c[2], c[3]).unwrap_or(tiny_skia::Color::BLACK)
        };
        let Some(shader) = LinearGradient::new(
            start,
            end,
            vec![
                GradientStop::new(0.0, color(c0)),
                GradientStop::new(1.0, color(c1)),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        ) else {
            return;
        };
        let paint = Paint {
            shader,
            anti_alias: false,
            ..Paint::default()
        };
        let mask = self.clip.take();
        self.px.fill_rect(
            rect,
            &paint,
            Transform::identity(),
            mask.as_ref().map(|m| &m.1),
        );
        self.clip = mask;
    }

    /// A line `width` points wide.
    pub(crate) fn stroke(&mut self, a: (f32, f32), b: (f32, f32), width: f32, c: Color) {
        let mut pb = PathBuilder::new();
        pb.move_to(a.0 * self.s, a.1 * self.s);
        pb.line_to(b.0 * self.s, b.1 * self.s);
        if let Some(path) = pb.finish() {
            self.stroke_path(&path, width, c);
        }
    }

    /// A polyline through `points`.
    pub(crate) fn polyline(&mut self, points: &[(f32, f32)], width: f32, c: Color) {
        let mut pb = PathBuilder::new();
        for (i, p) in points.iter().enumerate() {
            if i == 0 {
                pb.move_to(p.0 * self.s, p.1 * self.s);
            } else {
                pb.line_to(p.0 * self.s, p.1 * self.s);
            }
        }
        if let Some(path) = pb.finish() {
            self.stroke_path(&path, width, c);
        }
    }

    fn stroke_path(&mut self, path: &tiny_skia::Path, width: f32, c: Color) {
        let paint = self.paint(c);
        let stroke = Stroke {
            width: (width * self.s).max(1.0),
            ..Stroke::default()
        };
        let mask = self.clip.take();
        self.px.stroke_path(
            path,
            &paint,
            &stroke,
            Transform::identity(),
            mask.as_ref().map(|m| &m.1),
        );
        self.clip = mask;
    }

    /// A circle's outline.
    pub(crate) fn ring(&mut self, centre: (f32, f32), r: f32, width: f32, c: Color) {
        if let Some(path) =
            PathBuilder::from_circle(centre.0 * self.s, centre.1 * self.s, r * self.s)
        {
            self.stroke_path(&path, width, c);
        }
    }

    pub(crate) fn disc(&mut self, centre: (f32, f32), r: f32, c: Color) {
        if let Some(path) =
            PathBuilder::from_circle(centre.0 * self.s, centre.1 * self.s, r * self.s)
        {
            let paint = self.paint(c);
            self.fill_path(&path, &paint);
        }
    }

    /// A one-pixel line across from `x`, `w` points long.
    pub(crate) fn hline(&mut self, x: f32, y: f32, w: f32, c: Color) {
        self.fill(Rect::new(x, y, w, 1.0 / self.s), c);
    }

    pub(crate) fn vline(&mut self, x: f32, y: f32, h: f32, c: Color) {
        self.fill(Rect::new(x, y, 1.0 / self.s, h), c);
    }

    /// An octagon's edges: straight sides in `side`, cut corners in `corner`.
    pub(crate) fn outline_cut(&mut self, r: Rect, cut: f32, side: Color, corner: Color) {
        let t = 1.0 / self.s;
        self.hline(r.x + cut, r.y, r.w - 2.0 * cut, side);
        self.hline(r.x + cut, r.bottom() - t, r.w - 2.0 * cut, side);
        self.vline(r.x, r.y + cut, r.h - 2.0 * cut, side);
        self.vline(r.right() - t, r.y + cut, r.h - 2.0 * cut, side);
        if cut <= 0.0 {
            return;
        }
        let d = 0.7 * t;
        let (x0, y0, x1, y1) = (r.x + d, r.y + d, r.right() - d, r.bottom() - d);
        for (a, b) in [
            ((x0, y0 + cut), (x0 + cut, y0)),
            ((x1 - cut, y0), (x1, y0 + cut)),
            ((x1, y1 - cut), (x1 - cut, y1)),
            ((x0 + cut, y1), (x0, y1 - cut)),
        ] {
            self.stroke(a, b, 1.0, corner);
        }
    }

    /// A plain rectangle's outline, one pixel wide.
    pub(crate) fn frame(&mut self, r: Rect, c: Color) {
        self.outline_cut(r, 0.0, c, c);
    }

    /// Machined edges (mc-game `Ui::bevel`): cut corners, a faint inner line,
    /// ticks at the middles of the long edges, brighter corner pieces and one
    /// warm mark at the top left. `k` scales it all.
    pub(crate) fn bevel(&mut self, r: Rect, cut: f32, k: f32) {
        let line = rgb(palette::LINE, 0.22 * k);
        let bright = rgb(palette::LINE, 0.75 * k);
        self.outline_cut(r, cut, line, bright);
        let t = 1.0 / self.s;
        let arm = (cut * 1.6).min((r.w - 2.0 * cut) * 0.25);
        self.hline(r.x + cut, r.y, arm, bright);
        self.hline(r.right() - cut - arm, r.y, arm, bright);
        self.hline(r.x + cut, r.bottom() - t, arm, bright);
        self.hline(r.right() - cut - arm, r.bottom() - t, arm, bright);
        if r.w > 40.0 && r.h > 30.0 {
            let inset = 3.0;
            let faint = rgb(palette::LINE, 0.06 * k);
            self.outline_cut(r.inset(inset), (cut - inset * 0.414).max(0.0), faint, faint);
            let mid = r.mid_x();
            let tick = rgb(palette::LINE, 0.4 * k);
            self.hline(mid - 12.0, r.y + 2.0, 24.0, tick);
            self.hline(mid - 12.0, r.bottom() - 2.0 - t, 24.0, tick);
        }
        if r.h > 60.0 {
            let my = r.mid_y();
            self.vline(r.x + 2.0, my - 8.0, 16.0, rgb(palette::LINE, 0.3 * k));
            self.vline(
                r.right() - 2.0 - t,
                my - 8.0,
                16.0,
                rgb(palette::LINE, 0.3 * k),
            );
        }
        if cut >= 8.0 {
            self.stroke(
                (r.x + 3.0, r.y + cut + 2.0),
                (r.x + cut + 2.0, r.y + 3.0),
                2.0,
                rgb(palette::ACCENT, 0.9 * k),
            );
        }
    }

    /// Black frosted glass: the backdrop behind it blurred, darkened by `opacity`.
    pub(crate) fn frost_cut(&mut self, r: Rect, cut: f32, opacity: f32) {
        let Some(path) = self.cut_path(r, cut) else {
            return;
        };
        let pattern = self.backdrop.blurred_shader();
        if let Some(shader) = pattern {
            let paint = Paint {
                shader,
                anti_alias: true,
                ..Paint::default()
            };
            let mask = self.clip.take();
            // The pixmap borrowed for the shader is the backdrop's, not ours.
            let px = &mut self.px;
            px.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                mask.as_ref().map(|m| &m.1),
            );
            self.clip = mask;
        }
        let paint = self.paint(ink(opacity));
        self.fill_path(&path, &paint);
    }

    /// The glass every panel sits on (mc-game `Ui::panel`).
    pub(crate) fn panel(&mut self, r: Rect) {
        self.frost_cut(r, 10.0, 0.82);
        self.bevel(r, 10.0, 1.0);
    }

    /// A section heading: a white tick, the title, a line fading out to `w`.
    pub(crate) fn section(&mut self, x: f32, y: f32, w: f32, title: &str) {
        self.fill(Rect::new(x, y - 5.0, 2.0, 10.0), rgb(palette::LINE, 0.9));
        let end = self.text(
            x + 12.0,
            y,
            type_scale::CAPTION,
            rgb(palette::DIM, 1.0),
            title,
        );
        self.gradient_h(
            Rect::new(end + 10.0, y, (x + w - end - 10.0).max(0.0), 1.0 / self.s),
            rgb(palette::LINE, 0.28),
            rgb(palette::LINE, 0.0),
        );
    }

    /// The game's mark (mc-game `Ui::emblem`): a globe whose meridian stands
    /// proud of the poles, a longitude turned by `turn` (-1..1) and a
    /// red-orange point where it crosses the equator.
    pub(crate) fn emblem(&mut self, centre: (f32, f32), r: f32, turn: f32, c: Color) {
        use std::f32::consts::PI;
        let faint = |k: f32| [c[0], c[1], c[2], c[3] * k];
        let (cx, cy) = centre;
        self.ring(centre, r, 1.4, c);
        self.stroke((cx - r, cy), (cx + r, cy), 1.0, faint(0.35));
        self.stroke((cx, cy - r * 1.4), (cx, cy + r * 1.4), 1.4, c);
        let half = r * turn;
        let points: Vec<(f32, f32)> = (0..=20)
            .map(|i| {
                let a = PI * i as f32 / 20.0;
                (cx + half * a.sin(), cy - r * a.cos())
            })
            .collect();
        self.polyline(&points, 1.0, faint(0.5));
        self.disc(
            (cx + half, cy),
            (r * 0.13).max(2.0),
            rgb(palette::ACCENT, c[3]),
        );
    }

    /// Text at `x`, its line's middle at `y`; returns where it ends.
    pub(crate) fn text(&mut self, x: f32, y: f32, st: Style, c: Color, text: &str) -> f32 {
        let clip = self.clip.as_ref().map(|c| self.pixels(c.0));
        self.fonts.draw(
            &mut self.px,
            self.s,
            x,
            y,
            st,
            c,
            text,
            clip.map(PxRect::bounds),
        )
    }

    pub(crate) fn text_right(&mut self, right: f32, y: f32, st: Style, c: Color, text: &str) {
        let w = self.text_width(st, text);
        self.text(right - w, y, st, c, text);
    }

    pub(crate) fn text_centred(&mut self, centre: f32, y: f32, st: Style, c: Color, text: &str) {
        let w = self.text_width(st, text);
        self.text(centre - w * 0.5, y, st, c, text);
    }

    pub(crate) fn text_width(&mut self, st: Style, text: &str) -> f32 {
        self.fonts.width(self.s, st, text)
    }

    /// Text cut short with a full stop when it would run past `width`.
    pub(crate) fn text_fit(&mut self, x: f32, y: f32, width: f32, st: Style, c: Color, text: &str) {
        if self.text_width(st, text) <= width {
            self.text(x, y, st, c, text);
            return;
        }
        let mut cut = text.to_owned();
        while cut.pop().is_some() {
            let candidate = format!("{}\u{2026}", cut.trim_end());
            if self.text_width(st, &candidate) <= width {
                self.text(x, y, st, c, &candidate);
                return;
            }
        }
    }

    /// Splits `text` into lines no wider than `width`, between words.
    pub(crate) fn wrap(&mut self, st: Style, text: &str, width: f32) -> Vec<String> {
        let mut lines = Vec::new();
        let mut line = String::new();
        for word in text.split_whitespace() {
            let trial = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if self.text_width(st, &trial) > width && !line.is_empty() {
                lines.push(std::mem::replace(&mut line, word.to_owned()));
            } else {
                line = trial;
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
        lines
    }

    /// The frame as `0RGB` words for the window (opaque, so premultiplied is straight).
    pub(crate) fn to_words(&self, out: &mut [u32]) {
        for (o, p) in out.iter_mut().zip(self.px.pixels()) {
            *o = (p.red() as u32) << 16 | (p.green() as u32) << 8 | p.blue() as u32;
        }
    }
}

/// A rectangle in pixels.
#[derive(Clone, Copy)]
struct PxRect {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

impl PxRect {
    fn to_skia(self) -> Option<tiny_skia::Rect> {
        tiny_skia::Rect::from_ltrb(self.x0, self.y0, self.x1, self.y1)
    }

    fn bounds(self) -> (i32, i32, i32, i32) {
        (
            self.x0 as i32,
            self.y0 as i32,
            self.x1 as i32,
            self.y1 as i32,
        )
    }
}
