//! Which water a fleet can sail to.
//!
//! The navy used to be sent at the water nearest a contact or an enemy start.
//! On island maps that was often a pond or a cut-off channel: the path failed,
//! the ships dropped the order and sat idle where they were for the rest of the
//! match, sent there again every think.
use super::*;
use mc_data::MoveLayer;
use std::collections::VecDeque;

/// Spacing of the coarse grid the sea is flooded on, in metres. A channel
/// narrower than this may be missed; the fleet then keeps to the water it has.
const NODE_M: i32 = 128;

/// The water reachable from one spot, on a coarse grid.
pub(super) struct SeaReach {
    width: i32,
    height: i32,
    reached: Vec<bool>,
}

impl SeaReach {
    fn node(&self, pos: FxVec2) -> Option<usize> {
        let (x, y) = (pos.x.floor_int() / NODE_M, pos.y.floor_int() / NODE_M);
        (x >= 0 && y >= 0 && x < self.width && y < self.height)
            .then(|| (y * self.width + x) as usize)
    }

    pub(super) fn reaches(&self, pos: FxVec2) -> bool {
        self.node(pos).is_some_and(|i| self.reached[i])
    }

    fn centre(&self, i: usize) -> FxVec2 {
        let (x, y) = (i as i32 % self.width, i as i32 / self.width);
        FxVec2::from_ints(x * NODE_M + NODE_M / 2, y * NODE_M + NODE_M / 2)
    }

    /// The reached water nearest `to`.
    pub(super) fn nearest(&self, to: FxVec2) -> Option<FxVec2> {
        (0..self.reached.len())
            .filter(|&i| self.reached[i])
            .map(|i| self.centre(i))
            .min_by_key(|p| (p.distance_sq(to), p.x, p.y))
    }
}

impl World {
    /// Flood the water a hull of `layer`/`size` at `from` can sail to.
    pub(super) fn sea_reach(&self, layer: MoveLayer, size: u8, from: FxVec2) -> Option<SeaReach> {
        let map = self.terrain.size_metres();
        let mut sea = SeaReach {
            width: map.x.floor_int() / NODE_M,
            height: map.y.floor_int() / NODE_M,
            reached: Vec::new(),
        };
        sea.reached = vec![false; (sea.width * sea.height).max(0) as usize];
        let first = sea.node(from)?;
        // The ship's own node counts even if its centre is on the shore.
        sea.reached[first] = true;
        let mut queue = VecDeque::from([first]);
        while let Some(i) = queue.pop_front() {
            let (x, y) = (i as i32 % sea.width, i as i32 / sea.width);
            let here = if i == first { from } else { sea.centre(i) };
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= sea.width || ny >= sea.height {
                    continue;
                }
                let n = (ny * sea.width + nx) as usize;
                if sea.reached[n] {
                    continue;
                }
                let there = sea.centre(n);
                let mid = here.lerp(there, Fx::HALF);
                if self.nav.passable(layer, size, there) && self.nav.passable(layer, size, mid) {
                    sea.reached[n] = true;
                    queue.push_back(n);
                }
            }
        }
        Some(sea)
    }
}
