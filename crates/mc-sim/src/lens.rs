//! Gravitic lenses (`Weapon::lens`, the Regency's tech 3 lens): a gun that charges, sends
//! a front of gravity out to its target, and crushes all round the target where it
//! strikes.
//!
//! - **The shot** (`LensLaunched`): the gun charges and fires as any gun does (planted,
//!   on target, in reach, with a line of fire), but no round flies: a front leaves the
//!   muzzle and runs at the weapon's `speed` to the target, following it as it moves
//!   (`State::lenses`, one a gun). The gun does not fire again while it is out; its reload
//!   runs from the shot, as any gun's does.
//! - **The strike** (`LensStruck`): the front reaches the target and a well opens on it
//!   and collapses at once, one crush of the weapon's `damage` over `Lens::radius` round
//!   the target (`well_area.rs`: shields first, never aircraft or spacecraft), and a stain
//!   pressed into the ground. Then the front is gone.
//! - **A lost front:** once let go, the front runs on whatever becomes of its gun's
//!   orders: packing, a new target or a stun do not call it back, as they would not a
//!   round in flight. If its target or its lens is gone before it strikes (dead, or out
//!   of play), it is dropped where it is, striking nothing.
//! - **Shields:** a live enemy dome reaching into the well takes the crush as a unit
//!   does, and what stands under it is spared if the dome holds. A hull field takes its
//!   own unit's share.
//!
//! Friends are never touched.

use mc_core::{FxVec3, StateHasher, TICKS_PER_SECOND};
use serde::{Deserialize, Serialize};

use crate::tables::UnitId;
use crate::well_area::{Aircraft, WellArea};
use crate::{SimEvent, World};

const DT: i32 = TICKS_PER_SECOND as i32;

/// One gun's front, on its way to its target.
#[derive(Clone, Serialize, Deserialize)]
pub struct Tether {
    pub source: UnitId,
    pub weapon: u8,
    pub owner: u8,
    pub target: UnitId,
    /// Where the front is.
    pub front: FxVec3,
    /// Ticks since it was let go.
    pub age: u32,
}

pub(crate) fn hash_tethers(tethers: &[Tether], h: &mut StateHasher) {
    h.write_u64(tethers.len() as u64);
    for t in tethers {
        h.write_u64(t.source.0 as u64 | (t.weapon as u64) << 32 | (t.owner as u64) << 40);
        h.write_u64(t.target.0 as u64 | (t.age as u64) << 32);
        for v in [t.front.x, t.front.y, t.front.z] {
            h.write_i64(v.0);
        }
    }
}

impl World {
    /// Whether weapon `w` of `row` has a front out: it does not fire while it does.
    pub(crate) fn lens_out(&self, row: usize, w: usize) -> bool {
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
            age: 0,
        });
        self.events.push(SimEvent::LensLaunched {
            unit: source,
            weapon: w as u8,
            target,
            from: muzzle,
        });
    }

    /// This tick's fronts: each runs on toward its target, and strikes on reaching it; a
    /// front whose lens or target is gone is dropped.
    pub(crate) fn run_lenses(&mut self) {
        if self.state.lenses.is_empty() {
            return;
        }
        let fronts = std::mem::take(&mut self.state.lenses);
        let mut kept = Vec::with_capacity(fronts.len());
        for mut t in fronts {
            if self.run_front(&mut t) {
                t.age = t.age.saturating_add(1);
                kept.push(t);
            }
        }
        kept.append(&mut self.state.lenses);
        self.state.lenses = kept;
    }

    /// The front runs on toward the target, wherever it has gone; reaching it, it strikes.
    /// False once it is done with: struck, or dropped with its lens or target gone.
    fn run_front(&mut self, t: &mut Tether) -> bool {
        let units = &self.state.units;
        let (Some(row), Some(target)) = (units.row(t.source), units.row(t.target)) else {
            return false;
        };
        if !units.is_active(row) || !units.is_active(target) {
            return false;
        }
        let Some(weapon) = self.bp(row).weapons.get(t.weapon as usize) else {
            return false;
        };
        let Some(lens) = weapon.lens else {
            return false;
        };
        let step = weapon.projectile_speed / DT;
        let at = units.pos[target].extend(units.z[target] + self.bp(target).height / 2);
        let to = at - t.front;
        let left = to.length();
        if left > step {
            t.front += to * (step / left);
            return true;
        }
        // The well opens on the target and collapses at once: one crush round it.
        let mark = units.pos[target];
        let area = WellArea {
            mark,
            z: self.terrain.height_at(mark).max(self.terrain.water_level()),
            radius: lens.radius,
            owner: t.owner,
            source: t.source,
            per_tick: weapon.damage,
            aircraft: Aircraft::Spared,
        };
        self.events.push(SimEvent::LensStruck {
            unit: t.source,
            weapon: t.weapon,
            target: t.target,
            pos: at,
            radius: lens.radius,
        });
        self.crush_area(&area);
        self.add_stain(area.mark, area.radius, 60);
        false
    }
}
