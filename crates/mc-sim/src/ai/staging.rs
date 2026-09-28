//! Where a land wave gathers: a point the army can walk to from home.
//!
//! The staging point used to be a fixed 240 m from the start toward the enemy.
//! When a cliff or a river ran through that spot, units stopped at the edge,
//! never came within `STAGING_RADIUS` of it, were sent back to it every think,
//! and no wave ever left: the army piled up at home for the rest of the match.
use super::*;
use mc_data::MoveLayer;
use std::collections::VecDeque;

/// How far around the start the walkable ground is flooded, in 8 m cells.
const FLOOD_CELLS: i32 = 90;

/// The land around a start that a hull can walk to from it.
pub(super) struct HomeGround {
    origin: mc_path::Cell,
    reached: Vec<bool>,
}

impl HomeGround {
    fn index(&self, pos: FxVec2) -> Option<usize> {
        let c = mc_path::Cell::from_pos(pos);
        let (x, y) = (c.x - self.origin.x, c.y - self.origin.y);
        let side = 2 * FLOOD_CELLS + 1;
        (x >= 0 && y >= 0 && x < side && y < side).then(|| (y * side + x) as usize)
    }

    /// Whether `pos` can be walked to from home. Ground outside the flooded
    /// square counts as reachable: nothing is known about it.
    pub(super) fn reaches(&self, pos: FxVec2) -> bool {
        self.index(pos).is_none_or(|i| self.reached[i])
    }
}

/// Land units answer a raider only from walkable ground this close to it.
const ANSWER_REACH: Fx = Fx::from_int(150);

impl World {
    /// Whether the land army can get within shooting distance of `pos`: from
    /// walkable ground near it that is not cut off from home.
    pub(super) fn land_can_answer(&self, pos: FxVec2, home: Option<&HomeGround>) -> bool {
        self.nav
            .nearest_passable(MoveLayer::Land, 0, pos)
            .is_some_and(|p| p.distance(pos) <= ANSWER_REACH && home.is_none_or(|g| g.reaches(p)))
    }

    /// Flood the ground a land hull can walk to from `start`: stopped by
    /// cliffs and water, not by buildings, which come and go. `size` picks
    /// the seed: the nearest spot such a hull can stand.
    pub(super) fn home_ground(&self, start: FxVec2, size: u8) -> Option<HomeGround> {
        let seed = self.nav.nearest_passable(MoveLayer::Land, size, start)?;
        let c = mc_path::Cell::from_pos(start);
        let ground = HomeGround {
            origin: mc_path::Cell::new(c.x - FLOOD_CELLS, c.y - FLOOD_CELLS),
            reached: vec![false; ((2 * FLOOD_CELLS + 1) * (2 * FLOOD_CELLS + 1)) as usize],
        };
        let mut ground = ground;
        let mut queue = VecDeque::new();
        let first = ground.index(seed)?;
        ground.reached[first] = true;
        queue.push_back(mc_path::Cell::from_pos(seed));
        while let Some(cell) = queue.pop_front() {
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let next = mc_path::Cell::new(cell.x + dx, cell.y + dy);
                let pos = next.center();
                let Some(i) = ground.index(pos) else {
                    continue;
                };
                // Terrain only: the base's own buildings wall the start in, and
                // counting them left everything outside the yard "cut off".
                if !ground.reached[i]
                    && next.x >= 0
                    && next.y >= 0
                    && self.nav.passable_terrain(
                        (next.x as u32, next.y as u32),
                        (next.x as u32, next.y as u32),
                    )
                {
                    ground.reached[i] = true;
                    queue.push_back(next);
                }
            }
        }
        Some(ground)
    }

    /// The staging point nearest `want` that the army can walk to from home,
    /// looking around the start at about the same distance first.
    pub(super) fn reachable_staging(
        &self,
        start: FxVec2,
        want: FxVec2,
        ground: &HomeGround,
    ) -> FxVec2 {
        if ground.reaches(want) {
            return want;
        }
        let dist = want.distance(start);
        let facing = (want - start).angle();
        // Swing out to either side of the enemy line, then try nearer and
        // farther rings: the nearest point on the start's own shelf wins.
        for r in [
            dist,
            dist * Fx::ratio(2, 3),
            dist * Fx::ratio(3, 2),
            dist / 3,
        ] {
            for step in 1..=9i32 {
                for side in [1, -1] {
                    let turn = Angle((step * side * 2048) as i16 as u16);
                    let p = start + FxVec2::from_angle(facing + turn) * r;
                    if ground.reaches(p) && self.nav.passable(MoveLayer::Land, 0, p) {
                        return p;
                    }
                }
            }
            let p = start + FxVec2::from_angle(facing) * r;
            if ground.reaches(p) && self.nav.passable(MoveLayer::Land, 0, p) {
                return p;
            }
        }
        start
    }
}
