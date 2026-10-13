//! Gravitic lenses (`Weapon::lens`, the Regency's tech 3 lens): a gun that charges, sends
//! a front of gravity out to its target, and holds a gravity well locked on it where the
//! front strikes.
//!
//! - **The shot** (`LensLaunched`): the gun charges and fires as any gun does (planted,
//!   on target, in reach, with a line of fire), but no round flies: a front leaves the
//!   muzzle and runs at the weapon's `speed` to the target, following it as it moves
//!   (`State::lenses`, one a gun). The gun does not fire again while it is out.
//! - **The strike** (`LensStruck`): the front reaches the target and the well opens on it.
//! - **It holds** while the gun stands, is not stunned or paused, stays planted, still
//!   wants that target (`weapon_target`), and the target lives within the gun's range.
//!   No line of fire is asked for once it is out. Any of those ends it (`LensReleased`),
//!   and the gun is free to charge for what it wants next.
//! - **The well** sits on the target, `Lens::radius` round it, wherever it goes: each
//!   tick it crushes as every gravity well does (`well_area.rs`: shields first, never
//!   aircraft or spacecraft), the weapon's `damage` a second, and what is in it keeps
//!   `Lens::slow` of its speed (`lens_slow`), the same anywhere in it and however long it
//!   has been in it.
//! - **Shields:** a live enemy dome reaching into the well takes the crush as a unit
//!   does, and what stands under it is spared while it holds. A hull field takes its
//!   own unit's share.
//!
//! Friends are never touched.

use mc_core::{Fx, FxVec2, FxVec3, StateHasher, TICKS_PER_SECOND};
use mc_data::Lens;
use serde::{Deserialize, Serialize};

use crate::tables::UnitId;
use crate::well_area::{Aircraft, Sides, WellArea};
use crate::{SimEvent, World};

const DT: i32 = TICKS_PER_SECOND as i32;

/// One gun's front, or the well it holds on its target.
#[derive(Clone, Serialize, Deserialize)]
pub struct Tether {
    pub source: UnitId,
    pub weapon: u8,
    pub owner: u8,
    pub target: UnitId,
    /// Where the front is while it runs; it struck once `struck` is set.
    pub front: FxVec3,
    pub struck: bool,
    /// Ticks since it was let go.
    pub age: u32,
}

pub(crate) fn hash_tethers(tethers: &[Tether], h: &mut StateHasher) {
    h.write_u64(tethers.len() as u64);
    for t in tethers {
        h.write_u64(
            t.source.0 as u64
                | (t.weapon as u64) << 32
                | (t.owner as u64) << 40
                | (t.struck as u64) << 48,
        );
        h.write_u64(t.target.0 as u64 | (t.age as u64) << 32);
        for v in [t.front.x, t.front.y, t.front.z] {
            h.write_i64(v.0);
        }
    }
}

impl World {
    /// The lens on weapon `w` of `row`, if it is one.
    fn lens(&self, row: usize, w: u8) -> Option<Lens> {
        self.bp(row).weapons.get(w as usize).and_then(|x| x.lens)
    }

    /// Whether weapon `w` of `row` has a front out or a well held: it does not fire while
    /// it does.
    pub(crate) fn lens_holding(&self, row: usize, w: usize) -> bool {
        let id = self.state.units.id(row);
        self.state
            .lenses
            .iter()
            .any(|t| t.source == id && t.weapon as usize == w)
    }

    /// Weapon `w` of `row` fired its lens from `muzzle` at `target`: the front is let go.
    pub(crate) fn launch_lens(&mut self, row: usize, w: usize, muzzle: FxVec3, target: usize) {
        let units = &self.state.units;
        let (source, target) = (units.id(row), units.id(target));
        let owner = units.owner[row];
        self.state
            .lenses
            .retain(|t| !(t.source == source && t.weapon as usize == w));
        self.state.lenses.push(Tether {
            source,
            weapon: w as u8,
            owner,
            target,
            front: muzzle,
            struck: false,
            age: 0,
        });
        self.events.push(SimEvent::LensLaunched {
            unit: source,
            weapon: w as u8,
            target,
            from: muzzle,
        });
    }

