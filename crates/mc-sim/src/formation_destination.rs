//! Where a formation ordered onto occupied ground stands instead: the whole
//! layout shifted off parked hulls and structures, the slots kept together.
//! A long hull (`UnitBlueprint::body`) takes up its capsule, laid along the way
//! it will face there, and a parked one along its heading: two Regents may stand
//! abreast a beam apart, where their 1.9 km radii kept them 3.8 km apart.

use crate::body::{self, spines_gap};
use crate::spatial::kind;
use crate::tables::flag;
use crate::World;
use mc_core::{Angle, Fx, FxVec2};
use mc_data::MoveLayer;

impl World {
    /// Keep a layout intact when the clicked ground is occupied by a parked
    /// hull or a structure. Search whole-layout translations, not stacked slots.
    /// `offsets[i]` is `rows[i]`'s slot, and the layout faces `facing`.
    pub(crate) fn clear_formation_destination(
        &self,
        center: FxVec2,
        facing: Angle,
        offsets: &[FxVec2],
        rows: &[usize],
        spacing: Fx,
    ) -> FxVec2 {
        let selected: std::collections::BTreeSet<_> = rows.iter().copied().collect();
        let radius = rows
            .iter()
            .map(|&r| self.bp(r).radius)
            .max()
            .unwrap_or(Fx::ONE);
        let size = rows
            .iter()
            .map(|&r| self.bp(r).motion.unwrap().size_class)
            .max()
            .unwrap_or(0);
        let first = self.bp(rows[0]).motion.unwrap();
        let layer = if rows
            .iter()
            .any(|&r| self.bp(r).motion.unwrap().layer == MoveLayer::Land)
        {
            MoveLayer::Land
        } else {
            first.layer
        };
        let air = layer == MoveLayer::Air;
        // Striders go where they can put their feet, the sea included, and step over
        // whatever stands there, and floaters go anywhere: the nav grid and the crowd are
        // nothing to them.
        let striders = rows
            .iter()
            .all(|&r| self.bp(r).motion.is_some_and(|m| m.passes_over()));
        // Everything that could stand in the layout's way anywhere the search
        // below looks, gathered once and binned by 16 m cell: the slots of a
        // big block over a few hundred spots are then checked against these.
        let rings = 10;
        // The search steps out a slot's spacing a ring; a long hull's slot is spaced by its
        // radius, but it only needs to move over its beam to lie clear alongside.
        let step = rows
            .iter()
            .filter_map(|&r| self.bp(r).body.filter(|_| self.bp(r).growth.is_none()))
            .map(|b| b.beam * 2 + Fx::from_int(6))
            .fold(spacing, Fx::min)
            .max(Fx::from_int(12));
        let widest = offsets.iter().map(|o| o.length()).fold(Fx::ZERO, Fx::max);
        let area = widest + step * rings + radius + Fx::from_int(6);
        let mut blockers = Vec::new();
        let bin = |p: FxVec2| (p.x.floor_int() >> 4, p.y.floor_int() >> 4);
        self.index.query(center, area, kind::UNIT, |e| {
            let other = e.row as usize;
            if selected.contains(&other) || !self.unit_entry_is_current(e) {
                return true;
            }
            let Some(m) = self.bp(other).motion else {
                return true;
            };
            if (m.layer == MoveLayer::Air) != air
                || self.state.units.has_flag(other, flag::IN_FACTORY)
                || self.state.units.has_flag(other, flag::HAS_FIELD)
            {
                return true;
            }
            // Ships and dived submarines may share a spot, one under the other, and
            // so may a unit on a deck and one under it.
            if rows.iter().all(|&r| self.apart(r, other)) {
                return true;
            }
            if air && (m.altitude - first.altitude).abs() > self.bp(other).height + Fx::ONE {
                return true;
            }
            blockers.push((e.pos, self.footprint(other, e.pos, e.radius)));
            true
        });
        let widest_blocker = blockers
            .iter()
            .map(|&(at, (a, b, half))| at.distance(a).max(at.distance(b)) + half)
            .fold(Fx::ZERO, Fx::max);
        let reach = radius + widest_blocker + Fx::from_int(6);
        let blockers = Blockers::new(&blockers, bin, reach);
        let free = |candidate: FxVec2| {
            offsets.iter().zip(rows).all(|(offset, &row)| {
                let pos = candidate + *offset;
                if striders {
                    return rows.iter().all(|&r| {
                        self.bp(r)
                            .motion
                            .is_some_and(|m| self.passing_footing(&m, pos))
                    });
                }
                if !self.terrain.in_bounds(pos) || !self.nav.passable(layer, size, pos) {
                    return false;
                }
                if !blockers.near(bin(pos)) {
                    return true;
                }
                // A long hull is its capsule as it will lie there; anything else
                // the widest of the group's discs, as ever.
                let slot = match self.bp(row).body {
                    Some(_) if self.bp(row).growth.is_none() => {
                        body::spine(self.bp(row), pos, facing)
                    }
                    _ => (pos, pos, radius),
                };
                let (x0, y0) = bin(pos - FxVec2::new(reach, reach));
                let (x1, y1) = bin(pos + FxVec2::new(reach, reach));
                (y0..=y1).all(|y| {
                    (x0..=x1).all(|x| {
                        blockers
                            .at((x, y))
                            .iter()
                            .all(|&(_, other)| spines_gap(slot, other) >= Fx::from_int(6))
                    })
                })
            })
        };
        if free(center) {
            return center;
        }
        let sum = rows
            .iter()
            .fold(FxVec2::ZERO, |p, &r| p + self.state.units.pos[r]);
        let from = FxVec2::new(sum.x / rows.len() as i32, sum.y / rows.len() as i32);
        for ring in 1..=rings {
            let mut best = None;
            for spoke in 0..16 {
                let shift = FxVec2::from_angle(Angle(spoke * 4096)) * (step * ring);
                let candidate = center + shift;
                if free(candidate)
                    && best.is_none_or(|p: FxVec2| candidate.distance(from) < p.distance(from))
                {
                    best = Some(candidate);
                }
            }
            if let Some(candidate) = best {
                return candidate;
            }
        }
        center
    }

