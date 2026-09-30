//! The spatial index: uniform grids rebuilt by counting sort.
//!
//! Every proximity query in the simulation goes through here. Entries are
//! binned by centre into one of a few layers by size: small entries on a fine
//! grid, big structures and giants on coarser ones. A query widens its search by
//! each layer's largest radius, so one titan no longer makes every infantry
//! query scan a 300 m square. Results come out layer by layer (smallest first),
//! and within a layer in (cell row-major, insertion) order, which depends only
//! on table contents.

use mc_core::{Fx, FxVec2, PlayerMask};

/// Entity kinds an entry can refer to. Queries filter by a mask of these.
pub mod kind {
    pub const UNIT: u8 = 1 << 0;
    pub const WRECK: u8 = 1 << 1;
    pub const STAIN: u8 = 1 << 2;
    pub const PROP: u8 = 1 << 3;
    /// Set beside `UNIT` on an aircraft, so a search for aircraft passes over
    /// the ground army under them.
    pub const AIRCRAFT: u8 = 1 << 4;
}

#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub kind: u8,
    /// The owning player, or [`NO_OWNER`].
    pub owner: u8,
    /// Row in the entity's table.
    pub row: u32,
    pub pos: FxVec2,
    pub radius: Fx,
}

/// `Entry::owner` of what no player owns (wrecks, stains, props).
pub const NO_OWNER: u8 = u8::MAX;

const EMPTY: Entry = Entry {
    kind: 0,
    owner: NO_OWNER,
    row: 0,
    pos: FxVec2::ZERO,
    radius: Fx::ZERO,
};

/// The layers as (cell edge shift, largest radius held): 16 m cells for
/// anything up to 16 m (units, wrecks, stains), 64 m cells up to 64 m
/// (structures, big hulls), 256 m cells for the rest (titans, capital ships).
const LAYERS: [(u32, i32); 3] = [(4, 16), (6, 64), (8, i32::MAX)];

/// One grid. Entries are sorted by (cell row, cell column, insertion);
/// `row_start[y]..row_start[y + 1]` holds grid row `y`, and `cell_x` runs
/// alongside `sorted` so a query finds its first column by binary search. No
/// table per cell, so a fine grid over an 80 km map costs nothing to clear.
struct Layer {
    shift: u32,
    width: u32,
    height: u32,
    row_start: Vec<u32>,
    sorted: Vec<Entry>,
    cell_x: Vec<u32>,
    staged: Vec<(u32, u32, Entry)>,
    scratch: Vec<(u32, u32, Entry)>,
    counts: Vec<u32>,
    max_radius: Fx,
}

impl Layer {
    fn new(shift: u32, map_size: FxVec2) -> Layer {
        Layer {
            shift,
            width: (map_size.x.ceil_int().max(1) >> shift) as u32 + 1,
            height: (map_size.y.ceil_int().max(1) >> shift) as u32 + 1,
            row_start: Vec::new(),
            sorted: Vec::new(),
            cell_x: Vec::new(),
            staged: Vec::new(),
            scratch: Vec::new(),
            counts: Vec::new(),
            max_radius: Fx::ZERO,
        }
    }

    #[inline]
    fn cell(&self, p: FxVec2) -> (u32, u32) {
        (
            (p.x.floor_int() >> self.shift).clamp(0, self.width as i32 - 1) as u32,
            (p.y.floor_int() >> self.shift).clamp(0, self.height as i32 - 1) as u32,
        )
    }

    fn clear(&mut self) {
        self.staged.clear();
        self.max_radius = Fx::ZERO;
    }

    /// Two stable counting sorts, by column then by row: the result is in
    /// (row, column, insertion) order.
    fn build(&mut self) {
        let n = self.staged.len();
        self.scratch.clear();
        self.scratch.resize(n, (0, 0, EMPTY));
        stable_scatter(
            &self.staged,
            &mut self.scratch,
            &mut self.counts,
            self.width,
            |e| e.0,
        );
        self.staged.clear();
        self.staged.resize(n, (0, 0, EMPTY));
        stable_scatter(
            &self.scratch,
            &mut self.staged,
            &mut self.counts,
            self.height,
            |e| e.1,
        );
        self.row_start.clear();
        self.row_start.extend_from_slice(&self.counts);
        self.sorted.clear();
        self.cell_x.clear();
        for &(x, _, e) in &self.staged {
            self.sorted.push(e);
            self.cell_x.push(x);
        }
    }