    /// Whether the link from `t` still holds this tick.
    fn tether_holds(&self, t: &Tether) -> bool {
        let units = &self.state.units;
        let (Some(row), Some(target)) = (units.row(t.source), units.row(t.target)) else {
            return false;
        };
        let bp = self.bp(row);
        let Some(weapon) = bp.weapons.get(t.weapon as usize) else {
            return false;
        };
        let planted = bp
            .motion
            .is_none_or(|m| m.deploy_ticks == 0 || units.deploy[row] >= m.deploy_ticks);
        let reach = weapon.range_max + self.bp(target).radius;
        units.is_active(row)
            && units.is_active(target)
            && planted
            && !self.weapons_silent(row)
            && !self.powered_down(row)
            && units.weapon_target[row].get(t.weapon as usize) == Some(&t.target)
            && units.pos[row].distance(units.pos[target]) <= reach
    }

    /// The well `t` holds this tick, once its front has struck and while its target
    /// stands (`well_area.rs`): on the target, sparing aircraft.
    fn lens_area(&self, t: &Tether) -> Option<WellArea> {
        if !t.struck {
            return None;
        }
        let units = &self.state.units;
        let (row, target) = (units.row(t.source)?, units.row(t.target)?);
        let weapon = self.bp(row).weapons.get(t.weapon as usize)?;
        let lens = weapon.lens?;
        let mark = units.pos[target];
        Some(WellArea {
            mark,
            z: self.terrain.height_at(mark).max(self.terrain.water_level()),
            radius: lens.radius,
            owner: t.owner,
            source: t.source,
            per_tick: weapon.damage / DT,
            aircraft: Aircraft::Spared,
            edge: Fx::ONE,
            sides: Sides::Enemies,
        })
    }

    /// What is left of `speed` for the unit in `row` at `pos`: the slowest share of every
    /// enemy well it is in.
    pub(crate) fn lens_slow(&self, row: usize, pos: FxVec2, speed: Fx) -> Fx {
        let mut kept = Fx::ONE;
        for t in &self.state.lenses {
            let Some(area) = self.lens_area(t) else {
                continue;
            };
            if self.well_catches(&area, row, pos) {
                let slow = self
                    .state
                    .units
                    .row(t.source)
                    .and_then(|r| self.lens(r, t.weapon));
                kept = kept.min(slow.map_or(Fx::ONE, |l| l.slow));
            }
        }
        speed * kept
    }

    /// This tick's lenses: fronts running, wells crushing, the lost let go.
    pub(crate) fn run_lenses(&mut self) {
        if self.state.lenses.is_empty() {
            return;
        }
        let tethers = std::mem::take(&mut self.state.lenses);
        let mut kept = Vec::with_capacity(tethers.len());
        for mut t in tethers {
            if !self.tether_holds(&t) {
                let at = self
                    .state
                    .units
                    .row(t.target)
                    .map(|r| self.state.units.pos[r].extend(self.state.units.z[r]));
                self.events.push(SimEvent::LensReleased {
                    unit: t.source,
                    weapon: t.weapon,
                    pos: at,
                });
                continue;
            }
            if t.struck {
                self.crush_lens(&t);
            } else {
                self.run_front(&mut t);
            }
            t.age = t.age.saturating_add(1);
            kept.push(t);
        }
        kept.append(&mut self.state.lenses);
        self.state.lenses = kept;
    }

    /// The front runs on toward the target, wherever it has gone; reaching it, it strikes
    /// and the well opens.
    fn run_front(&mut self, t: &mut Tether) {
        let (Some(row), Some(target)) = (
            self.state.units.row(t.source),
            self.state.units.row(t.target),
        ) else {
            return;
        };
        let step = self.bp(row).weapons[t.weapon as usize].projectile_speed / DT;
        let units = &self.state.units;
        let at = units.pos[target].extend(units.z[target] + self.bp(target).height / 2);
        let to = at - t.front;
        let left = to.length();
        if left <= step {
            t.front = at;
            t.struck = true;
            let radius = self.lens(row, t.weapon).map_or(Fx::ZERO, |l| l.radius);
            self.events.push(SimEvent::LensStruck {
                unit: t.source,
                weapon: t.weapon,
                target: t.target,
                pos: at,
                radius,
            });
            self.crush_lens(t);
        } else {
            t.front += to * (step / left);
        }
    }

    /// One tick's crush over the well on `t`'s target (`well_area.rs`), and the ground
    /// pressed under it.
    fn crush_lens(&mut self, t: &Tether) {
        let Some(area) = self.lens_area(t) else {
            return;
        };
        self.crush_area(&area);
        if t.age.is_multiple_of(20) {
            self.add_stain(area.mark, area.radius, 60);
        }
    }
}
