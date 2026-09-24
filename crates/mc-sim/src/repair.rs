//! Hull repair: putting mass and energy back into a finished unit.
//!
//! An engineer told to assist a damaged friend already does this. One with
//! nothing to do looks for a wounded ally in reach and mends it where it
//! stands — it never walks, and it never holds an order, so it still counts
//! as idle. Live enemies, units still being built, and a hull a friend is
//! taking apart are left alone.

use crate::reclaim::{BeamInstance, WIDEST_TARGET};
use crate::spatial::kind;
use crate::tables::*;
use crate::{SimError, World};
use mc_core::Fx;

/// `BeamInstance::kind` of a repair beam. Reclaim is 0; 1 is kept for construction.
pub const BEAM_REPAIR: u32 = 2;

impl World {
    /// This tick's repair beams. Left out when the viewer can see neither end.
    /// Appends after reclaim so one list carries every work beam.
    pub(crate) fn write_repair_beams(
        &self,
        viewer: Option<u8>,
        out: &mut Vec<BeamInstance>,
        sources: &mut Vec<u32>,
    ) {
        let s = &self.state;
        let seen = |p: mc_core::FxVec2| {
            viewer.is_none_or(|v| !s.fog_enabled || self.fog.is_detected(p, self.team_mask(v)))
        };
        for row in s.units.slots.iter() {
            if s.units.flags[row] & flag::REPAIRING == 0 {
                continue;
            }
            let Some(t) = s.units.row(s.units.build_target[row]) else {
                continue;
            };
            let (from_pos, to) = (s.units.pos[row], s.units.pos[t].extend(s.units.z[t]));
            if !seen(from_pos) && !seen(to.xy()) {
                continue;
            }
            let from = self.builder_emitter(row);
            let bp = self.bp(t);
            sources.push(s.units.id(row).0);
            out.push(BeamInstance {
                from: from.to_f32(),
                kind: BEAM_REPAIR,
                to_prev: s.units.prev_pos[t].extend(s.units.prev_z[t]).to_f32(),
                radius: bp.radius.to_f32(),
                to: to.to_f32(),
                height: bp.height.to_f32(),
            });
        }
    }

    /// Whether the builder in `row` may mend the unit in `t`: a finished ally
    /// that is missing health. Never itself, an enemy, a site still going up,
    /// something assembled inside a factory, or a hull a friend is taking apart.
    pub(crate) fn can_repair_unit(&self, row: usize, t: usize) -> bool {
        let units = &self.state.units;
        if t == row
            || !units.slots.is_alive(t)
            || units.health[t] <= Fx::ZERO
            || units.health[t] >= self.unit_max_health(t)
            || units.flags[t] & (flag::IN_FACTORY | flag::UNDER_CONSTRUCTION) != 0
        {
            return false;
        }
        !self.are_enemies(units.owner[row], units.owner[t]) && !self.friendly_reclaiming(t)
    }

    /// True when someone on this unit's side has a reclaim order on it.
    fn friendly_reclaiming(&self, t: usize) -> bool {
        let units = &self.state.units;
        let (target, owner) = (units.id(t), units.owner[t]);
        units.slots.iter().any(|row| {
            !self.are_enemies(units.owner[row], owner)
                && self
                    .state
                    .orders
                    .front(units, row)
                    .is_some_and(|o| o.kind == OrderKind::ReclaimUnit && o.target == target)
        })
    }

    /// What a mobile builder with no orders does by itself: it mends wounded
    /// allies within reach. It never leaves where it stands, and it never
    /// holds an order, so it still counts as idle. A builder that carries
    /// weapons leaves hulls alone while there is anything about to shoot at.
    pub(crate) fn idle_repair(&mut self, row: usize) -> Result<bool, SimError> {
        let bp = self.bp(row);
        if bp.builder.is_none() || !bp.is_mobile() {
            return Ok(false);
        }
        let units = &self.state.units;
        if units.has_flag(row, flag::PASSIVE)
            || self.work_paused(row)
            || self.has_live_target(row)
            || self.enemy_in_gun_range(row)
        {
            return Ok(false);
        }
        let (pos, range) = (units.pos[row], self.work_range(row));
        let found = self
            .index
            .nearest(pos, range + WIDEST_TARGET, kind::UNIT, |e| {
                let t = e.row as usize;
                self.unit_entry_is_current(e)
                    && self.can_repair_unit(row, t)
                    && e.pos.distance(pos) <= range + e.radius
            });
        let Some(e) = found else {
            return Ok(false);
        };
        let t = e.row as usize;
        let target_pos = self.state.units.pos[t];
        self.state.units.flags[row] |= flag::HOLD;
        self.state.units.build_target[row] = self.state.units.id(t);
        let middle = self.state.units.z[t] + self.bp(t).height / 2;
        if self.face_work_at(row, target_pos, middle) {
            self.state.units.flags[row] |= flag::BUILDING | flag::REPAIRING;
        }
        Ok(true)
    }
}