    /// Visits the entries of cells `x0..=x1` by `y0..=y1`; false from `visit` stops.
    #[inline]
    fn scan(
        &self,
        center: FxVec2,
        radius: Fx,
        kinds: u8,
        skip: PlayerMask,
        tested: &mut u64,
        hits: &mut u64,
        cells: &mut i64,
        visit: &mut impl FnMut(&Entry) -> bool,
    ) -> bool {
        if self.sorted.is_empty() {
            return true;
        }
        let reach = radius + self.max_radius;
        let (x0, y0) = self.cell(FxVec2::new(center.x - reach, center.y - reach));
        let (x1, y1) = self.cell(FxVec2::new(center.x + reach, center.y + reach));
        *cells += (x1 - x0 + 1) as i64 * (y1 - y0 + 1) as i64;
        for cy in y0..=y1 {
            let (a, b) = (
                self.row_start[cy as usize] as usize,
                self.row_start[cy as usize + 1] as usize,
            );
            if a == b {
                continue;
            }
            let xs = &self.cell_x[a..b];
            let first = a + xs.partition_point(|&x| x < x0);
            for (i, e) in self.sorted[first..b].iter().enumerate() {
                if self.cell_x[first + i] > x1 {
                    break;
                }
                if e.kind & kinds == 0 || skip & owner_bit(e.owner) != 0 {
                    continue;
                }
                *tested += 1;
                let r = radius + e.radius;
                if e.pos.distance_sq(center) <= r * r {
                    *hits += 1;
                    if !visit(e) {
                        return false;
                    }
                }
            }
        }
        true
    }
}

/// The bit of `owner` in a mask of players; none for [`NO_OWNER`].
#[inline]
fn owner_bit(owner: u8) -> PlayerMask {
    PlayerMask::from(owner != NO_OWNER) << (owner as u32 % PlayerMask::BITS)
}

/// Stable counting sort of `from` into `to` by `key(e)` in `0..buckets`;
/// leaves `counts` as the bucket starts (length `buckets + 1`).
fn stable_scatter(
    from: &[(u32, u32, Entry)],
    to: &mut [(u32, u32, Entry)],
    counts: &mut Vec<u32>,
    buckets: u32,
    key: impl Fn(&(u32, u32, Entry)) -> u32,
) {
    counts.clear();
    counts.resize(buckets as usize + 1, 0);
    for e in from {
        counts[key(e) as usize + 1] += 1;
    }
    for i in 1..counts.len() {
        counts[i] += counts[i - 1];
    }
    let mut cursor = counts[..buckets as usize].to_vec();
    for e in from {
        let c = &mut cursor[key(e) as usize];
        to[*c as usize] = *e;
        *c += 1;
    }
}

pub struct SpatialIndex {
    layers: [Layer; LAYERS.len()],
}

impl SpatialIndex {
    pub fn new(map_size: FxVec2) -> SpatialIndex {
        SpatialIndex {
            layers: LAYERS.map(|(shift, _)| Layer::new(shift, map_size)),
        }
    }

    pub fn clear(&mut self) {
        for layer in &mut self.layers {
            layer.clear();
        }
    }

    #[inline]
    pub fn insert(&mut self, kind: u8, owner: u8, row: usize, pos: FxVec2, radius: Fx) {
        let at = LAYERS
            .iter()
            .position(|&(_, most)| radius <= Fx::from_int(most))
            .unwrap_or(LAYERS.len() - 1);
        let layer = &mut self.layers[at];
        let (cx, cy) = layer.cell(pos);
        layer.max_radius = layer.max_radius.max(radius);
        layer.staged.push((
            cx,
            cy,
            Entry {
                kind,
                owner,
                row: row as u32,
                pos,
                radius,
            },
        ));
    }

