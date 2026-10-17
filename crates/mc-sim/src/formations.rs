//! Stable, centered command layouts. Local X is forward, local Y is left.
//!
//! `plan` is pure: the sim lays out every move order with it, and the game with
//! it draws the formation a held right-click would give before it is let go.
use mc_core::{Angle, Fx, FxVec2};
use mc_data::{MoveLayer, UnitBlueprint};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Group {
    pub anchor: FxVec2,
    pub heading: Angle,
    /// 0 waits for its queued leg, 1 forms during travel, 2 travels, 3 passes an obstruction.
    pub phase: u8,
    pub speed: Fx,
}

/// Steps of `shape` either side of square: each doubles (or halves) the block's
/// width over its depth, so the widest is a line 8 times as wide as it is deep.
pub const SHAPES: i8 = 6;

/// Width over depth for each `shape` from `-SHAPES` to `SHAPES`, in steps of about the square root of 2.
const ASPECT: [(i64, i64); 13] = [
    (1, 8),
    (3, 17),
    (1, 4),
    (6, 17),
    (1, 2),
    (12, 17),
    (1, 1),
    (17, 12),
    (2, 1),
    (17, 6),
    (4, 1),
    (17, 3),
    (8, 1),
];

/// Columns across a block of `n` places at `shape` (0 square, more wider, less
/// deeper), at least `least` and no more than fit.
fn columns(n: i32, shape: i8, least: i32) -> i32 {
    let (num, den) = ASPECT[(shape.clamp(-SHAPES, SHAPES) + SHAPES) as usize];
    (Fx::from_int(n) * Fx::ratio(num, den))
        .sqrt()
        .ceil_int()
        .clamp(1, n.max(1))
        .max(least)
}

/// One member of a group being laid out.
#[derive(Clone, Copy, Debug)]
pub struct Member {
    /// Where it sets off from: where it stands, or where its queue ends.
    pub source: FxVec2,
    /// Its hull's width with room to spare round it.
    width: Fx,
    /// Land, sea or air, aircraft by altitude: each lays out on its own.
    layer: (u8, i64),
    /// Which way it points: a layer ordered onto its own middle keeps its first member's.
    heading: Angle,
}

impl Member {
    /// `None` for what does not move.
    pub fn new(bp: &UnitBlueprint, source: FxVec2, heading: Angle) -> Option<Member> {
        let m = bp.motion?;
        let layer = match m.layer {
            MoveLayer::Air => (2, m.altitude.0),
            MoveLayer::Naval => (1, 0),
            _ => (0, 0),
        };
        Some(Member {
            source,
            width: bp.radius * 2 + Fx::from_int(6),
            layer,
            heading,
        })
    }

    fn air(&self) -> bool {
        self.layer.0 == 2
    }
}

/// One layer's layout.
pub struct Laid {
    /// Indices into the members laid out, ranked; `offsets` pairs with it.
    pub members: Vec<usize>,
    /// Each member's slot about the target, turned to `facing`.
    pub offsets: Vec<FxVec2>,
    pub facing: Angle,
    /// Where the block starts from.
    pub centroid: FxVec2,
    /// Between neighbouring slots, metres.
    pub spacing: Fx,
}

/// Where each of `members` stands in a group ordered to `target`: independent
/// blocks for ground layers, repeating Vs for each air altitude, as wide for their
/// depth as `shape` asks (`columns`). Each layer faces `facing`, or without it the way it goes. Slot assignment is spatial and stable,
/// independent of member order but for ties, which go to the earlier member.
pub fn plan(
    members: &[Member],
    target: FxVec2,
    facing: Option<Angle>,
    spacing_level: u8,
    shape: i8,
) -> Vec<Laid> {
    let mut layers = BTreeMap::<(u8, i64), Vec<usize>>::new();
    for (i, m) in members.iter().enumerate() {
        layers.entry(m.layer).or_default().push(i);
    }
    layers
        .into_values()
        .map(|group| lay(members, &group, target, facing, spacing_level, shape))
        .collect()
}

