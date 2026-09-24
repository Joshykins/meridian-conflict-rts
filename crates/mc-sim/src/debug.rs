//! Debug commands: what the test range does to the world. They go through the
//! command stream like any order, so a range session is as deterministic (and
//! as replayable) as a match; all of them are ignored unless the match was
//! created with `cheats` on.

use crate::command::{Command, PlayerCommand};
use crate::tables::*;
use crate::world::snap_to_build_grid;
use crate::{SimError, World};
use mc_core::{Fx, FxVec2};

impl World {
    /// Applies `pc` if it is a debug command. Returns whether it was one.
    pub(crate) fn apply_debug(&mut self, pc: &PlayerCommand) -> Result<bool, SimError> {
        let is_debug = matches!(
            pc.command,
            Command::DebugSpawn { .. }
                | Command::DebugDamage { .. }
                | Command::DebugRemove { .. }
                | Command::DebugSetFlags { .. }
                | Command::DebugSetBuild { .. }
                | Command::DebugClear
                | Command::DebugControl { .. }
                | Command::DebugFreeBuild { .. }
                | Command::DebugStock { .. }
                | Command::DebugIncome { .. }
                | Command::DebugStorage { .. }
                | Command::DebugWrecks { .. }
        );
        if !is_debug || !self.state.cheats {
            return Ok(is_debug);
        }
        let player_count = self.state.players.len();
        match &pc.command {
            Command::DebugSpawn {
                owner,
                blueprint,
                pos,
                heading,
                count,
                flags,
                build,
            } => {
                if *owner as usize >= player_count
                    || blueprint.index() >= self.blueprints.units.len()
                {
                    return Ok(true);
                }
                let bp = self.blueprints.unit(*blueprint).clone();
                let mut rows = Vec::new();
                if bp.is_structure() {
                    let site = snap_to_build_grid(&bp, *pos);
                    if self.can_place(&bp, site) {
                        rows.push(self.spawn_unit(*blueprint, *owner, site, *heading, true)?);
                    }
                } else {
                    // A square block centred on `pos`.
                    let n = (*count).clamp(1, 1024) as i32;
                    let cols = (Fx::from_int(n).sqrt().ceil_int()).max(1);
                    let spacing = bp.radius * 2 + Fx::from_int(3);
                    for i in 0..n {
                        let offset = FxVec2::new(
                            spacing * (i % cols - cols / 2),
                            spacing * (i / cols - cols / 2),
                        );
                        let p = self.clamp_to_map(*pos + offset);
                        rows.push(self.spawn_unit(*blueprint, *owner, p, *heading, true)?);
                    }
                }
                for row in rows {
                    self.state.units.flags[row] |= flags & flag::DEBUG;
                    if *build < 1000 {
                        self.debug_set_build(row, *build)?;
                    }
                }
            }
            Command::DebugDamage { units, permille } => {
                for row in self.debug_rows(units) {
                    let full = self.unit_max_health(row);
                    let health = &mut self.state.units.health[row];
                    *health = (*health - full * *permille as i32 / 1000).min(full);
                }
            }
            Command::DebugRemove { units } => {
                for row in self.debug_rows(units) {
                    // A factory's product goes with it, so a row may already be gone.
                    if self.state.units.slots.is_alive(row) {
                        self.remove_unit_row(row, false)?;
                    }
                }
            }
            Command::DebugSetFlags { units, set, clear } => {
                for row in self.debug_rows(units) {
                    let flags = &mut self.state.units.flags[row];
                    *flags = (*flags & !(clear & flag::DEBUG)) | (set & flag::DEBUG);
                }
            }
            Command::DebugSetBuild { units, permille } => {
                for row in self.debug_rows(units) {
                    self.debug_set_build(row, *permille)?;
                }
            }
            Command::DebugClear => {
                for row in 0..self.state.units.slots.rows() {
                    if self.state.units.slots.is_alive(row) {
                        self.remove_unit_row(row, false)?;
                    }
                }
                let wrecks = &mut self.state.wrecks;
                for row in 0..wrecks.slots.rows() {
                    if wrecks.slots.is_alive(row) {
                        wrecks.slots.free(row);
                    }
                }
                self.state.projectiles.clear();
                self.state.stains.clear();
                self.state.fires.clear();
                self.state.pads.clear();
            }
            Command::DebugControl { player } => {
                if (*player as usize) < player_count {
                    self.state.players[pc.player as usize].acts_as = *player;
                }
            }
            Command::DebugFreeBuild { player, on } => {
                if let Some(p) = self.state.players.get_mut(*player as usize) {
                    p.free_build = *on;
                }
            }
            Command::DebugStock {
                player,
                mass,
                energy,
            } => {
                if let Some(p) = self.state.players.get_mut(*player as usize) {
                    let share = |permille: u16| Fx::from_int(permille.min(1000) as i32) / 1000;
                    if let Some(m) = mass {
                        p.mass = p.mass_capacity * share(*m);
                    }
                    if let Some(e) = energy {
                        p.energy = p.energy_capacity * share(*e);
                    }
                }
            }
            Command::DebugIncome {
                player,
                mass,
                energy,
            } => {
                if let Some(p) = self.state.players.get_mut(*player as usize) {
                    p.income_permille = [*mass, *energy];
                }
            }
            Command::DebugStorage {
                player,
                mass,
                energy,
            } => {
                if let Some(p) = self.state.players.get_mut(*player as usize) {
                    let whole = |v: u32| Fx::from_int(v.min(10_000_000) as i32);
                    p.bonus_storage = [whole(*mass), whole(*energy)];
                }
            }
            Command::DebugWrecks {
                blueprint,
                pos,
                count,
            } => {
                if blueprint.index() >= self.blueprints.units.len() {
                    return Ok(true);
                }
                let bp = self.blueprints.unit(*blueprint);
                let mass = bp.cost_mass * bp.wreck_fraction;
                if mass <= Fx::ZERO {
                    return Ok(true);
                }
                // A loose square block, a little apart so each can be picked on its own.
                let n = (*count).clamp(1, 256) as i32;
                let cols = (Fx::from_int(n).sqrt().ceil_int()).max(1);
                let spacing = bp.radius * 3 + Fx::from_int(4);
                for i in 0..n {
                    let offset = FxVec2::new(
                        spacing * (i % cols - cols / 2),
                        spacing * (i / cols - cols / 2),
                    );
                    let p = self.clamp_to_map(*pos + offset);
                    let z = self.terrain.height_at(p).max(self.terrain.water_level());
                    let heading = mc_core::Angle::from_degrees(i * 67 % 360);
                    if self.state.wrecks.slots.live() >= MAX_WRECKS {
                        break;
                    }
                    self.state.wrecks.spawn(*blueprint, p, z, heading, mass)?;
                }
            }
            _ => {}
        }
        Ok(true)
    }

