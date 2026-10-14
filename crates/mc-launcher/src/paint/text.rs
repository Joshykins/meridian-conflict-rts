//! Type: the game's Barlow faces (mc-render `overlay.rs`, SIL OFL), each glyph
//! rasterised once per pixel size by fontdue and blended straight into the
//! frame.

use std::collections::HashMap;

use tiny_skia::Pixmap;

use super::Color;

/// The UI typeface's weights, as the game's: light semi-condensed for display,
/// medium for running text, semibold semi-condensed for names and figures.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Face {
    Light,
    Medium,
    Bold,
}

/// A text style in points.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Style {
    pub(crate) face: Face,
    pub(crate) size: f32,
    pub(crate) tracking: f32,
}

pub(crate) const fn style(face: Face, size: f32, tracking: f32) -> Style {
    Style {
        face,
        size,
        tracking,
    }
}

/// The game's type scale (mc-game `ui::type_scale`), and the launcher's few
/// display sizes.
pub(crate) mod type_scale {
    use super::{style, Face, Style};
    pub(crate) const HERO: Style = style(Face::Light, 44.0, 1.0);
    pub(crate) const NAME_BOLD: Style = style(Face::Bold, 28.0, 0.3);
    pub(crate) const NAME_LIGHT: Style = style(Face::Light, 28.0, 0.3);
    pub(crate) const TITLE: Style = style(Face::Light, 26.0, 0.8);
    pub(crate) const ITEM: Style = style(Face::Bold, 19.0, 0.2);
    pub(crate) const PLAY: Style = style(Face::Bold, 24.0, 1.6);
    pub(crate) const BUTTON: Style = style(Face::Bold, 15.5, 0.8);
    pub(crate) const BODY: Style = style(Face::Medium, 15.0, 0.0);
    pub(crate) const VALUE: Style = style(Face::Bold, 15.0, 0.3);
    pub(crate) const CAPTION: Style = style(Face::Bold, 13.5, 0.4);
    pub(crate) const MICRO: Style = style(Face::Medium, 11.0, 0.9);
}

/// Cap height over the em, for placing a line by its middle (Barlow's 700/1000).
const CAP: f32 = 0.7;

struct Glyph {
    metrics: fontdue::Metrics,
    coverage: Vec<u8>,
}

pub(crate) struct Fonts {
    faces: [fontdue::Font; 3],
    /// By face, pixel size in quarter pixels, and character.
    glyphs: HashMap<(Face, u32, char), Glyph>,
}

fn bytes(face: Face) -> &'static [u8] {
    match face {
        Face::Light => {
            include_bytes!("../../../mc-render/assets/fonts/BarlowSemiCondensed-Light.ttf")
        }
        Face::Medium => include_bytes!("../../../mc-render/assets/fonts/Barlow-Medium.ttf"),
        Face::Bold => {
            include_bytes!("../../../mc-render/assets/fonts/BarlowSemiCondensed-SemiBold.ttf")
        }
    }
}

impl Fonts {
    pub(crate) fn new() -> Fonts {
        let load = |face| {
            fontdue::Font::from_bytes(bytes(face), fontdue::FontSettings::default())
                .expect("the bundled fonts parse")
        };
        Fonts {
            faces: [load(Face::Light), load(Face::Medium), load(Face::Bold)],
            glyphs: HashMap::new(),
        }
    }

    fn font(&self, face: Face) -> &fontdue::Font {
        &self.faces[face as usize]
    }

    fn glyph(&mut self, face: Face, px: f32, c: char) -> &Glyph {
        let key = (face, (px * 4.0).round() as u32, c);
        if !self.glyphs.contains_key(&key) {
            let (metrics, coverage) = self.faces[face as usize].rasterize(c, px);
            self.glyphs.insert(key, Glyph { metrics, coverage });
        }
        &self.glyphs[&key]
    }

    /// How wide `text` sets, in points.
    pub(crate) fn width(&mut self, s: f32, st: Style, text: &str) -> f32 {
        let px = st.size * s;
        let mut w = 0.0;
        let mut prev = None;
        for c in text.chars() {
            if let Some(p) = prev {
                w += self.font(st.face).horizontal_kern(p, c, px).unwrap_or(0.0);
            }
            w += self.glyph(st.face, px, c).metrics.advance_width + st.tracking * s;
            prev = Some(c);
        }
        if !text.is_empty() {
            w -= st.tracking * s;
        }
        w / s
    }

    /// Draws `text` from `x`, its capitals centred on `y` (points); returns
    /// where it ends. `clip` is in pixels: left, top, right, bottom.
    pub(crate) fn draw(
        &mut self,
        px_map: &mut Pixmap,
        s: f32,
        x: f32,
        y: f32,
        st: Style,
        color: Color,
        text: &str,
        clip: Option<(i32, i32, i32, i32)>,
    ) -> f32 {
        let px = st.size * s;
        let baseline = (y * s + CAP * px * 0.5).round();
        let mut pen = (x * s).round();
        let (w, h) = (px_map.width() as i32, px_map.height() as i32);
        let (cx0, cy0, cx1, cy1) = clip.unwrap_or((0, 0, w, h));
        let (cx0, cy0, cx1, cy1) = (cx0.max(0), cy0.max(0), cx1.min(w), cy1.min(h));
        let [r, g, b, a] = color;
        let mut prev = None;
        for c in text.chars() {
            if let Some(p) = prev {
                pen += self.font(st.face).horizontal_kern(p, c, px).unwrap_or(0.0);
            }
            prev = Some(c);
            let glyph = self.glyph(st.face, px, c);
            let m = glyph.metrics;
            let gx = pen.round() as i32 + m.xmin;
            let gy = baseline as i32 - m.height as i32 - m.ymin;
            let data = px_map.data_mut();
            for row in 0..m.height as i32 {
                let yy = gy + row;
                if yy < cy0 || yy >= cy1 {
                    continue;
                }
                for col in 0..m.width as i32 {
                    let xx = gx + col;
                    if xx < cx0 || xx >= cx1 {
                        continue;
                    }
                    let cov = glyph.coverage[(row * m.width as i32 + col) as usize];
                    if cov == 0 {
                        continue;
                    }
                    let k = cov as f32 / 255.0 * a;
                    let i = ((yy * w + xx) * 4) as usize;
                    let px = &mut data[i..i + 4];
                    // The frame is premultiplied; the colour is straight.
                    px[0] = (r * 255.0 * k + px[0] as f32 * (1.0 - k)).round() as u8;
                    px[1] = (g * 255.0 * k + px[1] as f32 * (1.0 - k)).round() as u8;
                    px[2] = (b * 255.0 * k + px[2] as f32 * (1.0 - k)).round() as u8;
                    px[3] = (255.0 * k + px[3] as f32 * (1.0 - k)).round() as u8;
                }
            }
            pen += m.advance_width + st.tracking * s;
        }
        if !text.is_empty() {
            pen -= st.tracking * s;
        }
        pen / s
    }
}