fn lay(
    all: &[Member],
    group: &[usize],
    target: FxVec2,
    facing: Option<Angle>,
    spacing_level: u8,
    shape: i8,
) -> Laid {
    let n = group.len() as i32;
    let source = |i: usize| all[i].source;
    let width = |i: usize| all[i].width;
    let mut centroid = FxVec2::ZERO;
    let mut widest = Fx::ZERO;
    for &i in group {
        centroid += source(i);
        widest = widest.max(width(i));
    }
    centroid = FxVec2::new(centroid.x / n, centroid.y / n);
    let facing = facing.unwrap_or_else(|| {
        if centroid == target {
            all[group[0]].heading
        } else {
            (target - centroid).angle()
        }
    });
    let air = all[group[0]].air();
    let scale = crate::reform::spacing_scale(spacing_level);
    // A flight flies its Vs at its widest wing's spacing; a ground
    // block gives each size its own, heavies in the middle.
    let (cell, laid) = if air {
        let slots = slots(n as usize, widest * scale, true, shape);
        (
            widest,
            slots.into_iter().map(|p| (p, 1)).collect::<Vec<_>>(),
        )
    } else {
        let widths: Vec<_> = group.iter().map(|&i| width(i)).collect();
        let at: Vec<_> = group
            .iter()
            .map(|&i| (source(i) - centroid).rotate(-facing))
            .collect();
        block(&widths, &at, scale, shape)
    };
    let spacing = cell * scale;
    let size = |i: usize| if air { 1 } else { cells(width(i), cell) };
    let offsets: Vec<_> = laid.iter().map(|&(p, _)| p).collect();
    let rotated: Vec<_> = offsets.iter().map(|p| p.rotate(facing)).collect();
    // Each size takes the slots laid out for its size. Within one,
    // sort ranks front-to-back, then left-to-right. This is O(n log n),
    // avoids selection-order crossings, and keeps large armies affordable.
    let mut classes: Vec<u8> = laid.iter().map(|&(_, k)| k).collect();
    classes.sort_unstable();
    classes.dedup();
    let mut ranked = Vec::with_capacity(group.len());
    let mut slots = Vec::with_capacity(group.len());
    for k in classes {
        let mut members: Vec<usize> = group.iter().copied().filter(|&i| size(i) == k).collect();
        members.sort_by_key(|&i| {
            let p = (source(i) - centroid).rotate(-facing);
            (-p.x.0.div_euclid(spacing.0), -p.y.0, i)
        });
        let mut mine: Vec<_> = (0..laid.len()).filter(|&s| laid[s].1 == k).collect();
        mine.sort_by_key(|&s| (-offsets[s].x.0, -offsets[s].y.0, s));
        // Remove crossing assignments before issuing the order. Pair swaps
        // strictly reduce squared travel, with fixed iteration order for replay.
        // Bound work for very large selections.
        if members.len() <= 256 {
            for _ in 0..4 {
                let mut changed = false;
                for a in 0..members.len() {
                    for b in a + 1..members.len() {
                        let pa = source(members[a]) - centroid;
                        let pb = source(members[b]) - centroid;
                        let oa = rotated[mine[a]];
                        let ob = rotated[mine[b]];
                        if (pa - pb).dot(oa - ob) < Fx::ZERO {
                            mine.swap(a, b);
                            changed = true;
                        }
                    }
                }
                if !changed {
                    break;
                }
            }
        }
        ranked.extend(members);
        slots.extend(mine);
    }
    Laid {
        members: ranked,
        offsets: slots.into_iter().map(|s| rotated[s]).collect(),
        facing,
        centroid,
        spacing,
    }
}

