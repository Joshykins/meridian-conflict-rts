//! The heads of a reclaimer (`mc_data::Reclaimer`): towers, salvage vehicles and boats,
//! the Argus, carriers' drones.
//!
//! Head `i` is posed as a gun on a house of its own in weapon slot `i`: it turns by
//! `weapon_yaw[i]` off the hull's heading about its pivot and pitches by
//! `arm_pitch[2 + i]` about the same point, and `weapon_cooldown[i]` counts the ticks it
//! has charged on its target (a reclaimer carries no weapons, so the slots are free).
//!
//! Left alone, each head works a wreck of its own within the unit's reach, taking the one
//! it is nearest to pointing at so it sweeps a field in order. On an order every head
//! works the one target. The unit's `power` is split evenly across the heads whose beams
//! are on this tick. Reach is measured across the map from the unit's middle, so a wreck
//! deep under a boat or far below an aircraft is reached; the heads pitch to look at it.

use crate::reclaim::WIDEST_TARGET;
use crate::spatial::kind;
use crate::tables::flag;
use crate::World;
use mc_core::{Angle, Fx, FxVec2, FxVec3};
use mc_data::MAX_RECLAIM_HEADS;

/// How near a head must point at its work before it charges, angle steps (4 degrees).
const HEAD_AIM_TOLERANCE: u16 = 728;
/// Added to the cost of a wreck another head of the unit has taken this tick: a head
/// only doubles up on a wreck when there is nothing else in reach.
const CLAIMED: i64 = 1 << 40;

/// What a head works on.
#[derive(Clone, Copy, Debug)]
pub(crate) enum HeadWork {
    Wreck(usize),
    Unit(usize),
}

impl World {
    /// Where head `i` turns about, on the map: its pivot, or the unit's turret middle.
    fn head_origin(&self, row: usize, i: usize) -> FxVec2 {
        let bp = self.bp(row);
        let units = &self.state.units;
        let heading = units.heading[row];
        let pivot = bp.reclaimer.and_then(|r| r.heads()[i].pivot);
        match (pivot, bp.turret_at) {
            (Some(p), _) => units.pos[row] + p.xy().rotate(heading),
            (None, Some(at)) => units.pos[row] + at.rotate(heading),
            (None, None) => units.pos[row],
        }
    }

    /// Where head `i`'s beam leaves the model, on the map, as the head is posed now.
    pub(crate) fn head_emitter(&self, row: usize, i: usize) -> FxVec3 {
        let bp = self.bp(row);
        let units = &self.state.units;
        let Some(head) = bp.reclaimer.map(|r| r.heads()[i]) else {
            return units.pos[row].extend(units.z[row] + bp.height);
        };
        let heading = units.heading[row];
        let facing = heading + units.weapon_yaw[row][i];
        match head.pivot {
            Some(p) => {
                let at = crate::world::pitched(head.emitter, Some(p), units.arm_pitch[row][2 + i]);
                (units.pos[row] + p.xy().rotate(heading) + (at.xy() - p.xy()).rotate(facing))
                    .extend(units.z[row] + at.z)
            }
            None => (units.pos[row] + bp.turret_point(head.emitter.xy(), heading, facing))
                .extend(units.z[row] + head.emitter.z),
        }
    }

    /// Where the middle of `work` is, and how wide it is. `None`: it is gone.
    fn head_target(&self, work: HeadWork) -> Option<(FxVec2, Fx, Fx)> {
        match work {
            HeadWork::Wreck(w) => {
                let wrecks = &self.state.wrecks;
                if !wrecks.slots.is_alive(w) || wrecks.mass[w] <= Fx::ZERO {
                    return None;
                }
                let bp = self.blueprints.unit(wrecks.blueprint[w]);
                Some((wrecks.pos[w], wrecks.z[w] + bp.height / 2, bp.radius))
            }
            HeadWork::Unit(t) => {
                let units = &self.state.units;
                if !units.slots.is_alive(t) || units.health[t] <= Fx::ZERO {
                    return None;
                }
                let bp = self.bp(t);
                Some((units.pos[t], units.z[t] + bp.height / 2, bp.radius))
            }
        }
    }

    /// Turns and pitches head `i` toward `pos`, its middle at height `z`. True once it
    /// points there. A fixed head always does.
    fn aim_head(&mut self, row: usize, i: usize, pos: FxVec2, z: Fx) -> bool {
        let Some(head) = self.bp(row).reclaimer.map(|r| r.heads()[i]) else {
            return true;
        };
        if head.turn == 0 {
            return true;
        }
        let origin = self.head_origin(row, i);
        let units = &mut self.state.units;
        units.flags[row] |= flag::WORKING;
        let offset = pos - origin;
        let pitched_on = match head.pivot {
            Some(p) => {
                let rise = z - (units.z[row] + p.z);
                let raw =
                    Angle::ZERO.delta_to(FxVec2::new(offset.length().max(Fx::ONE), rise).angle());
                let (lo, hi) = (
                    Angle::ZERO.delta_to(head.pitch_min),
                    Angle::ZERO.delta_to(head.pitch_max),
                );
                let want = Angle(raw.clamp(lo, hi) as u16);
                let slot = 2 + i;
                let pitch = units.arm_pitch[row][slot].turn_toward(want, (head.turn / 2).max(1));
                units.arm_pitch[row][slot] = pitch;
                pitch.delta_to(want).unsigned_abs() <= HEAD_AIM_TOLERANCE
            }
            None => true,
        };
        // Right under the pivot the head only has to look down.
        if offset.length_sq() < Fx::ONE {
            return pitched_on;
        }
        let want = offset.angle() - units.heading[row];
        let yaw = units.weapon_yaw[row][i].turn_toward(want, head.turn);
        units.weapon_yaw[row][i] = yaw;
        pitched_on && yaw.delta_to(want).unsigned_abs() <= HEAD_AIM_TOLERANCE
    }

