//! The side's picture of the battlefield (`docs/AI_COMMANDER.md`, "World model"): a
//! coarse grid of enemy threat by target class, enemy value, our own strength, and
//! how long since we last had eyes on each cell. Rebuilt each think from the side's
//! memory of contacts; only the last-seen ticks are kept between thinks.
use super::profile::{Profiles, Target, TARGETS};
use crate::World;
use mc_core::{Fx, FxVec2};
use mc_data::cat;

/// Metres a side of a cell.
pub(in crate::ai) const CELL: i32 = 512;

pub(in crate::ai) struct WorldModel {
    pub w: i32,
    pub h: i32,
    /// Enemy damage a second that reaches each cell, per target class.
    pub threat: Vec<[Fx; TARGETS]>,
    /// Enemy mass standing in each cell (the commander counted heavily).
    pub value: Vec<Fx>,
    /// Our own armed mass in each cell.
    pub ours: Vec<Fx>,
}

impl WorldModel {
    pub(in crate::ai) fn dims(size: FxVec2) -> (i32, i32) {
        (
            (size.x.floor_int() + CELL - 1) / CELL,
            (size.y.floor_int() + CELL - 1) / CELL,
        )
    }

    pub(in crate::ai) fn cell(&self, p: FxVec2) -> Option<usize> {
        let (x, y) = (p.x.floor_int() / CELL, p.y.floor_int() / CELL);
        (x >= 0 && y >= 0 && x < self.w && y < self.h).then(|| (y * self.w + x) as usize)
    }

    pub(in crate::ai) fn centre(&self, i: usize) -> FxVec2 {
        let (x, y) = (i as i32 % self.w, i as i32 / self.w);
        FxVec2::from_ints(x * CELL + CELL / 2, y * CELL + CELL / 2)
    }

    /// Enemy fire on class `t` at `p`.
    pub(in crate::ai) fn threat_at(&self, p: FxVec2, t: Target) -> Fx {
        self.cell(p)
            .map_or(Fx::ZERO, |i| self.threat[i][t as usize])
    }

    /// The most enemy fire of class `t` along the straight line from `a` to `b`,
    /// sampled every half cell.
    pub(in crate::ai) fn threat_along(&self, a: FxVec2, b: FxVec2, t: Target) -> Fx {
        let steps = (a.distance(b) / (CELL / 2)).floor_int().clamp(1, 200);
        (0..=steps)
            .map(|k| self.threat_at(a.lerp(b, Fx::from_int(k) / steps), t))
            .max()
            .unwrap_or(Fx::ZERO)
    }
}

impl World {
    /// The side's world model, from its contacts, own units and fog.
    pub(in crate::ai) fn world_model(&mut self, player: u8, profiles: &Profiles) -> WorldModel {
        let (w, h) = WorldModel::dims(self.terrain.size_metres());
        let n = (w * h).max(0) as usize;
        let mut m = WorldModel {
            w,
            h,
            threat: vec![[Fx::ZERO; TARGETS]; n],
            value: vec![Fx::ZERO; n],
            ours: vec![Fx::ZERO; n],
        };
        let contacts = &self.state.ai[player as usize].contacts;
        for c in contacts {
            let p = profiles.get(c.blueprint);
            let bp = self.blueprints.unit(c.blueprint);
            if let Some(i) = m.cell(c.pos) {
                m.value[i] += if bp.has(cat::COMMANDER) {
                    Fx::from_int(5000)
                } else {
                    bp.cost_mass
                };
            }
            if !p.armed() {
                continue;
            }
            // A unit that moves threatens only what is near it, for a while.
            let reach_cells = |r: Fx| (r / CELL).ceil_int() + 1;
            let longest = p.reach.iter().copied().max().unwrap_or(Fx::ZERO);
            // Map guns reach the whole map; counting them everywhere hides all else.
            let longest = longest.min(Fx::from_int(4 * CELL));
            let (cx, cy) = (c.pos.x.floor_int() / CELL, c.pos.y.floor_int() / CELL);
            let r = reach_cells(longest);
            for y in (cy - r).max(0)..=(cy + r).min(h - 1) {
                for x in (cx - r).max(0)..=(cx + r).min(w - 1) {
                    let i = (y * w + x) as usize;
                    let d = m.centre(i).distance(c.pos);
                    for t in 0..TARGETS {
                        if p.reach[t] > Fx::ZERO && d <= p.reach[t] + Fx::from_int(CELL / 2) {
                            m.threat[i][t] += p.dps[t];
                        }
                    }
                }
            }
        }
        let units = &self.state.units;
        for r in units.slots.iter() {
            if units.owner[r] != player || !units.is_active(r) {
                continue;
            }
            let p = profiles.get(units.blueprint[r]);
            if p.armed() && p.mobile() {
                if let Some(i) = m.cell(units.pos[r]) {
                    m.ours[i] += p.mass;
                }
            }
        }
        // Eyes on a cell now: it is fresh.
        let mask = self.team_mask(player);
        let tick = self.state.tick;
        let seen = &mut self.state.ai[player as usize].commander.seen;
        if seen.len() != n {
            *seen = vec![0; n];
        }
        for (i, s) in seen.iter_mut().enumerate() {
            if self.fog.is_visible(m.centre(i), mask) {
                *s = tick;
            }
        }
        m
    }
}