pub(crate) fn slots(count: usize, spacing: Fx, air: bool, shape: i8) -> Vec<FxVec2> {
    if count == 0 {
        return Vec::new();
    }
    let n = count as i32;
    let cols = columns(n, shape, 1);
    let flights = (n + 4) / 5;
    let flight_cols = columns(flights, shape, 1);
    let mut out: Vec<_> = (0..n)
        .map(|i| {
            if air {
                // Five-ship V: leader, inner pair, outer pair. Repeat the whole
                // motif across and behind, leaving one hull-spacing between Vs.
                let flight = i / 5;
                let wing = (i % 5 + 1) / 2;
                let side = if i % 5 == 0 {
                    0
                } else if (i % 5) % 2 == 0 {
                    -1
                } else {
                    1
                };
                FxVec2::new(
                    -spacing * ((flight / flight_cols) * 4 + wing),
                    spacing * ((flight % flight_cols) * 6 + side * wing),
                )
            } else {
                FxVec2::new(-spacing * (i / cols), spacing * (i % cols))
            }
        })
        .collect();
    let sum = out.iter().copied().fold(FxVec2::ZERO, |a, b| a + b);
    let mean = FxVec2::new(sum.x / n, sum.y / n);
    for p in &mut out {
        *p -= mean;
    }
    out
}

/// Cells of `cell` metres a member `width` metres across takes on a side.
pub(crate) fn cells(width: Fx, cell: Fx) -> u8 {
    (width / cell).ceil_int().clamp(1, 16) as u8
}

