//! `NavGrid`: terrain classes, structure blockers and per-layer clearance.
//!
//! A full map is 104 M cells, so nothing here is dense. Terrain and clearance
//! are stored per sector and a sector that is uniform (all open or all blocked
//! for a layer) carries no cell data at all. The tables are copy-on-write
//! (`Arc` rows of `Arc` sectors): cloning a `NavGrid` is a handful of refcount
//! bumps and gives a background build an immutable snapshot, while a later
//! `block_rect` on the live grid only copies the rows it touches.

use crate::wire::{Reader, Writer};
use crate::{
    Cell, CellRect, MoveLayer, PathError, SizeClass, BUILD_CELLS, MAX_MAP_CELLS, SECTOR_CELLS,
};
use mc_core::FxVec2;
use std::collections::BTreeSet;
use std::sync::Arc;

pub(crate) const SECTOR: usize = SECTOR_CELLS as usize;
pub(crate) const SECTOR_AREA: usize = SECTOR * SECTOR;
/// Clearance values are capped here: the largest size class needs 4.
pub(crate) const MAX_CAP: u8 = 4;
/// `nearest_passable` never searches further than this, whatever the caller asks.
pub const MAX_NEAREST_RADIUS: i32 = 64;

enum TerrainSector {
    Uniform(u8),
    Mixed(Box<[u8; SECTOR_AREA]>),
}

/// Per-cell "largest size class + 1 that fits here", 0 for impassable.
///
/// The square for `n` cells covers `[c - (n-1)/2, c + n/2]` on each axis, so
/// the squares nest and one byte answers every size class.
pub(crate) type Caps = [u8; SECTOR_AREA];

#[derive(Clone)]
pub(crate) enum SectorKind {
    /// Every cell fits every size class.
    Open,
    /// No cell is passable.
    Blocked,
    Mixed(Arc<Caps>),
}

#[derive(Clone)]
pub(crate) struct LayerSector {
    /// Grid version at which this sector's data last changed. Derived caches validate against it.
    pub version: u32,
    pub kind: SectorKind,
}

impl LayerSector {
    #[inline]
    pub fn cap(&self, local: usize) -> u8 {
        match &self.kind {
            SectorKind::Open => MAX_CAP,
            SectorKind::Blocked => 0,
            SectorKind::Mixed(c) => c[local],
        }
    }
}

type Rows<T> = Arc<Vec<Arc<Vec<T>>>>;
type BlockBits = Option<Arc<[u32; SECTOR]>>;

/// Sector ids per layer whose clearance data changed, sorted ascending.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Touched {
    pub layers: [Vec<u32>; 4],
}

impl Touched {
    pub fn is_empty(&self) -> bool {
        self.layers.iter().all(|l| l.is_empty())
    }
}

/// Row passability masks of one sector for the four layers.
enum Masks {
    Uniform([bool; 4]),
    Rows(Box<[[u32; SECTOR]; 4]>),
}

impl Masks {
    #[inline]
    fn row(&self, layer: usize, y: usize) -> u32 {
        match self {
            Masks::Uniform(u) => {
                if u[layer] {
                    !0
                } else {
                    0
                }
            }
            Masks::Rows(r) => r[layer][y],
        }
    }
}

#[derive(Clone)]
pub struct NavGrid {
    w: i32,
    h: i32,
    sw: i32,
    sh: i32,
    terrain: Arc<Vec<TerrainSector>>,
    blockers: Rows<BlockBits>,
    layers: [Rows<LayerSector>; 4],
    version: u32,
    blocker_hash: u64,
    blocked_cells: u64,
    sectors_recomputed: u64,
}

