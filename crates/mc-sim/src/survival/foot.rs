//! A Shaper's foot: the ground its tower stands on grows with it. As its built height
//! climbs (`Node::height`) the foot spreads (`rules::shaper_foot`, a disc of 30 m at
//! level 1 to about 180 m at level 20 for a printer), and the sim blocks the path cells
//! under it, so the bigger tower is walked round, built round and shot at as the
//! bigger thing it is (`World::hull_radius`).
//!
//! Units on the ground standing where it spreads to are shoved out to the nearest
//! ground they can stand on. A piece of its own base standing there unravels back
//! into it, to be raised again further out (`base::slot_at`, which places pieces past
//! the foot of the Shaper's level). Any other structure in the way holds the foot
//! where it is until it is gone: structures are never moved. When the Shaper dies its
//! foot's cells open again, save those a neighbour still stands on.

use super::rules::shaper_foot;
use crate::lots::CellRect as Cells;
use crate::spatial::kind;
use mc_core::{Fx, FxVec2};
use mc_data::MoveLayer;

/// Path cells (one run a row) whose centres lie within `radius` of `at`, on a map
/// `size` cells across.
pub(super) fn foot_cells(at: FxVec2, radius: Fx, size: (u32, u32)) -> Vec<Cells> {
    let cell = mc_map::CELL_SIZE_M;
    let r = radius.round_int().max(0);
    let (cx, cy) = (at.x.round_int(), at.y.round_int());
    let lo = |c: i32| ((c - r).max(0) / cell) as u32;
    let hi = |c: i32, n: u32| (((c + r).max(0) / cell) as u32).min(n.saturating_sub(1));
    let r2 = Fx::from_int(r) * Fx::from_int(r);
    let mut out = Vec::new();
    for y in lo(cy)..=hi(cy, size.1) {
        let mut run: Option<(u32, u32)> = None;
        for x in lo(cx)..=hi(cx, size.0) {
            let c = FxVec2::from_ints(x as i32 * cell + cell / 2, y as i32 * cell + cell / 2);
            if c.distance_sq(at) <= r2 {
                run = Some(run.map_or((x, x), |(a, _)| (a, x)));
            }
        }
        if let Some((a, b)) = run {
            out.push(((a, y), (b, y)));
        }
    }
    out
}

