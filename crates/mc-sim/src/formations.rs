//! Stable, centered command layouts. Local X is forward, local Y is left.
use mc_core::{Angle, Fx, FxVec2};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Group {
    pub anchor: FxVec2,
    pub heading: Angle,
    /// 0 waits for its queued leg, 1 forms during travel, 2 travels, 3 passes an obstruction.
    pub phase: u8,
    pub speed: Fx,
}

pub(crate) fn slots(count: usize, spacing: Fx, air: bool) -> Vec<FxVec2> {
    if count == 0 {
        return Vec::new();
    }
    let n = count as i32;
    let cols = Fx::from_int(n).sqrt().ceil_int().max(1);
    let flights = (n + 4) / 5;
    let flight_cols = Fx::from_int(flights).sqrt().ceil_int().max(1);
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
/// size lays out as `slots`. The cell is before `scale` spreads the block.
pub(crate) fn block(widths: &[Fx], stands: &[FxVec2], scale: Fx) -> (Fx, Vec<(FxVec2, u8)>) {
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
        return (cell, slots(widths.len(), spacing, false).into_iter().map(|p| (p, 1)).collect());
    }
    let total: i32 = sizes.iter().map(|&k| (k as i32) * (k as i32)).sum();
    let widest = *sizes.iter().max().unwrap() as i32;
    let cols = Fx::from_int(total).sqrt().ceil_int().max(widest);
    let ranks = (total + cols - 1) / cols;
    let depth = ranks + sizes.iter().filter(|&&k| k > 1).map(|&k| k as i32).sum::<i32>();
    let mut taken = vec![false; (cols * depth) as usize];
    let free = |taken: &[bool], c: i32, r: i32, k: i32| {
        (r..r + k).all(|y| (c..c + k).all(|x| !taken[(y * cols + x) as usize]))
    };
    // (half-cells forward of the front rank's centre, half-cells left), per member.
    let mut at = vec![(0i32, 0i32); sizes.len()];
    // Heavies first, biggest first, each as near where it stands as it fits.
    // In half-cells from the middle of the front rank's left end.
    let mut heavy: Vec<usize> = (0..sizes.len()).filter(|&i| sizes[i] > 1).collect();
    heavy.sort_by_key(|&i| (std::cmp::Reverse(sizes[i]), i));
    let half = cell / 2;
    for i in heavy {
        let k = sizes[i] as i32;
        let want_r = ranks - 1 - (stands[i].x / half).round_int();
        let want_c = cols - 1 + (stands[i].y / half).round_int();
        let (c, r) = (0..=depth - k)
            .flat_map(|r| (0..=cols - k).map(move |c| (c, r)))
            .filter(|&(c, r)| free(&taken, c, r, k))
            .min_by_key(|&(c, r)| {
                let (dr, dc) = ((2 * r + k - 1 - want_r) as i64, (2 * c + k - 1 - want_c) as i64);
                (dr * dr + dc * dc, r, c)
            })
            .expect("room in a block deep enough for every heavy");
        for y in r..r + k {
            for x in c..c + k {
                taken[(y * cols + x) as usize] = true;
            }
        }
        at[i] = (2 * r + k - 1, 2 * c + k - 1);
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
