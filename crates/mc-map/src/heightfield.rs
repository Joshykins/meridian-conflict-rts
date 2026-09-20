//! The simulation's terrain: every height sample of the map in one array.
//!
//! Integer-only. A cell is 8 m, which is `1 << 19` in raw `Fx`, so splitting a
//! position into a cell index and an in-cell fraction is a shift and a mask and
//! loses nothing. The renderer draws each cell as two triangles split along the
//! diagonal from `(x, y)` to `(x + 1, y + 1)`; every query here evaluates that
//! same surface, so a unit placed at `height_at` touches the visible ground.
//!
//! The heights are static input apart from structure flattening. The sim does
//! not hash 200 MB per tick; it keeps the [`FlattenRecord`]s in a table and
//! hashes those. A snapshot restores terrain by replaying them in order.

use crate::file::MapFile;
use crate::format::{MapError, MapInfo};
use crate::{
    CELL_SIZE_M, DEFAULT_MIN_Z, DEFAULT_Z_STEP, TILE_CELLS, TILE_SAMPLES, TILE_SAMPLE_COUNT,
};
use mc_core::{Fx, FxVec2, FxVec3, StateHasher};
use std::sync::Mutex;

const CELL_SHIFT: u32 = 19;
const CELL_RAW: i64 = 1 << CELL_SHIFT;
const _: () = assert!(Fx::from_int(CELL_SIZE_M).0 == CELL_RAW);

/// Longest ray [`Heightfield::raycast`] tests, per axis, in metres. A longer
/// segment is cut to this length (and is a bug in the caller: debug builds
/// assert). Projectiles cover a few hundred metres per tick at most; anything
/// longer must be split by the caller.
pub const RAYCAST_MAX_LENGTH_M: i32 = 1024;

/// Hard cap on the surface pieces one raycast visits. A ray of the maximum
/// length crosses at most `1024 / 8` lines of each of the two grid directions
/// and twice that many cell diagonals, so the cap never cuts a legal ray short;
/// it exists so the loop bound is visible.
pub const RAYCAST_MAX_STEPS: u32 = 4 * (RAYCAST_MAX_LENGTH_M / CELL_SIZE_M) as u32 + 4;

/// Ray parameter resolution: `t` runs from 0 to `1 << 30` along the segment.
const T_SHIFT: u32 = 30;
const T_ONE: i64 = 1 << T_SHIFT;

/// Edge of the coarse max-height grid used to reject rays early, in cells.
const BLOCK_CELLS: usize = 32;
/// A ray whose bounding box covers more blocks than this skips the early-out.
const MAX_EARLY_OUT_BLOCKS: usize = 64;

/// One structure's terrain edit: every sample touching the cells
/// `min..=max` was set to `sample`. Small, plain and order-dependent: the sim
/// stores these in a table (and hashes it), the renderer patches its GPU tiles
/// from the same records, and snapshots replay them through
/// [`Heightfield::apply_flatten`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct FlattenRecord {
    pub min_x: u16,
    pub min_y: u16,
    /// Inclusive.
    pub max_x: u16,
    /// Inclusive.
    pub max_y: u16,
    pub sample: u16,
}

impl FlattenRecord {
    /// Named like the sim tables' `hash`; `std::hash::Hash` is deliberately not
    /// implemented, because state hashing must not depend on a `Hasher` impl.
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u64(
            self.min_x as u64
                | (self.min_y as u64) << 16
                | (self.max_x as u64) << 32
                | (self.max_y as u64) << 48,
        );
        h.write_u32(self.sample as u32);
    }

    /// Sample columns and rows covered, inclusive: one more than the cells,
    /// because a cell's far corners belong to it too.
    #[inline]
    pub fn sample_rect(&self) -> ((u32, u32), (u32, u32)) {
        (
            (self.min_x as u32, self.min_y as u32),
            (self.max_x as u32 + 1, self.max_y as u32 + 1),
        )
    }
}

pub struct Heightfield {
    w: usize,
    h: usize,
    stride: usize,
    samples: Vec<u16>,
    /// Upper bound of the samples in each 32 x 32 cell block. Flattening only
    /// ever raises an entry, so it stays a bound without rescanning.
    block_max: Vec<u16>,
    blocks_w: usize,
    min_z: Fx,
    z_step: Fx,
    water_level: Fx,
}

/// A position resolved to a cell and the fraction inside it (`0..=CELL_RAW`).
struct Located {
    index: usize,
    fx: i64,
    fy: i64,
}

