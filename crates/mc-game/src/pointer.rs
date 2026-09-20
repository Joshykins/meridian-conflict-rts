//! The mouse pointer says what a click would do: attack, assist, reclaim, pick an
//! order up, or nothing at all.
//!
//! The pointers are the HUD's own vector glyphs, drawn through the same `Ui` into
//! an overlay of their own and rasterised here, once, into cursors the window
//! system draws: a pointer in an RTS must not wait for the next frame. Where the
//! platform takes no custom cursors, the nearest system cursor stands in.

use crate::audio::Audio;
use crate::hud::icons::{self, Glyph};
use crate::ui::{self, palette, Color, Ui};
use glam::Vec2;
use mc_render::overlay::OverlayVertex;
use mc_render::Overlay;
use std::f32::consts::TAU;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Cursor, CursorIcon, CustomCursor, Window};

/// A pointer is drawn in a box this many points across, at the interface's scale.
const BOX: f32 = 32.0;
/// Rasterised this many times finer each way, then averaged down.
const SUPERSAMPLE: usize = 4;
/// How far the dark rim reaches around a shape, in points: what keeps a pointer
/// legible over snow as well as over the night side of the map.
const RIM: f32 = 1.3;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Pointer {
    /// Nothing in particular: the interface, open ground.
    #[default]
    Arrow,
    /// A click selects what is under the pointer.
    Select,
    Move,
    Attack,
    AttackMove,
    Assist,
    Reclaim,
    /// A structure (or, on the range, a unit) goes down here.
    Place,
    /// With shift held: the order under the pointer can be picked up.
    Grab,
    /// An order is in hand and can be put down here.
    Grabbing,
    /// The camera is being dragged.
    Pan,
    /// What is armed, placed or in hand cannot go here.
    Denied,
}

pub const ALL: [Pointer; 12] = [
    Pointer::Arrow,
    Pointer::Select,
    Pointer::Move,
    Pointer::Attack,
    Pointer::AttackMove,
    Pointer::Assist,
    Pointer::Reclaim,
    Pointer::Place,
    Pointer::Grab,
    Pointer::Grabbing,
    Pointer::Pan,
    Pointer::Denied,
];

impl Pointer {
    /// The point of the box that does the pointing, in points.
    fn hotspot(self) -> Vec2 {
        match self {
            Pointer::Arrow | Pointer::Select => Vec2::new(3.0, 2.0),
            _ => Vec2::splat(BOX * 0.5),
        }
    }

    /// The system cursor nearest in meaning.
    fn system(self) -> CursorIcon {
        match self {
            Pointer::Arrow => CursorIcon::Default,
            Pointer::Select => CursorIcon::Pointer,
            Pointer::Move | Pointer::Place => CursorIcon::Crosshair,
            Pointer::Attack | Pointer::AttackMove => CursorIcon::Crosshair,
            Pointer::Assist => CursorIcon::Copy,
            Pointer::Reclaim => CursorIcon::Alias,
            Pointer::Grab => CursorIcon::Grab,
            Pointer::Grabbing => CursorIcon::Grabbing,
            Pointer::Pan => CursorIcon::AllScroll,
            Pointer::Denied => CursorIcon::NotAllowed,
        }
    }

