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
/// blocks for ground layers, repeating Vs for each air altitude. Each layer faces
/// `facing`, or without it the way it goes. Slot assignment is spatial and stable,
/// independent of member order but for ties, which go to the earlier member.
pub fn plan(
    members: &[Member],
    target: FxVec2,
    facing: Option<Angle>,
    spacing_level: u8,
) -> Vec<Laid> {
    let mut layers = BTreeMap::<(u8, i64), Vec<usize>>::new();
    for (i, m) in members.iter().enumerate() {
        layers.entry(m.layer).or_default().push(i);
    }
    layers
        .into_values()
        .map(|group| lay(members, &group, target, facing, spacing_level))
        .collect()
}

fn lay(
    all: &[Member],
    group: &[usize],
    target: FxVec2,
    facing: Option<Angle>,
    spacing_level: u8,
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
        let slots = slots(n as usize, widest * scale, true);
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
        block(&widths, &at, scale)
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
        return (
            cell,
            slots(widths.len(), spacing, false)
                .into_iter()
                .map(|p| (p, 1))
                .collect(),
        );
    }
    let total: i32 = sizes.iter().map(|&k| (k as i32) * (k as i32)).sum();
    let widest = *sizes.iter().max().unwrap() as i32;
    let cols = Fx::from_int(total).sqrt().ceil_int().max(widest);
    let ranks = (total + cols - 1) / cols;
    let depth = ranks
        + sizes
            .iter()
            .filter(|&&k| k > 1)
            .map(|&k| k as i32)
            .sum::<i32>();
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
        // The free square nearest where it wants to be, ties to the front then
        // the left. Ranks are tried nearest first, and once a rank is further
        // off than the best square found, none after it can be nearer: a big
        // army's heavies no longer each search the whole block.
        let mut near: Vec<i32> = (0..=depth - k).collect();
        near.sort_by_key(|&r| ((2 * r + k - 1 - want_r).abs(), r));
        // (distance squared, rank, column) of the best free square.
        let mut best: Option<(i64, i32, i32)> = None;
        for r in near {
            let dr = (2 * r + k - 1 - want_r) as i64;
            if best.is_some_and(|(d2, _, _)| dr * dr > d2) {
                break;
            }
            for c in 0..=cols - k {
                let dc = (2 * c + k - 1 - want_c) as i64;
                let key = (dr * dr + dc * dc, r, c);
                if best.is_none_or(|b| key < b) && free(&taken, c, r, k) {
                    best = Some(key);
                }
            }
        }
        let (_, r, c) = best.expect("room in a block deep enough for every heavy");
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