impl Heightfield {
    /// Loads every tile. Tiles are decoded on all cores; the result does not
    /// depend on how many there are.
    pub fn load(map: &MapFile) -> Result<Heightfield, MapError> {
        let info = map.info();
        let (w, h) = info.size_cells();
        let stride = w as usize + 1;
        let mut samples = vec![0u16; stride * (h as usize + 1)];

        // One band of rows per tile row. A band leaves its tiles' last sample
        // row to the band above it, so bands are disjoint slices and need no
        // synchronisation. Only the top band keeps its last row.
        let mut bands = Vec::with_capacity(info.tiles_h as usize);
        let mut rest = &mut samples[..];
        for ty in 0..info.tiles_h {
            let rows = if ty + 1 == info.tiles_h {
                TILE_SAMPLES
            } else {
                TILE_CELLS
            } as usize;
            let (band, tail) = std::mem::take(&mut rest).split_at_mut(rows * stride);
            bands.push((ty, band));
            rest = tail;
        }
        let threads = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .min(bands.len());
        let queue = Mutex::new(bands);
        let failure = Mutex::new(None);
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| {
                    let mut tile = vec![0u16; TILE_SAMPLE_COUNT];
                    loop {
                        let Some((ty, band)) = queue.lock().unwrap().pop() else {
                            break;
                        };
                        for tx in 0..info.tiles_w {
                            if let Err(e) = map.read_tile_into(tx, ty, &mut tile) {
                                *failure.lock().unwrap() = Some(e);
                                return;
                            }
                            let x0 = (tx * TILE_CELLS) as usize;
                            let n = TILE_SAMPLES as usize;
                            for (dst, src) in
                                band.chunks_exact_mut(stride).zip(tile.chunks_exact(n))
                            {
                                dst[x0..x0 + n].copy_from_slice(src);
                            }
                        }
                    }
                });
            }
        });
        if let Some(e) = failure.into_inner().unwrap() {
            return Err(e);
        }
        Ok(Self::assemble(
            w as usize,
            h as usize,
            samples,
            info.min_z,
            info.z_step,
            info.water_level,
        ))
    }

    /// Builds a heightfield from samples in memory: `(w_cells + 1) * (h_cells + 1)`
    /// values, row-major. For tests and tools that do not want a file.
    pub fn from_samples(
        w_cells: u32,
        h_cells: u32,
        samples: Vec<u16>,
        min_z: Fx,
        z_step: Fx,
        water_level: Fx,
    ) -> Heightfield {
        assert!(w_cells >= 1 && h_cells >= 1 && w_cells < 1 << 16 && h_cells < 1 << 16);
        assert_eq!(
            samples.len(),
            (w_cells as usize + 1) * (h_cells as usize + 1)
        );
        assert!(z_step > Fx::ZERO && z_step <= Fx::from_int(16));
        Self::assemble(
            w_cells as usize,
            h_cells as usize,
            samples,
            min_z,
            z_step,
            water_level,
        )
    }

    /// Level ground at `height`, in the default height encoding, water well below it.
    pub fn flat(w_cells: u32, h_cells: u32, height: Fx) -> Heightfield {
        let info = MapInfo {
            name: String::new(),
            tiles_w: 1,
            tiles_h: 1,
            min_z: DEFAULT_MIN_Z,
            z_step: DEFAULT_Z_STEP,
            water_level: DEFAULT_MIN_Z,
        };
        let count = (w_cells as usize + 1) * (h_cells as usize + 1);
        let samples = vec![info.height_to_sample(height); count];
        Self::from_samples(
            w_cells,
            h_cells,
            samples,
            info.min_z,
            info.z_step,
            info.water_level,
        )
    }

    fn assemble(
        w: usize,
        h: usize,
        samples: Vec<u16>,
        min_z: Fx,
        z_step: Fx,
        water_level: Fx,
    ) -> Heightfield {
        let stride = w + 1;
        let (blocks_w, blocks_h) = (w.div_ceil(BLOCK_CELLS), h.div_ceil(BLOCK_CELLS));
        let mut block_max = vec![0u16; blocks_w * blocks_h];
        for (y, row) in samples.chunks_exact(stride).enumerate() {
            // A sample on a block border bounds the blocks on both sides of it.
            let by_hi = (y / BLOCK_CELLS).min(blocks_h - 1);
            let by_lo = (y.saturating_sub(1) / BLOCK_CELLS).min(blocks_h - 1);
            for bx in 0..blocks_w {
                let x0 = bx * BLOCK_CELLS;
                let x1 = (x0 + BLOCK_CELLS).min(w);
                let m = row[x0..=x1].iter().copied().max().unwrap_or(0);
                for by in [by_lo, by_hi] {
                    let b = &mut block_max[by * blocks_w + bx];
                    *b = (*b).max(m);
                }
            }
        }
        Heightfield {
            w,
            h,
            stride,
            samples,
            block_max,
            blocks_w,
            min_z,
            z_step,
            water_level,
        }
    }

    #[inline]
    pub fn size_cells(&self) -> (u32, u32) {
        (self.w as u32, self.h as u32)
    }

    #[inline]
    pub fn size_metres(&self) -> FxVec2 {
        FxVec2::from_ints(self.w as i32 * CELL_SIZE_M, self.h as i32 * CELL_SIZE_M)
    }

    #[inline]
    pub fn water_level(&self) -> Fx {
        self.water_level
    }

    /// `0 <= p < size` on both axes, so [`Heightfield::cell_at`] of an
    /// in-bounds position is its true cell.
    #[inline]
    pub fn in_bounds(&self, p: FxVec2) -> bool {
        let size = self.size_metres();
        p.x >= Fx::ZERO && p.y >= Fx::ZERO && p.x < size.x && p.y < size.y
    }

    /// Cell under `p`, clamped to the map.
    #[inline]
    pub fn cell_at(&self, p: FxVec2) -> (u32, u32) {
        let cx = (p.x.0 >> CELL_SHIFT).clamp(0, self.w as i64 - 1);
        let cy = (p.y.0 >> CELL_SHIFT).clamp(0, self.h as i64 - 1);
        (cx as u32, cy as u32)
    }

    /// Raw sample at a grid vertex, `0..=w_cells` by `0..=h_cells`.
    #[inline]
    pub fn sample(&self, cx: u32, cy: u32) -> u16 {
        debug_assert!(cx as usize <= self.w && cy as usize <= self.h);
        self.samples[cy as usize * self.stride + cx as usize]
    }

    #[inline]
    pub fn sample_height(&self, cx: u32, cy: u32) -> Fx {
        self.decode(self.sample(cx, cy))
    }

    /// All samples, row-major, `w_cells + 1` per row.
    #[inline]
    pub fn samples(&self) -> &[u16] {
        &self.samples
    }

    #[inline]
    pub fn sample_to_height(&self, sample: u16) -> Fx {
        self.decode(sample)
    }

    /// Nearest sample value for a height, clamped to the encodable range.
    pub fn height_to_sample(&self, z: Fx) -> u16 {
        let steps = (z.0 - self.min_z.0 + self.z_step.0 / 2).div_euclid(self.z_step.0);
        steps.clamp(0, u16::MAX as i64) as u16
    }

    #[inline]
    fn decode(&self, sample: u16) -> Fx {
        Fx(self.min_z.0 + self.z_step.0 * sample as i64)
    }

    #[inline]
    fn locate(&self, p: FxVec2) -> Located {
        let x = p.x.0.clamp(0, (self.w as i64) << CELL_SHIFT);
        let y = p.y.0.clamp(0, (self.h as i64) << CELL_SHIFT);
        let (mut cx, mut fx) = ((x >> CELL_SHIFT) as usize, x & (CELL_RAW - 1));
        let (mut cy, mut fy) = ((y >> CELL_SHIFT) as usize, y & (CELL_RAW - 1));
        // The far edge belongs to the last cell, at fraction one.
        if cx == self.w {
            (cx, fx) = (self.w - 1, CELL_RAW);
        }
        if cy == self.h {
            (cy, fy) = (self.h - 1, CELL_RAW);
        }
        Located {
            index: cy * self.stride + cx,
            fx,
            fy,
        }
    }

    /// Corner samples of the cell at `index`: `(s00, s10, s01, s11)`.
    #[inline]
    fn corners(&self, index: usize) -> (i64, i64, i64, i64) {
        let s = &self.samples;
        (
            s[index] as i64,
            s[index + 1] as i64,
            s[index + self.stride] as i64,
            s[index + self.stride + 1] as i64,
        )
    }

    /// Height of the rendered surface under `p`. Positions outside the map
    /// take the height of the nearest edge point.
    pub fn height_at(&self, p: FxVec2) -> Fx {
        let l = self.locate(p);
        let (s00, s10, s01, s11) = self.corners(l.index);
        // Sample value in Q19. On the diagonal both branches reduce to
        // `s00 + (s11 - s00) * f`, so the surface has no seam there.
        let q = if l.fx >= l.fy {
            (s00 << CELL_SHIFT) + (s10 - s00) * l.fx + (s11 - s10) * l.fy
        } else {
            (s00 << CELL_SHIFT) + (s01 - s00) * l.fy + (s11 - s01) * l.fx
        };
        Fx(self.min_z.0 + ((self.z_step.0 * q) >> CELL_SHIFT))
    }

    /// `(dz/dx, dz/dy)` of the triangle under `p`: rise per metre along each axis.
    pub fn gradient_at(&self, p: FxVec2) -> FxVec2 {
        let l = self.locate(p);
        let (s00, s10, s01, s11) = self.corners(l.index);
        let (dx, dy) = if l.fx >= l.fy {
            (s10 - s00, s11 - s10)
        } else {
            (s11 - s01, s01 - s00)
        };
        self.gradient(dx, dy)
    }

    /// Steepness under `p` as rise over run: 0 is level, 1 is 45 degrees.
    #[inline]
    pub fn slope_at(&self, p: FxVec2) -> Fx {
        self.gradient_at(p).length()
    }

    /// Unit surface normal under `p`.
    pub fn normal_at(&self, p: FxVec2) -> FxVec3 {
        let g = self.gradient_at(p);
        FxVec3::new(-g.x, -g.y, Fx::ONE).normalize()
    }

    /// Steeper of a cell's two triangles, as rise over run. Passability wants
    /// one number per path cell.
    pub fn cell_slope(&self, cx: u32, cy: u32) -> Fx {
        debug_assert!((cx as usize) < self.w && (cy as usize) < self.h);
        let (s00, s10, s01, s11) = self.corners(cy as usize * self.stride + cx as usize);
        let lower = self.gradient(s10 - s00, s11 - s10).length();
        let upper = self.gradient(s11 - s01, s01 - s00).length();
        lower.max(upper)
    }

    /// Sample differences across one cell to a slope. Truncating division keeps
    /// mirrored terrain exactly mirrored.
    #[inline]
    fn gradient(&self, dx: i64, dy: i64) -> FxVec2 {
        let cell = CELL_SIZE_M as i64;
        FxVec2::new(Fx(dx * self.z_step.0 / cell), Fx(dy * self.z_step.0 / cell))
    }

    /// First point at which the segment `from -> to` is on or under the
    /// terrain, or `None` if it stays above it.
    ///
    /// Exact against the rendered surface: the segment is cut at every grid
    /// line and cell diagonal it crosses, between two cuts the ground under it
    /// is one flat triangle, and there the crossing is a linear solve. The
    /// returned point lies on the surface. A segment that starts under the
    /// terrain returns `from`. Outside the map the edge heights continue.
    ///
    /// Work is bounded by [`RAYCAST_MAX_STEPS`]; see [`RAYCAST_MAX_LENGTH_M`]
    /// for what happens to longer segments.
    pub fn raycast(&self, from: FxVec3, to: FxVec3) -> Option<FxVec3> {
        let mut d = to - from;
        let longest = d.x.abs().max(d.y.abs()).max(d.z.abs());
        let limit = Fx::from_int(RAYCAST_MAX_LENGTH_M);
        debug_assert!(
            longest <= limit,
            "raycast segment longer than RAYCAST_MAX_LENGTH_M; split it"
        );
        if longest > limit {
            d = FxVec3::new(
                d.x.mul_div(limit.0, longest.0),
                d.y.mul_div(limit.0, longest.0),
                d.z.mul_div(limit.0, longest.0),
            );
        }

        if self.above_everything_near(from, d) {
            return None;
        }

        // With |d| <= 2^26 raw and t <= 2^30 every product below fits an i64.
        let point = |t: i64| {
            FxVec3::new(
                Fx(from.x.0 + ((d.x.0 * t) >> T_SHIFT)),
                Fx(from.y.0 + ((d.y.0 * t) >> T_SHIFT)),
                Fx(from.z.0 + ((d.z.0 * t) >> T_SHIFT)),
            )
        };
        let clearance = |t: i64| {
            let p = point(t);
            p.z.0 - self.height_at(p.xy()).0
        };

        let mut t0 = 0i64;
        let mut c0 = clearance(0);
        if c0 <= 0 {
            return Some(from);
        }
        let mut lines = [
            GridLines::new(from.x.0, d.x.0),
            GridLines::new(from.y.0, d.y.0),
            GridLines::new(from.x.0 - from.y.0, d.x.0 - d.y.0),
        ];
        for _ in 0..RAYCAST_MAX_STEPS {
            let t1 = lines.iter().map(|l| l.next_t).min().unwrap();
            for l in &mut lines {
                if l.next_t == t1 && t1 < T_ONE {
                    l.advance();
                }
            }
            let c1 = clearance(t1);
            if c1 <= 0 {
                // Linear between the cuts: c0 > 0 >= c1.
                let t = t0 + ((t1 - t0) as i128 * c0 as i128 / (c0 as i128 - c1 as i128)) as i64;
                let hit = point(t);
                return Some(FxVec3::new(hit.x, hit.y, self.height_at(hit.xy())));
            }
            if t1 >= T_ONE {
                return None;
            }
            (t0, c0) = (t1, c1);
        }
        None
    }

    /// Cheap rejection for the common case of a projectile high in the air:
    /// true when the whole segment is above every block its bounding box touches.
    fn above_everything_near(&self, from: FxVec3, d: FxVec3) -> bool {
        let to = from + d;
        let (lo_x, lo_y) = self.cell_at(FxVec2::new(from.x.min(to.x), from.y.min(to.y)));
        let (hi_x, hi_y) = self.cell_at(FxVec2::new(from.x.max(to.x), from.y.max(to.y)));
        let (bx0, bx1) = (lo_x as usize / BLOCK_CELLS, hi_x as usize / BLOCK_CELLS);
        let (by0, by1) = (lo_y as usize / BLOCK_CELLS, hi_y as usize / BLOCK_CELLS);
        if (bx1 - bx0 + 1) * (by1 - by0 + 1) > MAX_EARLY_OUT_BLOCKS {
            return false;
        }
        let mut highest = 0u16;
        for by in by0..=by1 {
            let row = &self.block_max[by * self.blocks_w..];
            highest = highest.max(row[bx0..=bx1].iter().copied().max().unwrap_or(0));
        }
        from.z.min(to.z) > self.decode(highest)
    }

    /// Levels the ground under a structure: every sample touching the cells
    /// `min_cell..=max_cell` is set to one value. `target` defaults to the
    /// rounded mean of those samples, which moves the least earth. The rect is
    /// clamped to the map. Keep the returned record: it is the game state.
    pub fn flatten_rect(
        &mut self,
        min_cell: (u32, u32),
        max_cell: (u32, u32),
        target: Option<Fx>,
    ) -> FlattenRecord {
        debug_assert!(min_cell.0 <= max_cell.0 && min_cell.1 <= max_cell.1);
        let (last_x, last_y) = (self.w as u32 - 1, self.h as u32 - 1);
        let max_x = max_cell.0.min(last_x);
        let max_y = max_cell.1.min(last_y);
        let min_x = min_cell.0.min(max_x);
        let min_y = min_cell.1.min(max_y);
        let sample = match target {
            Some(z) => self.height_to_sample(z),
            None => {
                let mut sum = 0u64;
                for y in min_y as usize..=max_y as usize + 1 {
                    let row = &self.samples[y * self.stride..];
                    sum += row[min_x as usize..=max_x as usize + 1]
                        .iter()
                        .map(|&s| s as u64)
                        .sum::<u64>();
                }
                let count = (max_x - min_x + 2) as u64 * (max_y - min_y + 2) as u64;
                ((sum + count / 2) / count) as u16
            }
        };
        let record = FlattenRecord {
            min_x: min_x as u16,
            min_y: min_y as u16,
            max_x: max_x as u16,
            max_y: max_y as u16,
            sample,
        };
        self.apply_flatten(&record);
        record
    }

    /// Replays a record from [`Heightfield::flatten_rect`]. Records must be
    /// applied in the order they were made: overlapping ones do not commute.
    pub fn apply_flatten(&mut self, r: &FlattenRecord) {
        let max_x = (r.max_x as usize).min(self.w - 1);
        let max_y = (r.max_y as usize).min(self.h - 1);
        let (min_x, min_y) = ((r.min_x as usize).min(max_x), (r.min_y as usize).min(max_y));
        for y in min_y..=max_y + 1 {
            let row = &mut self.samples[y * self.stride..];
            row[min_x..=max_x + 1].fill(r.sample);
        }
        // The edge samples are shared with the cells just outside the rect.
        let blocks_h = self.block_max.len() / self.blocks_w;
        let bx1 = ((max_x + 1) / BLOCK_CELLS).min(self.blocks_w - 1);
        let by1 = ((max_y + 1) / BLOCK_CELLS).min(blocks_h - 1);
        for by in min_y.saturating_sub(1) / BLOCK_CELLS..=by1 {
            for bx in min_x.saturating_sub(1) / BLOCK_CELLS..=bx1 {
                let b = &mut self.block_max[by * self.blocks_w + bx];
                *b = (*b).max(r.sample);
            }
        }
    }
}

