//! The game's own mark: a heavy stencilled M on a gunmetal tile, split down
//! the middle by the meridian, a red-orange line. It is the program icon
//! (`assets/meridian.ico`, which this file writes). Design space 1000 units
//! square, x = 500 the meridian.

use super::paint::{hex, linear, poly, radial, Canvas};
use crate::ui::palette;
use glam::Vec2;
use tiny_skia::{Path, PathBuilder};

const AXIS: f32 = 500.0;
const TILE_TOP: u32 = 0x2A2E35;
const TILE_FOOT: u32 = 0x08090B;
const STEEL: u32 = 0xF2F2F0;
const STEEL_FOOT: u32 = 0xAEB4BC;

/// A rounded square `inset` in from the design box, corners of radius `r`.
fn tile(inset: f32, r: f32) -> Option<Path> {
    let (a, b) = (inset, 1000.0 - inset);
    let mut pb = PathBuilder::new();
    pb.move_to(a + r, a);
    pb.line_to(b - r, a);
    pb.quad_to(b, a, b, a + r);
    pb.line_to(b, b - r);
    pb.quad_to(b, b, b - r, b);
    pb.line_to(a + r, b);
    pb.quad_to(a, b, a, b - r);
    pb.line_to(a, a + r);
    pb.quad_to(a, a, a + r, a);
    pb.close();
    pb.finish()
}

/// The M's left half, mirrored for the right: a stem with its top outer
/// corner cut, and the diagonal running down to the meridian.
fn half(small: bool, mirror: bool) -> Option<Path> {
    // Heavier at small sizes, so the counters stay open and the strokes solid.
    let (stem, top, foot, v) = if small {
        (190.0, 250.0, 760.0, 690.0)
    } else {
        (212.0, 268.0, 742.0, 650.0)
    };
    let w = if small { 150.0 } else { 128.0 };
    let gap = if small { 0.0 } else { 20.0 };
    let pts = [
        Vec2::new(stem, foot),
        Vec2::new(stem, top + 60.0),
        Vec2::new(stem + 60.0, top),
        Vec2::new(stem + w, top),
        Vec2::new(AXIS - gap, v - 250.0),
        Vec2::new(AXIS - gap, v),
        Vec2::new(stem + w, v - 240.0),
        Vec2::new(stem + w, foot),
    ];
    let pts: Vec<Vec2> = pts
        .iter()
        .map(|p| {
            if mirror {
                Vec2::new(1000.0 - p.x, p.y)
            } else {
                *p
            }
        })
        .collect();
    poly(&pts)
}

/// The icon at `px` pixels square, straight-alpha RGBA. Below 40 px it drops
/// the stencil cuts and the rim light, which would only blur.
pub fn icon(px: usize) -> Vec<u8> {
    let small = px < 40;
    let mut c = Canvas::new([px, px], Vec2::splat(1000.0));
    let inset = if small { 20.0 } else { 36.0 };
    let body = tile(inset, 190.0);
    c.fill_with(
        &body,
        linear(
            Vec2::new(0.0, inset),
            Vec2::new(0.0, 1000.0 - inset),
            &[(0.0, hex(TILE_TOP, 1.0)), (1.0, hex(TILE_FOOT, 1.0))],
        ),
        None,
    );
    // Heat off the meridian's foot, as a lamp under the plate.
    let clip = c.mask(&body);
    c.fill_with(
        &body,
        radial(
            Vec2::new(AXIS, 900.0),
            520.0,
            &[
                (0.0, hex(palette::ACCENT_DEEP, 0.55)),
                (1.0, hex(palette::ACCENT_DEEP, 0.0)),
            ],
        ),
        clip.as_ref(),
    );
    if !small {
        c.stroke(&tile(inset + 6.0, 184.0), 10.0, 1.0, hex(0xFFFFFF, 0.10));
    }

    let steel = || {
        linear(
            Vec2::new(0.0, 250.0),
            Vec2::new(0.0, 760.0),
            &[(0.0, hex(STEEL, 1.0)), (1.0, hex(STEEL_FOOT, 1.0))],
        )
    };
    c.fill_with(&half(small, false), steel(), None);
    c.fill_with(&half(small, true), steel(), None);

    // The meridian: a line through the V, standing proud of the tile.
    let (from, to, weight) = if small {
        (130.0, 890.0, 70.0)
    } else {
        (120.0, 900.0, 34.0)
    };
    let line = poly(&[
        Vec2::new(AXIS - weight * 0.5, from),
        Vec2::new(AXIS + weight * 0.5, from),
        Vec2::new(AXIS + weight * 0.5, to),
        Vec2::new(AXIS - weight * 0.5, to),
    ]);
    if !small {
        let glow = poly(&[
            Vec2::new(AXIS - 60.0, from),
            Vec2::new(AXIS + 60.0, from),
            Vec2::new(AXIS + 60.0, to),
            Vec2::new(AXIS - 60.0, to),
        ]);
        c.fill_with(
            &glow,
            linear(
                Vec2::new(AXIS - 60.0, 0.0),
                Vec2::new(AXIS + 60.0, 0.0),
                &[
                    (0.0, hex(palette::ACCENT, 0.0)),
                    (0.5, hex(palette::ACCENT, 0.35)),
                    (1.0, hex(palette::ACCENT, 0.0)),
                ],
            ),
            clip.as_ref(),
        );
    }
    c.fill(&line, hex(palette::ACCENT, 1.0));
    c.into_rgba()
}

/// The sizes Windows asks an icon for: small and large icons at 100-250 %
/// scale, the taskbar, Explorer's views.
pub const SIZES: [usize; 9] = [16, 20, 24, 32, 40, 48, 64, 128, 256];

/// An `.ico` holding the icon at every size in `SIZES`, each image a PNG.
pub fn ico() -> Vec<u8> {
    let images: Vec<(usize, Vec<u8>)> = SIZES.iter().map(|&px| (px, png(px))).collect();
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len();
    for (px, data) in &images {
        // 256 is written as 0.
        let side = (*px % 256) as u8;
        out.extend_from_slice(&[side, side, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += data.len();
    }
    for (_, data) in images {
        out.extend_from_slice(&data);
    }
    out
}

fn png(px: usize) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, px as u32, px as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .and_then(|mut w| w.write_image_data(&icon(px)))
        .expect("a PNG into memory");
    out
}