#[inline]
fn mix(v: u64) -> u64 {
    let mut x = v.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Bit `l` set when layer `l` can cross terrain class `class`.
fn layer_bits(class: u8) -> u8 {
    let mut bits = 0;
    for l in MoveLayer::ALL {
        if l.passable(class) {
            bits |= 1 << l.index();
        }
    }
    bits
}

impl NavGrid {
    /// Builds the grid by calling `class_at(x, y)` once per cell, sector by
    /// sector. Edges must be multiples of the sector size.
    pub fn from_fn(
        w: i32,
        h: i32,
        mut class_at: impl FnMut(i32, i32) -> u8,
    ) -> Result<NavGrid, PathError> {
        if w <= 0
            || h <= 0
            || w % SECTOR_CELLS != 0
            || h % SECTOR_CELLS != 0
            || w > MAX_MAP_CELLS
            || h > MAX_MAP_CELLS
        {
            return Err(PathError::BadMapSize);
        }
        let (sw, sh) = (w / SECTOR_CELLS, h / SECTOR_CELLS);
        let mut terrain = Vec::with_capacity((sw * sh) as usize);
        let mut buf = [0u8; SECTOR_AREA];
        for sy in 0..sh {
            for sx in 0..sw {
                let (bx, by) = (sx * SECTOR_CELLS, sy * SECTOR_CELLS);
                for ly in 0..SECTOR_CELLS {
                    for lx in 0..SECTOR_CELLS {
                        buf[(ly * SECTOR_CELLS + lx) as usize] = class_at(bx + lx, by + ly);
                    }
                }
                let first = buf[0];
                terrain.push(if buf.iter().all(|&c| c == first) {
                    TerrainSector::Uniform(first)
                } else {
                    TerrainSector::Mixed(Box::new(buf))
                });
            }
        }
        let empty_row: Arc<Vec<BlockBits>> = Arc::new(vec![None; sw as usize]);
        let mut grid = NavGrid {
            w,
            h,
            sw,
            sh,
            terrain: Arc::new(terrain),
            blockers: Arc::new(vec![empty_row; sh as usize]),
            layers: std::array::from_fn(|_| Arc::new(Vec::new())),
            version: 0,
            blocker_hash: 0,
            blocked_cells: 0,
            sectors_recomputed: 0,
        };
        grid.derive_all();
        Ok(grid)
    }

    /// Row-major `classes`, `w * h` long.
    pub fn from_cells(w: i32, h: i32, classes: &[u8]) -> Result<NavGrid, PathError> {
        if w <= 0 || h <= 0 || classes.len() != w as usize * h as usize {
            return Err(PathError::BadMapSize);
        }
        Self::from_fn(w, h, |x, y| classes[y as usize * w as usize + x as usize])
    }

    /// Initial clearance for every sector. Mask rows are kept for three sector
    /// rows at a time so the pass stays linear without a map-sized scratch table.
    fn derive_all(&mut self) {
        let mut out: [Vec<Arc<Vec<LayerSector>>>; 4] =
            std::array::from_fn(|_| Vec::with_capacity(self.sh as usize));
        let mut window: [Vec<Masks>; 3] = [Vec::new(), Vec::new(), self.mask_row(0)];
        for sy in 0..self.sh {
            window.rotate_left(1);
            window[2] = if sy + 1 < self.sh {
                self.mask_row(sy + 1)
            } else {
                Vec::new()
            };
            let mut rows: [Vec<LayerSector>; 4] =
                std::array::from_fn(|_| Vec::with_capacity(self.sw as usize));
            for sx in 0..self.sw {
                let mut nb: [[Option<&Masks>; 3]; 3] = [[None; 3]; 3];
                for (dy, line) in nb.iter_mut().enumerate() {
                    for (dx, slot) in line.iter_mut().enumerate() {
                        let x = sx + dx as i32 - 1;
                        if x >= 0 && x < self.sw {
                            *slot = window[dy].get(x as usize);
                        }
                    }
                }
                let kinds = derive_sector(&nb);
                for (l, kind) in kinds.into_iter().enumerate() {
                    rows[l].push(LayerSector { version: 0, kind });
                }
            }
            for (l, row) in rows.into_iter().enumerate() {
                out[l].push(Arc::new(row));
            }
        }
        for (l, rows) in out.into_iter().enumerate() {
            self.layers[l] = Arc::new(rows);
        }
    }

    fn mask_row(&self, sy: i32) -> Vec<Masks> {
        (0..self.sw).map(|sx| self.masks(sx, sy)).collect()
    }

    /// Passability rows of one sector: terrain minus blockers.
    fn masks(&self, sx: i32, sy: i32) -> Masks {
        let blocked = &self.blockers[sy as usize][sx as usize];
        match (&self.terrain[(sy * self.sw + sx) as usize], blocked) {
            (TerrainSector::Uniform(c), None) => {
                let bits = layer_bits(*c);
                Masks::Uniform(std::array::from_fn(|l| bits & (1 << l) != 0))
            }
            (t, b) => {
                let mut rows = Box::new([[0u32; SECTOR]; 4]);
                match t {
                    TerrainSector::Uniform(c) => {
                        let bits = layer_bits(*c);
                        for (l, layer) in rows.iter_mut().enumerate() {
                            if bits & (1 << l) != 0 {
                                *layer = [!0; SECTOR];
                            }
                        }
                    }
                    TerrainSector::Mixed(cells) => {
                        let mut lut = [0u8; 256];
                        for (c, out) in lut.iter_mut().enumerate().take(16) {
                            *out = layer_bits(c as u8);
                        }
                        for y in 0..SECTOR {
                            for x in 0..SECTOR {
                                let bits = lut[(cells[y * SECTOR + x] & 15) as usize];
                                for (l, layer) in rows.iter_mut().enumerate() {
                                    layer[y] |= (((bits >> l) & 1) as u32) << x;
                                }
                            }
                        }
                    }
                }
                if let Some(b) = b {
                    for layer in rows.iter_mut() {
                        for (row, blocked) in layer.iter_mut().zip(b.iter()) {
                            *row &= !blocked;
                        }
                    }
                }
                Masks::Rows(rows)
            }
        }
    }

    #[inline]
    pub fn width(&self) -> i32 {
        self.w
    }

    #[inline]
    pub fn height(&self) -> i32 {
        self.h
    }

    /// Bumped by every `block_rect`/`unblock_rect` that changed a cell.
    #[inline]
    pub fn version(&self) -> u32 {
        self.version
    }

    #[inline]
    pub fn blocked_cell_count(&self) -> u64 {
        self.blocked_cells
    }

    /// Order-independent hash of the blocked cell set, maintained incrementally.
    #[inline]
    pub fn blocker_hash(&self) -> u64 {
        self.blocker_hash
    }

    /// Sector-layer clearance recomputations since construction (diagnostic).
    #[inline]
    pub fn sectors_recomputed(&self) -> u64 {
        self.sectors_recomputed
    }

    #[inline]
    pub fn contains(&self, c: Cell) -> bool {
        c.x >= 0 && c.y >= 0 && c.x < self.w && c.y < self.h
    }

    /// World position to cell, `None` outside the map.
    #[inline]
    pub fn cell_of(&self, pos: FxVec2) -> Option<Cell> {
        let c = Cell::from_pos(pos);
        self.contains(c).then_some(c)
    }

    #[inline]
    pub(crate) fn sector_dims(&self) -> (i32, i32) {
        (self.sw, self.sh)
    }

    #[inline]
    pub(crate) fn sector_index(&self, sx: i32, sy: i32) -> Option<u32> {
        (sx >= 0 && sy >= 0 && sx < self.sw && sy < self.sh).then(|| (sy * self.sw + sx) as u32)
    }

    #[inline]
    pub(crate) fn sector_xy(&self, sector: u32) -> (i32, i32) {
        (sector as i32 % self.sw, sector as i32 / self.sw)
    }

    #[inline]
    pub(crate) fn sector_of(&self, c: Cell) -> u32 {
        ((c.y / SECTOR_CELLS) * self.sw + c.x / SECTOR_CELLS) as u32
    }

    #[inline]
    pub(crate) fn cell_index(&self, c: Cell) -> u32 {
        (c.y * self.w + c.x) as u32
    }

    #[inline]
    pub(crate) fn cell_from_index(&self, i: u32) -> Cell {
        Cell::new(i as i32 % self.w, i as i32 / self.w)
    }

    #[inline]
    pub(crate) fn layer_sector(&self, layer: MoveLayer, sx: i32, sy: i32) -> &LayerSector {
        &self.layers[layer.index()][sy as usize][sx as usize]
    }

    /// Clearance at `c`: how many size classes fit (0 = impassable, capped at 4).
    #[inline]
    pub fn clearance(&self, layer: MoveLayer, c: Cell) -> u8 {
        if !self.contains(c) {
            return 0;
        }
        let local = (c.y % SECTOR_CELLS) as usize * SECTOR + (c.x % SECTOR_CELLS) as usize;
        self.layer_sector(layer, c.x / SECTOR_CELLS, c.y / SECTOR_CELLS)
            .cap(local)
    }

    #[inline]
    pub fn is_passable(&self, layer: MoveLayer, size: SizeClass, c: Cell) -> bool {
        self.clearance(layer, c) >= size.cells()
    }

    pub fn terrain_class(&self, c: Cell) -> u8 {
        if !self.contains(c) {
            return 0;
        }
        match &self.terrain[self.sector_of(c) as usize] {
            TerrainSector::Uniform(v) => *v,
            TerrainSector::Mixed(cells) => {
                cells[(c.y % SECTOR_CELLS) as usize * SECTOR + (c.x % SECTOR_CELLS) as usize]
            }
        }
    }

    pub fn is_blocked(&self, c: Cell) -> bool {
        if !self.contains(c) {
            return false;
        }
        match &self.blockers[(c.y / SECTOR_CELLS) as usize][(c.x / SECTOR_CELLS) as usize] {
            Some(b) => b[(c.y % SECTOR_CELLS) as usize] & (1 << (c.x % SECTOR_CELLS)) != 0,
            None => false,
        }
    }

    fn check_rect(&self, r: CellRect) -> Result<(), PathError> {
        let aligned = [r.min.x, r.min.y, r.max.x, r.max.y]
            .iter()
            .all(|v| v % BUILD_CELLS == 0);
        if !aligned
            || r.min.x >= r.max.x
            || r.min.y >= r.max.y
            || r.min.x < 0
            || r.min.y < 0
            || r.max.x > self.w
            || r.max.y > self.h
        {
            return Err(PathError::BadRect);
        }
        Ok(())
    }

    /// Whether a structure could go here: rect valid, every cell free of
    /// blockers and on terrain `layer` can cross (`Land` for land structures,
    /// `Naval` for shipyards).
    pub fn can_place(&self, rect: CellRect, layer: MoveLayer) -> bool {
        self.rect_cells(rect, |c| {
            !self.is_blocked(c) && layer.passable(self.terrain_class(c))
        })
    }

    /// Terrain `layer` can stand on; structure blockers are ignored.
    pub fn passable_terrain(&self, rect: CellRect, layer: MoveLayer) -> bool {
        self.rect_cells(rect, |c| layer.passable(self.terrain_class(c)))
    }

    /// No structure or city blocker in `rect`.
    pub fn no_blockers(&self, rect: CellRect) -> bool {
        self.rect_cells(rect, |c| !self.is_blocked(c))
    }

    fn rect_cells(&self, rect: CellRect, ok: impl Fn(Cell) -> bool) -> bool {
        if self.check_rect(rect).is_err() {
            return false;
        }
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                if !ok(Cell::new(x, y)) {
                    return false;
                }
            }
        }
        true
    }

    /// Closest cell to `pos` that `size` fits on, searching Chebyshev rings out
    /// to `max_radius_cells` (capped at `MAX_NEAREST_RADIUS`). The first ring
    /// with a hit wins; within it the smallest squared distance, then y, then x.
    pub fn nearest_passable(
        &self,
        layer: MoveLayer,
        size: SizeClass,
        pos: FxVec2,
        max_radius_cells: i32,
    ) -> Option<Cell> {
        let origin = Cell::from_pos(pos);
        let max_r = max_radius_cells.clamp(0, MAX_NEAREST_RADIUS);
        for r in 0..=max_r {
            let mut best: Option<(i64, Cell)> = None;
            let mut visit = |c: Cell| {
                if self.is_passable(layer, size, c) {
                    let d = c.center() - pos;
                    let (dx, dy) = (d.x.0 >> 8, d.y.0 >> 8);
                    let key = (dx * dx + dy * dy, c);
                    if best.is_none_or(|b| (key.0, key.1.y, key.1.x) < (b.0, b.1.y, b.1.x)) {
                        best = Some(key);
                    }
                }
            };
            if r == 0 {
                visit(origin);
            } else {
                for i in -r..=r {
                    visit(Cell::new(origin.x + i, origin.y - r));
                    visit(Cell::new(origin.x + i, origin.y + r));
                }
                for i in (1 - r)..r {
                    visit(Cell::new(origin.x - r, origin.y + i));
                    visit(Cell::new(origin.x + r, origin.y + i));
                }
            }
            if let Some((_, c)) = best {
                return Some(c);
            }
        }
        None
    }

    /// Marks a structure footprint impassable on every layer. Cells already
    /// blocked stay blocked; only sectors within reach of a changed cell are
    /// recomputed.
    pub fn block_rect(&mut self, rect: CellRect) -> Result<Touched, PathError> {
        self.set_rect(rect, true)
    }

    pub fn unblock_rect(&mut self, rect: CellRect) -> Result<Touched, PathError> {
        self.set_rect(rect, false)
    }

    fn set_rect(&mut self, rect: CellRect, block: bool) -> Result<Touched, PathError> {
        self.check_rect(rect)?;
        let mut flipped = false;
        let (s0x, s0y) = (rect.min.x / SECTOR_CELLS, rect.min.y / SECTOR_CELLS);
        let (s1x, s1y) = (
            (rect.max.x - 1) / SECTOR_CELLS,
            (rect.max.y - 1) / SECTOR_CELLS,
        );
        for sy in s0y..=s1y {
            for sx in s0x..=s1x {
                let (bx, by) = (sx * SECTOR_CELLS, sy * SECTOR_CELLS);
                let (x0, x1) = (
                    (rect.min.x - bx).max(0),
                    (rect.max.x - bx).min(SECTOR_CELLS),
                );
                let (y0, y1) = (
                    (rect.min.y - by).max(0),
                    (rect.max.y - by).min(SECTOR_CELLS),
                );
                let span = (((1u64 << (x1 - x0)) - 1) << x0) as u32;
                let old = self.blockers[sy as usize][sx as usize]
                    .as_deref()
                    .copied()
                    .unwrap_or([0; SECTOR]);
                let mut new = old;
                for row in &mut new[y0 as usize..y1 as usize] {
                    *row = if block { *row | span } else { *row & !span };
                }
                if new == old {
                    continue;
                }
                flipped = true;
                for y in 0..SECTOR {
                    let mut diff = new[y] ^ old[y];
                    while diff != 0 {
                        let x = diff.trailing_zeros() as i32;
                        diff &= diff - 1;
                        self.blocker_hash ^=
                            mix(((by + y as i32) as u64 * self.w as u64) + (bx + x) as u64);
                        if block {
                            self.blocked_cells += 1;
                        } else {
                            self.blocked_cells -= 1;
                        }
                    }
                }
                let rows = Arc::make_mut(&mut self.blockers);
                let row = Arc::make_mut(&mut rows[sy as usize]);
                row[sx as usize] = new.iter().any(|&r| r != 0).then(|| Arc::new(new));
            }
        }
        let mut touched = Touched::default();
        if !flipped {
            return Ok(touched);
        }
        self.version += 1;
        // A cell's clearance reads up to two cells away, so the rect grown by
        // two decides which sectors can change.
        let (t0x, t0y) = (
            (rect.min.x - 2).max(0) / SECTOR_CELLS,
            (rect.min.y - 2).max(0) / SECTOR_CELLS,
        );
        let (t1x, t1y) = (
            (rect.max.x + 1).min(self.w - 1) / SECTOR_CELLS,
            (rect.max.y + 1).min(self.h - 1) / SECTOR_CELLS,
        );
        for sy in t0y..=t1y {
            for sx in t0x..=t1x {
                self.rederive(sx, sy, &mut touched);
            }
        }
        Ok(touched)
    }

    fn rederive(&mut self, sx: i32, sy: i32, touched: &mut Touched) {
        let mut owned: Vec<Option<Masks>> = Vec::with_capacity(9);
        for dy in -1..=1 {
            for dx in -1..=1 {
                owned.push(
                    self.sector_index(sx + dx, sy + dy)
                        .map(|_| self.masks(sx + dx, sy + dy)),
                );
            }
        }
        let mut nb: [[Option<&Masks>; 3]; 3] = [[None; 3]; 3];
        for (i, m) in owned.iter().enumerate() {
            nb[i / 3][i % 3] = m.as_ref();
        }
        let kinds = derive_sector(&nb);
        self.sectors_recomputed += 4;
        for (l, kind) in kinds.into_iter().enumerate() {
            let old = &self.layers[l][sy as usize][sx as usize];
            let same = match (&old.kind, &kind) {
                (SectorKind::Open, SectorKind::Open)
                | (SectorKind::Blocked, SectorKind::Blocked) => true,
                (SectorKind::Mixed(a), SectorKind::Mixed(b)) => a[..] == b[..],
                _ => false,
            };
            if same {
                continue;
            }
            let rows = Arc::make_mut(&mut self.layers[l]);
            let row = Arc::make_mut(&mut rows[sy as usize]);
            row[sx as usize] = LayerSector {
                version: self.version,
                kind,
            };
            touched.layers[l].push((sy * self.sw + sx) as u32);
        }
    }

    /// Everything `block_rect`/`unblock_rect` ever changed: blocker bits,
    /// counters, and the sector versions (they decide tile reuse, so a restored
    /// grid must carry the same ones). Terrain is not included.
    pub(crate) fn write_dynamic(&self, out: &mut Writer) {
        out.i32(self.w);
        out.i32(self.h);
        out.u32(self.version);
        out.u64(self.sectors_recomputed);
        let blocked: Vec<(u32, &[u32; SECTOR])> = (0..self.sw * self.sh)
            .filter_map(|s| {
                self.blockers[(s / self.sw) as usize][(s % self.sw) as usize]
                    .as_deref()
                    .map(|b| (s as u32, b))
            })
            .collect();
        out.u32(blocked.len() as u32);
        for (s, bits) in blocked {
            out.u32(s);
            bits.iter().for_each(|&row| out.u32(row));
        }
        for layer in &self.layers {
            let versions: Vec<(u32, u32)> = layer
                .iter()
                .flat_map(|row| row.iter())
                .enumerate()
                .filter(|(_, ls)| ls.version != 0)
                .map(|(s, ls)| (s as u32, ls.version))
                .collect();
            out.u32(versions.len() as u32);
            for (s, v) in versions {
                out.u32(s);
                out.u32(v);
            }
        }
    }

    /// Applies `write_dynamic` output to a pristine grid of the same map.
    /// Clearance is re-derived from terrain and blockers, a pure function, so
    /// it matches the exporter's incrementally maintained data.
    pub(crate) fn read_dynamic(&mut self, r: &mut Reader) -> Result<(), PathError> {
        const BAD: PathError = PathError::BadSnapshot;
        if self.version != 0 || self.blocked_cells != 0 || r.i32()? != self.w || r.i32()? != self.h
        {
            return Err(BAD);
        }
        let sectors = (self.sw * self.sh) as u32;
        let version = r.u32()?;
        let recomputed = r.u64()?;
        // (sy, sx), so iteration is in sector id order and `scratch` comes out sorted.
        let mut dirty: BTreeSet<(i32, i32)> = BTreeSet::new();
        let mut last = None;
        for _ in 0..r.count(4 + 4 * SECTOR)? {
            let s = r.u32()?;
            if s >= sectors || last.is_some_and(|l| l >= s) {
                return Err(BAD);
            }
            last = Some(s);
            let mut bits = [0u32; SECTOR];
            for row in bits.iter_mut() {
                *row = r.u32()?;
            }
            if bits.iter().all(|&b| b == 0) {
                return Err(BAD);
            }
            let (sx, sy) = self.sector_xy(s);
            for (y, &row) in bits.iter().enumerate() {
                let mut rest = row;
                while rest != 0 {
                    let x = rest.trailing_zeros() as i32;
                    rest &= rest - 1;
                    self.blocker_hash ^= mix(((sy * SECTOR_CELLS + y as i32) as u64
                        * self.w as u64)
                        + (sx * SECTOR_CELLS + x) as u64);
                    self.blocked_cells += 1;
                }
            }
            Arc::make_mut(&mut Arc::make_mut(&mut self.blockers)[sy as usize])[sx as usize] =
                Some(Arc::new(bits));
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if self.sector_index(sx + dx, sy + dy).is_some() {
                        dirty.insert((sy + dy, sx + dx));
                    }
                }
            }
        }
        self.version = version;
        let mut scratch = Touched::default();
        for (sy, sx) in dirty {
            self.rederive(sx, sy, &mut scratch);
        }
        for l in 0..4 {
            // Every sector whose data differs from the base map changed at some
            // point, so the exporter lists a version for it; the list wins.
            let listed = r.count(8)?;
            let mut last = None;
            let mut seen = 0;
            for _ in 0..listed {
                let (s, v) = (r.u32()?, r.u32()?);
                if s >= sectors || v == 0 || v > version || last.is_some_and(|p| p >= s) {
                    return Err(BAD);
                }
                last = Some(s);
                let (sx, sy) = self.sector_xy(s);
                seen += scratch.layers[l].binary_search(&s).is_ok() as usize;
                Arc::make_mut(&mut Arc::make_mut(&mut self.layers[l])[sy as usize])[sx as usize]
                    .version = v;
            }
            if seen != scratch.layers[l].len() {
                return Err(BAD);
            }
        }
        self.sectors_recomputed = recomputed;
        Ok(())
    }

    /// Rough heap footprint in bytes (diagnostic; shared arrays are counted once per sector).
    pub fn memory_bytes(&self) -> usize {
        let sectors = (self.sw * self.sh) as usize;
        let mut bytes = sectors
            * (std::mem::size_of::<TerrainSector>()
                + std::mem::size_of::<BlockBits>()
                + 4 * std::mem::size_of::<LayerSector>());
        bytes += self
            .terrain
            .iter()
            .filter(|t| matches!(t, TerrainSector::Mixed(_)))
            .count()
            * SECTOR_AREA;
        for sy in 0..self.sh as usize {
            for sx in 0..self.sw as usize {
                if self.blockers[sy][sx].is_some() {
                    bytes += 4 * SECTOR;
                }
                let mut seen: Vec<*const Caps> = Vec::with_capacity(4);
                for l in 0..4 {
                    if let SectorKind::Mixed(c) = &self.layers[l][sy][sx].kind {
                        if !seen.contains(&Arc::as_ptr(c)) {
                            seen.push(Arc::as_ptr(c));
                            bytes += SECTOR_AREA + 16;
                        }
                    }
                }
            }
        }
        bytes
    }
}

