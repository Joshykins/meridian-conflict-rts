//! 2D overlay batch: rectangles, lines, arcs, images and text in pixel coordinates.
//! The game fills one of these per frame; the renderer draws it in one call.
//!
//! Everything textured samples one RGBA atlas that the overlay owns:
//!
//! * the 8x8 bitmap font in the top-left corner (`text`, `label`: the HUD and
//!   the profiler, where a fixed pitch is what you want);
//! * outline glyphs, rasterised on first use at exactly the pixel size they are
//!   drawn at, so type stays crisp at any UI scale (`type_text`);
//! * four 512 px image slots along the bottom (`set_image`, `image`), used for
//!   things like map previews.
//!
//! The renderer uploads whatever rows changed since it last looked.

use crate::textures::{self, FONT_ATLAS_H, FONT_ATLAS_W};
use std::cell::Cell;
use std::collections::HashMap;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct OverlayVertex {
    pub pos: [f32; 2],
    /// Atlas coordinates; negative x means a solid fill.
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

pub const MAX_OVERLAY_VERTICES: usize = 65_536;

/// Glyph cell of the bitmap font in pixels at scale 1.
pub const GLYPH: f32 = 8.0;

/// Image slots are squares of this many pixels along the bottom of the atlas.
pub const IMAGE_SLOT: usize = 512;
pub const IMAGE_SLOTS: usize = FONT_ATLAS_W / IMAGE_SLOT;
const IMAGES_Y: usize = FONT_ATLAS_H - IMAGE_SLOT;
/// Outline glyphs are packed below the bitmap font and above the image slots.
const GLYPHS_Y: usize = textures::BITMAP_FONT_H + 8;
const SOLID: [[f32; 2]; 4] = [[-1.0, 0.0]; 4];
/// Width of the soft edge that anti-aliases strokes and discs.
const FEATHER: f32 = 1.0;

/// The UI typeface's weights (Rajdhani, SIL OFL; see `assets/fonts/OFL.txt`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Face {
    Light,
    Medium,
    Bold,
}

/// How a run of text is set: weight, size in pixels, extra space between letters.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Type {
    pub face: Face,
    pub px: f32,
    pub tracking: f32,
}

impl Type {
    pub const fn new(face: Face, px: f32, tracking: f32) -> Type {
        Type { face, px, tracking }
    }
}

#[derive(Clone, Copy)]
struct Glyph {
    /// Atlas rectangle in pixels.
    at: [u16; 2],
    size: [u16; 2],
    /// Bitmap offset from the pen: left bearing, and bottom edge above the baseline.
    offset: [f32; 2],
    advance: f32,
}

struct Fonts {
    faces: [fontdue::Font; 3],
}

impl Fonts {
    fn load() -> Fonts {
        let face = |bytes: &[u8]| fontdue::Font::from_bytes(bytes, fontdue::FontSettings { scale: 40.0, ..Default::default() }).expect("the embedded font parses");
        Fonts {
            faces: [
                face(include_bytes!("../assets/fonts/Rajdhani-Light.ttf")),
                face(include_bytes!("../assets/fonts/Rajdhani-Medium.ttf")),
                face(include_bytes!("../assets/fonts/Rajdhani-SemiBold.ttf")),
            ],
        }
    }
}

pub struct Overlay {
    pub vertices: Vec<OverlayVertex>,
    /// Set when a frame wanted more than `MAX_OVERLAY_VERTICES`; shown by the profiler.
    pub overflowed: bool,
    /// RGBA8, `FONT_ATLAS_W` x `FONT_ATLAS_H`.
    atlas: Vec<u8>,
    /// Rows changed since the renderer last uploaded, as `start..end`.
    dirty: Cell<(usize, usize)>,
    /// Parsed on first use: headless tools that never set type do not pay for it.
    fonts: Option<Fonts>,
    glyphs: HashMap<(Face, u16, char), Glyph>,
    /// Shelf packer: next free position and the height of the current shelf.
    shelf: (usize, usize, usize),
}

