//! The map's crags: its steep walls outside the desert, which the terrain shader
//! breaks into rock (`crag_relief` in rock.wgsl). The heightfield draws a
//! mountain wall as a smooth sheet a few 8 m cells across; the shader raises and
//! lowers it into buttresses, gullies and, near the eye, fractured facets. The
//! canyon's walls have their own rock (`renderer/cliff_rocks.rs`).
//!
//! Two things, made once per map at load: the crag field, how much of a crag the
//! ground is (0-1, in the ground cover's ways layer, `ground_crag_at`), and the
//! blocks of terrain drawn finer than its samples for the facets (`CliffBlocks`).

use crate::ground_cover::GroundCover;
use glam::Vec2;
use mc_data::weather::{Climate, MapLook};
use mc_map::{MapFile, CELL_SIZE_M, TILE_CELLS, TILE_SAMPLES};

/// A block's edge, metres: the smallest terrain node (64 cells of 2 m).
const BLOCK: f32 = 128.0;
/// Slopes (rise over run, over 16 m either way) from where the ground starts to
/// be a crag to where it is all crag.
const CRAG_FROM: f32 = 0.6;
const CRAG_TO: f32 = 1.1;
/// The field is softened over about this many metres, so the relief comes in
/// gently at a crag's foot and rim.
const SOFTEN_M: f32 = 16.0;

/// The crag field over `(w, h)` texels `cell` metres across, 0-1: the steepest
/// ground in each texel, softened. Read tile by tile, so no more than one tile's
/// samples are held at once.
pub(crate) fn crag_field(map: &MapFile, cell: f32, (w, h): (usize, usize)) -> Vec<f32> {
    let info = map.info();
    let (min_z, step) = (info.min_z.to_f32(), info.z_step.to_f32());
    let (tiles_w, tiles_h) = map.size_tiles();
    let metres = CELL_SIZE_M as f32;
    let mut field = vec![0f32; w * h];
    let n = TILE_SAMPLES as usize;
    let mut samples = vec![0u16; n * n];
    for ty in 0..tiles_h {
        for tx in 0..tiles_w {
            if map.read_tile_into(tx, ty, &mut samples).is_err() {
                continue;
            }
            let at =
                |x: usize, y: usize| min_z + samples[y.min(n - 1) * n + x.min(n - 1)] as f32 * step;
            for y in 0..TILE_CELLS as usize {
                for x in 0..TILE_CELLS as usize {
                    let (x0, x1, y0, y1) = (x.saturating_sub(2), x + 2, y.saturating_sub(2), y + 2);
                    let gx = (at(x1, y) - at(x0, y)) / ((x1 - x0) as f32 * metres);
                    let gy = (at(x, y1) - at(x, y0)) / ((y1 - y0) as f32 * metres);
                    let crag = smoothstep(CRAG_FROM, CRAG_TO, Vec2::new(gx, gy).length());
                    if crag <= 0.0 {
                        continue;
                    }
                    let p = Vec2::new(
                        (tx * TILE_CELLS) as f32 + x as f32,
                        (ty * TILE_CELLS) as f32 + y as f32,
                    ) * metres;
                    let (cx, cy) = ((p.x / cell) as usize, (p.y / cell) as usize);
                    if cx < w && cy < h {
                        let t = &mut field[cy * w + cx];
                        *t = t.max(crag);
                    }
                }
            }
        }
    }
    crate::ground_cover::blur(
        &mut field,
        w,
        h,
        ((SOFTEN_M / cell).round() as usize).max(1),
    );
    field
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub struct CliffBlocks {
    w: usize,
    h: usize,
    /// Per block: whether any of it is crag.
    crag: Vec<bool>,
    /// Per block: drawn finer. Crag blocks outside the desert and every block
    /// next to one, so the finer patches end a block short of any relief and
    /// meet the plain terrain on its own undisplaced surface.
    fine: Vec<bool>,
}

impl CliffBlocks {
    /// The blocks with any crag in `cover`'s field, for a map `size` metres across.
    pub fn new(cover: &GroundCover, size: Vec2) -> Self {
        let (w, h) = (
            (size.x / BLOCK).ceil() as usize,
            (size.y / BLOCK).ceil() as usize,
        );
        let mut crag = vec![false; w * h];
        let (cw, ch) = (cover.width as usize, cover.height as usize);
        let cell = size / Vec2::new(cw as f32, ch as f32);
        for y in 0..ch {
            for x in 0..cw {
                if cover.ways[(y * cw + x) * 4 + 3] == 0 {
                    continue;
                }
                let mid = (Vec2::new(x as f32, y as f32) + 0.5) * cell;
                let (bx, by) = ((mid.x / BLOCK) as usize, (mid.y / BLOCK) as usize);
                if bx < w && by < h {
                    crag[by * w + bx] = true;
                }
            }
        }
        let mut blocks = Self {
            w,
            h,
            crag,
            fine: Vec::new(),
        };
        blocks.set_look(&MapLook::default());
        blocks
    }

    /// Leaves the desert's walls at 8 m: their rock is the canyon's pieces.
    pub fn set_look(&mut self, look: &MapLook) {
        self.fine = vec![false; self.w * self.h];
        for by in 0..self.h {
            for bx in 0..self.w {
                let mid = (Vec2::new(bx as f32, by as f32) + 0.5) * BLOCK;
                if !self.crag[by * self.w + bx] || look.climate_at(mid.x, mid.y) == Climate::Desert
                {
                    continue;
                }
                for y in by.saturating_sub(1)..(by + 2).min(self.h) {
                    for x in bx.saturating_sub(1)..(bx + 2).min(self.w) {
                        self.fine[y * self.w + x] = true;
                    }
                }
            }
        }
    }

    /// Whether any block under the square at `origin`, `size` metres a side, is
    /// drawn finer.
    pub fn any_fine(&self, origin: Vec2, size: f32) -> bool {
        let lo = (origin / BLOCK).floor().max(Vec2::ZERO);
        let hi = ((origin + Vec2::splat(size)) / BLOCK).ceil();
        let (x1, y1) = ((hi.x as usize).min(self.w), (hi.y as usize).min(self.h));
        (lo.y as usize..y1).any(|y| (lo.x as usize..x1).any(|x| self.fine[y * self.w + x]))
    }

    /// Every block of a map `size` metres across drawn finer: all of it crag.
    #[cfg(test)]
    pub fn all(size: Vec2) -> Self {
        let (w, h) = (
            (size.x / BLOCK).ceil() as usize,
            (size.y / BLOCK).ceil() as usize,
        );
        Self {
            w,
            h,
            crag: vec![true; w * h],
            fine: vec![true; w * h],
        }
    }

    /// No block drawn finer: a map with no crags, or a test's.
    #[cfg(test)]
    pub fn none() -> Self {
        Self {
            w: 0,
            h: 0,
            crag: Vec::new(),
            fine: Vec::new(),
        }
    }
}