    /// Live units in `ids`, whoever owns them; units still inside a factory are left alone.
    fn debug_rows(&self, ids: &[UnitId]) -> Vec<usize> {
        let units = &self.state.units;
        ids.iter()
            .filter_map(|id| units.row(*id))
            .filter(|&row| !units.has_flag(row, flag::IN_FACTORY))
            .collect()
    }

    /// Turns a unit into a construction site that is `permille` thousandths
    /// built, or finishes it. Nobody is building it: it stays as it is until an
    /// engineer is told to help.
    fn debug_set_build(&mut self, row: usize, permille: u16) -> Result<(), SimError> {
        let under_construction = self.state.units.has_flag(row, flag::UNDER_CONSTRUCTION);
        if permille >= 1000 {
            if under_construction {
                self.complete_unit(row)?;
            }
            return Ok(());
        }
        if !under_construction {
            self.clear_orders(row)?;
        }
        let (full, build_time) = (self.bp(row).health, self.bp(row).build_time);
        let units = &mut self.state.units;
        units.flags[row] |= flag::UNDER_CONSTRUCTION;
        units.build_progress[row] = build_time * permille as i32 / 1000;
        // As a real site: a tenth of full health to start with, the rest with progress.
        units.health[row] = full / 10 + full * Fx::ratio(9, 10) * permille as i32 / 1000;
        units.speed[row] = Fx::ZERO;
        units.weapon_target[row] = [Handle::NONE; mc_data::MAX_WEAPONS];
        units.shield_hp[row] = Fx::ZERO;
        units.shield_open[row] = 0;
        units.prev_shield_open[row] = 0;
        units.shield_recharge[row] = 0;
        Ok(())
    }
}
