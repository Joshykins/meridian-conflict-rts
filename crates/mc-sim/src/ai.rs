//! The skirmish AI.
//!
//! It runs inside the simulation on every machine, reads only game state and
//! its own hashed `AiState`, and acts by queueing ordinary commands for the
//! next tick. It gets no information a player would not have except the enemy
//! start positions, and no resource bonus.

use crate::command::{Command, PlayerCommand};
use crate::spatial::kind;
use crate::tables::*;
use crate::world::snap_to_build_grid;
use crate::{SimError, World};
use mc_core::{Angle, Fx, FxVec2, StateHasher};
use mc_data::{cat, BlueprintId, UnitBlueprint};
use serde::{Deserialize, Serialize};

/// Ticks between decisions for one AI player.
const THINK_PERIOD: u32 = 10;
/// Build sites tried per placement search.
const MAX_SITE_PROBES: i32 = 400;
/// Enemies this close to the start position count as a raid on the base.
const BASE_DEFENCE_RADIUS: Fx = Fx::from_int(700);

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AiState {
    /// Attack waves sent so far; later waves wait for more units.
    pub waves: u32,
    /// Rotates through the factory roster.
    pub production_counter: u32,
}

impl AiState {
    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.waves as u64 | (self.production_counter as u64) << 32);
    }
}

#[derive(Default)]
struct Census {
    builders_idle: Vec<usize>,
    factories: Vec<usize>,
    factories_idle: Vec<usize>,
    extractors: Vec<usize>,
    engineers: usize,
    power: usize,
    army_idle: Vec<usize>,
    army: usize,
    sites: usize,
}

impl World {
    pub(crate) fn run_ai(&mut self) -> Result<(), SimError> {
        for p in 0..self.state.players.len() {
            let player = &self.state.players[p];
            if player.controller != Controller::Ai || player.defeated {
                continue;
            }
            if (self.state.tick + p as u32 * 3) % THINK_PERIOD == 0 {
                self.think(p as u8);
            }
        }
        Ok(())
    }

    fn census(&self, player: u8) -> Census {
        let units = &self.state.units;
        let mut c = Census::default();
        for row in units.slots.iter() {
            if units.owner[row] != player || units.has_flag(row, flag::IN_FACTORY) {
                continue;
            }
            let bp = self.bp(row);
            if !units.is_active(row) {
                c.sites += 1;
                continue;
            }
            let idle = units.order_head[row] == NO_ORDER;
            if bp.has(cat::FACTORY) {
                c.factories.push(row);
                if idle {
                    c.factories_idle.push(row);
                }
            } else if bp.has(cat::EXTRACTOR) {
                c.extractors.push(row);
            } else if bp.has(cat::POWER) {
                c.power += 1;
            } else if bp.is_mobile() && bp.builder.is_some() {
                if bp.has(cat::ENGINEER) && !bp.has(cat::COMMANDER) {
                    c.engineers += 1;
                }
                if idle {
                    c.builders_idle.push(row);
                }
            } else if bp.is_mobile() && !bp.weapons.is_empty() {
                c.army += 1;
                if idle {
                    c.army_idle.push(row);
                }
            }
        }
        c
    }

