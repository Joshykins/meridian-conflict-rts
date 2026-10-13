//! The crush every gravity well deals, whoever holds it: the gravitic engine's
//! (`gravitic.rs`, The Fallen) and a gravitic lens's (`lens.rs`); a surge shell's wake
//! (`surge.rs`) burns the same way over the disc it has rolled over.
//!
//! Each tick a well deals `WellArea::per_tick` the same anywhere in it (the Edict's
//! molten wave, `edict.rs`, falls off to `WellArea::edge` of it at its rim):
//! - **Shields first:** a live enemy dome reaching into the well takes it once, and what
//!   stands under a dome that still holds is spared. A hull field takes its unit's share.
//! - **Units:** every active enemy whose hull reaches into the well (`well_catches`),
//!   never a spacecraft. Aircraft as `WellArea::aircraft` says: crushed with a share of
//!   it, or spared outright.
//! - **The ground:** the wrecks and city blocks in it wear down as a blast wears them.
//!
//! Friends are never touched (`Sides::Enemies`), but for the Edict's wave
//! (`Sides::Everyone`), which spares only the ship that fired it. What each kind of well
//! does besides (the engine's trees and scar, a lens's stain) is its own.

use mc_core::{Fx, FxVec2, FxVec3};
use mc_data::{cat, MoveLayer};

use crate::shields::in_dome;
use crate::spatial::kind;
use crate::tables::UnitId;
use crate::World;

/// Metres past the well a hull's middle may stand and still be found: more than the
/// widest hull's radius, so the search never misses one the well's own test takes.
const HULL_MARGIN: i32 = 60;

/// What a well does to aircraft over it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Aircraft {
    /// Crushed with this share of the well's crush (a hull field too), and slowed in full.
    Crushed(Fx),
    /// Untouched: neither crushed nor slowed.
    Spared,
}

/// Whom a well touches.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sides {
    /// Only the owner's enemies.
    Enemies,
    /// Every side's units and domes, the source's own hull excepted.
    Everyone,
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
    /// The share of `per_tick` dealt at `radius`, falling evenly from the mark: one for
    /// the same everywhere.
    pub(crate) edge: Fx,
    pub(crate) sides: Sides,
}

impl WellArea {
    /// What it deals this tick `reach` metres from the mark.
    fn per_at(&self, reach: Fx) -> Fx {
        if self.edge == Fx::ONE || self.radius <= Fx::ZERO {
            return self.per_tick;
        }
        let out = (reach / self.radius).clamp(Fx::ZERO, Fx::ONE);
        self.per_tick * (Fx::ONE - (Fx::ONE - self.edge) * out)
    }

    /// Whether the well touches what `owner` owns, standing as `unit`.
    fn touches(&self, world: &World, owner: u8, unit: UnitId) -> bool {
        match self.sides {
            Sides::Enemies => world.are_enemies(self.owner, owner),
            Sides::Everyone => unit != self.source,
        }
    }
}

impl World {
    /// Whether the unit in `row`, standing at `pos`, is in the well: an enemy whose hull
    /// reaches into it, not a spacecraft, and not an aircraft the well spares.
    pub(crate) fn well_catches(&self, area: &WellArea, row: usize, pos: FxVec2) -> bool {
        let bp = self.bp(row);
        let air = bp.motion.is_some_and(|m| m.layer == MoveLayer::Air);
        area.touches(self, self.state.units.owner[row], self.state.units.id(row))
            && !bp.has(cat::SPACE)
            && !(air && area.aircraft == Aircraft::Spared)
            && pos.distance(area.mark) - self.hull_radius(row) <= area.radius
    }

    /// One tick's crush over `area`: shields first, then everything of the enemy's they do
    /// not cover, then the wrecks and city blocks.
    pub(crate) fn crush_area(&mut self, area: &WellArea) {
        let (mark, radius) = (area.mark, area.radius);
        let floor = self.dome_floor();
        // Domes reaching into the well take the crush; what stands under one is spared
        // while it holds.
        let mut domes = Vec::new();
        for &row in &self.scratch.shielded {
            if !self.state.units.slots.is_alive(row)
                || !self.shield_blocking(row)
                || self.bp(row).shield.is_some_and(|s| s.is_hull())
                || !area.touches(self, self.state.units.owner[row], self.state.units.id(row))
            {
                continue;
            }
            let r = self.dome_radius(row);
            let d = self.state.units.pos[row].distance(mark) - r;
            if d <= radius {
                let center = self.state.units.pos[row].extend(self.state.units.z[row]);
                domes.push((row, center, r, d.max(Fx::ZERO)));
            }
        }
        for &(row, _, _, d) in &domes {
            self.damage_shield(row, area.per_at(d));
        }
        // A dome the crush just broke covers nothing.
        domes.retain(|&(row, ..)| self.shield_blocking(row));
        let covered = |p: FxVec3| domes.iter().any(|&(_, c, r, _)| in_dome(p, c, r, floor));

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
            let reach =
                (self.state.units.pos[r].distance(mark) - self.hull_radius(r)).max(Fx::ZERO);
            let per = area.per_at(reach);
            let per = match area.aircraft {
                Aircraft::Crushed(share) if air => per * share,
                _ => per,
            };
            if hull {
                self.damage_shield(r, per);
            } else {
                // A friend burnt by the Edict's wave is no one's kill, as under a warhead.
                let by = if self.are_enemies(area.owner, self.state.units.owner[r]) {
                    area.owner
                } else {
                    u8::MAX
                };
                self.damage_unit(r, per, by, area.source);
            }
        }

        let point = mark.extend(area.z + Fx::from_int(8));
        self.wear_wrecks(point, radius, |reach| area.per_at(reach));
        self.blast_structures(point, radius, crate::city::Blow::BLAST, |reach| {
            area.per_at(reach)
        });
    }
}