    /// Draws the pointer into its box. With `rim` set everything is that colour: the dark pass underneath.
    fn draw(self, ui: &mut Ui, rim: Option<Color>) {
        let tone = |hex: u32| rim.unwrap_or(ui::rgb(hex, 1.0));
        let c = Vec2::splat(BOX * 0.5);
        let arrow = |ui: &mut Ui, color: Color| {
            let (tip, heel, notch, wing) = (
                Vec2::new(3.0, 2.0),
                Vec2::new(3.0, 21.5),
                Vec2::new(8.2, 16.6),
                Vec2::new(16.2, 16.6),
            );
            ui.triangle(tip, heel, notch, color);
            ui.triangle(tip, notch, wing, color);
        };
        // Four chevrons on the axes, `at` from the centre, pointing out or in.
        let chevrons = |ui: &mut Ui, at: f32, size: f32, outward: bool, color: Color| {
            for d in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
                icons::arrow_head(
                    ui,
                    c + d * at,
                    if outward { d } else { -d },
                    size,
                    2.0,
                    color,
                );
            }
        };
        match self {
            Pointer::Arrow => arrow(ui, tone(palette::TEXT)),
            Pointer::Select => {
                arrow(ui, tone(palette::TEXT));
                // Selection brackets in the pointer's lee.
                let (m, h, arm, color) = (Vec2::new(22.5, 23.0), 5.5, 3.2, tone(palette::ACCENT));
                for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                    let corner = m + Vec2::new(sx, sy) * h;
                    ui.stroke(corner, corner - Vec2::new(sx * arm, 0.0), 1.8, color);
                    ui.stroke(corner, corner - Vec2::new(0.0, sy * arm), 1.8, color);
                }
            }
            Pointer::Move => {
                chevrons(ui, 5.0, 4.6, false, tone(palette::ACCENT));
                ui.disc(c, 1.5, tone(palette::TEXT));
            }
            Pointer::Attack => icons::glyph(ui, Glyph::Attack, c, 11.0, tone(palette::BAD)),
            Pointer::AttackMove => {
                // The glyph's reticle sits up and to the right of its middle: bring it under the hotspot.
                let r = 9.0;
                icons::glyph(
                    ui,
                    Glyph::AttackMove,
                    c + Vec2::new(-0.3, 0.3) * r,
                    r,
                    tone(palette::BAD),
                );
            }
            Pointer::Assist => icons::glyph(ui, Glyph::Assist, c, 10.5, tone(palette::WARN)),
            Pointer::Reclaim => {
                // The reclaim glyph in the beam's own colours: its three arrows heat from red
                // through orange to white as they chase each other round, like what goes up the beam.
                let (r, t) = (11.5, 2.0);
                for (i, heat) in [0xF2401F, 0xFF8A1E, 0xFFF0D2].into_iter().enumerate() {
                    let a = TAU * i as f32 / 3.0 - std::f32::consts::FRAC_PI_2;
                    let end = a + TAU / 3.0 - 0.35;
                    ui.arc(c, r * 0.78, a + 0.25, end, t, tone(heat));
                    icons::arrow_head(
                        ui,
                        c + Vec2::from_angle(end + 0.22) * r * 0.78,
                        Vec2::from_angle(end + std::f32::consts::FRAC_PI_2),
                        r * 0.38,
                        t,
                        tone(heat),
                    );
                }
                ui.disc(c, 1.6, tone(0xFFF0D2));
            }
            Pointer::Place => {
                for d in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
                    ui.stroke(c + d * 4.0, c + d * 11.0, 1.8, tone(palette::TEXT));
                }
                ui.disc(c, 1.3, tone(palette::ACCENT));
            }
            Pointer::Grab => {
                ui.arc(c, 3.6, 0.0, TAU, 1.8, tone(palette::TEXT));
                chevrons(ui, 12.0, 4.2, true, tone(palette::TEXT));
            }
            Pointer::Grabbing => {
                ui.disc(c, 3.4, tone(palette::WARN));
                chevrons(ui, 9.5, 4.2, true, tone(palette::TEXT));
            }
            Pointer::Pan => {
                for d in [Vec2::X, Vec2::Y] {
                    ui.stroke(c - d * 12.0, c + d * 12.0, 1.8, tone(palette::TEXT));
                }
                chevrons(ui, 12.5, 4.0, true, tone(palette::TEXT));
            }
            Pointer::Denied => {
                let color = tone(palette::BAD);
                ui.arc(c, 9.5, 0.0, TAU, 2.4, color);
                let d = Vec2::from_angle(TAU / 8.0) * 9.5;
                ui.stroke(c - d, c + d, 2.4, color);
            }
        }
    }

    /// The pointer as straight-alpha sRGB RGBA, `side` pixels square, and its hotspot in pixels.
    pub fn image(self, scale: f32) -> (Vec<u8>, usize, (usize, usize)) {
        let side = (BOX * scale).ceil() as usize;
        let fine = scale * SUPERSAMPLE as f32;
        let (mut overlay, mut memory) = (Overlay::default(), ui::Memory::default());
        let (input, audio) = (ui::Input::default(), Audio::silent());
        let mut ui = Ui::new(
            &mut overlay,
            &input,
            &mut memory,
            &audio,
            Vec2::splat(BOX * fine),
            1.0,
            0.0,
            0.0,
        );
        ui.s = fine;
        // The rim: the shape in ink, nudged every way round; then the shape itself.
        let ink = ui::rgb(palette::INK, 1.0);
        for i in 0..12 {
            ui.shift = Vec2::from_angle(TAU * i as f32 / 12.0) * RIM;
            self.draw(&mut ui, Some(ink));
        }
        ui.shift = Vec2::ZERO;
        self.draw(&mut ui, None);
        let rgba = rasterise(&overlay.vertices, side, SUPERSAMPLE);
        let hot = (self.hotspot() * scale).round();
        (rgba, side, (hot.x as usize, hot.y as usize))
    }
}