    /// Aims head `i` at `pos` and waits out its charge. True once its beam may come on.
    /// Losing the aim dumps the charge.
    fn head_ready(&mut self, row: usize, i: usize, pos: FxVec2, z: Fx) -> bool {
        if !self.aim_head(row, i, pos, z) {
            self.state.units.weapon_cooldown[row][i] = 0;
            return false;
        }
        let need = self.bp(row).reclaimer.map_or(0, |r| r.charge_ticks);
        let charged = self.state.units.weapon_cooldown[row][i];
        if charged < need {
            self.state.units.weapon_cooldown[row][i] = charged + 1;
            return false;
        }
        true
    }

    /// One tick of the heads whose beams are on (`jobs[i]` for head `i`), the unit's power
    /// split evenly between them. True once a target has nothing left to give.
    fn heads_drain(&mut self, row: usize, jobs: &[Option<HeadWork>]) -> bool {
        let working = jobs.iter().flatten().count();
        let Some(power) = self
            .bp(row)
            .reclaimer
            .filter(|_| working > 0)
            .map(|r| r.power / working as i32)
        else {
            return false;
        };
        let mut done = false;
        for (i, job) in jobs.iter().enumerate() {
            let Some(work) = *job else { continue };
            if self.head_target(work).is_none() {
                continue;
            }
            done |= match work {
                HeadWork::Wreck(w) => self.drain_wreck(row, w, power, i as u8),
                HeadWork::Unit(t) => self.drain_unit(row, t, power, i as u8),
            };
        }
        done
    }

    /// Every head that reaches it works `work`, on an order. True once it is gone.
    pub(crate) fn heads_work(&mut self, row: usize, work: HeadWork) -> bool {
        let Some((pos, z, _)) = self.head_target(work) else {
            return true;
        };
        let heads = self.bp(row).reclaimer.map_or(0, |r| r.heads().len());
        let mut jobs = [None; MAX_RECLAIM_HEADS];
        for (i, job) in jobs.iter_mut().enumerate().take(heads) {
            if self.head_ready(row, i, pos, z) {
                *job = Some(work);
            }
        }
        self.heads_drain(row, &jobs[..heads])
    }

    /// The wreck head `i` takes by itself: within the unit's reach, the one it is nearest
    /// to pointing at, the nearer the better among those alike; one another head has
    /// already taken only when there is no other.
    fn head_pick_wreck(&self, row: usize, i: usize, taken: &[usize]) -> Option<usize> {
        let r = self.bp(row).reclaimer?;
        let turns = r.heads()[i].turn > 0;
        let units = &self.state.units;
        let (pos, origin) = (units.pos[row], self.head_origin(row, i));
        let aim = units.heading[row] + units.weapon_yaw[row][i];
        let wrecks = &self.state.wrecks;
        let mut best: Option<(i64, usize)> = None;
        self.index
            .query(pos, r.range + WIDEST_TARGET, kind::WRECK, |e| {
                let w = e.row as usize;
                if wrecks.slots.is_alive(w)
                    && wrecks.pos[w] == e.pos
                    && wrecks.mass[w] > Fx::ZERO
                    && e.pos.distance(pos) <= r.range + e.radius
                {
                    let turn = if turns {
                        aim.delta_to((e.pos - origin).angle()).unsigned_abs() as i64
                    } else {
                        0
                    };
                    // A degree of turn (182 of a u16 turn) weighs as much as 18 m.
                    let cost = turn
                        + (e.pos.distance(origin) * 10).floor_int() as i64
                        + if taken.contains(&w) { CLAIMED } else { 0 };
                    if best.is_none_or(|b| (cost, w) < b) {
                        best = Some((cost, w));
                    }
                }
                true
            });
        best.map(|(_, w)| w)
    }

    /// Each head clears a wreck of its own within reach: what a reclaimer does idle, and
    /// what a mobile one does on the move.
    pub(crate) fn heads_clear_wrecks(&mut self, row: usize) {
        let heads = self.bp(row).reclaimer.map_or(0, |r| r.heads().len());
        let mut taken = [usize::MAX; MAX_RECLAIM_HEADS];
        let mut jobs = [None; MAX_RECLAIM_HEADS];
        for i in 0..heads {
            let Some(w) = self.head_pick_wreck(row, i, &taken[..i]) else {
                self.state.units.weapon_cooldown[row][i] = 0;
                continue;
            };
            taken[i] = w;
            if let Some((pos, z, _)) = self.head_target(HeadWork::Wreck(w)) {
                if self.head_ready(row, i, pos, z) {
                    jobs[i] = Some(HeadWork::Wreck(w));
                }
            }
        }
        self.heads_drain(row, &jobs[..heads]);
    }
}
