//! Pockets: small patches of open ground that structures' hulls have shut in, such
//! as the strip of apron between a column of fabricators and the power generators
//! either side, built round the engineer that stood there (replay 20261004-172909
//! mark 2). Nothing reaches one along the ground, so the sim counts a pocket as
//! ground no hull stands on: a unit in one is stranded and walks out through the
//! hulls the short way, and no goal is put in one.
//!
//! Kept up as hulls come and go, and snapshotted with the rest of pathing.

use mc_path::{Cell, CellRect, MoveLayer, SizeClass, SIZE_CLASSES};
use std::collections::BTreeSet;

/// An open region of at most this many path cells (8 m), shut in by hulls, is a pocket.
const MAX_CELLS: usize = 32;

const LAYERS: [MoveLayer; 4] = [
    MoveLayer::Land,
    MoveLayer::Amphibious,
    MoveLayer::Naval,
    MoveLayer::Hover,
];

/// The eight neighbours, axis steps first.
const STEPS: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

/// (layer, size class, cell) of every cell of every pocket.
type Key = (u8, u8, Cell);

#[derive(Default)]
pub(super) struct Pockets {
    cells: BTreeSet<Key>,
}

impl Pockets {
    pub(super) fn contains(&self, layer: MoveLayer, size: SizeClass, c: Cell) -> bool {
        !self.cells.is_empty() && self.cells.contains(&(layer as u8, size.index(), c))
    }

    /// Works the pockets out again round `rect`, whose cells were just blocked or opened.
    pub(super) fn update(&mut self, nav: &mc_path::Nav, rect: CellRect) {
        for layer in LAYERS {
            for class in 0..SIZE_CLASSES {
                let Ok(size) = SizeClass::new(class) else {
                    continue;
                };
                // A blocker changes which cells a hull fits in up to its own size away.
                let grow = i32::from(size.cells()) + 1;
                let window = CellRect::new(
                    Cell::new(rect.min.x - grow, rect.min.y - grow),
                    Cell::new(rect.max.x + grow, rect.max.y + grow),
                );
                self.forget(layer, size, window);
                self.find(nav, layer, size, window);
            }
        }
    }

    /// Drops every pocket with a cell in `window`, whole: one may reach out of it.
    fn forget(&mut self, layer: MoveLayer, size: SizeClass, window: CellRect) {
        let (l, s) = (layer as u8, size.index());
        let lo = (l, s, Cell::new(window.min.x, i32::MIN));
        let hi = (l, s, Cell::new(window.max.x, i32::MIN));
        let mut stack: Vec<Cell> = self
            .cells
            .range(lo..hi)
            .map(|k| k.2)
            .filter(|c| window.contains(*c))
            .collect();
        while let Some(c) = stack.pop() {
            if !self.cells.remove(&(l, s, c)) {
                continue;
            }
            for (dx, dy) in STEPS {
                let n = Cell::new(c.x + dx, c.y + dy);
                if self.cells.contains(&(l, s, n)) {
                    stack.push(n);
                }
            }
        }
    }

    /// Finds the pockets with a cell in `window`.
    fn find(&mut self, nav: &mc_path::Nav, layer: MoveLayer, size: SizeClass, window: CellRect) {
        let pass = |c: Cell| nav.is_passable(layer, size, c);
        // Cells already flooded from another seed: none starts a second flood.
        let mut seen: BTreeSet<Cell> = BTreeSet::new();
        // Cells known to be in a region too big to be a pocket.
        let mut open: BTreeSet<Cell> = BTreeSet::new();
        for y in window.min.y..window.max.y {
            for x in window.min.x..window.max.x {
                let seed = Cell::new(x, y);
                if seen.contains(&seed) || !pass(seed) {
                    continue;
                }
                let region = flood(seed, pass, |c| open.contains(&c));
                seen.extend(region.cells.iter().copied());
                if !region.shut {
                    open.extend(region.cells);
                    continue;
                }
                // Shut in by the terrain alone (a ledge, a pond), it is the map's
                // shape, not something built round a unit.
                let grid = nav.grid();
                let ground = |c: Cell| grid.contains(c) && layer.passable(grid.terrain_class(c));
                if !flood(seed, ground, |_| false).shut {
                    let (l, s) = (layer as u8, size.index());
                    self.cells
                        .extend(region.cells.into_iter().map(|c| (l, s, c)));
                }
            }
        }
    }

    pub(super) fn export(&self) -> Vec<(u8, u8, i32, i32)> {
        self.cells
            .iter()
            .map(|&(l, s, c)| (l, s, c.x, c.y))
            .collect()
    }

    /// From a snapshot's `export`; entries naming no layer or size class are dropped.
    pub(super) fn import(cells: &[(u8, u8, i32, i32)]) -> Pockets {
        Pockets {
            cells: cells
                .iter()
                .filter(|&&(l, s, _, _)| usize::from(l) < LAYERS.len() && s < SIZE_CLASSES)
                .map(|&(l, s, x, y)| (l, s, Cell::new(x, y)))
                .collect(),
        }
    }

    /// Nothing while there are none, so a match with no pocket hashes as before.
    pub(super) fn hash(&self, h: &mut mc_core::StateHasher) {
        if self.cells.is_empty() {
            return;
        }
        h.write_u64(self.cells.len() as u64);
        for &(l, s, c) in &self.cells {
            h.write_u64(u64::from(l) << 56 | u64::from(s) << 48);
            h.write_u64((c.x as u32 as u64) << 32 | c.y as u32 as u64);
        }
    }

    pub(super) fn len(&self) -> usize {
        self.cells.len()
    }
}

struct Region {
    cells: Vec<Cell>,
    /// Whether it is no bigger than `MAX_CELLS`.
    shut: bool,
}

/// The region of `pass` cells round `seed`, moving as units do (a diagonal step
/// needs both cells beside it open), up to one cell past `MAX_CELLS`. Reaching a
/// cell `known_open` says it is open without going further.
fn flood(seed: Cell, pass: impl Fn(Cell) -> bool, known_open: impl Fn(Cell) -> bool) -> Region {
    let mut cells = vec![seed];
    let mut next = 0;
    while next < cells.len() {
        let c = cells[next];
        next += 1;
        for (dx, dy) in STEPS {
            let n = Cell::new(c.x + dx, c.y + dy);
            if cells.contains(&n) || !pass(n) {
                continue;
            }
            if dx != 0
                && dy != 0
                && !(pass(Cell::new(c.x + dx, c.y)) && pass(Cell::new(c.x, c.y + dy)))
            {
                continue;
            }
            if known_open(n) {
                return Region { cells, shut: false };
            }
            cells.push(n);
            if cells.len() > MAX_CELLS {
                return Region { cells, shut: false };
            }
        }
    }
    Region { cells, shut: true }
}
