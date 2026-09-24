//! Shield generators and hull wraps: open and close the field, regenerate it,
//! and drop it when the economy stalls. Every shield, dome or hull wrap, runs
//! on its side's energy: while the grid cannot pay, it is down. An in-place
//! refit keeps a dome up.
//!
//! A shattered dome starts filling at once, stays down while it fills, and
//! only rises again at full charge. Engineers can feed a live (damaged)
//! bubble extra regen for energy; they cannot hurry that recovery.

use crate::mirror::SimEvent;
use crate::World;
use mc_core::{Fx, TICKS_PER_SECOND};

const DT: i32 = TICKS_PER_SECOND as i32;
/// How far `shield_open` moves toward open each tick (~2 s for a full rise).
const OPEN_STEP: u8 = 13;
/// Closing on a stall is a little quicker than opening.
const CLOSE_STEP: u8 = 16;
/// A shattered dome peels in about 0.6 s: fast enough to read as a break,
/// slow enough that the glass is seen coming apart instead of popping off.
const BREAK_STEP: u8 = 42;
/// The bubble only stops shots once it is mostly there.
pub(crate) const SHIELD_BLOCKING_OPEN: u8 = 200;
/// A broken dome refills this many times faster than a live one regenerates,
/// so a generator is not out of the fight for long after it blips.
const BREAK_REGEN_MUL: i32 = 2;

impl World {
    /// A live, mostly-open field that still has hit points.
    pub(crate) fn shield_blocking(&self, row: usize) -> bool {
        self.bp(row).shield.is_some()
            && self.state.units.is_active(row)
            && self.state.units.shield_open[row] >= SHIELD_BLOCKING_OPEN
            && self.state.units.shield_hp[row] > Fx::ZERO
            && self.state.units.shield_recharge[row] == 0
            && !self.shields_unpowered(self.state.units.owner[row])
    }

    pub(crate) fn energy_stalling(&self, player: u8) -> bool {
        let p = &self.state.players[player as usize];
        !p.free_build && p.energy <= Fx::ZERO && p.efficiency < Fx::ONE
    }

    /// The side cannot pay its upkeep: every shield it owns is down. Upkeep is
    /// paid before building, so a side short only on construction keeps them.
    pub(crate) fn shields_unpowered(&self, player: u8) -> bool {
        let p = &self.state.players[player as usize];
        !p.free_build && p.energy <= Fx::ZERO && p.upkeep_efficiency < Fx::ONE
    }

    fn shield_wants_up(&self, row: usize) -> bool {
        self.bp(row).shield.is_some()
            && self.state.units.is_active(row)
            && self.state.units.shield_recharge[row] == 0
            && !self.shields_unpowered(self.state.units.owner[row])
    }

    /// A live dome that is missing charge: engineers may pump extra regen into
    /// it. A shattered bubble filling while down is not this.
    pub(crate) fn shield_assistable(&self, row: usize) -> bool {
        let Some(spec) = self.bp(row).shield else {
            return false;
        };
        let hp = self.state.units.shield_hp[row];
        self.state.units.is_active(row)
            && self.state.units.health[row] >= self.unit_max_health(row)
            && self.state.units.shield_recharge[row] == 0
            && hp > Fx::ZERO
            && hp < spec.health
            && !self.energy_stalling(self.state.units.owner[row])
    }

    /// Build-time units left to restore a live dome, so several engineers share
    /// what remains the same way they share a repair.
    pub(crate) fn shield_work_remaining(&self, row: usize) -> Fx {
        let Some(spec) = self.bp(row).shield else {
            return Fx::ZERO;
        };
        let missing = spec.health - self.state.units.shield_hp[row];
        if missing > Fx::ZERO {
            let time = self.bp(row).build_time;
            (missing * time / spec.health.max(Fx::ONE)).max(Fx::EPSILON)
        } else {
            Fx::ZERO
        }
    }

    /// Raise, lower and regenerate every bubble. Called after the economy so a
    /// stall this tick drops shields the same tick, and so engineer assist
    /// lands before the generator's own regen.
    pub(crate) fn run_shields(&mut self) {
        let rows: Vec<usize> = self.state.units.slots.iter().collect();
        for row in rows {
            let Some(spec) = self.bp(row).shield else {
                continue;
            };
            if !self.state.units.is_active(row) {
                continue;
            }
            if self.state.units.shield_recharge[row] > 0 {
                self.recover_shield(row, spec.health, spec.regen);
            }
            let want = self.shield_wants_up(row);
            let open = self.state.units.shield_open[row];
            self.state.units.shield_open[row] = if want {
                open.saturating_add(OPEN_STEP)
            } else if self.state.units.shield_recharge[row] > 0 {
                open.saturating_sub(BREAK_STEP)
            } else {
                open.saturating_sub(CLOSE_STEP)
            };
            if !want || self.state.units.shield_open[row] < SHIELD_BLOCKING_OPEN {
                continue;
            }
            let hp = self.state.units.shield_hp[row];
            if hp < spec.health {
                self.state.units.shield_hp[row] = (hp + spec.regen / DT).min(spec.health);
            }
        }
    }

