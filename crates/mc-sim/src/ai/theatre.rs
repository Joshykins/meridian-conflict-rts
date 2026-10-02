//! Whether a side's land army can walk to its enemies at all.
//!
//! On an island map (The Axis) no tank ever reaches an enemy: the AI built
//! land factories and tanks all the same, sent them off in waves whose paths
//! failed at the shore, and 20 to 38 of them per side stood parked at home by
//! twenty minutes while no shipyard was ever built. The land route is found
//! once per match, from the terrain alone, and decides what the side builds
//! and which units go out (`LandRoute`).
use super::*;
use mc_data::MoveLayer;
use std::collections::VecDeque;

/// Spacing of the coarse grid the land is flooded on, in metres. A land
/// bridge narrower than this may be missed.
const NODE_M: i32 = 64;
/// Land-only combat units a side with no land route still keeps at home, to
/// meet what lands on its island.
pub(super) const HOME_GUARD: usize = 8;

/// Which enemy starts a side's land army can walk to, found once per match.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct LandRoute {
    /// Bit `p` set: player `p`'s start can be walked to from this side's.
    pub reaches: u32,
}

impl LandRoute {
    fn reaches_player(self, p: usize) -> bool {
        self.reaches >> p & 1 != 0
    }
}

/// Whether a unit moves on the land alone: it cannot cross water.
pub(super) fn land_bound(bp: &UnitBlueprint) -> bool {
    bp.motion.is_some_and(|m| m.layer == MoveLayer::Land)
}

impl World {
    /// Flood the land walkable from `from` on a coarse grid and see which starts
    /// it reaches. Terrain only, so the answer never changes over a match.
    fn find_land_route(&self, player: u8) -> LandRoute {
        let start = self.state.players[player as usize].start;
        let map = self.terrain.size_metres();
        let (w, h) = (map.x.floor_int() / NODE_M, map.y.floor_int() / NODE_M);
        let node = |p: FxVec2| {
            let (x, y) = (p.x.floor_int() / NODE_M, p.y.floor_int() / NODE_M);
            (x >= 0 && y >= 0 && x < w && y < h).then(|| (y * w + x) as usize)
        };
        let centre = |i: usize| {
            let (x, y) = (i as i32 % w, i as i32 / w);
            FxVec2::from_ints(x * NODE_M + NODE_M / 2, y * NODE_M + NODE_M / 2)
        };
        let walkable = |p: FxVec2| {
            let c = mc_path::Cell::from_pos(p);
            c.x >= 0
                && c.y >= 0
                && self
                    .nav
                    .passable_terrain((c.x as u32, c.y as u32), (c.x as u32, c.y as u32))
        };
        let mut reached = vec![false; (w * h).max(0) as usize];
        let Some(first) = self
            .nav
            .nearest_passable(MoveLayer::Land, 0, start)
            .and_then(node)
        else {
            return LandRoute::default();
        };
        reached[first] = true;
        let mut queue = VecDeque::from([first]);
        while let Some(i) = queue.pop_front() {
            let (x, y) = (i as i32 % w, i as i32 / w);
            let here = centre(i);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w || ny >= h {
                    continue;
                }
                let n = (ny * w + nx) as usize;
                let there = centre(n);
                if !reached[n] && walkable(there) && walkable(here.lerp(there, Fx::HALF)) {
                    reached[n] = true;
                    queue.push_back(n);
                }
            }
        }
        let mut route = LandRoute::default();
        for (p, pl) in self.state.players.iter().enumerate() {
            let seed = self.nav.nearest_passable(MoveLayer::Land, 0, pl.start);
            if seed.and_then(node).is_some_and(|i| reached[i]) {
                route.reaches |= 1 << p;
            }
        }
        route
    }

    /// Finds `player`'s land route on its first think.
    pub(super) fn find_land_route_once(&mut self, player: u8) {
        if self.state.ai[player as usize].land_route.is_none() {
            let route = self.find_land_route(player);
            self.state.ai[player as usize].land_route = Some(route);
        }
    }

    /// Whether `player`'s land army can walk to an enemy still in the game
    /// (true until the route has been found).
    pub(super) fn land_route_to_enemy(&self, player: u8) -> bool {
        let Some(route) = self.state.ai[player as usize].land_route else {
            return true;
        };
        self.state.players.iter().enumerate().any(|(p, pl)| {
            !pl.defeated && self.are_enemies(player, p as u8) && route.reaches_player(p)
        })
    }

    /// The water nearest `start` where a shipyard of `bp` can stand, looked for
    /// in rings out to 2.4 km. A shipyard's site search around the base's
    /// yard reached the coast only on small islands.
    pub(super) fn shipyard_anchor(&self, bp: &UnitBlueprint, start: FxVec2) -> Option<FxVec2> {
        (1..=30).find_map(|ring| {
            let r = Fx::from_int(ring * 80);
            let n = 8 + ring * 2;
            (0..n)
                .map(|k| start + FxVec2::from_angle(Angle((k * 65536 / n) as u16)) * r)
                .map(|p| snap_to_build_grid(bp, p))
                .filter(|&p| self.terrain.in_bounds(p) && self.can_place(bp, p))
                .min_by_key(|p| (p.distance_sq(start), p.x, p.y))
        })
    }

    /// Land units of the home guard a side with no land route keeps back, plus
    /// what its lift ships could carry, so landings have cargo (`theatre.rs`).
    pub(super) fn land_guard_cap(&self, player: u8, census: &Census) -> usize {
        let commander_lands = self.state.ai[player as usize]
            .commander
            .plan(super::commander::state::PlanKind::Landing)
            > super::commander::state::Stake::Off;
        if !commander_lands {
            return HOME_GUARD;
        }
        let room: usize = census
            .lifts
            .iter()
            .filter_map(|&r| self.bp(r).transport)
            .map(|t| t.capacity as usize / 2)
            .sum();
        HOME_GUARD + room.max(4)
    }
}

#[cfg(test)]
#[path = "theatre_tests.rs"]
mod tests;
