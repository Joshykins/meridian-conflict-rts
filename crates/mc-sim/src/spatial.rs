//! The spatial index: uniform grids rebuilt by counting sort.
//!
//! Every proximity query in the simulation goes through here. Each kind of
//! entry has grids of its own, and within a kind entries are binned by centre
//! into one of a few layers by size: small entries on a fine grid, big
//! structures and giants on coarser ones. A query widens its search by each
//! layer's largest radius, so one titan no longer makes every infantry query
//! scan a 300 m square; a wide query reads one coarse grid of the kind
//! instead, and the giants' layer beside it. The kilometres-long hulls
//! (survival's machines) have a layer of their own on 1 km cells and keep out of the coarse
//! grid and the owner map's reach, so one of them does not widen every query
//! on the map by its radius. Results come out kind by kind, layer by layer
//! (smallest first, the giants last), and within a layer in (cell row-major,
//! insertion) order, which depends only on table contents and the query's size.

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

/// The layers as (cell edge shift, largest radius held): 16 m cells for
/// anything up to 16 m (units, wrecks, stains), 64 m cells up to 64 m
/// (structures, big hulls), 256 m cells up to 400 m (titans, capital ships),
/// 1 km cells for the rest (`GIANT`: survival's machines).
const LAYERS: [(u32, i32); 4] = [(4, 16), (6, 64), (8, 400), (10, i32::MAX)];
/// The giants' layer: not in the coarse grid (`Grids::wide`), which a wide
/// query reads it beside, nor the owner map's reach (`SpatialIndex::unit_radius`):
/// each giant marks every owner cell its radius covers instead.
const GIANT: usize = LAYERS.len() - 1;

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
    /// `build`'s two orders of `staged`, by index.
    by_column: Vec<u32>,
    by_row: Vec<u32>,
    counts: Vec<u32>,
    max_radius: Fx,
    /// Cleared or staged since the last `build`; a layer left alone keeps what it holds.
    dirty: bool,
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
            by_column: Vec::new(),
            by_row: Vec::new(),
            counts: Vec::new(),
            max_radius: Fx::ZERO,
            dirty: true,
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
        self.dirty = true;
    }

    /// Two stable counting sorts, by column then by row: the result is in
    /// (row, column, insertion) order.
    fn build(&mut self) {
        if !std::mem::replace(&mut self.dirty, false) {
            return;
        }
        let n = self.staged.len();
        if n == 0 {
            // Nothing to sort: `scan` reads an empty layer as nothing.
            self.sorted.clear();
            self.cell_x.clear();
            return;
        }
        // Indices go through the sorts, not entries: a tenth of the bytes moved, and a
        // battle of a hundred thousand builds a dozen layers three times a tick.
        let staged = &self.staged;
        self.by_row.clear();
        self.by_row.extend(0..n as u32);
        self.by_column.clear();
        self.by_column.resize(n, 0);
        stable_scatter(
            &self.by_row,
            &mut self.by_column,
            &mut self.counts,
            self.width,
            |i| staged[i as usize].0,
        );
        stable_scatter(
            &self.by_column,
            &mut self.by_row,
            &mut self.counts,
            self.height,
            |i| staged[i as usize].1,
        );
        self.row_start.clear();
        self.row_start.extend_from_slice(&self.counts);
        self.sorted.clear();
        self.cell_x.clear();
        for &i in &self.by_row {
            let (x, _, e) = staged[i as usize];
            self.sorted.push(e);
            self.cell_x.push(x);
        }
        // `staged` stays in insertion order: entries added before the next build land
        // after these in their cells, as they did when it was left sorted.
    }

    /// Visits the entries of cells `x0..=x1` by `y0..=y1`; false from `visit` stops.
    #[inline]
    /// Entries whose centre is within `inner` of `center` may be passed over unvisited
    /// (a ring query's caller has read them already); `Fx` below zero for none.
    #[expect(
        clippy::too_many_arguments,
        reason = "the scan's inputs and its three counters, all used on every entry"
    )]
    fn scan(
        &self,
        center: FxVec2,
        radius: Fx,
        inner: Fx,
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
            let (sx0, sx1) = self.inside(center, inner, cy);
            let mut j = a + xs.partition_point(|&x| x < x0);
            while j < b {
                let cx = self.cell_x[j];
                if cx > x1 {
                    break;
                }
                if (sx0..=sx1).contains(&cx) {
                    j = a + xs.partition_point(|&x| x <= sx1);
                    continue;
                }
                let e = &self.sorted[j];
                j += 1;
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

    /// The columns of row `cy` whose cells lie wholly within `inner` of `center` (less
    /// a metre), as an inclusive range; empty when none do. Cells on the map's edge
    /// hold what lies past it too, and are never in it.
    fn inside(&self, center: FxVec2, inner: Fx, cy: u32) -> (u32, u32) {
        let none = (1, 0);
        let inner = inner - Fx::ONE;
        if inner <= Fx::ZERO || cy == 0 || cy + 1 >= self.height {
            return none;
        }
        let side = Fx::from_int(1 << self.shift);
        let (lo, hi) = (side * cy as i32, side * (cy as i32 + 1));
        let fy = (lo - center.y).abs().max((hi - center.y).abs());
        if fy >= inner {
            return none;
        }
        let w = (inner * inner - fy * fy).sqrt();
        // Columns whose cells run from at least `center.x - w` to at most `center.x + w`.
        let first = ((center.x - w) / side).ceil_int().max(1);
        let last = ((center.x + w) / side).floor_int() - 1;
        let last = last.min(self.width as i32 - 2);
        if first > last {
            return none;
        }
        (first as u32, last as u32)
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
    from: &[u32],
    to: &mut [u32],
    counts: &mut Vec<u32>,
    buckets: u32,
    key: impl Fn(u32) -> u32,
) {
    counts.clear();
    counts.resize(buckets as usize + 1, 0);
    for &e in from {
        counts[key(e) as usize + 1] += 1;
    }
    for i in 1..counts.len() {
        counts[i] += counts[i - 1];
    }
    let mut cursor = counts[..buckets as usize].to_vec();
    for &e in from {
        let c = &mut cursor[key(e) as usize];
        to[*c as usize] = e;
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

    /// The layers a query of `radius` reads, in order: each layer, or for a wide one
    /// the coarse grid and then the giants'.
    fn read(&self, radius: Fx) -> impl Iterator<Item = &Layer> {
        let wide = radius >= WIDE_QUERY;
        let layers = (!wide).then(|| self.layers.iter()).into_iter().flatten();
        let coarse = wide
            .then(|| [&self.wide, &self.layers[GIANT]])
            .into_iter()
            .flatten();
        layers.chain(coarse)
    }
}

pub struct SpatialIndex {
    map_size: FxVec2,
    /// Per coarse (`WIDE_SHIFT`) cell, the players with a unit centred in it:
    /// an enemy search round an army with no enemy near ends before it starts.
    owners: Vec<PlayerMask>,
    owners_width: i32,
    owners_height: i32,
    /// The widest unit but the giants, how far a unit centred in one cell reaches
    /// out of it (a giant marks every cell it reaches, `insert`).
    unit_radius: Fx,
    /// One set of grids per exact entry kind (units, aircraft, wrecks,
    /// stains), in kind order: a search for units never walks past the scorch
    /// marks of a battle, nor one for aircraft past the army under them.
    kinds: Vec<(u8, Grids)>,
    /// The unit kinds' grids again, one set per owner: a search for enemies
    /// reads only the players it is after, never the shooter's own army
    /// round it. (Their big-entry grids stay empty.)
    by_owner: Vec<Vec<(u8, Grids)>>,
    /// Per kind given to `restage`, in kind order, the entries its grids were last
    /// built from: (row, centre, radius).
    statics: Vec<(u8, Vec<StaticEntry>)>,
    static_scratch: Vec<StaticEntry>,
}

type StaticEntry = (u32, FxVec2, Fx);

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
            statics: Vec::new(),
            static_scratch: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.statics.clear();
        self.clear_moving();
    }

    /// Clears every kind but those `restage` keeps: they are cleared by it, when they change.
    pub(crate) fn clear_moving(&mut self) {
        let statics = &self.statics;
        for (_, grids) in self
            .kinds
            .iter_mut()
            .filter(|(k, _)| statics.binary_search_by_key(k, |(s, _)| *s).is_err())
            .chain(self.by_owner.iter_mut().flatten())
        {
            for layer in grids.all_layers() {
                layer.clear();
            }
        }
        self.owners.fill(0);
        self.unit_radius = Fx::ZERO;
    }

    /// Stages every entry of `kind` (a kind with no owner: wrecks, scorch marks), as
    /// (row, centre, radius), after `clear_moving`. When they are exactly the entries
    /// its grids were last built from, the grids are kept as they are and `build`
    /// passes them over: wrecks and scorch marks mostly lie still from one rebuild
    /// to the next, and were most of a late battle's index. What a query finds, and
    /// in what order, depends only on the entries, so keeping is the same as rebuilding.
    pub(crate) fn restage(&mut self, kind: u8, entries: impl Iterator<Item = (usize, FxVec2, Fx)>) {
        debug_assert!(kind & UNIT_KINDS == 0, "units have owners and move");
        let mut now = std::mem::take(&mut self.static_scratch);
        now.clear();
        now.extend(entries.map(|(row, pos, radius)| (row as u32, pos, radius)));
        let at = match self.statics.binary_search_by_key(&kind, |(k, _)| *k) {
            Ok(i) => i,
            Err(i) => {
                // Not kept before: whatever its grids hold goes.
                self.statics.insert(i, (kind, Vec::new()));
                if let Ok(g) = self.kinds.binary_search_by_key(&kind, |(k, _)| *k) {
                    self.kinds[g].1.all_layers().for_each(Layer::clear);
                }
                i
            }
        };
        if self.statics[at].1 == now {
            self.static_scratch = now;
            return;
        }
        if let Ok(g) = self.kinds.binary_search_by_key(&kind, |(k, _)| *k) {
            self.kinds[g].1.all_layers().for_each(Layer::clear);
        }
        for &(row, pos, radius) in &now {
            self.insert(kind, NO_OWNER, row as usize, pos, radius);
        }
        self.static_scratch = std::mem::replace(&mut self.statics[at].1, now);
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
            layer.dirty = true;
        };
        let giant = at == GIANT;
        if kind & UNIT_KINDS != 0 && owner != NO_OWNER {
            if giant {
                self.mark_owner(pos, radius, owner);
            } else {
                let (x, y) = self.owner_cell(pos);
                self.owners[(y * self.owners_width + x) as usize] |= owner_bit(owner);
                self.unit_radius = self.unit_radius.max(radius);
            }
            let owner = owner as usize % PlayerMask::BITS as usize;
            if self.by_owner.len() <= owner {
                self.by_owner.resize_with(owner + 1, Vec::new);
            }
            let grids = Self::grids_of(&mut self.by_owner[owner], kind, self.map_size);
            stage(&mut grids.layers[at]);
            if !giant {
                stage(&mut grids.wide);
            }
        }
        let grids = Self::grids_of(&mut self.kinds, kind, self.map_size);
        // Only units are searched by size (`query_wider`): a battle's wrecks and scorch
        // marks would fill the big-entry grids for nothing.
        let wider = BIG_FLOORS
            .iter()
            .take_while(|&&floor| kind & UNIT_KINDS != 0 && radius > Fx::from_int(floor))
            .count();
        let (layers, wide, big) = (&mut grids.layers, &mut grids.wide, &mut grids.big);
        let bigs = big[..wider].iter_mut().map(|layers| &mut layers[at]);
        let wide = (!giant).then_some(wide);
        for layer in std::iter::once(&mut layers[at]).chain(wide).chain(bigs) {
            stage(layer);
        }
    }

    /// Marks `owner` in every owner cell a giant at `pos` of `radius` reaches.
    fn mark_owner(&mut self, pos: FxVec2, radius: Fx, owner: u8) {
        let (x0, y0) = self.owner_cell(FxVec2::new(pos.x - radius, pos.y - radius));
        let (x1, y1) = self.owner_cell(FxVec2::new(pos.x + radius, pos.y + radius));
        for y in y0..=y1 {
            let row = &mut self.owners[(y * self.owners_width) as usize..];
            for cell in &mut row[x0 as usize..=x1 as usize] {
                *cell |= owner_bit(owner);
            }
        }
    }

    /// Sorts staged entries into cells. Stable, so insertion order survives within a cell.
    pub fn build(&mut self) {
        for layer in self.all_layers() {
            layer.build();
        }
    }

    /// [`build`](Self::build), the layers side by side on `pool`.
    pub(crate) fn build_on(&mut self, pool: &mc_jobs::Pool) {
        let (mut full, empty): (Vec<&mut Layer>, Vec<&mut Layer>) = self
            .all_layers()
            .partition(|layer| !layer.staged.is_empty());
        for layer in empty {
            layer.build();
        }
        pool.parallel_chunks_mut(&mut full, 1, |_, layers| {
            for layer in layers {
                layer.build();
            }
        });
    }

    fn all_layers(&mut self) -> impl Iterator<Item = &mut Layer> {
        self.kinds
            .iter_mut()
            .chain(self.by_owner.iter_mut().flatten())
            .flat_map(|(_, grids)| grids.all_layers())
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.kinds
            .iter()
            .map(|(_, g)| g.wide.sorted.len() + g.layers[GIANT].sorted.len())
            .sum()
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
        visit: impl FnMut(&Entry) -> bool,
    ) {
        self.query_foes_ring(center, Fx::from_int(-1), radius, kinds, friends, visit);
    }

    /// [`query_foes`](Self::query_foes) for a caller that has read what lies within
    /// `inner` already: entries there may be passed over unvisited, whole cells at a
    /// time. Those it visits come in the order `query_foes` visits them.
    pub fn query_foes_ring(
        &self,
        center: FxVec2,
        inner: Fx,
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
                    for layer in grids.read(radius) {
                        if !layer.scan(
                            center,
                            radius,
                            inner,
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
            for layer in grids.read(radius) {
                if !layer.scan(
                    center,
                    radius,
                    inner,
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
    /// visited in the order `query` visits them. Units only: no other kind has
    /// big-entry grids.
    pub fn query_wider(
        &self,
        center: FxVec2,
        radius: Fx,
        kinds: u8,
        wider_than: Fx,
        mut visit: impl FnMut(&Entry) -> bool,
    ) {
        debug_assert!(kinds & !UNIT_KINDS == 0, "only units have big-entry grids");
        let floor = BIG_FLOORS
            .iter()
            .rposition(|&floor| Fx::from_int(floor) <= wider_than);
        let inner = Fx::from_int(-1);
        let (mut tested, mut hits, mut cells) = (0u64, 0u64, 0i64);
        mc_core::perf_count!("spatial.queries");
        let mut keep = |e: &Entry| e.radius <= wider_than || visit(e);
        'kinds: for (_, grids) in self.kinds.iter().filter(|(k, _)| k & kinds != 0) {
            // Narrow, the grids of the entries wider than the floor; else what `query` reads.
            let big = floor.filter(|_| radius < WIDE_QUERY);
            let narrow = big.map(|f| grids.big[f].iter()).into_iter().flatten();
            let plain = big.is_none().then(|| grids.read(radius));
            for layer in narrow.chain(plain.into_iter().flatten()) {
                if !layer.scan(
                    center,
                    radius,
                    inner,
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
            .map(|(_, g)| g.wide.max_radius.max(g.layers[GIANT].max_radius))
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
            // Mostly small, with a few structures and giants in every layer, and a
            // kilometres-long hull or two in the giants' own.
            let radius = match row % 50 {
                0 if row % 700 == 0 => rng.range(Fx::from_int(401), Fx::from_int(1900)),
                0 => rng.range(Fx::from_int(65), Fx::from_int(200)),
                1..=5 => rng.range(Fx::from_int(17), Fx::from_int(64)),
                _ => rng.range(Fx::ONE, Fx::from_int(16)),
            };
            index.insert(kind::UNIT, NO_OWNER, row, pos, radius);
            all.push((pos, radius));
        }
        index.build();
        for q in 0..200 {
            let c = FxVec2::new(rng.range(Fx::ZERO, size.x), rng.range(Fx::ZERO, size.y));
            // Some wide enough to read the coarse grid and the giants' layer beside it.
            let r = rng.range(Fx::ONE, Fx::from_int(if q % 8 == 0 { 900 } else { 300 }));
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
            // A few titans, and two kilometres-long hulls reaching far out of their clumps
            // (they mark the owner map over all they reach).
            let radius = if row % 1499 == 0 {
                Fx::from_int(1900)
            } else if row % 97 == 0 {
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
    fn kept_static_kinds_answer_as_a_fresh_build() {
        let size = FxVec2::from_ints(2048, 2048);
        let mut rng = Rng::new(5);
        let spot = |rng: &mut Rng| {
            let pos = FxVec2::new(rng.range(Fx::ZERO, size.x), rng.range(Fx::ZERO, size.y));
            (pos, rng.range(Fx::ONE, Fx::from_int(30)))
        };
        let mut wrecks: Vec<(FxVec2, Fx)> = (0..400).map(|_| spot(&mut rng)).collect();
        let units: Vec<(FxVec2, Fx)> = (0..300).map(|_| spot(&mut rng)).collect();
        let mut kept = SpatialIndex::new(size);
        // Unchanged, unchanged, one moved, one gone, unchanged.
        for step in 0..5 {
            match step {
                2 => wrecks[17].0 = FxVec2::from_ints(5, 5),
                3 => {
                    wrecks.pop();
                }
                _ => {}
            }
            let mut fresh = SpatialIndex::new(size);
            fresh.clear();
            kept.clear_moving();
            for index in [&mut fresh, &mut kept] {
                for (row, &(pos, radius)) in units.iter().enumerate() {
                    index.insert(kind::UNIT, (row % 3) as u8, row, pos, radius);
                }
                let entries = wrecks.iter().enumerate().map(|(row, &(p, r))| (row, p, r));
                index.restage(kind::WRECK, entries);
                index.build();
            }
            for _ in 0..100 {
                let (c, r) = spot(&mut rng);
                let r = r * 8;
                let mut got = [Vec::new(), Vec::new()];
                for (i, index) in [&fresh, &kept].into_iter().enumerate() {
                    index.query(c, r, kind::UNIT | kind::WRECK, |e| {
                        got[i].push((e.kind, e.row));
                        true
                    });
                }
                assert_eq!(got[0], got[1], "step {step}");
            }
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
