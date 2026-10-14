//! Authored doodad salvage uses the same tools, storage limits and beams as wrecks.
use crate::{reclaim::ReclaimWork, reclaim_heads::HeadWork, Handle, SimError, World};
use mc_core::{Fx, FxVec2};

impl World {
    /// Authored rows are bounded by the placement limit, iterated in stable prop
    /// order. A collapsed doodad retains its salvage until collected.
    pub(crate) fn nearest_salvage_prop(
        &self,
        row: usize,
        centre: FxVec2,
        radius: Fx,
    ) -> Option<usize> {
        let owner = self.state.units.owner[row];
        let from = self.state.units.pos[row];
        let reach = self.work_range(row);
        let mut best = None;
        for (&prop, d) in &self.map.doodads {
            if d.reclaim == 0 {
                continue;
            }
            let Ok(p) = self.state.city.prop.binary_search(&prop) else {
                continue;
            };
            if self.state.city.salvage[p] <= Fx::ZERO {
                continue;
            }
            let (at, size, _) = self.city_shapes.outline(p);
            if at.xy().distance(centre) > radius + size {
                continue;
            }
            let mask = self.team_mask(owner);
            if self.state.fog_enabled
                && !self.fog.is_explored(at.xy(), mask)
                && !self.fog.is_detected(at.xy(), mask)
            {
                continue;
            }
            if self.bp(row).motion.is_some_and(|m| {
                self.nav
                    .nearest_passable(m.layer, m.size_class, at.xy())
                    .is_none_or(|spot| spot.distance(at.xy()) > reach + size)
            }) {
                continue;
            }
            let key = (from.distance(at.xy()), p);
            if best.is_none_or(|b| key < b) {
                best = Some(key);
            }
        }
        best.map(|(_, p)| p)
    }

    pub(crate) fn run_reclaim_prop(&mut self, row: usize, p: usize) -> Result<(), SimError> {
        let (at, radius, _) = self.city_shapes.outline(p);
        if !self.approach(row, at.xy(), radius)? {
            // Unreachable geometry must not trap an area order forever.
            if self.state.units.stuck_ticks[row] == u16::MAX {
                self.finish_order(row);
                return Ok(());
            }
            return self.reclaim_on_the_way(row);
        }
        if self.bp(row).reclaimer.is_some() {
            self.heads_work(row, HeadWork::Prop(p));
        } else if self.reclaim_ready(row, at.xy()) {
            self.drain_prop(row, p, self.tool_power(row), 0);
        }
        Ok(())
    }

    pub(crate) fn drain_prop(&mut self, row: usize, p: usize, power: Fx, head: u8) -> bool {
        let player = &self.state.players[self.state.units.owner[row] as usize];
        let left = self.state.city.salvage[p];
        let room = if player.free_build {
            left
        } else {
            (player.materials_capacity - player.materials).max(Fx::ZERO)
        };
        let take = (power * World::reclaim_rate()).min(left).min(room);
        if take <= Fx::ZERO {
            return left == Fx::ZERO;
        }
        let (at, radius, height) = self.city_shapes.outline(p);
        let s = &mut self.state.city;
        s.touch(p);
        s.salvage[p] -= take;
        let gone = s.salvage[p] == Fx::ZERO;
        if gone {
            s.health[p] = 0;
            s.flags[p] =
                (s.flags[p] & !crate::city::BURNING) | crate::city::DOWN | crate::city::RECLAIMED;
            s.since[p] = self.state.tick;
            let prop = s.prop[p] as usize;
            self.state.props_dead[prop / 64] |= 1 << (prop % 64);
            self.open_prop_cells(prop, at, radius);
        }
        self.reclaims.push(ReclaimWork {
            source: self.state.units.id(row),
            unit: Handle::NONE,
            at,
            radius,
            height,
            relay: false,
            head,
        });
        self.credit(row, take);
        gone
    }
}