    fn think(&mut self, player: u8) {
        let census = self.census(player);
        let mut out: Vec<Command> = Vec::new();
        let pl = &self.state.players[player as usize];
        let start = pl.start;
        let (mass_income, energy_income) = (pl.mass_income, pl.energy_income);
        let energy_short = pl.energy_demand > pl.energy_income || pl.energy < pl.energy_capacity / 5;
        let mass_rich = pl.mass > pl.mass_capacity * Fx::ratio(7, 10);

        // Builders: one job each, decided in a fixed priority order. Sites
        // chosen this think are remembered so two builders do not pick the same one.
        let mut claimed: Vec<FxVec2> = Vec::new();
        let mut planned_factories = census.factories.len();
        let mut planned_power = census.power;
        for &row in &census.builders_idle {
            let builder_pos = self.state.units.pos[row];
            let is_commander = self.bp(row).has(cat::COMMANDER);
            let want_factories = 1 + (mass_income / Fx::from_int(7)).floor_int().clamp(0, 5) as usize;
            let want_power = 2 + planned_factories * 3 + (energy_short as usize) * 2;

            let choice = if planned_factories == 0 {
                self.pick_structure(row, cat::FACTORY).map(|bp| (bp, start))
            } else if planned_power < want_power.min(4) || (energy_short && planned_power < want_power) {
                self.pick_structure(row, cat::POWER).map(|bp| (bp, start))
            } else if let Some(deposit) = self.free_deposit(player, builder_pos, &claimed, if is_commander { Fx::from_int(500) } else { Fx::from_int(6000) }) {
                self.pick_structure(row, cat::EXTRACTOR).map(|bp| (bp, deposit))
            } else if planned_factories < want_factories {
                self.pick_structure(row, cat::FACTORY).map(|bp| (bp, start))
            } else if planned_power < want_power || energy_income < mass_income * 12 {
                self.pick_structure(row, cat::POWER).map(|bp| (bp, start))
            } else if mass_rich {
                self.pick_structure(row, cat::DEFENSE | cat::DIRECT_FIRE).map(|bp| (bp, start))
            } else {
                None
            };

            match choice {
                Some((blueprint, near)) => {
                    let bp = self.blueprints.unit(blueprint).clone();
                    let site = if bp.needs_deposit { Some(snap_to_build_grid(&bp, near)) } else { self.find_site(&bp, near, &claimed) };
                    if let Some(site) = site.filter(|s| self.can_place(&bp, *s)) {
                        claimed.push(site);
                        planned_factories += bp.has(cat::FACTORY) as usize;
                        planned_power += bp.has(cat::POWER) as usize;
                        out.push(Command::Build { units: vec![self.state.units.id(row)], blueprint, pos: site, heading: Angle::ZERO, queue: false });
                    }
                }
                None => {
                    // Nothing to build: help the first factory.
                    if let Some(&f) = census.factories.first() {
                        out.push(Command::Assist { units: vec![self.state.units.id(row)], target: self.state.units.id(f), queue: false });
                    }
                }
            }
        }

        // Factories: keep a few engineers, otherwise cycle the combat roster, favouring higher tech.
        let mut counter = self.state.ai[player as usize].production_counter;
        for &row in &census.factories_idle {
            let Some(builder) = &self.bp(row).builder else { continue };
            let want_engineers = 2 + census.factories.len() * 2;
            let engineer = builder.builds.iter().rev().copied().find(|b| self.blueprints.unit(*b).has(cat::ENGINEER));
            let fighters: Vec<BlueprintId> = builder.builds.iter().copied().filter(|b| !self.blueprints.unit(*b).weapons.is_empty()).collect();
            let top_tech = fighters.iter().map(|b| self.blueprints.unit(*b).tech).max().unwrap_or(1);
            let best: Vec<BlueprintId> = fighters.iter().copied().filter(|b| self.blueprints.unit(*b).tech + 1 >= top_tech && !self.blueprints.unit(*b).has(cat::SCOUT)).collect();
            let blueprint = if census.engineers < want_engineers && counter % 3 == 0 {
                engineer
            } else if best.is_empty() {
                None
            } else {
                Some(best[counter as usize % best.len()])
            };
            counter = counter.wrapping_add(1);
            if let Some(blueprint) = blueprint {
                out.push(Command::Produce { factories: vec![self.state.units.id(row)], blueprint, count: 1 });
            }
        }
        self.state.ai[player as usize].production_counter = counter;

        // Tech up when the economy can carry it.
        if mass_income >= Fx::from_int(6) && mass_rich {
            let upgradable = census
                .factories_idle
                .iter()
                .chain(&census.extractors)
                .copied()
                .find(|&row| self.bp(row).upgrades_to.is_some() && self.state.units.order_head[row] == NO_ORDER);
            if let Some(row) = upgradable {
                out.push(Command::Upgrade { units: vec![self.state.units.id(row)] });
            }
        }

        // Army: defend the base first, otherwise attack in growing waves.
        if !census.army_idle.is_empty() {
            let ids: Vec<UnitId> = census.army_idle.iter().take(crate::command::MAX_COMMAND_UNITS).map(|&r| self.state.units.id(r)).collect();
            let raid = self.index.nearest(start, BASE_DEFENCE_RADIUS, kind::UNIT, |e| {
                self.unit_entry_is_current(e) && self.are_enemies(player, self.state.units.owner[e.row as usize]) && self.detects(player, e.row as usize)
            });
            let wave_size = (6 + self.state.ai[player as usize].waves * 3).min(60) as usize;
            if let Some(raider) = raid {
                out.push(Command::AttackMove { units: ids, target: raider.pos, queue: false });
            } else if census.army_idle.len() >= wave_size {
                if let Some(target) = self.attack_target(player, start) {
                    self.state.ai[player as usize].waves += 1;
                    out.push(Command::AttackMove { units: ids, target, queue: false });
                }
            }
        }

        self.state.ai_pending.extend(out.into_iter().map(|command| PlayerCommand { player, command }));
    }

