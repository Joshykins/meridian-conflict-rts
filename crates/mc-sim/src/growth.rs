//! Units that grow by level (`UnitBlueprint::growth`, `mc_data::growth`): survival's
//! Shapers. `State::levels` holds each grown unit's level; everything a level
//! changes (hit points, the shield, the Lance, the interceptors) is worked out
//! from the blueprint's figures and its growth block when it is asked for, so
//! the level is the only state. A unit with no growth block, or none set, is
//! level 1: its blueprint as written.
//!
//! The systems ask through the accessors here (`unit_max_health`,
//! `shield_spec`, `lance_spec`, `launcher_spec`), never the blueprint's figures
//! straight, so a grown unit is grown everywhere. `grown_stats` gathers them
//! for the interface.

use std::borrow::Cow;
use std::collections::BTreeMap;

use mc_core::{Fx, StateHasher};
use mc_data::lance::Lance;
use mc_data::strategic::{NuclearBlast, Strategic, StrategicKind};
use mc_data::Shield;
use serde::{Deserialize, Serialize};

use crate::tables::UnitId;
use crate::World;

/// Grown units' levels (`State::levels`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Levels {
    by_unit: BTreeMap<UnitId, u16>,
}

impl Levels {
    pub(crate) fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.by_unit.len() as u64);
        for (id, level) in &self.by_unit {
            h.write_u64(id.0 as u64 | (*level as u64) << 32);
        }
    }

    /// The level of `unit`: 1 unless it was set.
    pub fn of(&self, unit: UnitId) -> u16 {
        self.by_unit.get(&unit).copied().unwrap_or(1)
    }
}

/// Everything a unit's level makes of it, for the interface: each figure as it
/// stands at that level (seconds and metres as `f32`s are left to the reader).
#[derive(Clone, Copy, Debug)]
pub struct GrownStats {
    pub level: u16,
    pub health: Fx,
    pub shield: Option<Shield>,
    pub lance: Option<Lance>,
    /// Interceptors held at most, the coverage, and one round's build time (`Strategic`).
    pub interceptors: Option<(u8, Fx, Fx)>,
    /// A silo's: warheads held at most, one round's build time, and the blast.
    pub warheads: Option<(u8, Fx, NuclearBlast)>,
}

impl World {
    /// The level of the unit in `row`: 1 for a unit that does not grow.
    pub fn unit_level(&self, row: usize) -> u16 {
        if self.bp(row).growth.is_none() {
            return 1;
        }
        self.state.levels.of(self.state.units.id(row))
    }

    /// Sets the level of the grown unit in `row` (nothing for one that does not
    /// grow). It keeps the damage it has taken, gains the hit points and shield
    /// the level adds, and its dome swells out to the new radius.
    pub(crate) fn set_unit_level(&mut self, row: usize, level: u16) {
        if self.bp(row).growth.is_none() {
            return;
        }
        let level = level.max(1);
        let id = self.state.units.id(row);
        let before = (self.unit_max_health(row), self.shield_spec(row));
        let radius = self.dome_radius(row);
        if level == 1 {
            self.state.levels.by_unit.remove(&id);
        } else {
            self.state.levels.by_unit.insert(id, level);
        }
        let (old_health, old_shield) = before;
        let full = self.unit_max_health(row);
        let health = &mut self.state.units.health[row];
        *health = (*health + full - old_health).clamp(Fx::ZERO, full);
        if let (Some(old), Some(new)) = (old_shield, self.shield_spec(row)) {
            let units = &mut self.state.units;
            if units.shield_recharge[row] == 0 {
                units.shield_hp[row] =
                    (units.shield_hp[row] + new.health - old.health).clamp(Fx::ZERO, new.health);
            }
            if new.radius != old.radius {
                units.shield_from[row] = radius;
                units.shield_grow[row] = crate::shields::SHIELD_GROW_TICKS;
            }
        }
    }

    /// Hit points at the unit's level, before rank.
    pub(crate) fn grown_health(&self, row: usize) -> Fx {
        let bp = self.bp(row);
        match &bp.growth {
            Some(g) => g.health.at(bp.health, self.unit_level(row)),
            None => bp.health,
        }
    }

