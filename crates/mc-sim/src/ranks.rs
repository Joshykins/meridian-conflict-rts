//! Keeping a ground block's ranks on the march: which way the block faces,
//! and which member takes which rank as the block changes shape.

use crate::nav::Steer;
use crate::World;
use mc_core::{Angle, Fx, FxVec2};

/// How far along its way a block looks to set its facing, in field steps.
const WAY_AHEAD: i32 = 64;
const TRACE_STEP: i32 = 8;

impl World {
    /// Which way the field leads from `anchor` over the next stretch: the
    /// chord to where it is `WAY_AHEAD` metres on, or `reach` if nearer.
    /// The field bends in steps under a moving anchor; a block that turned
    /// with every one of them would wheel its ranks back and forth.
    pub(crate) fn way_ahead(&self, field: u32, anchor: FxVec2, route: FxVec2, reach: Fx) -> FxVec2 {
        let mut p = anchor;
        let mut along = Fx::ZERO;
        while along < reach.min(Fx::from_int(WAY_AHEAD)) {
            match self.nav.sample(field, p) {
                Steer::Direction(d) => p += d * Fx::from_int(TRACE_STEP),
                _ => break,
            }
            along += Fx::from_int(TRACE_STEP);
        }
        if p == anchor {
            route
        } else {
            (p - anchor).normalize()
        }
    }

    /// Hand ranks round a block that is out of order: two members whose
    /// slots cross swap them, as when the order was given. A block that came
    /// through a pass in file, went round an obstacle in two streams or is
    /// wheeling through a turn otherwise has members driving through each
    /// other to reach slots on the far side, and pairs that shove each other
    /// to a standstill.
    pub(crate) fn regroup_ranks(&mut self, rows: &[usize], anchor: FxVec2, heading: Angle) {
        // Same bound as the order's own assignment.
        if rows.len() > 256 {
            return;
        }
        let units = &self.state.units;
        let nodes: Vec<u32> = rows.iter().map(|&row| units.order_head[row]).collect();
        let orders = &mut self.state.orders.order;
        for a in 0..rows.len() {
            for b in a + 1..rows.len() {
                let (oa, ob) = (orders[nodes[a] as usize], orders[nodes[b] as usize]);
                if oa.heading != ob.heading {
                    continue;
                }
                let sa = anchor + oa.offset.rotate(heading - oa.heading);
                let sb = anchor + ob.offset.rotate(heading - ob.heading);
                let (pa, pb) = (units.pos[rows[a]], units.pos[rows[b]]);
                if (pa - pb).dot(sa - sb) < Fx::ZERO {
                    orders[nodes[a] as usize].offset = ob.offset;
                    orders[nodes[b] as usize].offset = oa.offset;
                }
            }
        }
    }
}