/// A ground block for members of mixed size, `widths` being each member's
/// hull width plus its margin. The grid's cell suits the block's bulk, not its
/// biggest hull: a heavy takes a square of cells and the rest keep their own
/// spacing round it, rather than every tank standing a Fulgur's width from
/// the next. Each heavy's square goes as near as it fits to where that
/// member `stands` now in the block's frame (local X forward, Y left, from the
/// middle of the group): a Fulgur leading the army takes the front ranks. Put
/// in the middle, it would crawl ahead of its rank waiting for the block, in
/// the way of every tank whose rank lies past it. Returns the cell and one slot per member, as
/// (offset, cells a side), in no particular member's order: slots are handed
/// to members of the same size, `cells(width, cell)` a side. All members one
/// size lays out as `slots`. The cell is before `scale` spreads the block, and
/// `shape` sets its width for its depth.
pub(crate) fn block(
    widths: &[Fx],
    stands: &[FxVec2],
    scale: Fx,
    shape: i8,
) -> (Fx, Vec<(FxVec2, u8)>) {
    if widths.is_empty() {
        return (Fx::ONE, Vec::new());
    }
    // The cell that wastes least ground over the whole block.
    let mut candidates: Vec<Fx> = widths.to_vec();
    candidates.sort_unstable();
    candidates.dedup();
    let area = |cell: Fx| -> i64 {
        widths
            .iter()
            .map(|&w| {
                let side = (cell * cells(w, cell) as i32).ceil_int() as i64;
                side * side
            })
            .sum()
    };
    let cell = candidates
        .iter()
        .copied()
        .min_by_key(|&c| (area(c), c))
        .expect("a width");
    let sizes: Vec<u8> = widths.iter().map(|&w| cells(w, cell)).collect();
    let spacing = cell * scale;
    if sizes.iter().all(|&k| k == 1) {
        return (
            cell,
            slots(widths.len(), spacing, false, shape)
                .into_iter()
                .map(|p| (p, 1))
                .collect(),
        );
    }
    let total: i32 = sizes.iter().map(|&k| (k as i32) * (k as i32)).sum();
    let widest = *sizes.iter().max().unwrap() as i32;
    let cols = columns(total, shape, widest);
    let ranks = (total + cols - 1) / cols;
    let depth = ranks
        + sizes
            .iter()
            .filter(|&&k| k > 1)
            .map(|&k| k as i32)
            .sum::<i32>();
    let mut taken = vec![false; (cols * depth) as usize];
    // (half-cells forward of the front rank's centre, half-cells left), per member.
    let mut at = vec![(0i32, 0i32); sizes.len()];
    let free = |taken: &[bool], c: i32, r: i32, k: i32| {
        (r..r + k).all(|y| (c..c + k).all(|x| !taken[(y * cols + x) as usize]))
    };
    // Heavies first, biggest first, each as near where it stands as it fits.
    // In half-cells from the middle of the front rank's left end. Past `CELL_SEARCH`
    // heavies of a size, they go down on a grid of squares their size (`Squares`),
    // which finds the free square nearest a spot in a look a rank however many have
    // filled in round it: searched cell by cell, the 10 000 heavy tanks of an order
    // to 50 000 (all wanting the ground their block stood on) took 8 s.
    const CELL_SEARCH: usize = 256;
    let mut heavy: Vec<usize> = (0..sizes.len()).filter(|&i| sizes[i] > 1).collect();
    heavy.sort_by_key(|&i| (std::cmp::Reverse(sizes[i]), i));
    let half = cell / 2;
    for same in heavy.chunk_by(|&a, &b| sizes[a] == sizes[b]) {
        let k = sizes[same[0]] as i32;
        let mut squares = (same.len() > CELL_SEARCH).then(|| Squares::new(&taken, cols, depth, k));
        for &i in same {
            let want_r = ranks - 1 - (stands[i].x / half).round_int();
            let want_c = cols - 1 + (stands[i].y / half).round_int();
            let found = match squares.as_mut() {
                Some(squares) => squares.nearest(want_r, want_c),
                None => nearest_cells(want_r, want_c, k, depth, cols, |r, c| free(&taken, c, r, k)),
            };
            let (r, c) = found.expect("room in a block deep enough for every heavy");
            for y in r..r + k {
                for x in c..c + k {
                    taken[(y * cols + x) as usize] = true;
                }
            }
            at[i] = (2 * r + k - 1, 2 * c + k - 1);
        }
    }
    // The rest fill rank by rank, front to back, left to right.
    let mut open = (0..cols * depth).filter(|&cell| !taken[cell as usize]);
    for i in (0..sizes.len()).filter(|&i| sizes[i] == 1) {
        let cell = open.next().expect("room in the block");
        at[i] = (2 * (cell / cols), 2 * (cell % cols));
    }
    let mut out: Vec<_> = at
        .iter()
        .zip(&sizes)
        .map(|(&(r, c), &k)| (FxVec2::new(-spacing * r / 2, spacing * c / 2), k))
        .collect();
    let n = out.len() as i32;
    let sum = out.iter().fold(FxVec2::ZERO, |a, &(p, _)| a + p);
    let mean = FxVec2::new(sum.x / n, sum.y / n);
    for (p, _) in &mut out {
        *p -= mean;
    }
    (cell, out)
}

/// The free square `k` cells a side nearest the half-cell `(want_r, want_c)` at any cell
/// of a `cols` x `depth` block (`free(rank, column)` of its first cell): its first cell.
/// Ties go to the front, then the left. Ranks are tried nearest first, and once a rank
/// is further off than the best square found, none after it can be nearer; within a
/// rank the columns go nearest first too, so its first free square is its best.
fn nearest_cells(
    want_r: i32,
    want_c: i32,
    k: i32,
    depth: i32,
    cols: i32,
    free: impl Fn(i32, i32) -> bool,
) -> Option<(i32, i32)> {
    // (distance squared, rank, column) of the best free square.
    let mut best: Option<(i64, i32, i32)> = None;
    for r in outward(want_r, k, 1, depth - k) {
        let dr = (2 * r + k - 1 - want_r) as i64;
        if best.is_some_and(|(d2, _, _)| dr * dr > d2) {
            break;
        }
        for c in outward(want_c, k, 1, cols - k) {
            let dc = (2 * c + k - 1 - want_c) as i64;
            let key = (dr * dr + dc * dc, r, c);
            // The rank's squares come in rising key order: none after this beats it.
            if best.is_some_and(|b| key > b) {
                break;
            }
            if free(r, c) {
                best = Some(key);
                break;
            }
        }
    }
    best.map(|(_, r, c)| (r, c))
}