/// One family of parallel lines (x = 8k, y = 8k or x - y = 8k) crossed by a
/// ray; yields the ray parameter of each crossing in order.
struct GridLines {
    origin: i64,
    delta: i64,
    line: i64,
    next_t: i64,
}

impl GridLines {
    fn new(origin: i64, delta: i64) -> GridLines {
        // First line strictly ahead of the origin in the direction of travel.
        let line = if delta > 0 {
            ((origin >> CELL_SHIFT) + 1) << CELL_SHIFT
        } else {
            (-((-origin) >> CELL_SHIFT) - 1) << CELL_SHIFT
        };
        let mut lines = GridLines {
            origin,
            delta,
            line,
            next_t: T_ONE,
        };
        lines.update();
        lines
    }

    fn advance(&mut self) {
        self.line += if self.delta > 0 { CELL_RAW } else { -CELL_RAW };
        self.update();
    }

    /// Each crossing is computed from the line itself, not accumulated, so
    /// rounding does not build up along the ray.
    fn update(&mut self) {
        self.next_t = if self.delta == 0 {
            T_ONE
        } else {
            (((self.line - self.origin) << T_SHIFT) / self.delta).min(T_ONE)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::baked_4km;
    use mc_core::Rng;

    const W: u32 = 64;

    /// Flat ground at 10 m with a 100 m cone, 160 m in radius, centred on cell vertex (32, 32).
    fn hill() -> Heightfield {
        let flat = Heightfield::flat(W, W, Fx::from_int(10));
        let mut samples = flat.samples().to_vec();
        for y in 0..=W as i64 {
            for x in 0..=W as i64 {
                let d2 = ((x - 32) * (x - 32) + (y - 32) * (y - 32)) * 64;
                let dist = Fx::from_int(d2 as i32).sqrt();
                let z = Fx::from_int(100) - dist * Fx::ratio(90, 160);
                samples[(y * (W as i64 + 1) + x) as usize] =
                    flat.height_to_sample(z.max(Fx::from_int(10)));
            }
        }
        Heightfield::from_samples(W, W, samples, DEFAULT_MIN_Z, DEFAULT_Z_STEP, Fx::ZERO)
    }

    /// Bumpy terrain without any structure a bug could hide behind.
    fn rough(seed: u64) -> Heightfield {
        let mut rng = Rng::new(seed);
        let samples = (0..(W + 1) * (W + 1))
            .map(|_| 16_000 + rng.below(1500) as u16)
            .collect();
        Heightfield::from_samples(W, W, samples, DEFAULT_MIN_Z, DEFAULT_Z_STEP, Fx::ZERO)
    }

    /// The same triangulation in floating point.
    fn reference_height(hf: &Heightfield, p: FxVec2) -> f64 {
        let (w, h) = hf.size_cells();
        let x = (p.x.to_f64() / 8.0).clamp(0.0, w as f64);
        let y = (p.y.to_f64() / 8.0).clamp(0.0, h as f64);
        let (cx, cy) = ((x.floor() as u32).min(w - 1), (y.floor() as u32).min(h - 1));
        let (fx, fy) = (x - cx as f64, y - cy as f64);
        let z = |dx, dy| hf.sample_height(cx + dx, cy + dy).to_f64();
        if fx >= fy {
            z(0, 0) + (z(1, 0) - z(0, 0)) * fx + (z(1, 1) - z(1, 0)) * fy
        } else {
            z(0, 0) + (z(0, 1) - z(0, 0)) * fy + (z(1, 1) - z(0, 1)) * fx
        }
    }

    fn random_point(rng: &mut Rng, hf: &Heightfield, margin: i32) -> FxVec2 {
        let size = hf.size_metres();
        let m = Fx::from_int(margin);
        FxVec2::new(rng.range(-m, size.x + m), rng.range(-m, size.y + m))
    }

    #[test]
    fn height_at_corners_is_the_sample() {
        let hf = rough(1);
        for cy in 0..=W {
            for cx in 0..=W {
                let p = FxVec2::from_ints(cx as i32 * 8, cy as i32 * 8);
                assert_eq!(hf.height_at(p), hf.sample_height(cx, cy));
            }
        }
    }

    #[test]
    fn height_at_matches_float_reference() {
        let hf = rough(2);
        let mut rng = Rng::new(9);
        for _ in 0..20_000 {
            let p = random_point(&mut rng, &hf, 40);
            let (got, want) = (hf.height_at(p).to_f64(), reference_height(&hf, p));
            assert!(
                (got - want).abs() <= 2.0 / 65536.0,
                "{p:?}: {got} vs {want}"
            );
        }
    }

    #[test]
    fn height_is_continuous_across_the_diagonal_and_cell_edges() {
        let hf = rough(3);
        let mut rng = Rng::new(4);
        // Steepest possible rise over one raw step of travel, plus rounding.
        let tolerance = Fx(4);
        for _ in 0..5_000 {
            let cell = FxVec2::from_ints(rng.below(W) as i32 * 8, rng.below(W) as i32 * 8);
            let f = rng.range(Fx::ZERO, Fx::from_int(8));
            // Straddle the diagonal.
            let on = cell + FxVec2::new(f, f);
            for step in [FxVec2::new(Fx(1), Fx(0)), FxVec2::new(Fx(0), Fx(1))] {
                assert!((hf.height_at(on + step) - hf.height_at(on)).abs() <= tolerance);
                assert!((hf.height_at(on - step) - hf.height_at(on)).abs() <= tolerance);
            }
            // Straddle the cell's left and bottom edges.
            let left = cell + FxVec2::new(Fx::ZERO, f);
            assert!(
                (hf.height_at(left - FxVec2::new(Fx(1), Fx(0))) - hf.height_at(left)).abs()
                    <= tolerance
            );
            let bottom = cell + FxVec2::new(f, Fx::ZERO);
            assert!(
                (hf.height_at(bottom - FxVec2::new(Fx(0), Fx(1))) - hf.height_at(bottom)).abs()
                    <= tolerance
            );
        }
    }

    #[test]
    fn positions_outside_clamp_to_the_edge() {
        let hf = rough(5);
        let size = hf.size_metres();
        assert_eq!(
            hf.height_at(FxVec2::from_ints(-500, -500)),
            hf.sample_height(0, 0)
        );
        assert_eq!(
            hf.height_at(FxVec2::from_ints(9999, 9999)),
            hf.sample_height(W, W)
        );
        assert_eq!(hf.height_at(size), hf.sample_height(W, W));
        let edge = FxVec2::new(size.x, Fx::from_int(100));
        assert_eq!(
            hf.height_at(edge + FxVec2::from_ints(50, 0)),
            hf.height_at(edge)
        );
        assert!(hf.in_bounds(FxVec2::ZERO));
        assert!(!hf.in_bounds(size));
        assert!(!hf.in_bounds(FxVec2::new(Fx(-1), Fx::ZERO)));
        assert_eq!(hf.cell_at(size), (W - 1, W - 1));
    }

    #[test]
    fn loaded_map_is_seamless_across_tile_borders() {
        let map = MapFile::open(baked_4km()).unwrap();
        let hf = Heightfield::load(&map).unwrap();
        assert_eq!(hf.size_cells(), (512, 512));
        assert_eq!(hf.size_metres(), FxVec2::from_ints(4096, 4096));
        assert_eq!(hf.water_level(), map.info().water_level);

        // Neighbouring tiles agree on their shared samples, and the
        // heightfield holds exactly those.
        let tiles: Vec<Vec<u16>> = [(0, 0), (1, 0), (0, 1), (1, 1)]
            .iter()
            .map(|&(tx, ty)| map.read_tile(tx, ty).unwrap())
            .collect();
        for i in 0..257usize {
            assert_eq!(tiles[0][i * 257 + 256], tiles[1][i * 257]);
            assert_eq!(tiles[2][i * 257 + 256], tiles[3][i * 257]);
            assert_eq!(tiles[0][256 * 257 + i], tiles[2][i]);
            assert_eq!(tiles[1][256 * 257 + i], tiles[3][i]);
            assert_eq!(hf.sample(256, i as u32), tiles[1][i * 257]);
            assert_eq!(hf.sample(i as u32, 256), tiles[2][i]);
            assert_eq!(
                hf.sample(256 + i as u32, 256 + i as u32),
                tiles[3][i * 257 + i]
            );
        }

        // Walking across the border in raw steps never jumps, and the float
        // reference agrees on both sides.
        let border = Fx::from_int(2048);
        for k in 0..400 {
            let along = Fx::from_int(5 * k) + Fx::ratio(1, 3);
            for (a, b) in [
                (
                    FxVec2::new(border - Fx(1), along),
                    FxVec2::new(border, along),
                ),
                (
                    FxVec2::new(along, border - Fx(1)),
                    FxVec2::new(along, border),
                ),
            ] {
                assert!((hf.height_at(a) - hf.height_at(b)).abs() <= Fx(4));
                for p in [a, b] {
                    assert!(
                        (hf.height_at(p).to_f64() - reference_height(&hf, p)).abs()
                            <= 2.0 / 65536.0
                    );
                }
            }
        }
    }

    #[test]
    fn slopes_and_normals() {
        // A plane rising 1 m per 8 m cell along +X.
        let step = (Fx::ONE / DEFAULT_Z_STEP).round_int() as u16;
        let samples = (0..(W + 1) * (W + 1))
            .map(|i| 1000 + (i % (W + 1)) as u16 * step)
            .collect();
        let hf = Heightfield::from_samples(W, W, samples, DEFAULT_MIN_Z, DEFAULT_Z_STEP, Fx::ZERO);
        let p = FxVec2::from_ints(100, 37);
        assert_eq!(hf.gradient_at(p), FxVec2::new(Fx::ratio(1, 8), Fx::ZERO));
        assert_eq!(hf.slope_at(p), Fx::ratio(1, 8));
        assert_eq!(hf.cell_slope(3, 60), Fx::ratio(1, 8));
        let n = hf.normal_at(p);
        assert!(n.x < Fx::ZERO && n.y == Fx::ZERO && n.z > Fx::ratio(9, 10));
        assert!((n.length() - Fx::ONE).abs() <= Fx(4));

        let flat = Heightfield::flat(8, 8, Fx::from_int(25));
        assert_eq!(flat.height_at(FxVec2::from_ints(13, 50)), Fx::from_int(25));
        assert_eq!(
            flat.normal_at(FxVec2::from_ints(13, 50)),
            FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE)
        );
    }

    #[test]
    fn raycast_hits_the_hill_and_misses_over_flat_ground() {
        let hf = hill();
        let z50 = Fx::from_int(50);

        // Level shot at 50 m along the row through the summit: the cone is
        // 50 m high 88.9 m before the centre (x = 256).
        let hit = hf.raycast(
            FxVec3::new(Fx::ZERO, Fx::from_int(256), z50),
            FxVec3::new(Fx::from_int(500), Fx::from_int(256), z50),
        );
        let hit = hit.expect("the hill is in the way");
        assert!(
            (hit.x - Fx::ratio(256 * 9 - 800, 9)).abs() < Fx::ratio(1, 10),
            "{hit:?}"
        );
        assert_eq!(hit.y, Fx::from_int(256));
        assert!((hit.z - z50).abs() < Fx::ratio(1, 100));
        assert_eq!(hit.z, hf.height_at(hit.xy()));

        // Same shot over the flat part of the map, and one that clears the summit.
        assert_eq!(
            hf.raycast(
                FxVec3::new(Fx::ZERO, Fx::from_int(40), z50),
                FxVec3::new(Fx::from_int(500), Fx::from_int(40), z50)
            ),
            None
        );
        let high = Fx::from_int(101);
        assert_eq!(
            hf.raycast(
                FxVec3::new(Fx::ZERO, Fx::from_int(256), high),
                FxVec3::new(Fx::from_int(500), Fx::from_int(256), high)
            ),
            None
        );

        // Diving into flat ground: from 30 m to -10 m over 40 m of travel meets 10 m halfway.
        let hit = hf
            .raycast(
                FxVec3::new(Fx::from_int(100), Fx::from_int(43), Fx::from_int(30)),
                FxVec3::new(Fx::from_int(140), Fx::from_int(43), Fx::from_int(-10)),
            )
            .unwrap();
        assert!((hit.x - Fx::from_int(120)).abs() <= Fx(64), "{hit:?}");
        assert_eq!(hit.z, Fx::from_int(10));

        // Stops short of the ground: no hit. Starts under it: hit at once.
        let from = FxVec3::new(Fx::from_int(100), Fx::from_int(43), Fx::from_int(30));
        assert_eq!(
            hf.raycast(
                from,
                FxVec3::new(Fx::from_int(110), Fx::from_int(43), Fx::from_int(11))
            ),
            None
        );
        let buried = FxVec3::new(Fx::from_int(256), Fx::from_int(256), Fx::from_int(20));
        assert_eq!(hf.raycast(buried, from), Some(buried));
        // Straight down, and a zero-length ray.
        let drop = hf.raycast(
            FxVec3::new(Fx::from_int(300), Fx::from_int(300), Fx::from_int(500)),
            FxVec3::new(Fx::from_int(300), Fx::from_int(300), Fx::from_int(-20)),
        );
        assert_eq!(drop.unwrap().z, hf.height_at(FxVec2::from_ints(300, 300)));
        assert_eq!(hf.raycast(from, from), None);
    }

    #[test]
    fn raycast_agrees_with_brute_force() {
        let hf = rough(6);
        let mut rng = Rng::new(11);
        let steps = 4000;
        let (mut hits, mut misses) = (0, 0);
        for _ in 0..1500 {
            let a = random_point(&mut rng, &hf, 30);
            let b = a + FxVec2::new(
                rng.range(Fx::from_int(-300), Fx::from_int(300)),
                rng.range(Fx::from_int(-300), Fx::from_int(300)),
            );
            let from = a.extend(hf.height_at(a) + rng.range(Fx::ratio(1, 10), Fx::from_int(25)));
            let to = b.extend(hf.height_at(b) + rng.range(Fx::from_int(-25), Fx::from_int(25)));
            // March the segment finely and compare against the reported hit.
            let march = |i: i64| from.lerp(to, Fx::ratio(i, steps));
            let slack = Fx::ratio(1, 20);
            match hf.raycast(from, to) {
                Some(hit) => {
                    hits += 1;
                    // The hit is a real crossing: on the surface and on the segment...
                    assert_eq!(hit.z, hf.height_at(hit.xy()));
                    let along = hit.distance(from);
                    let on_segment = from.lerp(to, along / (to - from).length());
                    assert!(
                        on_segment.distance(hit) <= slack,
                        "{hit:?} is off the segment"
                    );
                    // ...and the first one: everything before it is above ground.
                    for i in 0..=steps {
                        let p = march(i);
                        if p.distance(from) < along - slack {
                            assert!(
                                p.z > hf.height_at(p.xy()) - slack,
                                "earlier hit at step {i}"
                            );
                        }
                    }
                }
                None => {
                    misses += 1;
                    for i in 0..=steps {
                        let p = march(i);
                        assert!(
                            p.z > hf.height_at(p.xy()) - slack,
                            "missed a hit between {from:?} and {to:?}"
                        );
                    }
                }
            }
        }
        assert!(hits > 200 && misses > 200, "{hits} hits, {misses} misses");
    }

    #[test]
    fn raycast_early_out_never_hides_a_hit() {
        let mut hf = hill();
        // Raise a pad above the old summit: the block bounds must follow.
        let pad = hf.flatten_rect((5, 5), (6, 6), Some(Fx::from_int(150)));
        assert_eq!(hf.sample_to_height(pad.sample), Fx::from_int(150));
        let z = Fx::from_int(120);
        let hit = hf.raycast(
            FxVec3::new(Fx::ZERO, Fx::from_int(48), z),
            FxVec3::new(Fx::from_int(200), Fx::from_int(48), z),
        );
        assert!(hit.is_some());
        let (bx, by) = (W as usize / BLOCK_CELLS, W as usize / BLOCK_CELLS);
        assert_eq!(hf.block_max.len(), bx * by);
    }

    #[test]
    fn flatten_levels_the_rect_and_replays() {
        let mut hf = rough(7);
        let mut replay = rough(7);
        let before = hf.samples().to_vec();

        let record = hf.flatten_rect((10, 20), (13, 22), None);
        assert_eq!(
            (record.min_x, record.min_y, record.max_x, record.max_y),
            (10, 20, 13, 22)
        );
        let mut sum = 0u64;
        for y in 20..=23 {
            for x in 10..=14 {
                sum += before[y * (W as usize + 1) + x] as u64;
            }
        }
        assert_eq!(record.sample as u64, (sum + 10) / 20);

        // Level everywhere on the footprint, borders included; untouched outside.
        let z = hf.sample_to_height(record.sample);
        for (x, y) in [(80, 160), (112, 184), (99, 170), (111, 183), (81, 161)] {
            let p = FxVec2::from_ints(x, y) + FxVec2::new(Fx::ratio(1, 3), Fx::ratio(2, 3));
            let inside = p.x < Fx::from_int(112) && p.y < Fx::from_int(184);
            let p = FxVec2::new(p.x.min(Fx::from_int(112)), p.y.min(Fx::from_int(184)));
            assert_eq!(hf.height_at(p), z);
            // On the far border `slope_at` already reports the neighbouring cell.
            if inside {
                assert_eq!(hf.slope_at(p), Fx::ZERO);
            }
        }
        let changed = hf
            .samples()
            .iter()
            .zip(&before)
            .filter(|(a, b)| a != b)
            .count();
        assert!(changed <= 20);
        assert_eq!(hf.sample(9, 20), before[20 * (W as usize + 1) + 9]);
        assert_eq!(hf.sample(15, 23), before[23 * (W as usize + 1) + 15]);

        // A second, overlapping edit with an explicit height; then replay both.
        let second = hf.flatten_rect((12, 21), (20, 30), Some(Fx::from_int(42)));
        assert_eq!(hf.height_at(FxVec2::from_ints(130, 200)), Fx::from_int(42));
        replay.apply_flatten(&record);
        replay.apply_flatten(&second);
        assert_eq!(hf.samples(), replay.samples());
        assert_eq!(hf.block_max, replay.block_max);

        // Rects are clamped to the map.
        let clamped = hf.flatten_rect((60, 60), (500, 500), None);
        assert_eq!((clamped.max_x, clamped.max_y), (W as u16 - 1, W as u16 - 1));
        assert_eq!(clamped.sample_rect(), ((60, 60), (W, W)));

        let hash = |r: &FlattenRecord| {
            let mut h = StateHasher::new();
            r.hash(&mut h);
            h.finish()
        };
        assert_ne!(hash(&record), hash(&second));
        assert_ne!(
            hash(&record),
            hash(&FlattenRecord {
                sample: record.sample + 1,
                ..record
            })
        );
    }
}