impl crate::World {
    /// Every standing Shaper's foot spreads toward its built height's
    /// (`shaper_foot`), a path cell at a time: the new ring of cells is blocked and
    /// whatever stands on the ground there is shoved out; a structure there holds it.
    pub(super) fn survival_feet(&mut self) {
        let Some(s) = self.state.survival.as_ref() else {
            return;
        };
        let cell = Fx::from_int(mc_map::CELL_SIZE_M);
        let due: Vec<(usize, Fx)> = s
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.raised)
            .filter_map(|(i, n)| {
                // Whole cells only, so it re-blocks as a cell's worth comes, not every tick.
                let want = Fx::from_int((shaper_foot(n.kind, n.height) / cell).floor_int()) * cell;
                (want > n.foot).then_some((i, want))
            })
            .collect();
        for (i, want) in due {
            let n = &self.state.survival.as_ref().unwrap().nodes[i];
            let Some(row) = self.state.units.row(n.unit) else {
                continue;
            };
            let at = self.state.units.pos[row];
            let cells = foot_cells(at, want, self.terrain.size_cells());
            if self.unravel_pieces_on(i, &cells).is_err()
                || self.structure_on(row, at, want, &cells)
            {
                continue;
            }
            for &(min, max) in &cells {
                self.nav.block_cells(min, max);
                self.nav.reseal(min, max);
            }
            self.shove_out(row, at, want);
            self.state.survival.as_mut().unwrap().nodes[i].foot = want;
        }
    }

    /// Pieces of Shaper `i`'s own base standing on any of `cells` unravel back into it
    /// (its base raises them again past the foot, `base::slot_at`).
    fn unravel_pieces_on(&mut self, i: usize, cells: &[Cells]) -> Result<(), crate::SimError> {
        let pieces = self.state.survival.as_ref().unwrap().nodes[i]
            .pieces
            .clone();
        for p in pieces {
            let Some(r) = self.state.units.row(p.unit) else {
                continue;
            };
            let units = &self.state.units;
            let hull = crate::lots::hull_cells(self.bp(r), units.pos[r], units.heading[r]);
            if !hull
                .iter()
                .any(|h| cells.iter().any(|c| crate::lots::cells_overlap(*h, *c)))
            {
                continue;
            }
            self.despawn_unit(r, false)?;
            // Gone on purpose, not lost: no wait before it is raised again.
            self.state.survival.as_mut().unwrap().nodes[i]
                .pieces
                .retain(|q| q.unit != p.unit);
        }
        Ok(())
    }

    /// Whether a structure other than the one in `shaper` stands on any of `cells`
    /// (round `at`, out to `reach`).
    fn structure_on(&self, shaper: usize, at: FxVec2, reach: Fx, cells: &[Cells]) -> bool {
        let mut found = false;
        let far = reach + Fx::from_int(200);
        self.index.query(at, far, kind::UNIT, |e| {
            let r = e.row as usize;
            if r == shaper || !self.unit_entry_is_current(e) {
                return true;
            }
            let units = &self.state.units;
            let bp = self.bp(r);
            if !bp.is_structure() {
                return true;
            }
            let hull = crate::lots::hull_cells(bp, units.pos[r], units.heading[r]);
            found = hull
                .iter()
                .any(|h| cells.iter().any(|c| crate::lots::cells_overlap(*h, *c)));
            !found
        });
        found
    }

    /// Units on the ground within `reach` of `at` (their hull reaching in) are set down on
    /// the nearest ground they can stand on outside it.
    fn shove_out(&mut self, shaper: usize, at: FxVec2, reach: Fx) {
        let mut inside = Vec::new();
        self.index
            .query(at, reach + Fx::from_int(60), kind::UNIT, |e| {
                let r = e.row as usize;
                if r != shaper && self.unit_entry_is_current(e) {
                    inside.push(r);
                }
                true
            });
        inside.sort_unstable();
        inside.dedup();
        for r in inside {
            let bp = self.bp(r);
            let Some(m) = bp.motion.filter(|m| m.layer != MoveLayer::Air) else {
                continue;
            };
            let pos = self.state.units.pos[r];
            if pos.distance(at) - bp.radius > reach {
                continue;
            }
            let out = at + (pos - at).normalize() * (reach + bp.radius + Fx::from_int(8));
            let Some(to) = self.nav.nearest_passable(m.layer, m.size_class, out) else {
                continue;
            };
            let units = &mut self.state.units;
            units.pos[r] = to;
            units.prev_pos[r] = to;
        }
    }

    /// A Shaper that died: its foot's cells open again, save those a neighbour or a
    /// solid prop still stands on.
    pub(super) fn survival_release_foot(&mut self, at: FxVec2, foot: Fx) {
        if foot <= Fx::ZERO {
            return;
        }
        let cells = foot_cells(at, foot, self.terrain.size_cells());
        for &(min, max) in &cells {
            self.nav.unblock_cells(min, max);
        }
        self.restore_blockers(usize::MAX, &cells, None);
    }

    /// The radius of the unit in `row`'s hull: its blueprint's, or a grown Shaper's
    /// foot. What its enemies' guns reach for, hit and blast.
    pub(crate) fn hull_radius(&self, row: usize) -> Fx {
        let bp = self.bp(row);
        if bp.growth.is_none() {
            return bp.radius;
        }
        let id = self.state.units.id(row);
        self.state
            .survival
            .as_ref()
            .and_then(|s| s.nodes.iter().find(|n| n.unit == id))
            .map_or(bp.radius, |n| n.foot.max(bp.radius))
    }
}