impl Default for Overlay {
    fn default() -> Overlay {
        Overlay {
            vertices: Vec::new(),
            overflowed: false,
            atlas: textures::font_atlas(),
            dirty: Cell::new((0, 0)),
            fonts: None,
            glyphs: HashMap::new(),
            shelf: (0, GLYPHS_Y, 0),
        }
    }
}

fn with_alpha(c: [f32; 4], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], c[3] * a]
}

impl Overlay {
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.overflowed = false;
    }

    // -- atlas ------------------------------------------------------------------

    pub fn atlas(&self) -> &[u8] {
        &self.atlas
    }

    /// Rows of the atlas changed since the last call, if any. The renderer calls
    /// this once per frame and uploads them.
    pub fn take_dirty_rows(&self) -> Option<std::ops::Range<usize>> {
        let (start, end) = self.dirty.replace((0, 0));
        (end > start).then_some(start..end)
    }

    fn mark_dirty(&self, start: usize, end: usize) {
        let (s, e) = self.dirty.get();
        self.dirty.set(if e > s { (s.min(start), e.max(end)) } else { (start, end) });
    }

    /// Copies an RGBA8 (sRGB) image of at most `IMAGE_SLOT` pixels a side into a slot.
    pub fn set_image(&mut self, slot: usize, width: usize, height: usize, rgba: &[u8]) {
        assert!(slot < IMAGE_SLOTS && width <= IMAGE_SLOT && height <= IMAGE_SLOT && rgba.len() == width * height * 4);
        for y in 0..height {
            let at = ((IMAGES_Y + y) * FONT_ATLAS_W + slot * IMAGE_SLOT) * 4;
            self.atlas[at..at + width * 4].copy_from_slice(&rgba[y * width * 4..(y + 1) * width * 4]);
        }
        self.mark_dirty(IMAGES_Y, IMAGES_Y + height);
    }

    /// Draws the `src` rectangle (x, y, width, height in pixels) of an image slot.
    pub fn image(&mut self, slot: usize, src: [f32; 4], x: f32, y: f32, w: f32, h: f32, tint: [f32; 4]) {
        // Half a texel in, so linear filtering never reaches the neighbouring slot.
        let (left, top) = ((slot * IMAGE_SLOT) as f32 + src[0] + 0.5, IMAGES_Y as f32 + src[1] + 0.5);
        let (u0, v0) = (left / FONT_ATLAS_W as f32, top / FONT_ATLAS_H as f32);
        let (u1, v1) = ((left + src[2] - 1.0) / FONT_ATLAS_W as f32, (top + src[3] - 1.0) / FONT_ATLAS_H as f32);
        self.quad([[x, y], [x + w, y], [x + w, y + h], [x, y + h]], [[u0, v0], [u1, v0], [u1, v1], [u0, v1]], [tint; 4]);
    }

    fn glyph(&mut self, face: Face, px: u16, ch: char) -> Glyph {
        if let Some(g) = self.glyphs.get(&(face, px, ch)) {
            return *g;
        }
        let fonts = self.fonts.get_or_insert_with(Fonts::load);
        let (metrics, coverage) = fonts.faces[face as usize].rasterize(ch, px as f32);
        let (w, h) = (metrics.width, metrics.height);
        let (mut x, mut y, mut shelf_h) = self.shelf;
        if x + w + 1 > FONT_ATLAS_W {
            (x, y, shelf_h) = (0, y + shelf_h + 1, 0);
        }
        if y + h + 1 > IMAGES_Y {
            // Out of room: start over. Quads already batched this frame keep their old
            // coordinates, so one frame may show wrong glyphs; it takes thousands of
            // distinct sizes to get here.
            log::warn!("the glyph atlas filled up and was reset");
            for texel in self.atlas[GLYPHS_Y * FONT_ATLAS_W * 4..IMAGES_Y * FONT_ATLAS_W * 4].chunks_exact_mut(4) {
                texel.copy_from_slice(&[255, 255, 255, 0]);
            }
            self.mark_dirty(GLYPHS_Y, IMAGES_Y);
            self.glyphs.clear();
            (x, y, shelf_h) = (0, GLYPHS_Y, 0);
        }
        for row in 0..h {
            for col in 0..w {
                let at = ((y + row) * FONT_ATLAS_W + x + col) * 4;
                self.atlas[at..at + 4].copy_from_slice(&[255, 255, 255, coverage[row * w + col]]);
            }
        }
        self.mark_dirty(y, y + h.max(1));
        self.shelf = (x + w + 1, y, shelf_h.max(h));
        let g = Glyph { at: [x as u16, y as u16], size: [w as u16, h as u16], offset: [metrics.xmin as f32, metrics.ymin as f32], advance: metrics.advance_width };
        self.glyphs.insert((face, px, ch), g);
        g
    }

    // -- primitives -------------------------------------------------------------

    fn quad(&mut self, corners: [[f32; 2]; 4], uvs: [[f32; 2]; 4], colors: [[f32; 4]; 4]) {
        if self.vertices.len() + 6 > MAX_OVERLAY_VERTICES {
            self.overflowed = true;
            return;
        }
        for i in [0, 1, 2, 0, 2, 3] {
            self.vertices.push(OverlayVertex { pos: corners[i], uv: uvs[i], color: colors[i] });
        }
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.quad([[x, y], [x + w, y], [x + w, y + h], [x, y + h]], SOLID, [color; 4]);
    }

    /// A rectangle with a colour per corner: top-left, top-right, bottom-right, bottom-left.
    pub fn gradient(&mut self, x: f32, y: f32, w: f32, h: f32, colors: [[f32; 4]; 4]) {
        self.quad([[x, y], [x + w, y], [x + w, y + h], [x, y + h]], SOLID, colors);
    }

    /// Any convex quadrilateral, corners in order.
    pub fn quad_fill(&mut self, corners: [[f32; 2]; 4], color: [f32; 4]) {
        self.quad(corners, SOLID, [color; 4]);
    }

    pub fn frame(&mut self, x: f32, y: f32, w: f32, h: f32, thickness: f32, color: [f32; 4]) {
        self.rect(x, y, w, thickness, color);
        self.rect(x, y + h - thickness, w, thickness, color);
        self.rect(x, y + thickness, thickness, h - 2.0 * thickness, color);
        self.rect(x + w - thickness, y + thickness, thickness, h - 2.0 * thickness, color);
    }

    pub fn line(&mut self, a: [f32; 2], b: [f32; 2], thickness: f32, color: [f32; 4]) {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-3 {
            return;
        }
        let (nx, ny) = (-dy / len * thickness * 0.5, dx / len * thickness * 0.5);
        self.quad([[a[0] + nx, a[1] + ny], [b[0] + nx, b[1] + ny], [b[0] - nx, b[1] - ny], [a[0] - nx, a[1] - ny]], SOLID, [color; 4]);
    }

    /// An anti-aliased line: a solid core with a soft edge either side. For
    /// anything that is not axis-aligned; `rect` is sharper for what is.
    pub fn stroke(&mut self, a: [f32; 2], b: [f32; 2], thickness: f32, color: [f32; 4]) {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-3 {
            return;
        }
        let (nx, ny) = (-dy / len, dx / len);
        // A hairline keeps its brightness by fading the core rather than vanishing.
        let core = (thickness - FEATHER).max(0.0) * 0.5;
        let color = with_alpha(color, (thickness / FEATHER).min(1.0));
        let clear = with_alpha(color, 0.0);
        let at = |p: [f32; 2], d: f32| [p[0] + nx * d, p[1] + ny * d];
        let outer = core + FEATHER;
        self.quad([at(a, -outer), at(b, -outer), at(b, -core), at(a, -core)], SOLID, [clear, clear, color, color]);
        if core > 0.0 {
            self.quad([at(a, -core), at(b, -core), at(b, core), at(a, core)], SOLID, [color; 4]);
        }
        self.quad([at(a, core), at(b, core), at(b, outer), at(a, outer)], SOLID, [color, color, clear, clear]);
    }

    /// An anti-aliased circular arc from angle `from` to `to` (radians, clockwise
    /// on screen from +X). A full turn draws a ring.
    pub fn arc(&mut self, centre: [f32; 2], radius: f32, from: f32, to: f32, thickness: f32, color: [f32; 4]) {
        let sweep = to - from;
        let segments = ((sweep.abs() * radius / 5.0).ceil() as usize).clamp(2, 256);
        let core = (thickness - FEATHER).max(0.0) * 0.5;
        let color = with_alpha(color, (thickness / FEATHER).min(1.0));
        let clear = with_alpha(color, 0.0);
        let radii = [radius - core - FEATHER, radius - core, radius + core, radius + core + FEATHER];
        let point = |angle: f32, r: f32| [centre[0] + angle.cos() * r, centre[1] + angle.sin() * r];
        for i in 0..segments {
            let (a0, a1) = (from + sweep * i as f32 / segments as f32, from + sweep * (i + 1) as f32 / segments as f32);
            for (band, colors) in [[clear, clear, color, color], [color; 4], [color, color, clear, clear]].into_iter().enumerate() {
                if band == 1 && core <= 0.0 {
                    continue;
                }
                let (inner, outer) = (radii[band].max(0.0), radii[band + 1].max(0.0));
                self.quad([point(a0, inner), point(a1, inner), point(a1, outer), point(a0, outer)], SOLID, colors);
            }
        }
    }

    /// A filled, anti-aliased circle.
    pub fn disc(&mut self, centre: [f32; 2], radius: f32, color: [f32; 4]) {
        let segments = ((radius * 1.3).ceil() as usize).clamp(8, 96);
        let clear = with_alpha(color, 0.0);
        let inner = (radius - FEATHER * 0.5).max(0.0);
        let point = |i: usize, r: f32| {
            let a = i as f32 / segments as f32 * std::f32::consts::TAU;
            [centre[0] + a.cos() * r, centre[1] + a.sin() * r]
        };
        for i in 0..segments {
            self.quad([centre, point(i, inner), point(i + 1, inner), centre], SOLID, [color; 4]);
            self.quad([point(i, inner), point(i + 1, inner), point(i + 1, inner + FEATHER), point(i, inner + FEATHER)], SOLID, [color, color, clear, clear]);
        }
    }

    // -- text -------------------------------------------------------------------

    /// Draws ASCII text in the bitmap font; returns the x position after the last glyph.
    pub fn text(&mut self, x: f32, y: f32, scale: f32, color: [f32; 4], text: &str) -> f32 {
        let size = GLYPH * scale;
        let mut pen = x;
        for ch in text.chars() {
            let code = if ch.is_ascii() { ch as usize } else { b'?' as usize };
            if code != b' ' as usize {
                // Half a texel inside the cell: scaled text is filtered, and must not pick up the glyphs next door.
                let (u0, v0) = (((code % 16) as f32 * 8.0 + 0.5) / FONT_ATLAS_W as f32, ((code / 16) as f32 * 8.0 + 0.5) / FONT_ATLAS_H as f32);
                let (du, dv) = (7.0 / FONT_ATLAS_W as f32, 7.0 / FONT_ATLAS_H as f32);
                self.quad(
                    [[pen, y], [pen + size, y], [pen + size, y + size], [pen, y + size]],
                    [[u0, v0], [u0 + du, v0], [u0 + du, v0 + dv], [u0, v0 + dv]],
                    [color; 4],
                );
            }
            pen += size;
        }
        pen
    }

    /// Bitmap text with a one-pixel drop shadow, readable over terrain.
    pub fn label(&mut self, x: f32, y: f32, scale: f32, color: [f32; 4], text: &str) -> f32 {
        self.text(x + scale, y + scale, scale, [0.0, 0.0, 0.0, color[3] * 0.8], text);
        self.text(x, y, scale, color, text)
    }

    pub fn text_width(scale: f32, text: &str) -> f32 {
        text.chars().count() as f32 * GLYPH * scale
    }

    /// Sets a run of type with its baseline at `y`; returns the x position after it.
    /// Glyphs land on whole pixels, so text is as sharp as the rasteriser made it.
    pub fn type_text(&mut self, x: f32, y: f32, style: Type, color: [f32; 4], text: &str) -> f32 {
        let px = style.px.round().clamp(4.0, 400.0) as u16;
        let (mut pen, baseline) = (x, y.round());
        for ch in text.chars() {
            let g = self.glyph(style.face, px, ch);
            if g.size[0] > 0 && g.size[1] > 0 {
                let (w, h) = (g.size[0] as f32, g.size[1] as f32);
                let (left, top) = ((pen + g.offset[0]).round(), baseline - g.offset[1] - h);
                let (u0, v0) = (g.at[0] as f32 / FONT_ATLAS_W as f32, g.at[1] as f32 / FONT_ATLAS_H as f32);
                let (u1, v1) = (u0 + w / FONT_ATLAS_W as f32, v0 + h / FONT_ATLAS_H as f32);
                self.quad([[left, top], [left + w, top], [left + w, top + h], [left, top + h]], [[u0, v0], [u1, v0], [u1, v1], [u0, v1]], [color; 4]);
            }
            pen += g.advance + style.tracking;
        }
        pen
    }

    /// Width of a run as `type_text` sets it, without the trailing letter space.
    pub fn type_width(&mut self, style: Type, text: &str) -> f32 {
        let px = style.px.round().clamp(4.0, 400.0) as u16;
        let mut width = 0.0;
        let mut count = 0;
        for ch in text.chars() {
            width += self.glyph(style.face, px, ch).advance;
            count += 1;
        }
        width + style.tracking * (count as f32 - 1.0).max(0.0)
    }

    /// Height of a capital letter at this size: what UI text is centred on.
    pub fn cap_height(&mut self, style: Type) -> f32 {
        let px = style.px.round().clamp(4.0, 400.0) as u16;
        let g = self.glyph(style.face, px, 'H');
        g.size[1] as f32 + g.offset[1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyphs_are_cached_and_mark_the_atlas_dirty() {
        let mut o = Overlay::default();
        assert!(o.take_dirty_rows().is_none());
        let style = Type::new(Face::Medium, 18.0, 2.0);
        let end = o.type_text(10.0, 40.0, style, [1.0; 4], "SKIRMISH");
        assert!((end - 10.0 - o.type_width(style, "SKIRMISH") - style.tracking).abs() < 1e-3);
        let rows = o.take_dirty_rows().expect("new glyphs were rasterised");
        assert!(rows.start >= GLYPHS_Y && rows.end <= IMAGES_Y);
        // Six distinct letters, eight quads; a second run adds no glyphs.
        assert_eq!(o.glyphs.len(), 6);
        assert_eq!(o.vertices.len(), 8 * 6);
        o.type_text(10.0, 80.0, style, [1.0; 4], "SKIRMISH");
        assert!(o.take_dirty_rows().is_none());
        let cap = o.cap_height(style);
        assert!((10.0..16.0).contains(&cap), "cap height {cap}");
    }

    #[test]
    fn images_land_in_their_slot() {
        let mut o = Overlay::default();
        o.set_image(1, 2, 2, &[9u8; 16]);
        assert_eq!(o.take_dirty_rows(), Some(IMAGES_Y..IMAGES_Y + 2));
        let at = (IMAGES_Y * FONT_ATLAS_W + IMAGE_SLOT) * 4;
        assert_eq!(o.atlas()[at..at + 8], [9u8; 8]);
        assert_eq!(o.atlas()[at - 4..at], [255, 255, 255, 0]);
    }
}
