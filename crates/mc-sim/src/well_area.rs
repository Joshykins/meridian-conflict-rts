//! The crush every gravity well deals, whoever holds it: the gravitic engine's
//! (`gravitic.rs`, The Fallen) and a gravitic lens's (`lens.rs`).
//!
//! Each tick a well deals `WellArea::per_tick` the same anywhere in it:
//! - **Shields first:** a live enemy dome reaching into the well takes it once, and what
//!   stands under a dome that still holds is spared. A hull field takes its unit's share.
//! - **Units:** every active enemy whose hull reaches into the well (`well_catches`),
//!   never a spacecraft. Aircraft as `WellArea::aircraft` says: torn out of the sky up to
//!   a tech, or spared outright.
//! - **The ground:** the wrecks and city blocks in it wear down as a blast wears them.
//!
//! Friends are never touched. What each kind of well does besides (the engine's trees and
//! scar, a lens's stain) is its own.

use mc_core::{Fx, FxVec2, FxVec3};
use mc_data::{cat, MoveLayer};

use crate::shields::in_dome;
use crate::spatial::kind;
use crate::tables::UnitId;
use crate::{SimEvent, World};

/// Metres past the well a hull's middle may stand and still be found: more than the
/// widest hull's radius, so the search never misses one the well's own test takes.
const HULL_MARGIN: i32 = 60;
/// A downed aircraft is thrown at the ground this fast, metres a tick (60 m/s).
const SLAM: i32 = 6;

/// What a well does to aircraft over it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Aircraft {
    /// Torn out of the sky, up to and including this tech; crushed as any unit above it.
    Downs(u8),
    /// Untouched: neither crushed nor slowed.
    Spared,
}

/// One tick of one well: where it is, how far it reaches, whose it is and how hard it
/// crushes.
pub(crate) struct WellArea {
    pub(crate) mark: FxVec2,
    /// The ground at the mark (the sea's surface over water).
    pub(crate) z: Fx,
    pub(crate) radius: Fx,
    pub(crate) owner: u8,
    pub(crate) source: UnitId,
    pub(crate) per_tick: Fx,
    pub(crate) aircraft: Aircraft,
}

impl World {
    /// Whether the unit in `row`, standing at `pos`, is in the well: an enemy whose hull
    /// reaches into it, not a spacecraft, and not an aircraft the well spares.
    pub(crate) fn well_catches(&self, area: &WellArea, row: usize, pos: FxVec2) -> bool {
        let bp = self.bp(row);
        let air = bp.motion.is_some_and(|m| m.layer == MoveLayer::Air);
        self.are_enemies(area.owner, self.state.units.owner[row])
            && !bp.has(cat::SPACE)
            && !(air && area.aircraft == Aircraft::Spared)
            && pos.distance(area.mark) - bp.radius <= area.radius
    }

    /// One tick's crush over `area`: shields first, then everything of the enemy's they do
    /// not cover, then the wrecks and city blocks.
    pub(crate) fn crush_area(&mut self, area: &WellArea) {
        let (mark, radius, per) = (area.mark, area.radius, area.per_tick);
        let floor = self.dome_floor();
        // Domes reaching into the well take the crush; what stands under one is spared
        // while it holds.
        let mut domes = Vec::new();
        for &row in &self.scratch.shielded {
            if !self.state.units.slots.is_alive(row)
                || !self.shield_blocking(row)
                || self.bp(row).shield.is_some_and(|s| s.is_hull())
                || !self.are_enemies(area.owner, self.state.units.owner[row])
            {
                continue;
            }
            let r = self.dome_radius(row);
            let d = self.state.units.pos[row].distance(mark) - r;
            if d <= radius {
                let center = self.state.units.pos[row].extend(self.state.units.z[row]);
                domes.push((row, center, r));
            }
        }
        for &(row, ..) in &domes {
            self.damage_shield(row, per);
        }
        // A dome the crush just broke covers nothing.
        domes.retain(|&(row, ..)| self.shield_blocking(row));
        let covered = |p: FxVec3| domes.iter().any(|&(_, c, r)| in_dome(p, c, r, floor));

        let mut struck_units = Vec::new();
        self.index
            .query(mark, radius + Fx::from_int(HULL_MARGIN), kind::UNIT, |e| {
                let r = e.row as usize;
                if self.unit_entry_is_current(e)
                    && self.state.units.is_active(r)
                    && self.well_catches(area, r, self.state.units.pos[r])
                {
                    struck_units.push(r);
                }
                true
            });
        for r in struck_units {
            let bp = self.bp(r);
            let body = self.state.units.pos[r].extend(self.state.units.z[r] + bp.height / 2);
            if covered(body) {
                continue;
            }
            let hull = bp.shield.is_some_and(|s| s.is_hull()) && self.shield_blocking(r);
            let air = bp.motion.is_some_and(|m| m.layer == MoveLayer::Air);
            let ground = self
                .terrain
                .height_at(self.state.units.pos[r])
                .max(self.terrain.water_level());
            let downs = matches!(area.aircraft, Aircraft::Downs(tech) if bp.tech <= tech);
            if air && !hull && downs && self.state.units.z[r] > ground + Fx::ONE {
                self.down_aircraft(r, area.source, area.owner);
                continue;
            }
            if hull {
                self.damage_shield(r, per);
            } else {
                self.damage_unit(r, per, area.owner, area.source);
            }
        }

        let point = mark.extend(area.z + Fx::from_int(8));
        self.wear_wrecks(point, radius, |_| per);
        self.blast_structures(point, radius, crate::city::Blow::BLAST, |_| per);
    }

    /// Tears the aircraft in `row` out of the sky: it dies, thrown hard at the ground.
    fn down_aircraft(&mut self, row: usize, source: UnitId, owner: u8) {
        let units = &mut self.state.units;
        // A dead aircraft falls with the way it last made (`despawn_unit`): this tick's
        // drop is its throw at the ground.
        units.prev_z[row] = units.z[row] + Fx::from_int(SLAM);
        let pos = units.pos[row].extend(units.z[row]);
        let id = units.id(row);
        let health = units.health[row];
        self.events.push(SimEvent::AircraftDowned {
            unit: id,
            pos,
            by: source,
        });
        self.damage_unit(row, health, owner, source);
    }
}