    /// Sorts staged entries into cells. Stable, so insertion order survives within a cell.
    pub fn build(&mut self) {
        for layer in &mut self.layers {
            layer.build();
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.layers.iter().map(|l| l.sorted.len()).sum()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Calls `visit` for every entry of a kind in `kinds` whose bounding circle
    /// touches the query circle. Return `false` from `visit` to stop early.
    pub fn query(&self, center: FxVec2, radius: Fx, kinds: u8, visit: impl FnMut(&Entry) -> bool) {
        self.query_foes(center, radius, kinds, 0, visit);
    }

    /// [`query`](Self::query) that passes over entries owned by the players in
    /// `friends` (bit `p` for player `p`) before any test: a gun looking for an
    /// enemy in the middle of its own army never sees the army.
    pub fn query_foes(
        &self,
        center: FxVec2,
        radius: Fx,
        kinds: u8,
        friends: PlayerMask,
        mut visit: impl FnMut(&Entry) -> bool,
    ) {
        let (mut tested, mut hits, mut cells) = (0u64, 0u64, 0i64);
        for layer in &self.layers {
            if !layer.scan(
                center,
                radius,
                kinds,
                friends,
                &mut tested,
                &mut hits,
                &mut cells,
                &mut visit,
            ) {
                break;
            }
        }
        mc_core::perf_count!("spatial.queries");
        mc_core::perf_count!("spatial.cells", cells);
        mc_core::perf_count!("spatial.tested", tested);
        mc_core::perf_count!("spatial.hits", hits);
    }

    /// Radius of the largest entry.
    pub fn max_radius(&self) -> Fx {
        self.layers
            .iter()
            .map(|l| l.max_radius)
            .fold(Fx::ZERO, Fx::max)
    }

    /// Nearest entry accepted by `accept`, by centre distance. Ties go to the
    /// entry found first: the smaller layer, then the lower cell, then the
    /// lower insertion order.
    pub fn nearest(
        &self,
        center: FxVec2,
        radius: Fx,
        kinds: u8,
        accept: impl FnMut(&Entry) -> bool,
    ) -> Option<Entry> {
        self.nearest_foe(center, radius, kinds, 0, accept)
    }

    /// [`nearest`](Self::nearest) among entries not owned by `friends`, as
    /// [`query_foes`](Self::query_foes).
    pub fn nearest_foe(
        &self,
        center: FxVec2,
        radius: Fx,
        kinds: u8,
        friends: PlayerMask,
        mut accept: impl FnMut(&Entry) -> bool,
    ) -> Option<Entry> {
        let mut best: Option<(Fx, Entry)> = None;
        self.query_foes(center, radius, kinds, friends, |e| {
            let d = e.pos.distance_sq(center);
            if best.as_ref().is_none_or(|(bd, _)| d < *bd) && accept(e) {
                best = Some((d, *e));
            }
            true
        });
        best.map(|(_, e)| e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_core::Rng;

    #[test]
    fn query_matches_brute_force() {
        let size = FxVec2::from_ints(4096, 4096);
        let mut index = SpatialIndex::new(size);
        let mut rng = Rng::new(3);
        let mut all = Vec::new();
        for row in 0..2000 {
            let pos = FxVec2::new(rng.range(Fx::ZERO, size.x), rng.range(Fx::ZERO, size.y));
            // Mostly small, with a few structures and giants in every layer.
            let radius = match row % 50 {
                0 => rng.range(Fx::from_int(65), Fx::from_int(200)),
                1..=5 => rng.range(Fx::from_int(17), Fx::from_int(64)),
                _ => rng.range(Fx::ONE, Fx::from_int(16)),
            };
            index.insert(kind::UNIT, NO_OWNER, row, pos, radius);
            all.push((pos, radius));
        }
        index.build();
        for _ in 0..200 {
            let c = FxVec2::new(rng.range(Fx::ZERO, size.x), rng.range(Fx::ZERO, size.y));
            let r = rng.range(Fx::ONE, Fx::from_int(300));
            let mut got = Vec::new();
            index.query(c, r, kind::UNIT, |e| {
                got.push(e.row);
                true
            });
            got.sort_unstable();
            let want: Vec<u32> = all
                .iter()
                .enumerate()
                .filter(|(_, (p, pr))| p.distance_sq(c) <= (r + *pr) * (r + *pr))
                .map(|(i, _)| i as u32)
                .collect();
            assert_eq!(got, want);
        }
    }

    #[test]
    fn kinds_filter_and_early_exit() {
        let mut index = SpatialIndex::new(FxVec2::from_ints(1024, 1024));
        index.insert(kind::UNIT, NO_OWNER, 0, FxVec2::from_ints(10, 10), Fx::ONE);
        index.insert(kind::WRECK, NO_OWNER, 1, FxVec2::from_ints(12, 10), Fx::ONE);
        index.insert(kind::UNIT, NO_OWNER, 2, FxVec2::from_ints(14, 10), Fx::ONE);
        index.build();
        let mut seen = 0;
        index.query(
            FxVec2::from_ints(10, 10),
            Fx::from_int(50),
            kind::WRECK,
            |e| {
                assert_eq!(e.row, 1);
                seen += 1;
                true
            },
        );
        assert_eq!(seen, 1);
        let mut visits = 0;
        index.query(
            FxVec2::from_ints(10, 10),
            Fx::from_int(50),
            kind::UNIT | kind::WRECK,
            |_| {
                visits += 1;
                false
            },
        );
        assert_eq!(visits, 1);
        let near = index
            .nearest(
                FxVec2::from_ints(13, 10),
                Fx::from_int(50),
                kind::UNIT,
                |_| true,
            )
            .unwrap();
        assert_eq!(near.row, 2);
    }
}
