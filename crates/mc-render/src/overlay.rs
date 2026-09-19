//! 2D overlay batch: rectangles, lines and bitmap text in pixel coordinates.
//! The game fills one of these per frame; the renderer draws it in one call.

use crate::textures::{FONT_ATLAS_H, FONT_ATLAS_W};

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct OverlayVertex {
    pub pos: [f32; 2],
    /// Font atlas coordinates; negative x means a solid fill.
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

pub const MAX_OVERLAY_VERTICES: usize = 65_536;

/// Glyph cell in pixels at scale 1.
pub const GLYPH: f32 = 8.0;

#[derive(Default)]
pub struct Overlay {
    pub vertices: Vec<OverlayVertex>,
    /// Set when a frame wanted more than `MAX_OVERLAY_VERTICES`; shown by the profiler.
    pub overflowed: bool,
}

impl Overlay {
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.overflowed = false;
    }

    fn quad(&mut self, corners: [[f32; 2]; 4], uvs: [[f32; 2]; 4], color: [f32; 4]) {
        if self.vertices.len() + 6 > MAX_OVERLAY_VERTICES {
            self.overflowed = true;
            return;
        }
        for i in [0, 1, 2, 0, 2, 3] {
            self.vertices.push(OverlayVertex { pos: corners[i], uv: uvs[i], color });
        }
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.quad([[x, y], [x + w, y], [x + w, y + h], [x, y + h]], [[-1.0, 0.0]; 4], color);
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
        self.quad([[a[0] + nx, a[1] + ny], [b[0] + nx, b[1] + ny], [b[0] - nx, b[1] - ny], [a[0] - nx, a[1] - ny]], [[-1.0, 0.0]; 4], color);
    }

    /// Draws ASCII text; returns the x position after the last glyph.
    pub fn text(&mut self, x: f32, y: f32, scale: f32, color: [f32; 4], text: &str) -> f32 {
        let size = GLYPH * scale;
        let mut pen = x;
        for ch in text.chars() {
            let code = if ch.is_ascii() { ch as usize } else { b'?' as usize };
            if code != b' ' as usize {
                let (u0, v0) = ((code % 16) as f32 * 8.0 / FONT_ATLAS_W as f32, (code / 16) as f32 * 8.0 / FONT_ATLAS_H as f32);
                let (du, dv) = (8.0 / FONT_ATLAS_W as f32, 8.0 / FONT_ATLAS_H as f32);
                self.quad(
                    [[pen, y], [pen + size, y], [pen + size, y + size], [pen, y + size]],
                    [[u0, v0], [u0 + du, v0], [u0 + du, v0 + dv], [u0, v0 + dv]],
                    color,
                );
            }
            pen += size;
        }
        pen
    }

    /// Text with a one-pixel drop shadow, readable over terrain.
    pub fn label(&mut self, x: f32, y: f32, scale: f32, color: [f32; 4], text: &str) -> f32 {
        self.text(x + scale, y + scale, scale, [0.0, 0.0, 0.0, color[3] * 0.8], text);
        self.text(x, y, scale, color, text)
    }

    pub fn text_width(scale: f32, text: &str) -> f32 {
        text.chars().count() as f32 * GLYPH * scale
    }
}
