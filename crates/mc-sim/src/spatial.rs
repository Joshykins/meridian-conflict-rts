//! The spatial index: uniform grids rebuilt by counting sort.
//!
//! Every proximity query in the simulation goes through here. Each kind of
//! entry has grids of its own, and within a kind entries are binned by centre
//! into one of a few layers by size: small entries on a fine grid, big
//! structures and giants on coarser ones. A query widens its search by each
//! layer's largest radius, so one titan no longer makes every infantry query
//! scan a 300 m square; a wide query reads one coarse grid of the kind
//! instead. Results come out kind by kind, layer by layer (smallest first), and
//! within a layer in (cell row-major, insertion) order, which depends only on
//! table contents and the query's size.

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

/// The kinds that are units, the only entries with owners.
const UNIT_KINDS: u8 = kind::UNIT | kind::AIRCRAFT;

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

/// A query this wide or wider (a fighter's sweep, an artillery reach) reads one
/// coarse 128 m grid over everything instead: row by row, the fine layers
/// would walk hundreds of rows for it.
const WIDE_QUERY: Fx = Fx::from_int(512);
/// Cell edge shift of the coarse grid.
const WIDE_SHIFT: u32 = 7;

/// A nearest search reads this far round its centre first, then four times as
/// far, and so on out to its reach: in a crowd the nearest is near, and the
/// rest of a gun's reach need never be read.
const NEAREST_FIRST: Fx = Fx::from_int(48);

