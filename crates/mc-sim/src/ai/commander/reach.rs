//! The ground a side's land army can walk to from home, found once a match on a
//! coarse grid from the terrain alone (cliffs and water stop it, buildings do not),
//! and kept in its state. Operations ask it before sending a land group anywhere.
use crate::World;
use mc_core::FxVec2;
use mc_data::MoveLayer;
use std::collections::VecDeque;

/// Metres between the grid's nodes.
const NODE: i32 = 128;

pub(in crate::ai) struct Reach {
    w: i32,
    h: i32,
    bits: Vec<u64>,
}

impl Reach {
    fn node(&self, p: FxVec2) -> Option<usize> {
        let (x, y) = (p.x.floor_int() / NODE, p.y.floor_int() / NODE);
        (x >= 0 && y >= 0 && x < self.w && y < self.h).then(|| (y * self.w + x) as usize)
    }

    /// Whether `p`, or a node beside it, is walkable from home.
    pub(in crate::ai) fn reaches(&self, p: FxVec2) -> bool {
        let hit = |i: usize| {
            self.bits
                .get(i / 64)
                .is_some_and(|b| b >> (i % 64) & 1 != 0)
        };
        let Some(i) = self.node(p) else {
            return false;
        };
        let (x, y) = (i as i32 % self.w, i as i32 / self.w);
        (-1..=1).any(|dy| {
            (-1..=1).any(|dx| {
                let (nx, ny) = (x + dx, y + dy);
                nx >= 0 && ny >= 0 && nx < self.w && ny < self.h && hit((ny * self.w + nx) as usize)
            })
        })
    }
}

impl World {
    /// `player`'s walkable ground from home, flooded on its first think.
    pub(in crate::ai) fn home_reach(&mut self, player: u8) -> Reach {
        let size = self.terrain.size_metres();
        let (w, h) = (size.x.floor_int() / NODE, size.y.floor_int() / NODE);
        let n = (w * h).max(0) as usize;
        if self.state.ai[player as usize].commander.land.len() != n.div_ceil(64) {
            let bits = self.flood_land(player, w, h);
            self.state.ai[player as usize].commander.land = bits;
        }
        Reach {
            w,
            h,
            bits: self.state.ai[player as usize].commander.land.clone(),
        }
    }

    fn flood_land(&self, player: u8, w: i32, h: i32) -> Vec<u64> {
        let n = (w * h).max(0) as usize;
        let mut bits = vec![0u64; n.div_ceil(64)];
        let centre = |i: usize| {
            FxVec2::from_ints(
                (i as i32 % w) * NODE + NODE / 2,
                (i as i32 / w) * NODE + NODE / 2,
            )
        };
        let walkable = |p: FxVec2| {
            let c = mc_path::Cell::from_pos(p);
            c.x >= 0
                && c.y >= 0
                && self
                    .nav
                    .passable_terrain((c.x as u32, c.y as u32), (c.x as u32, c.y as u32))
        };
        let start = self.state.players[player as usize].start;
        let Some(seed) = self.nav.nearest_passable(MoveLayer::Land, 0, start) else {
            return bits;
        };
        let (sx, sy) = (seed.x.floor_int() / NODE, seed.y.floor_int() / NODE);
        if sx < 0 || sy < 0 || sx >= w || sy >= h {
            return bits;
        }
        let first = (sy * w + sx) as usize;
        bits[first / 64] |= 1 << (first % 64);
        let mut queue = VecDeque::from([first]);
        while let Some(i) = queue.pop_front() {
            let (x, y) = (i as i32 % w, i as i32 / w);
            let here = centre(i);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w || ny >= h {
                    continue;
                }
                let j = (ny * w + nx) as usize;
                if bits[j / 64] >> (j % 64) & 1 != 0 {
                    continue;
                }
                let there = centre(j);
                if walkable(there) && walkable(here.lerp(there, mc_core::Fx::HALF)) {
                    bits[j / 64] |= 1 << (j % 64);
                    queue.push_back(j);
                }
            }
        }
        bits
    }
}