/// Clearance of one sector for all four layers from the 3 x 3 mask
/// neighbourhood (`None` = off the map = impassable). Layers with identical
/// results share one array, which is the common case away from coasts.
fn derive_sector(nb: &[[Option<&Masks>; 3]; 3]) -> [SectorKind; 4] {
    let center = nb[1][1].expect("sector exists");
    let mut out: [SectorKind; 4] = std::array::from_fn(|_| SectorKind::Blocked);
    // Interior fast path: a uniform neighbourhood needs no cell work.
    let uniform: Option<[bool; 4]> = nb
        .iter()
        .flatten()
        .try_fold(None, |acc: Option<[bool; 4]>, m| match m {
            Some(Masks::Uniform(u)) if acc.is_none_or(|a| a == *u) => Some(Some(*u)),
            _ => None,
        })
        .flatten();
    if let Some(u) = uniform {
        for (l, kind) in out.iter_mut().enumerate() {
            *kind = if u[l] {
                SectorKind::Open
            } else {
                SectorKind::Blocked
            };
        }
        return out;
    }
    for l in 0..4 {
        if matches!(center, Masks::Uniform(u) if !u[l]) {
            continue;
        }
        // Window rows: bit i is x = i - 1, row j is y = j - 1.
        let mut w = [0u64; SECTOR + 3];
        for (j, row) in w.iter_mut().enumerate() {
            let y = j as i32 - 1;
            let (dy, ly) = if y < 0 {
                (0, SECTOR - 1)
            } else if y < SECTOR as i32 {
                (1, y as usize)
            } else {
                (2, y as usize - SECTOR)
            };
            let get = |dx: usize| nb[dy][dx].map_or(0, |m| m.row(l, ly)) as u64;
            *row = (get(0) >> 31) | (get(1) << 1) | ((get(2) & 3) << 33);
        }
        let h = |n: u32| -> [u64; SECTOR + 3] {
            std::array::from_fn(|j| (0..n).fold(!0u64, |a, s| a & (w[j] >> s)))
        };
        let (h2, h3, h4) = (h(2), h(3), h(4));
        let mut caps = [0u8; SECTOR_AREA];
        let (mut all_open, mut any) = (true, false);
        for y in 0..SECTOR {
            let p1 = (w[y + 1] >> 1) as u32;
            let p2 = ((h2[y + 1] & h2[y + 2]) >> 1) as u32;
            let p3 = (h3[y] & h3[y + 1] & h3[y + 2]) as u32;
            let p4 = (h4[y] & h4[y + 1] & h4[y + 2] & h4[y + 3]) as u32;
            all_open &= p4 == !0;
            any |= p1 != 0;
            for x in 0..SECTOR {
                caps[y * SECTOR + x] =
                    (((p1 >> x) & 1) + ((p2 >> x) & 1) + ((p3 >> x) & 1) + ((p4 >> x) & 1)) as u8;
            }
        }
        out[l] = if all_open {
            SectorKind::Open
        } else if !any {
            SectorKind::Blocked
        } else {
            let shared = out[..l].iter().find_map(|k| match k {
                SectorKind::Mixed(c) if c[..] == caps[..] => Some(c.clone()),
                _ => None,
            });
            SectorKind::Mixed(shared.unwrap_or_else(|| Arc::new(caps)))
        };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::*;

    fn rect(x0: i32, y0: i32, x1: i32, y1: i32) -> CellRect {
        CellRect::new(Cell::new(x0, y0), Cell::new(x1, y1))
    }

    /// Reference clearance straight from the definition.
    fn brute_cap(g: &NavGrid, layer: MoveLayer, c: Cell) -> u8 {
        let free = |x: i32, y: i32| {
            let p = Cell::new(x, y);
            g.contains(p) && layer.passable(g.terrain_class(p)) && !g.is_blocked(p)
        };
        let mut cap = 0;
        for n in 1..=4i32 {
            let (lo, hi) = ((n - 1) / 2, n / 2);
            let ok = (c.y - lo..=c.y + hi).all(|y| (c.x - lo..=c.x + hi).all(|x| free(x, y)));
            if !ok {
                break;
            }
            cap = n as u8;
        }
        cap
    }

    fn noisy(x: i32, y: i32) -> u8 {
        let h = mix((x as u64) << 32 | y as u64);
        match h % 23 {
            0 => LAND | STEEP,
            1 => SHALLOW,
            2 => DEEP,
            _ => LAND,
        }
    }

    #[test]
    fn rejects_bad_sizes() {
        assert_eq!(
            NavGrid::from_fn(100, 64, |_, _| LAND).err(),
            Some(PathError::BadMapSize)
        );
        assert_eq!(
            NavGrid::from_fn(0, 64, |_, _| LAND).err(),
            Some(PathError::BadMapSize)
        );
        assert!(NavGrid::from_cells(32, 32, &[LAND; 1024]).is_ok());
    }

    #[test]
    fn clearance_matches_definition() {
        let mut g = NavGrid::from_fn(96, 96, |x, y| {
            if (30..70).contains(&x) && (30..70).contains(&y) {
                noisy(x, y)
            } else {
                LAND
            }
        })
        .unwrap();
        g.block_rect(rect(30, 60, 36, 66)).unwrap();
        for layer in MoveLayer::ALL {
            for y in -1..97 {
                for x in -1..97 {
                    let c = Cell::new(x, y);
                    assert_eq!(
                        g.clearance(layer, c),
                        brute_cap(&g, layer, c),
                        "{layer:?} {c:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn uniform_sectors_are_implicit() {
        let g = NavGrid::from_fn(160, 160, |_, _| LAND).unwrap();
        assert!(matches!(
            g.layer_sector(MoveLayer::Land, 2, 2).kind,
            SectorKind::Open
        ));
        assert!(matches!(
            g.layer_sector(MoveLayer::Naval, 2, 2).kind,
            SectorKind::Blocked
        ));
        // The map edge counts as a wall, so border sectors carry data.
        assert!(matches!(
            g.layer_sector(MoveLayer::Land, 0, 2).kind,
            SectorKind::Mixed(_)
        ));
        // Layers that agree share one array.
        let (SectorKind::Mixed(a), SectorKind::Mixed(b)) = (
            &g.layer_sector(MoveLayer::Land, 0, 2).kind,
            &g.layer_sector(MoveLayer::Hover, 0, 2).kind,
        ) else {
            panic!("expected mixed")
        };
        assert!(Arc::ptr_eq(a, b));
    }

    #[test]
    fn block_touches_only_nearby_sectors_and_round_trips() {
        let mut g = NavGrid::from_fn(320, 320, |_, _| LAND).unwrap();
        let before = g.clone();
        let t = g.block_rect(rect(100, 100, 104, 104)).unwrap();
        assert_eq!(t.layers[MoveLayer::Land.index()], vec![3 * 10 + 3]);
        assert!(t.layers[MoveLayer::Naval.index()].is_empty());
        assert_eq!(g.sectors_recomputed(), 4);
        assert!(!g.is_passable(MoveLayer::Land, SizeClass::SMALL, Cell::new(101, 101)));
        assert!(g.is_passable(MoveLayer::Land, SizeClass::SMALL, Cell::new(99, 101)));
        assert!(!g.is_passable(MoveLayer::Land, SizeClass::LARGE, Cell::new(99, 101)));
        // The snapshot taken before the change still sees the old world.
        assert!(before.is_passable(MoveLayer::Land, SizeClass::SMALL, Cell::new(101, 101)));
        assert!(g.block_rect(rect(100, 100, 104, 104)).unwrap().is_empty());
        // A rect on a sector corner reaches the three neighbours.
        let t = g.block_rect(rect(126, 126, 128, 128)).unwrap();
        assert_eq!(t.layers[0], vec![33, 34, 43, 44]);
        g.unblock_rect(rect(126, 126, 128, 128)).unwrap();
        g.unblock_rect(rect(100, 100, 104, 104)).unwrap();
        assert_eq!(g.blocker_hash(), 0);
        assert_eq!(g.blocked_cell_count(), 0);
        assert!(matches!(
            g.layer_sector(MoveLayer::Land, 3, 3).kind,
            SectorKind::Open
        ));
        assert_eq!(
            g.block_rect(rect(1, 0, 1, 2)).err(),
            Some(PathError::BadRect)
        );
    }

    #[test]
    fn placement_and_nearest() {
        let mut g = NavGrid::from_fn(64, 64, |x, _| if x < 32 { LAND } else { DEEP }).unwrap();
        assert!(g.can_place(rect(4, 4, 8, 8), MoveLayer::Land));
        assert!(!g.can_place(rect(30, 4, 34, 8), MoveLayer::Land));
        assert!(g.can_place(rect(40, 4, 44, 8), MoveLayer::Naval));
        g.block_rect(rect(4, 4, 8, 8)).unwrap();
        assert!(!g.can_place(rect(6, 6, 10, 10), MoveLayer::Land));
        assert!(
            g.passable_terrain(rect(4, 4, 8, 8), MoveLayer::Land),
            "blockers are not terrain"
        );
        assert!(!g.no_blockers(rect(4, 4, 8, 8)));
        let inside = Cell::new(5, 5).center();
        let near = g
            .nearest_passable(MoveLayer::Land, SizeClass::SMALL, inside, 8)
            .unwrap();
        assert_eq!(near, Cell::new(5, 3));
        assert_eq!(
            g.nearest_passable(MoveLayer::Naval, SizeClass::SMALL, inside, 8),
            None
        );
        assert_eq!(
            g.nearest_passable(MoveLayer::Naval, SizeClass::SMALL, inside, 40),
            Some(Cell::new(32, 5))
        );
    }
}
