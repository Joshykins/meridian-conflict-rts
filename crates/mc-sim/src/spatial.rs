//! The spatial index: a uniform grid rebuilt by counting sort.
//!
//! Every proximity query in the simulation goes through here. Entries are
//! binned by centre; queries widen their search by the largest radius in the
//! index so big structures are still found from their edges. Results come out
//! in (cell row-major, insertion) order, which depends only on table contents.

use mc_core::{Fx, FxVec2};

/// Entity kinds an entry can refer to. Queries filter by a mask of these.
pub mod kind {
    pub const UNIT: u8 = 1 << 0;
    pub const WRECK: u8 = 1 << 1;
    pub const STAIN: u8 = 1 << 2;
    pub const PROP: u8 = 1 << 3;
}

#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub kind: u8,
    /// Row in the entity's table.
    pub row: u32,
    pub pos: FxVec2,
    pub radius: Fx,
}

/// Grid cell edge: 128 m. Coarse enough that rebuilding the cell table of an
/// 80 km map stays well under a millisecond, fine enough that a weapon-range
/// query touches a handful of cells.
const CELL_SHIFT: u32 = 7;

pub struct SpatialIndex {
    width: i32,
    height: i32,
    /// `cell_start[c]..cell_start[c + 1]` indexes `sorted`.
    cell_start: Vec<u32>,
    sorted: Vec<Entry>,
    staged: Vec<(u32, Entry)>,
    cursor: Vec<u32>,
    max_radius: Fx,
}

impl SpatialIndex {
    pub fn new(map_size: FxVec2) -> SpatialIndex {
        let width = (map_size.x.ceil_int() >> CELL_SHIFT).max(1) + 1;
        let height = (map_size.y.ceil_int() >> CELL_SHIFT).max(1) + 1;
        SpatialIndex {
            width,
            height,
            cell_start: vec![0; (width * height) as usize + 1],
            sorted: Vec::new(),
            staged: Vec::new(),
            cursor: Vec::new(),
            max_radius: Fx::ZERO,
        }
    }

    #[inline]
    fn cell_coords(&self, p: FxVec2) -> (i32, i32) {
        (
            (p.x.floor_int() >> CELL_SHIFT).clamp(0, self.width - 1),
            (p.y.floor_int() >> CELL_SHIFT).clamp(0, self.height - 1),
        )
    }

    pub fn clear(&mut self) {
        self.staged.clear();
        self.max_radius = Fx::ZERO;
    }

    #[inline]
    pub fn insert(&mut self, kind: u8, row: usize, pos: FxVec2, radius: Fx) {
        let (cx, cy) = self.cell_coords(pos);
        self.max_radius = self.max_radius.max(radius);
        self.staged.push((
            (cy * self.width + cx) as u32,
            Entry {
                kind,
                row: row as u32,
                pos,
                radius,
            },
        ));
    }

    /// Sorts staged entries into cells. Stable, so insertion order survives within a cell.
    pub fn build(&mut self) {
        self.cell_start.fill(0);
        for (cell, _) in &self.staged {
            self.cell_start[*cell as usize + 1] += 1;
        }
        for i in 1..self.cell_start.len() {
            self.cell_start[i] += self.cell_start[i - 1];
        }
        self.sorted.clear();
        self.sorted.resize(
            self.staged.len(),
            Entry {
                kind: 0,
                row: 0,
                pos: FxVec2::ZERO,
                radius: Fx::ZERO,
            },
        );
        // Walk backwards with a moving cursor per cell to keep the sort stable.
        self.cursor.clear();
        self.cursor.extend_from_slice(&self.cell_start[1..]);
        for (cell, entry) in self.staged.iter().rev() {
            let c = &mut self.cursor[*cell as usize];
            *c -= 1;
            self.sorted[*c as usize] = *entry;
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.sorted.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.sorted.is_empty()
    }

    /// Calls `visit` for every entry of a kind in `kinds` whose bounding circle
    /// touches the query circle. Return `false` from `visit` to stop early.
    pub fn query(
        &self,
        center: FxVec2,
        radius: Fx,
        kinds: u8,
        mut visit: impl FnMut(&Entry) -> bool,
    ) {
        let reach = radius + self.max_radius;
        let (x0, y0) = self.cell_coords(FxVec2::new(center.x - reach, center.y - reach));
        let (x1, y1) = self.cell_coords(FxVec2::new(center.x + reach, center.y + reach));
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                let c = (cy * self.width + cx) as usize;
                for e in &self.sorted[self.cell_start[c] as usize..self.cell_start[c + 1] as usize]
                {
                    if e.kind & kinds == 0 {
                        continue;
                    }
                    let r = radius + e.radius;
                    if e.pos.distance_sq(center) <= r * r && !visit(e) {
                        return;
                    }
                }
            }
        }
    }

    /// Nearest entry accepted by `accept`, by centre distance. Ties go to the
    /// entry found first, i.e. the lower cell then the lower insertion order.
    pub fn nearest(
        &self,
        center: FxVec2,
        radius: Fx,
        kinds: u8,
        mut accept: impl FnMut(&Entry) -> bool,
    ) -> Option<Entry> {
        let mut best: Option<(Fx, Entry)> = None;
        self.query(center, radius, kinds, |e| {
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
            let radius = rng.range(Fx::ONE, Fx::from_int(40));
            index.insert(kind::UNIT, row, pos, radius);
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
        index.insert(kind::UNIT, 0, FxVec2::from_ints(10, 10), Fx::ONE);
        index.insert(kind::WRECK, 1, FxVec2::from_ints(12, 10), Fx::ONE);
        index.insert(kind::UNIT, 2, FxVec2::from_ints(14, 10), Fx::ONE);
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