/// The squares `k` cells a side a block of `cols` x `depth` cells holds edge to edge
/// from its corner, and which of them are free. Each rank of squares points every
/// square at the nearest free one to its right and to its left (pointers that skip
/// what is taken, halved as they are followed), so the nearest free square either
/// side of a spot is a few steps however full the rank is.
struct Squares {
    k: i32,
    /// Squares in a rank.
    across: i32,
    ranks: i32,
    /// Per rank, `across + 1` entries: a square at or right of each, on the way to the
    /// nearest free one; entry `across` (none) points at itself.
    right: Vec<i32>,
    /// Per rank, `across + 1` entries for squares `-1..across` (entry 0 is none,
    /// pointing at itself): a square at or left of each, plus one.
    left: Vec<i32>,
}

impl Squares {
    fn new(taken: &[bool], cols: i32, depth: i32, k: i32) -> Squares {
        let (across, ranks) = (cols / k, depth / k);
        let mut right = vec![0; ((across + 1) * ranks) as usize];
        let mut left = right.clone();
        for r in 0..ranks {
            let base = (r * (across + 1)) as usize;
            for c in 0..across {
                let free = (0..k)
                    .all(|y| (0..k).all(|x| !taken[((r * k + y) * cols + c * k + x) as usize]));
                right[base + c as usize] = if free { c } else { c + 1 };
                left[base + c as usize + 1] = if free { c + 1 } else { c };
            }
            right[base + across as usize] = across;
            left[base] = 0;
        }
        Squares {
            k,
            across,
            ranks,
            right,
            left,
        }
    }

    /// The first free square at or right of `c` in rank `r`; `across` if none.
    fn free_right(&mut self, r: i32, c: i32) -> i32 {
        let base = (r * (self.across + 1)) as usize;
        let mut c = c as usize;
        while self.right[base + c] as usize != c {
            let next = self.right[base + c] as usize;
            self.right[base + c] = self.right[base + next];
            c = next;
        }
        c as i32
    }

    /// The first free square at or left of `c` in rank `r`; -1 if none.
    fn free_left(&mut self, r: i32, c: i32) -> i32 {
        let base = (r * (self.across + 1)) as usize;
        let mut v = (c + 1) as usize;
        while self.left[base + v] as usize != v {
            let next = self.left[base + v] as usize;
            self.left[base + v] = self.left[base + next];
            v = next;
        }
        v as i32 - 1
    }

    /// The free square nearest the half-cell `(want_r, want_c)`, taken: its first cell's
    /// rank and column. Ties go to the front, then the left.
    fn nearest(&mut self, want_r: i32, want_c: i32) -> Option<(i32, i32)> {
        let k = self.k;
        if self.across == 0 {
            return None;
        }
        // (distance squared, rank, column) of the best free square, in cells.
        let mut best: Option<(i64, i32, i32)> = None;
        // The square nearest across sits at or just left of where the distance bottoms out:
        // left of it they only get further away going left, right of it going right.
        let c0 = (want_c - k + 1).div_euclid(2 * k).clamp(0, self.across - 1);
        for sr in outward(want_r, k, k, self.ranks - 1) {
            let r = sr * k;
            let dr = (2 * r + k - 1 - want_r) as i64;
            if best.is_some_and(|(d2, _, _)| dr * dr > d2) {
                break;
            }
            for sc in [self.free_left(sr, c0), self.free_right(sr, c0 + 1)] {
                if !(0..self.across).contains(&sc) {
                    continue;
                }
                let c = sc * k;
                let dc = (2 * c + k - 1 - want_c) as i64;
                let key = (dr * dr + dc * dc, r, c);
                if best.is_none_or(|b| key < b) {
                    best = Some(key);
                }
            }
        }
        let (_, r, c) = best?;
        let (sr, sc) = (r / k, c / k);
        let base = (sr * (self.across + 1)) as usize;
        self.right[base + sc as usize] = sc + 1;
        self.left[base + sc as usize + 1] = sc;
        Some((r, c))
    }
}