    /// The unit's shield at its level.
    pub(crate) fn shield_spec(&self, row: usize) -> Option<Shield> {
        let bp = self.bp(row);
        let base = bp.shield?;
        let Some(g) = &bp.growth else {
            return Some(base);
        };
        let level = self.unit_level(row);
        Some(Shield {
            radius: g.shield_radius.at(base.radius, level),
            health: g.shield_health.at(base.health, level),
            regen: g.shield_regen.at(base.regen, level),
            recharge: g.shield_regen.at(base.recharge, level),
            ..base
        })
    }

    /// The unit's Lance at its level.
    pub(crate) fn lance_spec(&self, row: usize) -> Option<Lance> {
        let bp = self.bp(row);
        let base = bp.lance?;
        let Some(g) = &bp.growth else {
            return Some(base);
        };
        let level = self.unit_level(row);
        let ticks = |grow: mc_data::growth::Grow, t: u32| {
            grow.at(Fx::from_int(t as i32), level).round_int().max(1) as u32
        };
        Some(Lance {
            charge_ticks: ticks(g.lance_charge, base.charge_ticks),
            reload_ticks: ticks(g.lance_reload, base.reload_ticks),
            damage: g.lance_damage.at(base.damage, level),
            splash: g.lance_splash.at(base.splash, level),
            range: g.lance_range.at(base.range, level),
            ..base
        })
    }

    /// The unit's strategic launcher at its level: an interceptor array's stock,
    /// coverage and round time grow; a silo's stock, round time and blast.
    pub(crate) fn launcher_spec(&self, row: usize) -> Option<Cow<'_, Strategic>> {
        let bp = self.bp(row);
        let base = bp.strategic.as_ref()?;
        let Some(g) = &bp.growth else {
            return Some(Cow::Borrowed(base));
        };
        let level = self.unit_level(row);
        if level == 1 {
            return Some(Cow::Borrowed(base));
        }
        let (stock, round) = match base.kind {
            StrategicKind::Interceptor => (g.interceptor_stock, g.interceptor_round),
            StrategicKind::Nuke => (g.warhead_stock, g.warhead_round),
        };
        let stock = stock
            .at(Fx::from_int(base.stock as i32), level)
            .floor_int()
            .clamp(1, u8::MAX as i32) as u8;
        let blast = base.blast.map(|b| {
            let radius = g.blast_radius.at(b.radius, level).max(Fx::ONE);
            let damage = g.blast_damage.at(b.damage, level);
            NuclearBlast {
                radius,
                // The core and the edge keep their share of the radius and the damage.
                core: b.core * radius / b.radius.max(Fx::ONE),
                edge: b.edge * damage / b.damage.max(Fx::ONE),
                damage,
                ..b
            }
        });
        Some(Cow::Owned(Strategic {
            stock,
            coverage: g.interceptor_coverage.at(base.coverage, level),
            round_time: round.at(base.round_time, level).max(Fx::ONE),
            blast,
            ..base.clone()
        }))
    }

    /// Everything the unit in `row`'s level makes of it, for the interface.
    pub fn grown_stats(&self, row: usize) -> GrownStats {
        GrownStats {
            level: self.unit_level(row),
            health: self.unit_max_health(row),
            shield: self.shield_spec(row),
            lance: self.lance_spec(row),
            interceptors: self
                .launcher_spec(row)
                .filter(|s| s.kind == StrategicKind::Interceptor)
                .map(|s| (s.stock, s.coverage, s.round_time)),
            warheads: self
                .launcher_spec(row)
                .filter(|s| s.kind == StrategicKind::Nuke)
                .and_then(|s| Some((s.stock, s.round_time, s.blast?))),
        }
    }

    /// Forgets the levels of units that are gone.
    pub(crate) fn prune_levels(&mut self) {
        if self.state.levels.by_unit.is_empty() {
            return;
        }
        let units = &self.state.units;
        self.state
            .levels
            .by_unit
            .retain(|id, _| units.row(*id).is_some());
    }
}