/// Solid overlay triangles to straight-alpha sRGB pixels. The triangles are in
/// `side * supersample` space; blending is linear-light, like the overlay's own.
fn rasterise(vertices: &[OverlayVertex], side: usize, supersample: usize) -> Vec<u8> {
    let n = side * supersample;
    // Premultiplied linear RGBA.
    let mut fine = vec![[0.0f32; 4]; n * n];
    let edge = |a: [f32; 2], b: [f32; 2], p: [f32; 2]| {
        (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
    };
    for tri in vertices.chunks_exact(3) {
        let (a, mut b, mut c) = (tri[0], tri[1], tri[2]);
        let mut area = edge(a.pos, b.pos, c.pos);
        if area == 0.0 {
            continue;
        }
        if area < 0.0 {
            (b, c) = (c, b);
            area = -area;
        }
        // A sample exactly on an edge two triangles share belongs to one of them only.
        let owns =
            |from: [f32; 2], to: [f32; 2]| to[1] > from[1] || (to[1] == from[1] && to[0] > from[0]);
        let inside = |e: f32, from: [f32; 2], to: [f32; 2]| e > 0.0 || (e == 0.0 && owns(from, to));
        let span = |i: usize| {
            let (lo, hi) = (
                a.pos[i].min(b.pos[i]).min(c.pos[i]),
                a.pos[i].max(b.pos[i]).max(c.pos[i]),
            );
            (
                lo.floor().max(0.0) as usize,
                (hi.ceil().max(0.0) as usize).min(n),
            )
        };
        let ((x0, x1), (y0, y1)) = (span(0), span(1));
        for y in y0..y1 {
            for x in x0..x1 {
                let p = [x as f32 + 0.5, y as f32 + 0.5];
                let (wa, wb, wc) = (
                    edge(b.pos, c.pos, p),
                    edge(c.pos, a.pos, p),
                    edge(a.pos, b.pos, p),
                );
                if !(inside(wa, b.pos, c.pos)
                    && inside(wb, c.pos, a.pos)
                    && inside(wc, a.pos, b.pos))
                {
                    continue;
                }
                let mix = |i: usize| (a.color[i] * wa + b.color[i] * wb + c.color[i] * wc) / area;
                let alpha = mix(3).clamp(0.0, 1.0);
                let dst = &mut fine[y * n + x];
                for i in 0..3 {
                    dst[i] = mix(i) * alpha + dst[i] * (1.0 - alpha);
                }
                dst[3] = alpha + dst[3] * (1.0 - alpha);
            }
        }
    }
    let encode = |c: f32| {
        if c <= 0.003_130_8 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        }
    };
    let mut out = Vec::with_capacity(side * side * 4);
    for y in 0..side {
        for x in 0..side {
            let mut sum = [0.0f32; 4];
            for sy in 0..supersample {
                for sx in 0..supersample {
                    let p = fine[(y * supersample + sy) * n + x * supersample + sx];
                    for i in 0..4 {
                        sum[i] += p[i];
                    }
                }
            }
            let alpha = sum[3] / (supersample * supersample) as f32;
            for i in 0..3 {
                let straight = if sum[3] > 0.0 { sum[i] / sum[3] } else { 0.0 };
                out.push((encode(straight.clamp(0.0, 1.0)) * 255.0).round() as u8);
            }
            out.push((alpha * 255.0).round() as u8);
        }
    }
    out
}