    /// The ground a parked `row` at `pos` takes up in a destination search: a long
    /// hull's capsule along its heading, else the disc of `radius`.
    fn footprint(&self, row: usize, pos: FxVec2, radius: Fx) -> Spine {
        let bp = self.bp(row);
        match bp.body {
            Some(_) if bp.growth.is_none() => body::spine(bp, pos, self.state.units.heading[row]),
            _ => (pos, pos, radius),
        }
    }
}

/// A hull's spine and half width about it (`body::spine`).
type Spine = (FxVec2, FxVec2, Fx);

/// What may stand in a formation's way where its destination is searched
/// ([`World::clear_formation_destination`]), binned by 16 m cell on a grid
/// over them, with the cells a slot must look round marked.
struct Blockers {
    /// Grid origin and width, in bins.
    origin: (i32, i32),
    width: i32,
    height: i32,
    /// `items[starts[c]..starts[c + 1]]`: the blockers of cell `c`.
    starts: Vec<u32>,
    items: Vec<(FxVec2, Spine)>,
    /// The cells within `reach` of some blocker's cell: a slot in any other
    /// has nothing near it to look at.
    near: Vec<bool>,
}

impl Blockers {
    fn new(all: &[(FxVec2, Spine)], bin: impl Fn(FxVec2) -> (i32, i32), reach: Fx) -> Blockers {
        // A slot looks `reach` round it: so far, and a cell over for the binning.
        let pad = (reach.ceil_int() >> 4) + 1;
        let bins: Vec<(i32, i32)> = all.iter().map(|&(p, _)| bin(p)).collect();
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for &(x, y) in &bins {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
        if bins.is_empty() {
            (x0, y0, x1, y1) = (0, 0, -1, -1);
        }
        let (x0, y0, x1, y1) = (x0 - pad, y0 - pad, x1 + pad, y1 + pad);
        let (width, height) = ((x1 - x0 + 1).max(0), (y1 - y0 + 1).max(0));
        let cell = |(x, y): (i32, i32)| ((y - y0) * width + (x - x0)) as usize;
        let cells = (width * height) as usize;
        let mut starts = vec![0u32; cells + 1];
        for &b in &bins {
            starts[cell(b) + 1] += 1;
        }
        for c in 1..starts.len() {
            starts[c] += starts[c - 1];
        }
        let mut fill = starts.clone();
        let mut items = vec![(FxVec2::ZERO, (FxVec2::ZERO, FxVec2::ZERO, Fx::ZERO)); all.len()];
        let mut near = vec![false; cells];
        for (&blocker, &(bx, by)) in all.iter().zip(&bins) {
            let c = cell((bx, by));
            items[fill[c] as usize] = blocker;
            fill[c] += 1;
            for y in by - pad..=by + pad {
                let row = ((y - y0) * width) as usize;
                near[row + (bx - pad - x0) as usize..=row + (bx + pad - x0) as usize].fill(true);
            }
        }
        Blockers {
            origin: (x0, y0),
            width,
            height,
            starts,
            items,
            near,
        }
    }

    fn index(&self, (x, y): (i32, i32)) -> Option<usize> {
        let (x, y) = (x - self.origin.0, y - self.origin.1);
        (x >= 0 && y >= 0 && x < self.width && y < self.height)
            .then(|| (y * self.width + x) as usize)
    }

    /// Whether a blocker's cell lies within reach of cell `c`.
    fn near(&self, c: (i32, i32)) -> bool {
        self.index(c).is_some_and(|c| self.near[c])
    }

    /// The blockers in cell `c`.
    fn at(&self, c: (i32, i32)) -> &[(FxVec2, Spine)] {
        self.index(c).map_or(&[], |c| {
            &self.items[self.starts[c] as usize..self.starts[c + 1] as usize]
        })
    }
}
