//! The picture behind the launcher (`assets/backdrop.png`: the ARC fleet over
//! the coast, the Dominion leading), framed so its subject fills the open
//! window at the top right, graded down, and a blurred copy the glass panels
//! frost.

use tiny_skia::{FilterQuality, Pattern, Pixmap, PixmapPaint, Shader, SpreadMode, Transform};

const ART: &[u8] = include_bytes!("../../assets/backdrop.png");

/// How the art is framed: scaled to cover the window, then `zoom` times
/// more, with the point `focus` of the art (fractions of its width and
/// height) put as near the point `spot` of the window as its edges allow.
struct Framing {
    zoom: f32,
    focus: (f32, f32),
    spot: (f32, f32),
}

/// The Dominion (centred at 58% across and 20% down the art) in the middle
/// of the open window above the news strip, a little larger than covering.
const FRAMING: Framing = Framing {
    zoom: 1.10,
    focus: (0.583, 0.198),
    spot: (0.667, 0.180),
};

pub(crate) struct Backdrop {
    art: Option<Pixmap>,
    /// The art fitted to the last frame's size, graded, and its blurred copy.
    fitted: Option<(Pixmap, Pixmap)>,
}

impl Backdrop {
    pub(crate) fn new() -> Backdrop {
        Backdrop {
            art: decode(ART),
            fitted: None,
        }
    }

    /// Puts the backdrop down as the frame's first layer.
    pub(crate) fn draw(&mut self, px: &mut Pixmap) {
        let (w, h) = (px.width(), px.height());
        let stale = self
            .fitted
            .as_ref()
            .is_none_or(|(f, _)| f.width() != w || f.height() != h);
        if stale {
            self.fitted = Some(fit(self.art.as_ref(), w, h));
        }
        if let Some((sharp, _)) = &self.fitted {
            px.data_mut().copy_from_slice(sharp.data());
        }
    }

    /// The blurred backdrop, for glass: drawn through a panel's shape.
    pub(crate) fn blurred_shader(&self) -> Option<Shader<'_>> {
        let (_, blurred) = self.fitted.as_ref()?;
        Some(Pattern::new(
            blurred.as_ref(),
            SpreadMode::Pad,
            FilterQuality::Nearest,
            1.0,
            Transform::identity(),
        ))
    }
}

/// The PNG as a premultiplied pixmap; `None` (a plain dark backdrop) if it
/// does not decode.
fn decode(bytes: &[u8]) -> Option<Pixmap> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let mut px = Pixmap::new(info.width, info.height)?;
    let channels = info.color_type.samples();
    for (i, p) in px.data_mut().as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let s = &buf[i * channels..];
        let (r, g, b) = match channels {
            1 | 2 => (s[0], s[0], s[0]),
            _ => (s[0], s[1], s[2]),
        };
        p.copy_from_slice(&[r, g, b, 255]);
    }
    Some(px)
}

/// `art` scaled and placed by [`FRAMING`] to cover `w` x `h`, graded darker
/// towards the left and the foot; and a blurred copy.
fn fit(art: Option<&Pixmap>, w: u32, h: u32) -> (Pixmap, Pixmap) {
    let mut out = Pixmap::new(w, h).expect("a backdrop of the window's size");
    out.fill(tiny_skia::Color::from_rgba8(8, 9, 11, 255));
    if let Some(art) = art {
        let (aw, ah) = (art.width() as f32, art.height() as f32);
        let (w, h) = (w as f32, h as f32);
        let k = (w / aw).max(h / ah) * FRAMING.zoom;
        // The focus lands on its spot unless that would bare an edge.
        let place = |art_len: f32, win: f32, focus: f32, spot: f32| {
            (spot * win - focus * art_len * k).clamp(win - art_len * k, 0.0)
        };
        let dx = place(aw, w, FRAMING.focus.0, FRAMING.spot.0);
        let dy = place(ah, h, FRAMING.focus.1, FRAMING.spot.1);
        out.draw_pixmap(
            0,
            0,
            art.as_ref(),
            &PixmapPaint {
                quality: FilterQuality::Bicubic,
                ..PixmapPaint::default()
            },
            Transform::from_row(k, 0.0, 0.0, k, dx, dy),
            None,
        );
    }
    grade(&mut out);
    let blurred = blur(&out, (w.max(h) / 90).max(4));
    (out, blurred)
}

/// The game's front end grades its backdrop down a little and darkens the
/// side its menu stands on; so does the launcher, and the foot for the links.
fn grade(px: &mut Pixmap) {
    let (w, h) = (px.width() as f32, px.height() as f32);
    let width = px.width() as usize;
    for (i, p) in px.data_mut().as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (x, y) = ((i % width) as f32 / w, (i / width) as f32 / h);
        let left = (1.0 - x / 0.55).clamp(0.0, 1.0);
        let foot = ((y - 0.78) / 0.22).clamp(0.0, 1.0);
        let keep = 0.86 * (1.0 - 0.45 * left * left) * (1.0 - 0.5 * foot * foot);
        for c in &mut p[..3] {
            *c = (*c as f32 * keep) as u8;
        }
    }
}

/// Three box blurs of radius `r` each way: near enough a Gaussian.
fn blur(src: &Pixmap, r: u32) -> Pixmap {
    let (w, h) = (src.width() as usize, src.height() as usize);
    let mut a: Vec<[u32; 3]> = src
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| [p[0] as u32, p[1] as u32, p[2] as u32])
        .collect();
    let mut b = a.clone();
    for _ in 0..3 {
        box_pass(&a, &mut b, w, h, r as usize, true);
        box_pass(&b, &mut a, w, h, r as usize, false);
    }
    let mut out = Pixmap::new(src.width(), src.height()).expect("same size");
    for (p, c) in out.data_mut().as_chunks_mut::<4>().0.iter_mut().zip(&a) {
        p.copy_from_slice(&[c[0] as u8, c[1] as u8, c[2] as u8, 255]);
    }
    out
}

/// One running-sum box blur along rows (`across`) or columns.
fn box_pass(src: &[[u32; 3]], dst: &mut [[u32; 3]], w: usize, h: usize, r: usize, across: bool) {
    let (lines, len) = if across { (h, w) } else { (w, h) };
    let at = |line: usize, i: usize| if across { line * w + i } else { i * w + line };
    let n = (2 * r + 1) as u32;
    for line in 0..lines {
        let mut sum = [0u32; 3];
        // The edge pixel stands in for what lies past the edge.
        for k in 0..=2 * r {
            let i = k.saturating_sub(r).min(len - 1);
            for c in 0..3 {
                sum[c] += src[at(line, i)][c];
            }
        }
        for i in 0..len {
            for c in 0..3 {
                dst[at(line, i)][c] = sum[c] / n;
            }
            let out = i.saturating_sub(r);
            let inn = (i + r + 1).min(len - 1);
            for c in 0..3 {
                sum[c] = sum[c] + src[at(line, inn)][c] - src[at(line, out)][c];
            }
        }
    }
}