/// Every pointer in a row, over dark and over bright ground: what `--dump-cursors` writes.
pub fn sheet(scale: f32) -> (Vec<u8>, u32, u32) {
    let images: Vec<_> = ALL.iter().map(|p| p.image(scale)).collect();
    let side = images[0].1;
    let (cell, rows) = (
        side + side / 2,
        [[0x10u8, 0x1A, 0x22], [0x5E, 0x8A, 0x3C], [0xE4, 0xE8, 0xE6]],
    );
    let (w, h) = (cell * images.len(), cell * rows.len());
    let mut out = vec![0u8; w * h * 4];
    for (row, ground) in rows.iter().enumerate() {
        for y in 0..cell {
            for x in 0..w {
                let at = ((row * cell + y) * w + x) * 4;
                out[at..at + 4].copy_from_slice(&[ground[0], ground[1], ground[2], 255]);
            }
        }
        for (i, (rgba, _, hot)) in images.iter().enumerate() {
            let (ox, oy) = (i * cell + side / 4, row * cell + side / 4);
            for y in 0..side {
                for x in 0..side {
                    let (src, at) = (
                        &rgba[(y * side + x) * 4..][..4],
                        ((oy + y) * w + ox + x) * 4,
                    );
                    // The hotspot, marked so it can be checked against the drawing.
                    let src = if (x, y) == *hot {
                        &[255, 0, 255, 255]
                    } else {
                        src
                    };
                    let alpha = src[3] as f32 / 255.0;
                    for k in 0..3 {
                        out[at + k] = (src[k] as f32 * alpha + out[at + k] as f32 * (1.0 - alpha))
                            .round() as u8;
                    }
                }
            }
        }
    }
    (out, w as u32, h as u32)
}

/// The window's cursors: built once per interface scale, changed only when the pointer does.
#[derive(Default)]
pub struct Cursors {
    /// By `ALL`'s order; `None` where the platform would not take the image.
    built: Vec<Option<CustomCursor>>,
    /// The scale `built` was drawn at, in quarters.
    quarters: u32,
    shown: Option<Pointer>,
}

impl Cursors {
    /// `scale` is the interface's: pixels per point.
    pub fn show(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: &Window,
        pointer: Pointer,
        scale: f32,
    ) {
        let quarters = (scale.clamp(1.0, 3.0) * 4.0).round() as u32;
        if quarters != self.quarters {
            self.quarters = quarters;
            self.shown = None;
            self.built = ALL
                .iter()
                .map(|p| {
                    let (rgba, side, hot) = p.image(quarters as f32 / 4.0);
                    match CustomCursor::from_rgba(
                        rgba,
                        side as u16,
                        side as u16,
                        hot.0 as u16,
                        hot.1 as u16,
                    ) {
                        Ok(source) => Some(event_loop.create_custom_cursor(source)),
                        Err(e) => {
                            log::warn!("no custom cursor for {p:?}: {e}");
                            None
                        }
                    }
                })
                .collect();
        }
        if self.shown != Some(pointer) {
            self.shown = Some(pointer);
            let index = ALL.iter().position(|p| *p == pointer).unwrap_or(0);
            match self.built.get(index).cloned().flatten() {
                Some(cursor) => window.set_cursor(cursor),
                None => window.set_cursor(Cursor::Icon(pointer.system())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pointer_draws_inside_its_box_with_its_hotspot() {
        for pointer in ALL {
            let (rgba, side, hot) = pointer.image(1.5);
            assert_eq!(rgba.len(), side * side * 4);
            assert!(
                hot.0 < side && hot.1 < side,
                "{pointer:?} points outside its box"
            );
            let inked = rgba.chunks_exact(4).filter(|p| p[3] > 200).count();
            assert!(
                inked > 60,
                "{pointer:?} is all but invisible: {inked} solid pixels"
            );
            // Nothing is cut off: the outermost ring of pixels stays clear.
            let edge = (0..side)
                .flat_map(|i| [(i, 0), (i, side - 1), (0, i), (side - 1, i)])
                .filter(|&(x, y)| rgba[(y * side + x) * 4 + 3] > 8)
                .count();
            assert_eq!(edge, 0, "{pointer:?} runs into the edge of its box");
        }
    }

    #[test]
    fn a_shared_edge_is_covered_once() {
        // A half-transparent square as two triangles: the diagonal runs through sample points.
        let mut overlay = Overlay::default();
        overlay.quad_fill(
            [[2.0, 2.0], [6.0, 2.0], [6.0, 6.0], [2.0, 6.0]],
            [1.0, 1.0, 1.0, 0.5],
        );
        let rgba = rasterise(&overlay.vertices, 8, 1);
        for y in 2..6 {
            for x in 2..6 {
                assert_eq!(rgba[(y * 8 + x) * 4 + 3], 128, "pixel {x},{y}");
            }
        }
        assert_eq!(rgba[3], 0);
    }
}