/// Every place `0..=last` along a grid of `step` cells a square `k` cells a side can
/// start at (place `v` its first cell `step * v`), nearest first to the half-cell
/// `want` (its middle `2 step v + k - 1` half-cells along), ties to the lower: two
/// cursors walking out either side of the nearest.
fn outward(want: i32, k: i32, step: i32, last: i32) -> impl Iterator<Item = i32> {
    let off = move |v: i32| (2 * step * v + k - 1 - want).abs();
    let start = (want - k + 1).div_euclid(2 * step).clamp(0, last.max(0));
    let (mut lo, mut hi) = (start, start + 1);
    std::iter::from_fn(move || {
        let down = (lo >= 0 && lo <= last).then(|| (off(lo), lo));
        let up = (hi <= last).then(|| (off(hi), hi));
        match (down, up) {
            (Some(d), Some(u)) if d <= u => {
                lo -= 1;
                Some(d.1)
            }
            (_, Some(u)) => {
                hi += 1;
                Some(u.1)
            }
            (Some(d), None) => {
                lo -= 1;
                Some(d.1)
            }
            (None, None) => None,
        }
    })
}

#[cfg(test)]
mod tests {
    /// `outward` walks the places nearest first, ties to the lower.
    #[test]
    fn outward_is_nearest_first() {
        for last in -1..12 {
            for k in 1..4 {
                for step in [1, k] {
                    for want in -6..30 {
                        let mut sorted: Vec<i32> = (0..=last).collect();
                        sorted.sort_by_key(|&v| ((2 * step * v + k - 1 - want).abs(), v));
                        let walked: Vec<i32> = super::outward(want, k, step, last).collect();
                        assert_eq!(
                            walked, sorted,
                            "want {want}, k {k}, step {step}, last {last}"
                        );
                    }
                }
            }
        }
    }

    /// `Squares` finds the free square a search of every square would, and takes it.
    #[test]
    fn squares_find_the_nearest_free() {
        let (cols, depth) = (23, 17);
        let mut seed = 12345u32;
        let mut next = move || {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            seed >> 16
        };
        for k in 2..5 {
            for round in 0..20 {
                let taken: Vec<bool> = (0..cols * depth).map(|_| next() % 7 < round % 6).collect();
                let mut squares = super::Squares::new(&taken, cols, depth, k);
                let mut held = taken.clone();
                for _ in 0..30 {
                    let (want_r, want_c) = ((next() % 50) as i32 - 8, (next() % 60) as i32 - 8);
                    let free = |held: &[bool], r: i32, c: i32| {
                        (0..k).all(|y| (0..k).all(|x| !held[((r + y) * cols + c + x) as usize]))
                    };
                    let mut best = None;
                    for r in (0..depth / k).map(|s| s * k) {
                        for c in (0..cols / k).map(|s| s * k) {
                            let (dr, dc) = (
                                (2 * r + k - 1 - want_r) as i64,
                                (2 * c + k - 1 - want_c) as i64,
                            );
                            let key = (dr * dr + dc * dc, r, c);
                            if free(&held, r, c) && best.is_none_or(|b| key < b) {
                                best = Some(key);
                            }
                        }
                    }
                    let found = squares.nearest(want_r, want_c);
                    assert_eq!(found, best.map(|(_, r, c)| (r, c)), "k {k} round {round}");
                    if let Some((r, c)) = found {
                        for y in r..r + k {
                            for x in c..c + k {
                                held[(y * cols + x) as usize] = true;
                            }
                        }
                    }
                }
            }
        }
    }
}
