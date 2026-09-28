//! The drawing surface faction marks are made on: tiny-skia paths in a
//! design space scaled to the pixels asked for, plus type set from the UI's
//! own Barlow outlines, straight or bent round an arc.

use glam::Vec2;
use mc_render::overlay::face_bytes;
use mc_render::Face;
use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Mask, Paint, Path, PathBuilder, Pixmap, Point,
    RadialGradient, Shader, SpreadMode, Stroke, Transform,
};

/// An sRGB colour, 0xRRGGBB, at `alpha`.
pub fn hex(rgb: u32, alpha: f32) -> Color {
    let c = |shift: u32| ((rgb >> shift) & 0xFF) as f32 / 255.0;
    Color::from_rgba(c(16), c(8), c(0), alpha.clamp(0.0, 1.0)).unwrap_or(Color::BLACK)
}

/// A closed polygon through `points`.
pub fn poly(points: &[Vec2]) -> Option<Path> {
    let mut pb = PathBuilder::new();
    let (first, rest) = points.split_first()?;
    pb.move_to(first.x, first.y);
    for p in rest {
        pb.line_to(p.x, p.y);
    }
    pb.close();
    pb.finish()
}

pub fn circle(c: Vec2, r: f32) -> Option<Path> {
    PathBuilder::from_circle(c.x, c.y, r)
}

/// A shader running through `stops` (offset, colour) from `a` to `b`.
pub fn linear(a: Vec2, b: Vec2, stops: &[(f32, Color)]) -> Shader<'static> {
    let stops = stops
        .iter()
        .map(|&(t, c)| GradientStop::new(t, c))
        .collect();
    LinearGradient::new(
        Point::from_xy(a.x, a.y),
        Point::from_xy(b.x, b.y),
        stops,
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(Color::WHITE))
}

/// A shader running through `stops` outward from `c` to radius `r`.
pub fn radial(c: Vec2, r: f32, stops: &[(f32, Color)]) -> Shader<'static> {
    let stops = stops
        .iter()
        .map(|&(t, c)| GradientStop::new(t, c))
        .collect();
    RadialGradient::new(
        Point::from_xy(c.x, c.y),
        Point::from_xy(c.x, c.y),
        r,
        stops,
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(Color::TRANSPARENT))
}

/// A picture being drawn: design units in, pixels out.
pub struct Canvas {
    pixmap: Pixmap,
    /// Design space to pixels.
    ts: Transform,
    /// Pixels per design unit.
    pub scale: f32,
}

impl Canvas {
    /// A clear canvas `size` pixels across showing the design box `design`
    /// (width, height in design units), scaled to fit and centred.
    pub fn new(size: [usize; 2], design: Vec2) -> Canvas {
        let (w, h) = (size[0].max(1) as f32, size[1].max(1) as f32);
        let scale = (w / design.x).min(h / design.y);
        let (dx, dy) = ((w - design.x * scale) * 0.5, (h - design.y * scale) * 0.5);
        Canvas {
            pixmap: Pixmap::new(size[0].max(1) as u32, size[1].max(1) as u32)
                .unwrap_or_else(|| Pixmap::new(1, 1).expect("a 1x1 pixmap")),
            ts: Transform::from_row(scale, 0.0, 0.0, scale, dx, dy),
            scale,
        }
    }