    /// Fill after a break at `BREAK_REGEN_MUL` times the live rate. The dome
    /// stays down until it is full; a stall pauses the fill.
    /// `shield_recharge` stays set until then.
    fn recover_shield(&mut self, row: usize, max: Fx, regen: Fx) {
        if self.shields_unpowered(self.state.units.owner[row]) {
            return;
        }
        let hp = self.state.units.shield_hp[row];
        if hp < max {
            self.state.units.shield_hp[row] = (hp + regen * BREAK_REGEN_MUL / DT).min(max);
        }
        if self.state.units.shield_hp[row] >= max {
            self.state.units.shield_recharge[row] = 0;
        }
    }

    pub(crate) fn arm_shield(&mut self, row: usize, instant: bool) {
        let Some(spec) = self.bp(row).shield else {
            return;
        };
        self.state.units.shield_hp[row] = spec.health;
        self.state.units.shield_recharge[row] = 0;
        let open = if instant { 255 } else { 0 };
        self.state.units.shield_open[row] = open;
        self.state.units.prev_shield_open[row] = open;
    }

    /// An upgrade that just finished keeps its parent's bubble: same opening,
    /// same charge delay, hit points scaled onto the new dome.
    pub(crate) fn inherit_shield(&mut self, child: usize) {
        if self.bp(child).shield.is_none() {
            return;
        }
        let id = self.state.units.id(child);
        let Some(parent) = self
            .state
            .units
            .slots
            .iter()
            .find(|&row| self.state.units.build_target[row] == id)
        else {
            self.arm_shield(child, false);
            return;
        };
        let Some(old) = self.bp(parent).shield else {
            self.arm_shield(child, false);
            return;
        };
        let spec = self.bp(child).shield.unwrap();
        let ratio =
            (self.state.units.shield_hp[parent] / old.health.max(Fx::ONE)).clamp(Fx::ZERO, Fx::ONE);
        self.state.units.shield_hp[child] = spec.health * ratio;
        self.state.units.shield_recharge[child] = self.state.units.shield_recharge[parent];
        self.state.units.shield_open[child] = self.state.units.shield_open[parent];
        self.state.units.prev_shield_open[child] = self.state.units.prev_shield_open[parent];
    }

    pub(crate) fn break_shield(&mut self, row: usize) {
        let Some(spec) = self.bp(row).shield else {
            return;
        };
        let units = &mut self.state.units;
        units.shield_hp[row] = Fx::ZERO;
        // Shots already pass (`hp` and `recharge`). Peel the glass this tick so
        // the collapse is seen interpolating, not snapped off between frames.
        units.shield_open[row] = units.shield_open[row].saturating_sub(BREAK_STEP * 2);
        units.shield_recharge[row] = 1;
        self.events.push(SimEvent::ShieldBroken {
            pos: units.pos[row].extend(units.z[row]),
            radius: spec.radius,
            owner: units.owner[row],
        });
    }

    pub(crate) fn damage_shield(&mut self, row: usize, damage: Fx) {
        if !self.shield_blocking(row) || damage <= Fx::ZERO {
            return;
        }
        // Survival's veil: it takes every hit and gives nothing.
        if self
            .state
            .units
            .has_flag(row, crate::tables::flag::INVULNERABLE)
        {
            return;
        }
        let hp = self.state.units.shield_hp[row];
        self.state.units.shield_hp[row] = hp - damage.min(hp);
        if self.state.units.shield_hp[row] <= Fx::ZERO {
            self.break_shield(row);
        }
    }

    /// The closest live bubble of `owner`'s team that contains `pos`.
    pub(crate) fn shield_covering(&self, pos: mc_core::FxVec2, owner: u8) -> Option<usize> {
        let team = self.state.players[owner as usize].team;
        let mut best = None;
        let mut best_d = Fx::MAX;
        for row in self.state.units.slots.iter() {
            if self.state.players[self.state.units.owner[row] as usize].team != team {
                continue;
            }
            if !self.shield_blocking(row) {
                continue;
            }
            let spec = self.bp(row).shield.unwrap();
            // A hull wrap only covers its carrier; splash on a neighbour
            // must not charge the Paladin standing next to it.
            if spec.is_hull() {
                continue;
            }
            let radius = spec.radius;
            let d = self.state.units.pos[row].distance_sq(pos);
            if d <= radius * radius && d < best_d {
                best_d = d;
                best = Some(row);
            }
        }
        best
    }
}
