//! Where a formation ordered onto occupied ground stands instead: the whole
//! layout shifted off parked hulls and structures, the slots kept together.

use crate::spatial::kind;
use crate::tables::flag;
use crate::World;
use mc_core::{Angle, Fx, FxVec2};
use mc_data::MoveLayer;

impl World {
    /// Keep a layout intact when the clicked ground is occupied by a parked
    /// hull or a structure. Search whole-layout translations, not stacked slots.
    pub(crate) fn clear_formation_destination(
        &self,
        center: FxVec2,
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
        // whatever stands there: the nav grid and the crowd are nothing to them.
        let striders = rows
            .iter()
            .all(|&r| self.bp(r).motion.is_some_and(|m| m.stride));
        // Everything that could stand in the layout's way anywhere the search
        // below looks, gathered once and binned by 16 m cell: the slots of a
        // big block over a few hundred spots are then checked against these.
        let rings = 10;
        let widest = offsets.iter().map(|o| o.length()).fold(Fx::ZERO, Fx::max);
        let area = widest + spacing.max(Fx::from_int(12)) * rings + radius + Fx::from_int(6);
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
            // Ships and dived submarines may share a spot, one under the other.
            if rows.iter().all(|&r| self.hulls_pass(r, other)) {
                return true;
            }
            if air && (m.altitude - first.altitude).abs() > self.bp(other).height + Fx::ONE {
                return true;
            }
            blockers.push((e.pos, e.radius));
            true
        });
        let widest_blocker = blockers.iter().map(|&(_, r)| r).fold(Fx::ZERO, Fx::max);
        let reach = radius + widest_blocker + Fx::from_int(6);
        let blockers = Blockers::new(&blockers, bin, reach);
        let free = |candidate: FxVec2| {
            offsets.iter().all(|offset| {
                let pos = candidate + *offset;
                if striders {
                    return self.stride_footing(pos);
                }
                if !self.terrain.in_bounds(pos) || !self.nav.passable(layer, size, pos) {
                    return false;
                }
                if !blockers.near(bin(pos)) {
                    return true;
                }
                let (x0, y0) = bin(pos - FxVec2::new(reach, reach));
                let (x1, y1) = bin(pos + FxVec2::new(reach, reach));
                (y0..=y1).all(|y| {
                    (x0..=x1).all(|x| {
                        blockers
                            .at((x, y))
                            .iter()
                            .all(|&(at, r)| pos.distance(at) >= radius + r + Fx::from_int(6))
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
                let shift = FxVec2::from_angle(Angle(spoke * 4096))
                    * (spacing.max(Fx::from_int(12)) * ring);
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
}

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
    items: Vec<(FxVec2, Fx)>,
    /// The cells within `reach` of some blocker's cell: a slot in any other
    /// has nothing near it to look at.
    near: Vec<bool>,
}

impl Blockers {
    fn new(all: &[(FxVec2, Fx)], bin: impl Fn(FxVec2) -> (i32, i32), reach: Fx) -> Blockers {
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
        let mut items = vec![(FxVec2::ZERO, Fx::ZERO); all.len()];
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
    fn at(&self, c: (i32, i32)) -> &[(FxVec2, Fx)] {
        self.index(c).map_or(&[], |c| {
            &self.items[self.starts[c] as usize..self.starts[c + 1] as usize]
        })
    }
}