    fn paint(shader: Shader<'static>) -> Paint<'static> {
        Paint {
            shader,
            anti_alias: true,
            ..Paint::default()
        }
    }

    pub fn fill(&mut self, path: &Option<Path>, color: Color) {
        self.fill_with(path, Shader::SolidColor(color), None);
    }

    /// Fills with a gradient (or any shader) in design space (tiny-skia moves the
    /// shader with the path), through `clip` if given.
    pub fn fill_with(&mut self, path: &Option<Path>, shader: Shader<'static>, clip: Option<&Mask>) {
        let Some(path) = path else { return };
        self.pixmap
            .fill_path(path, &Self::paint(shader), FillRule::EvenOdd, self.ts, clip);
    }

    /// A stroke `width` design units wide, never thinner than `min_px` pixels.
    pub fn stroke(&mut self, path: &Option<Path>, width: f32, min_px: f32, color: Color) {
        self.stroke_with(path, width, min_px, Shader::SolidColor(color), None);
    }

    pub fn stroke_with(
        &mut self,
        path: &Option<Path>,
        width: f32,
        min_px: f32,
        shader: Shader<'static>,
        clip: Option<&Mask>,
    ) {
        let Some(path) = path else { return };
        let stroke = Stroke {
            width: width.max(min_px / self.scale),
            line_join: tiny_skia::LineJoin::Round,
            line_cap: tiny_skia::LineCap::Round,
            ..Stroke::default()
        };
        self.pixmap
            .stroke_path(path, &Self::paint(shader), &stroke, self.ts, clip);
    }

    /// Cuts `path` out of whatever is drawn already.
    pub fn erase(&mut self, path: &Option<Path>) {
        let Some(path) = path else { return };
        let paint = Paint {
            blend_mode: tiny_skia::BlendMode::Clear,
            anti_alias: true,
            ..Paint::default()
        };
        self.pixmap
            .fill_path(path, &paint, FillRule::Winding, self.ts, None);
    }

    /// Clears a band `width` design units wide along `path`: a gap between shapes.
    pub fn erase_stroke(&mut self, path: &Option<Path>, width: f32) {
        let Some(path) = path else { return };
        let paint = Paint {
            blend_mode: tiny_skia::BlendMode::Clear,
            anti_alias: true,
            ..Paint::default()
        };
        let stroke = Stroke {
            width,
            line_join: tiny_skia::LineJoin::Round,
            ..Stroke::default()
        };
        self.pixmap
            .stroke_path(path, &paint, &stroke, self.ts, None);
    }

    /// Runs `draw` with the design space moved by `offset` and scaled by `k`:
    /// one mark drawn inside another.
    pub fn within(&mut self, offset: Vec2, k: f32, draw: impl FnOnce(&mut Canvas)) {
        let (ts, scale) = (self.ts, self.scale);
        self.ts = Transform::from_row(k, 0.0, 0.0, k, offset.x, offset.y).post_concat(ts);
        self.scale = scale * k;
        draw(self);
        (self.ts, self.scale) = (ts, scale);
    }

    /// A mask of the inside of `path`, to clip later fills to.
    pub fn mask(&self, path: &Option<Path>) -> Option<Mask> {
        let mut mask = Mask::new(self.pixmap.width(), self.pixmap.height())?;
        mask.fill_path(path.as_ref()?, FillRule::Winding, true, self.ts);
        Some(mask)
    }

    /// Straight-alpha sRGB RGBA, row by row, as the overlay takes it.
    pub fn into_rgba(self) -> Vec<u8> {
        let mut data = self.pixmap.take();
        for px in data.as_chunks_mut::<4>().0 {
            let a = px[3];
            if a > 0 && a < 255 {
                for c in &mut px[..3] {
                    *c = ((*c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
                }
            }
        }
        data
    }
}

/// Glyph outlines into a path builder, moved and turned as they go.
struct Pen<'a> {
    pb: &'a mut PathBuilder,
    /// Font units to design space.
    ts: Transform,
}

impl Pen<'_> {
    fn at(&self, x: f32, y: f32) -> (f32, f32) {
        let mut p = [Point::from_xy(x, y)];
        self.ts.map_points(&mut p);
        (p[0].x, p[0].y)
    }
}

impl ttf_parser::OutlineBuilder for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.at(x, y);
        self.pb.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.at(x, y);
        self.pb.line_to(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let ((x1, y1), (x, y)) = (self.at(x1, y1), self.at(x, y));
        self.pb.quad_to(x1, y1, x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let ((x1, y1), (x2, y2), (x, y)) = (self.at(x1, y1), self.at(x2, y2), self.at(x, y));
        self.pb.cubic_to(x1, y1, x2, y2, x, y);
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

/// Type set in one of the UI's faces, `cap` design units tall at the capitals.
pub struct Type {
    face: ttf_parser::Face<'static>,
    /// Design units per font unit.
    k: f32,
    /// Extra space after each letter, design units.
    tracking: f32,
}

impl Type {
    pub fn new(face: Face, cap: f32, tracking: f32) -> Option<Type> {
        let face = ttf_parser::Face::parse(face_bytes(face), 0).ok()?;
        let cap_units = face
            .capital_height()
            .map_or(face.units_per_em() as f32 * 0.7, |c| c as f32);
        Some(Type {
            k: cap / cap_units,
            face,
            tracking,
        })
    }

    fn advance(&self, ch: char) -> f32 {
        let g = self.face.glyph_index(ch);
        g.and_then(|g| self.face.glyph_hor_advance(g))
            .map_or(0.0, |a| a as f32 * self.k)
    }

    /// Width of `text` as `line` sets it.
    pub fn width(&self, text: &str) -> f32 {
        let n = text.chars().count() as f32;
        text.chars().map(|c| self.advance(c)).sum::<f32>() + self.tracking * (n - 1.0).max(0.0)
    }

    fn glyph(&self, pb: &mut PathBuilder, ch: char, ts: Transform) {
        if let Some(g) = self.face.glyph_index(ch) {
            // Font units are y-up: flip, then scale, then place.
            let ts = Transform::from_row(self.k, 0.0, 0.0, -self.k, 0.0, 0.0).post_concat(ts);
            self.face.outline_glyph(g, &mut Pen { pb, ts });
        }
    }

    /// `text` centred on `centre.x` with its baseline at `centre.y`.
    pub fn line(&self, text: &str, centre: Vec2) -> Option<Path> {
        let mut pb = PathBuilder::new();
        let mut x = centre.x - self.width(text) * 0.5;
        for ch in text.chars() {
            self.glyph(&mut pb, ch, Transform::from_translate(x, centre.y));
            x += self.advance(ch) + self.tracking;
        }
        pb.finish()
    }

    /// `text` round a circle about `centre`, its baseline `radius` out, centred
    /// on the angle `at` (radians, y down: -PI/2 is the top). Along the top
    /// (`outside`) the letters stand on the arc and read left to right; along the
    /// bottom they hang from it, still upright and left to right.
    pub fn arc(
        &self,
        text: &str,
        centre: Vec2,
        radius: f32,
        at: f32,
        outside: bool,
    ) -> Option<Path> {
        let mut pb = PathBuilder::new();
        let total = self.width(text) / radius;
        let dir = if outside { 1.0 } else { -1.0 };
        let mut a = at - dir * total * 0.5;
        for ch in text.chars() {
            let w = self.advance(ch);
            // The glyph's middle sits on the arc, turned to its tangent.
            let mid = a + dir * (w * 0.5) / radius;
            let p = centre + Vec2::new(mid.cos(), mid.sin()) * radius;
            let turn = if outside {
                mid + std::f32::consts::FRAC_PI_2
            } else {
                mid - std::f32::consts::FRAC_PI_2
            };
            let ts = Transform::from_translate(-w * 0.5, 0.0)
                .post_concat(Transform::from_rotate(turn.to_degrees()))
                .post_concat(Transform::from_translate(p.x, p.y));
            self.glyph(&mut pb, ch, ts);
            a += dir * (w + self.tracking) / radius;
        }
        pb.finish()
    }
}