/// The big-entry grids hold, again, the entries wider than each of these
/// radii, metres. A search for hulls bigger than the one asking (a tank
/// looking out for a Fulgur) reads the grids of the widest floor under its
/// limit instead of walking past every tank of the crowd round it.
const BIG_FLOORS: [i32; 3] = [4, 6, 9];

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
        if n == 0 {
            // Nothing to sort: `scan` reads an empty layer as nothing.
            self.sorted.clear();
            self.cell_x.clear();
            return;
        }
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
                if skip & owner_bit(e.owner) != 0 {
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

/// The layers of one kind of entry, and the coarse grid over them all.
struct Grids {
    layers: [Layer; LAYERS.len()],
    /// Every entry of the kind again, for wide queries.
    wide: Layer,
    /// `big[i]`: the layers again with only the entries wider than
    /// `BIG_FLOORS[i]`, each in the layer and order it has in `layers`.
    big: [[Layer; LAYERS.len()]; BIG_FLOORS.len()],
}

impl Grids {
    fn new(size: FxVec2) -> Grids {
        Grids {
            layers: LAYERS.map(|(shift, _)| Layer::new(shift, size)),
            wide: Layer::new(WIDE_SHIFT, size),
            big: BIG_FLOORS.map(|_| LAYERS.map(|(shift, _)| Layer::new(shift, size))),
        }
    }

    fn all_layers(&mut self) -> impl Iterator<Item = &mut Layer> {
        self.layers
            .iter_mut()
            .chain(std::iter::once(&mut self.wide))
            .chain(self.big.iter_mut().flatten())
    }
}

pub struct SpatialIndex {
    map_size: FxVec2,
    /// Per coarse (`WIDE_SHIFT`) cell, the players with a unit centred in it:
    /// an enemy search round an army with no enemy near ends before it starts.
    owners: Vec<PlayerMask>,
    owners_width: i32,
    owners_height: i32,
    /// The widest unit, how far a unit centred in one cell reaches out of it.
    unit_radius: Fx,
    /// One set of grids per exact entry kind (units, aircraft, wrecks,
    /// stains), in kind order: a search for units never walks past the scorch
    /// marks of a battle, nor one for aircraft past the army under them.
    kinds: Vec<(u8, Grids)>,
    /// The unit kinds' grids again, one set per owner: a search for enemies
    /// reads only the players it is after, never the shooter's own army
    /// round it. (Their big-entry grids stay empty.)
    by_owner: Vec<Vec<(u8, Grids)>>,
}

impl SpatialIndex {
    pub fn new(map_size: FxVec2) -> SpatialIndex {
        let (w, h) = (
            (map_size.x.ceil_int().max(1) >> WIDE_SHIFT) + 1,
            (map_size.y.ceil_int().max(1) >> WIDE_SHIFT) + 1,
        );
        SpatialIndex {
            map_size,
            owners: vec![0; (w * h) as usize],
            owners_width: w,
            owners_height: h,
            unit_radius: Fx::ZERO,
            kinds: Vec::new(),
            by_owner: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        for (_, grids) in self
            .kinds
            .iter_mut()
            .chain(self.by_owner.iter_mut().flatten())
        {
            for layer in grids.all_layers() {
                layer.clear();
            }
        }
        self.owners.fill(0);
        self.unit_radius = Fx::ZERO;
    }

    #[inline]
    fn owner_cell(&self, p: FxVec2) -> (i32, i32) {
        (
            (p.x.floor_int() >> WIDE_SHIFT).clamp(0, self.owners_width - 1),
            (p.y.floor_int() >> WIDE_SHIFT).clamp(0, self.owners_height - 1),
        )
    }

    /// The players with a unit that may touch the circle.
    fn owners_near(&self, center: FxVec2, radius: Fx) -> PlayerMask {
        let reach = radius + self.unit_radius;
        let (x0, y0) = self.owner_cell(FxVec2::new(center.x - reach, center.y - reach));
        let (x1, y1) = self.owner_cell(FxVec2::new(center.x + reach, center.y + reach));
        (y0..=y1).fold(0, |near, y| {
            let row = &self.owners[(y * self.owners_width) as usize..];
            row[x0 as usize..=x1 as usize]
                .iter()
                .fold(near, |near, &m| near | m)
        })
    }

    /// The grids of `kind`, made the first time one is asked for.
    fn grids_of(list: &mut Vec<(u8, Grids)>, kind: u8, size: FxVec2) -> &mut Grids {
        let slot = match list.binary_search_by_key(&kind, |(k, _)| *k) {
            Ok(i) => i,
            Err(i) => {
                list.insert(i, (kind, Grids::new(size)));
                i
            }
        };
        &mut list[slot].1
    }

    #[inline]
    pub fn insert(&mut self, kind: u8, owner: u8, row: usize, pos: FxVec2, radius: Fx) {
        let at = LAYERS
            .iter()
            .position(|&(_, most)| radius <= Fx::from_int(most))
            .unwrap_or(LAYERS.len() - 1);
        let entry = Entry {
            kind,
            owner,
            row: row as u32,
            pos,
            radius,
        };
        let stage = |layer: &mut Layer| {
            let (cx, cy) = layer.cell(pos);
            layer.max_radius = layer.max_radius.max(radius);
            layer.staged.push((cx, cy, entry));
        };
        if kind & UNIT_KINDS != 0 && owner != NO_OWNER {
            let (x, y) = self.owner_cell(pos);
            self.owners[(y * self.owners_width + x) as usize] |= owner_bit(owner);
            self.unit_radius = self.unit_radius.max(radius);
            let owner = owner as usize % PlayerMask::BITS as usize;
            if self.by_owner.len() <= owner {
                self.by_owner.resize_with(owner + 1, Vec::new);
            }
            let grids = Self::grids_of(&mut self.by_owner[owner], kind, self.map_size);
            stage(&mut grids.layers[at]);
            stage(&mut grids.wide);
        }
        let grids = Self::grids_of(&mut self.kinds, kind, self.map_size);
        let wider = BIG_FLOORS
            .iter()
            .take_while(|&&floor| radius > Fx::from_int(floor))
            .count();
        let (layers, wide, big) = (&mut grids.layers, &mut grids.wide, &mut grids.big);
        let bigs = big[..wider].iter_mut().map(|layers| &mut layers[at]);
        for layer in [&mut layers[at], wide].into_iter().chain(bigs) {
            stage(layer);
        }
    }

    /// Sorts staged entries into cells. Stable, so insertion order survives within a cell.
    pub fn build(&mut self) {
        for (_, grids) in self
            .kinds
            .iter_mut()
            .chain(self.by_owner.iter_mut().flatten())
        {
            for layer in grids.all_layers() {
                layer.build();
            }
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.kinds.iter().map(|(_, g)| g.wide.sorted.len()).sum()
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
    /// `friends` (bit `p` for player `p`): a gun looking for an enemy in the
    /// middle of its own army never reads the army. A search for units alone
    /// reads the grids of each other player near, in player order (then kind,
    /// layer and cell); any other reads the kinds' grids, skipping friends.
    pub fn query_foes(
        &self,
        center: FxVec2,
        radius: Fx,
        kinds: u8,
        friends: PlayerMask,
        mut visit: impl FnMut(&Entry) -> bool,
    ) {
        let (mut tested, mut hits, mut cells) = (0u64, 0u64, 0i64);
        mc_core::perf_count!("spatial.queries");
        // Only units have owners: a search for them alone asks the owner map
        // which enemies are near at all, and reads only their grids.
        if friends != 0 && kinds & !UNIT_KINDS == 0 {
            let mut foes = self.owners_near(center, radius) & !friends;
            if foes == 0 {
                mc_core::perf_count!("spatial.no_foe");
            }
            'owners: while foes != 0 {
                let owner = foes.trailing_zeros() as usize;
                foes &= foes - 1;
                let Some(list) = self.by_owner.get(owner) else {
                    continue;
                };
                for (_, grids) in list.iter().filter(|(k, _)| k & kinds != 0) {
                    let layers = if radius >= WIDE_QUERY {
                        std::slice::from_ref(&grids.wide)
                    } else {
                        &grids.layers[..]
                    };
                    for layer in layers {
                        if !layer.scan(
                            center,
                            radius,
                            0,
                            &mut tested,
                            &mut hits,
                            &mut cells,
                            &mut visit,
                        ) {
                            break 'owners;
                        }
                    }
                }
            }
            mc_core::perf_count!("spatial.cells", cells);
            mc_core::perf_count!("spatial.tested", tested);
            mc_core::perf_count!("spatial.hits", hits);
            return;
        }
        'kinds: for (_, grids) in self.kinds.iter().filter(|(k, _)| k & kinds != 0) {
            let layers = if radius >= WIDE_QUERY {
                std::slice::from_ref(&grids.wide)
            } else {
                &grids.layers[..]
            };
            for layer in layers {
                if !layer.scan(
                    center,
                    radius,
                    friends,
                    &mut tested,
                    &mut hits,
                    &mut cells,
                    &mut visit,
                ) {
                    break 'kinds;
                }
            }
        }
        mc_core::perf_count!("spatial.cells", cells);
        mc_core::perf_count!("spatial.tested", tested);
        mc_core::perf_count!("spatial.hits", hits);
    }

    /// [`query`](Self::query) for the entries wider than `wider_than` alone,
    /// visited in the order `query` visits them.
    pub fn query_wider(
        &self,
        center: FxVec2,
        radius: Fx,
        kinds: u8,
        wider_than: Fx,
        mut visit: impl FnMut(&Entry) -> bool,
    ) {
        let floor = BIG_FLOORS
            .iter()
            .rposition(|&floor| Fx::from_int(floor) <= wider_than);
        let (mut tested, mut hits, mut cells) = (0u64, 0u64, 0i64);
        mc_core::perf_count!("spatial.queries");
        let mut keep = |e: &Entry| e.radius <= wider_than || visit(e);
        'kinds: for (_, grids) in self.kinds.iter().filter(|(k, _)| k & kinds != 0) {
            let layers = match floor {
                _ if radius >= WIDE_QUERY => std::slice::from_ref(&grids.wide),
                Some(floor) => &grids.big[floor][..],
                None => &grids.layers[..],
            };
            for layer in layers {
                if !layer.scan(
                    center,
                    radius,
                    0,
                    &mut tested,
                    &mut hits,
                    &mut cells,
                    &mut keep,
                ) {
                    break 'kinds;
                }
            }
        }
        mc_core::perf_count!("spatial.cells", cells);
        mc_core::perf_count!("spatial.tested", tested);
        mc_core::perf_count!("spatial.hits", hits);
    }

    /// Radius of the largest entry.
    pub fn max_radius(&self) -> Fx {
        self.kinds
            .iter()
            .map(|(_, g)| g.wide.max_radius)
            .fold(Fx::ZERO, Fx::max)
    }

    /// Nearest entry accepted by `accept`, by centre distance. Ties go to the
    /// entry found first: the lower kind, then the smaller layer (a wide
    /// search has one), then the lower cell, then the lower insertion order
    /// (for a foe search, the lower owner before all of those).
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
        let mut within = |r: Fx| {
            let mut best: Option<(Fx, Entry)> = None;
            self.query_foes(center, r, kinds, friends, |e| {
                let d = e.pos.distance_sq(center);
                if best.as_ref().is_none_or(|(bd, _)| d < *bd) && accept(e) {
                    best = Some((d, *e));
                }
                true
            });
            best
        };
        // Short of a wide search, a smaller circle reads the same layers in the
        // same order, so the nearest it finds no further out than its own radius
        // is the answer for the whole reach, ties and all: everything it did not
        // read lies further out than that. (`accept` is asked again on a wider
        // ring; it only reads.)
        let mut r = NEAREST_FIRST;
        while r < radius && radius < WIDE_QUERY {
            if let Some((d, e)) = within(r) {
                if d <= r * r {
                    return Some(e);
                }
            }
            r = r * 4;
        }
        within(radius).map(|(_, e)| e)
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
    fn wider_queries_match_full_queries() {
        let size = FxVec2::from_ints(4096, 4096);
        let mut index = SpatialIndex::new(size);
        let mut rng = Rng::new(9);
        for row in 0..3000 {
            let pos = FxVec2::new(rng.range(Fx::ZERO, size.x), rng.range(Fx::ZERO, size.y));
            let radius = match row % 40 {
                0 => rng.range(Fx::from_int(65), Fx::from_int(200)),
                1..=3 => rng.range(Fx::from_int(17), Fx::from_int(64)),
                _ => rng.range(Fx::ONE, Fx::from_int(16)),
            };
            index.insert(kind::UNIT, NO_OWNER, row, pos, radius);
        }
        index.build();
        for q in 0..300 {
            let c = FxVec2::new(rng.range(Fx::ZERO, size.x), rng.range(Fx::ZERO, size.y));
            let r = rng.range(Fx::ONE, Fx::from_int(if q % 10 == 0 { 700 } else { 120 }));
            let wider = rng.range(Fx::ZERO, Fx::from_int(20));
            let mut want = Vec::new();
            index.query(c, r, kind::UNIT, |e| {
                if e.radius > wider {
                    want.push(e.row);
                }
                true
            });
            let mut got = Vec::new();
            index.query_wider(c, r, kind::UNIT, wider, |e| {
                got.push(e.row);
                true
            });
            // The same entries in the same order: callers may keep the first of a tie.
            assert_eq!(got, want, "query {q}");
        }
    }

    /// The ring-by-ring nearest search gives the answer of one search over the
    /// whole reach, the first found of a tie included.
    #[test]
    fn nearest_matches_one_full_search() {
        let size = FxVec2::from_ints(4096, 4096);
        let mut index = SpatialIndex::new(size);
        let mut rng = Rng::new(11);
        for row in 0..4000 {
            // On a 4 m lattice in places, so ties in distance are common.
            let pos = if row % 3 == 0 {
                FxVec2::from_ints(
                    rng.below(200) as i32 * 4 + 1000,
                    rng.below(200) as i32 * 4 + 1000,
                )
            } else {
                FxVec2::new(rng.range(Fx::ZERO, size.x), rng.range(Fx::ZERO, size.y))
            };
            let radius = match row % 60 {
                0 => rng.range(Fx::from_int(17), Fx::from_int(64)),
                _ => rng.range(Fx::ONE, Fx::from_int(12)),
            };
            index.insert(kind::UNIT, (row % 4) as u8, row, pos, radius);
        }
        index.build();
        for q in 0..400 {
            let c = if q % 2 == 0 {
                FxVec2::from_ints(
                    rng.below(200) as i32 * 4 + 1000,
                    rng.below(200) as i32 * 4 + 1000,
                )
            } else {
                FxVec2::new(rng.range(Fx::ZERO, size.x), rng.range(Fx::ZERO, size.y))
            };
            let r = rng.range(Fx::ONE, Fx::from_int(600));
            let friends: PlayerMask = 1 << (q % 4);
            let kept = |e: &Entry| !e.row.is_multiple_of(7);
            let mut best: Option<(Fx, Entry)> = None;
            index.query_foes(c, r, kind::UNIT, friends, |e| {
                let d = e.pos.distance_sq(c);
                if best.as_ref().is_none_or(|(bd, _)| d < *bd) && kept(e) {
                    best = Some((d, *e));
                }
                true
            });
            let got = index.nearest_foe(c, r, kind::UNIT, friends, kept);
            assert_eq!(got.map(|e| e.row), best.map(|(_, e)| e.row), "query {q}");
        }
    }

    #[test]
    fn foe_queries_match_brute_force() {
        let size = FxVec2::from_ints(8192, 8192);
        let mut index = SpatialIndex::new(size);
        let mut rng = Rng::new(5);
        let mut all = Vec::new();
        for row in 0..3000 {
            // Armies in a few clumps, so most searches have no foe near.
            let clump =
                FxVec2::from_ints((row % 5) as i32 * 1600 + 400, (row % 3) as i32 * 2500 + 400);
            let jitter = FxVec2::new(
                rng.range(Fx::ZERO, Fx::from_int(300)),
                rng.range(Fx::ZERO, Fx::from_int(300)),
            );
            let pos = clump + jitter;
            let radius = if row % 97 == 0 {
                Fx::from_int(120)
            } else {
                rng.range(Fx::ONE, Fx::from_int(12))
            };
            let owner = (row % 5) as u8 * 6 + (row % 2) as u8;
            let kinds = if row % 7 == 0 {
                kind::UNIT | kind::AIRCRAFT
            } else {
                kind::UNIT
            };
            index.insert(kinds, owner, row, pos, radius);
            all.push((pos, radius, owner, kinds));
        }
        index.build();
        for q in 0..400 {
            let c = FxVec2::new(rng.range(Fx::ZERO, size.x), rng.range(Fx::ZERO, size.y));
            let r = rng.range(Fx::ONE, Fx::from_int(700));
            let friends: PlayerMask = 0b11 << ((q % 5) * 6);
            let kinds = if q % 3 == 0 {
                kind::AIRCRAFT
            } else {
                kind::UNIT
            };
            let mut got = Vec::new();
            index.query_foes(c, r, kinds, friends, |e| {
                got.push(e.row);
                true
            });
            got.sort_unstable();
            let want: Vec<u32> = all
                .iter()
                .enumerate()
                .filter(|(_, (p, pr, o, k))| {
                    k & kinds != 0
                        && friends & owner_bit(*o) == 0
                        && p.distance_sq(c) <= (r + *pr) * (r + *pr)
                })
                .map(|(i, _)| i as u32)
                .collect();
            assert_eq!(got, want, "query {q}");
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
