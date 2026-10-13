//! Lances (`UnitBlueprint::lance`, `mc_data::lance`): a structure's great beam. Each
//! tick a Lance that has cooled takes the nearest enemy it may strike in range (or
//! keeps the one it has), charges on it, and once charged strikes where it stands:
//! the Lance's damage over everything of the enemy's within its splash, shields
//! first (`crush_area`, the one tick of crush a well deals). Then it cools.
//!
//! The figures are the unit's at its level (`World::lance_spec`), so a grown
//! Shaper's Lance charges longer, strikes harder and wider, reaches further and
//! cools sooner. Losing the target while charging keeps the charge for the next.

use std::collections::BTreeMap;

use mc_core::{Fx, FxVec2, FxVec3, StateHasher};
use serde::{Deserialize, Serialize};

use crate::mirror::SimEvent;
use crate::spatial::kind;
use crate::tables::UnitId;
use crate::well_area::{Aircraft, Sides, WellArea};
use crate::{Handle, World};

/// Metres past its range a hull's middle may stand and still be found: more than the
/// widest hull's radius, so the search never misses one the range test takes.
const HULL_MARGIN: i32 = 160;

/// One Lance at work (`State::lances`).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct LanceCharge {
    /// What it is charging on; `Handle::NONE` with nothing in reach.
    pub target: UnitId,
    /// Ticks charged so far.
    pub charged: u32,
    /// Ticks left to cool after a strike.
    pub cooling: u32,
    /// Where it last aimed: the target's middle.
    pub aim: FxVec3,
}

/// Every Lance's charge, by unit.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Lances {
    pub by_unit: BTreeMap<UnitId, LanceCharge>,
}

impl Lances {
    pub(crate) fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.by_unit.len() as u64);
        for (id, c) in &self.by_unit {
            h.write_u64(id.0 as u64 | (c.target.0 as u64) << 32);
            h.write_u64(c.charged as u64 | (c.cooling as u64) << 32);
            for v in [c.aim.x, c.aim.y, c.aim.z] {
                h.write_i64(v.0);
            }
        }
    }
}

impl World {
    /// Every Lance charges, strikes or cools a tick.
    pub(crate) fn run_lances(&mut self) {
        let units = &self.state.units;
        self.state
            .lances
            .by_unit
            .retain(|id, _| units.row(*id).is_some());
        let rows: Vec<usize> = self
            .state
            .units
            .slots
            .iter()
            .filter(|&r| self.bp(r).lance.is_some() && self.state.units.is_active(r))
            .collect();
        for row in rows {
            self.step_lance(row);
        }
    }

    fn step_lance(&mut self, row: usize) {
        let Some(spec) = self.lance_spec(row) else {
            return;
        };
        let id = self.state.units.id(row);
        let mut c = self
            .state
            .lances
            .by_unit
            .get(&id)
            .copied()
            .unwrap_or(LanceCharge {
                target: Handle::NONE,
                charged: 0,
                cooling: 0,
                aim: FxVec3::ZERO,
            });
        if c.cooling > 0 {
            c.cooling -= 1;
        } else if self.weapons_silent(row) || self.powered_down(row) {
            c.charged = c.charged.saturating_sub(1);
        } else {
            let keep = self
                .state
                .units
                .row(c.target)
                .filter(|&t| self.lance_takes(row, t, &spec));
            match keep.or_else(|| self.lance_pick(row, &spec)) {
                Some(t) => {
                    let units = &self.state.units;
                    c.target = units.id(t);
                    c.aim = units.pos[t].extend(units.z[t] + self.bp(t).height / 2);
                    c.charged += 1;
                    if c.charged >= spec.charge_ticks {
                        self.lance_strike(row, t, &spec);
                        c.charged = 0;
                        c.cooling = spec.reload_ticks;
                    }
                }
                None => {
                    c.target = Handle::NONE;
                    // With nothing to strike the charge bleeds away.
                    c.charged = c.charged.saturating_sub(1);
                }
            }
        }
        self.state.lances.by_unit.insert(id, c);
    }

    /// Whether the Lance in `row` may strike `target`: an enemy it can hit and sees,
    /// its hull within range.
    fn lance_takes(&self, row: usize, target: usize, spec: &mc_data::lance::Lance) -> bool {
        let units = &self.state.units;
        let owner = units.owner[row];
        units.slots.is_alive(target)
            && units.is_active(target)
            && self.are_enemies(owner, units.owner[target])
            && self.hittable(target, spec.target_mask)
            && units.pos[row].distance(units.pos[target]) - self.hull_radius(target) <= spec.range
            && self.detects(owner, target)
    }

    /// The nearest enemy the Lance in `row` may strike (ties: the lowest id).
    fn lance_pick(&self, row: usize, spec: &mc_data::lance::Lance) -> Option<usize> {
        let at = self.state.units.pos[row];
        let mut best: Option<(Fx, UnitId, usize)> = None;
        let reach = spec.range + Fx::from_int(HULL_MARGIN);
        self.index.query(at, reach, kind::UNIT, |e| {
            let t = e.row as usize;
            if self.unit_entry_is_current(e) && self.lance_takes(row, t, spec) {
                let key = (
                    at.distance_sq(self.state.units.pos[t]),
                    self.state.units.id(t),
                    t,
                );
                if best.is_none_or(|b| (key.0, key.1) < (b.0, b.1)) {
                    best = Some(key);
                }
            }
            true
        });
        best.map(|b| b.2)
    }

    /// The Lance in `row` strikes `target`: its damage over its splash round the
    /// target's foot, shields first.
    fn lance_strike(&mut self, row: usize, target: usize, spec: &mc_data::lance::Lance) {
        let units = &self.state.units;
        let mark: FxVec2 = units.pos[target];
        let z = self.terrain.height_at(mark).max(self.terrain.water_level());
        let owner = units.owner[row];
        let source = units.id(row);
        let to = mark.extend(units.z[target] + self.bp(target).height / 2);
        self.crush_area(&WellArea {
            mark,
            z,
            radius: spec.splash,
            owner,
            source,
            per_tick: spec.damage,
            aircraft: Aircraft::Crushed(Fx::ONE),
            edge: Fx::ONE,
            sides: Sides::Enemies,
        });
        self.events.push(SimEvent::LanceStrike {
            unit: source,
            owner,
            to,
            splash: spec.splash,
            damage: spec.damage,
        });
    }
}