    /// The most advanced structure with all of `categories` this builder can make.
    fn pick_structure(&self, builder_row: usize, categories: u32) -> Option<BlueprintId> {
        let builder = self.bp(builder_row).builder.as_ref()?;
        builder
            .builds
            .iter()
            .copied()
            .filter(|b| {
                let bp = self.blueprints.unit(*b);
                bp.has(categories) && bp.is_structure() && !bp.has(cat::WALL)
            })
            .max_by_key(|b| (self.blueprints.unit(*b).tech, std::cmp::Reverse(b.0)))
    }

    /// Nearest deposit without an extractor on it, within `range` of `from`.
    fn free_deposit(&self, player: u8, from: FxVec2, claimed: &[FxVec2], range: Fx) -> Option<FxVec2> {
        let _ = player;
        self.map
            .deposits
            .iter()
            .copied()
            .filter(|d| d.distance(from) <= range && self.structure_at(*d, 0).is_none() && !claimed.iter().any(|c| c.distance(*d) < Fx::from_int(32)))
            .min_by_key(|d| (d.distance_sq(from), d.x, d.y))
    }

    /// Square spiral over a lattice around `near` until a buildable, unclaimed site turns up.
    fn find_site(&self, bp: &UnitBlueprint, near: FxVec2, claimed: &[FxVec2]) -> Option<FxVec2> {
        // Lattice pitch leaves a lane between neighbouring structures.
        let pitch = Fx::from_int((bp.footprint.0.max(bp.footprint.1) as i32 + 2) * mc_map::BUILD_CELL_M);
        let clearance = pitch;
        let (mut x, mut y, mut dx, mut dy) = (0i32, 0i32, 0i32, -1i32);
        for _ in 0..MAX_SITE_PROBES {
            if (x, y) != (0, 0) || bp.needs_deposit {
                let site = snap_to_build_grid(bp, near + FxVec2::new(pitch * x, pitch * y));
                let free = self.can_place(bp, site)
                    && !claimed.iter().any(|c| c.distance(site) < clearance)
                    && !self.map.deposits.iter().any(|d| d.distance(site) < clearance);
                if free {
                    return Some(site);
                }
            }
            if x == y || (x < 0 && x == -y) || (x > 0 && x == 1 - y) {
                (dx, dy) = (-dy, dx);
            }
            x += dx;
            y += dy;
        }
        None
    }

    /// Where to send a wave: the closest enemy start that still has an owner standing.
    fn attack_target(&self, player: u8, from: FxVec2) -> Option<FxVec2> {
        self.state
            .players
            .iter()
            .enumerate()
            .filter(|(i, p)| !p.defeated && self.are_enemies(player, *i as u8))
            .map(|(_, p)| self.state.units.row(p.commander).map_or(p.start, |row| {
                // Head for the commander once it has been seen, else its start position.
                if self.detects(player, row) { self.state.units.pos[row] } else { p.start }
            }))
            .min_by_key(|t| (t.distance_sq(from), t.x, t.y))
    }
}
