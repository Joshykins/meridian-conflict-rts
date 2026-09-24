//! Trees that are in the way: a builder raises a reclaim field over a lot and
//! vaporizes the trees on it before it lays the structure down, and a big
//! walker knocks over the trees it walks into.
//!
//! The field is one effect over the whole lot, not a beam per tree: a wave
//! runs out from the middle every `WAVE_TICKS`, each taking the trees out to
//! its own reach, until the last reaches the edge. Trees carry no mass, so it
//! pays nothing. Either way the tree is gone from `props_dead`'s point of view;
//! the renderer is told which (`SimEvent::TreeVaporized`, `TreeTrampled`) so it
//! can dissolve it or lay it down.

use crate::mirror::SimEvent;
use crate::reclaim::ReclaimWork;
use crate::spatial::kind;
use crate::tables::*;
use crate::World;
use mc_core::{Fx, FxVec2};
use mc_data::MoveLayer;

/// Waves a clearing field sends out, the last reaching the lot's edge.
pub const CLEAR_WAVES: u8 = 4;
/// Ticks from the field going up to the first wave, and between waves: the
/// whole lot goes in `CLEAR_WAVES * WAVE_TICKS`, 3.2 s.
pub const WAVE_TICKS: u16 = 8;
/// Crowns overhang the trunk: trees this close outside a lot are cleared too.
const LOT_MARGIN: Fx = Fx::from_int(2);
/// Walkers at least this wide knock trees over.
pub(crate) const TRAMPLE_RADIUS: Fx = Fx::from_int(10);
/// Share of a walker's radius that touches trunks: its legs and hull, not its reach.
const TRAMPLE_REACH: Fx = Fx::ratio(3, 4);

impl World {
    fn fell(&mut self, prop: usize) {
        self.state.props_dead[prop / 64] |= 1 << (prop % 64);
    }

    /// Half the side of the square a clearing field covers for a lot: lots may
    /// be turned a quarter, so it covers both ways, and the crowns overhang.
    fn lot_reach(footprint: (u8, u8)) -> Fx {
        Fx::from_int(footprint.0.max(footprint.1) as i32 * mc_map::BUILD_CELL_M / 2) + LOT_MARGIN
    }

    /// The live trees within `half` of `pos` on both axes.
    fn trees_on_lot(&self, pos: FxVec2, half: Fx) -> Vec<usize> {
        let mut found = Vec::new();
        self.prop_index.query(pos, half * 3 / 2, kind::PROP, |e| {
            let prop = e.row as usize;
            let d = e.pos - pos;
            if d.x.abs() <= half
                && d.y.abs() <= half
                && self.map.props[prop].kind.is_tree()
                && self.is_prop_alive(prop)
            {
                found.push(prop);
            }
            true
        });
        found.sort_unstable();
        found
    }

    /// One tick of clearing trees off a lot before building on it. True once
    /// there are none left. The builder's reclaim beam feeds a field over the
    /// whole lot (`reclaim_charge` counts its ticks); nothing is credited.
    pub(crate) fn clear_lot(&mut self, row: usize, footprint: (u8, u8), pos: FxVec2) -> bool {
        let half = Self::lot_reach(footprint);
        let trees = self.trees_on_lot(pos, half);
        let last = CLEAR_WAVES as u16 * WAVE_TICKS;
        let charge = self.state.units.reclaim_charge[row];
        // Left over from a clearing that was broken off: start this one afresh.
        let charge = if charge >= last { 0 } else { charge };
        if trees.is_empty() {
            self.state.units.reclaim_charge[row] = 0;
            return true;
        }
        let charge = charge + 1;
        self.state.units.reclaim_charge[row] = charge;
        self.state.units.flags[row] |= flag::HOLD | flag::RECLAIMING;
        let ground = self.terrain.height_at(pos);
        self.reclaims.push(ReclaimWork {
            source: self.state.units.id(row),
            unit: Handle::NONE,
            at: pos.extend(ground),
            radius: half,
            height: Fx::from_int(3),
            relay: false,
        });
        let wave = (charge % WAVE_TICKS == 0).then_some((charge / WAVE_TICKS) as u8);
        if charge == 1 || wave.is_some() {
            self.events.push(SimEvent::LotClearing {
                pos,
                half,
                wave: wave.unwrap_or(0),
            });
        }
        if let Some(wave) = wave {
            let reach = half * Fx::from_int(wave as i32) / CLEAR_WAVES as i32;
            for prop in trees {
                let d = self.map.props[prop].pos - pos;
                if wave == CLEAR_WAVES || (d.x.abs() <= reach && d.y.abs() <= reach) {
                    self.fell(prop);
                    self.events.push(SimEvent::TreeVaporized {
                        prop: prop as u32,
                        center: pos,
                    });
                }
            }
        }
        false
    }

    /// Big walkers flatten the trees they walk into, away from themselves.
    pub(crate) fn run_trampling(&mut self) {
        let units = &self.state.units;
        let mut felled = Vec::new();
        for row in units.slots.iter() {
            let bp = self.bp(row);
            let walks = bp
                .motion
                .as_ref()
                .is_some_and(|m| matches!(m.layer, MoveLayer::Land | MoveLayer::Amphibious));
            let (pos, prev) = (units.pos[row], units.prev_pos[row]);
            if !walks
                || bp.radius < TRAMPLE_RADIUS
                || pos == prev
                || units.has_flag(row, flag::IN_FACTORY | flag::UNDER_CONSTRUCTION)
            {
                continue;
            }
            let reach = bp.radius * TRAMPLE_REACH;
            self.prop_index.query(pos, reach, kind::PROP, |e| {
                let prop = e.row as usize;
                if e.pos.distance_sq(pos) <= reach * reach
                    && self.map.props[prop].kind.is_tree()
                    && self.is_prop_alive(prop)
                {
                    felled.push((prop, pos, pos - prev));
                }
                true
            });
        }
        for (prop, from, motion) in felled {
            if self.is_prop_alive(prop) {
                self.fell(prop);
                self.events.push(SimEvent::TreeTrampled {
                    prop: prop as u32,
                    from,
                    motion,
                });
            }
        }
    }
}
