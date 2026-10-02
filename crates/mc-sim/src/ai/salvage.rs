//! Salvage: the AI puts reclaim towers by the wreck fields near home, fields a salvage
//! unit or two early (a Magpie, a Gleaner), and sends idle ones to the richest field
//! that is safe to work. Reclaim is cheap and quick to pay back, so a field is worth a
//! tower long before it is worth an army.

use super::{Census, Intel, Job, Place, Planned};
use crate::World;
use mc_core::{Fx, FxVec2};
use mc_data::{cat, BlueprintId};

/// How far from home the AI looks for wrecks to salvage.
const SALVAGE_RADIUS: i32 = 2600;
/// Wrecks within this of one another are one field.
const FIELD_RADIUS: i32 = 450;
/// A field worth putting a tower by, in mass.
const TOWER_FIELD: i32 = 250;
/// Wreck mass in reach of home that is worth one more salvage unit.
const MASS_PER_SALVAGER: i32 = 1500;
/// Most salvage units and towers the AI keeps.
const MOST_SALVAGERS: usize = 3;
const MOST_TOWERS: usize = 4;
/// Deliberate cap: the wrecks nearest home that are sorted into fields each think.
/// Farther ones wait until the near ones are cleared.
const MOST_WRECKS: usize = 256;
/// Deliberate cap: fields kept per think.
const MOST_FIELDS: usize = 6;

/// A field of wrecks: where its richest part is, and how much lies within `FIELD_RADIUS` of it.
#[derive(Clone, Copy, Debug)]
pub(super) struct Field {
    pub(super) at: FxVec2,
    pub(super) mass: Fx,
}

impl World {
    /// The wreck fields near `start` that are safe to work, richest first.
    pub(super) fn wreck_fields(&self, start: FxVec2, intel: &Intel) -> Vec<Field> {
        let wrecks = &self.state.wrecks;
        let mut near: Vec<(Fx, usize)> = wrecks
            .slots
            .iter()
            .filter(|&w| {
                wrecks.mass[w] > Fx::ZERO
                    && wrecks.pos[w].distance(start) <= Fx::from_int(SALVAGE_RADIUS)
            })
            .map(|w| (wrecks.pos[w].distance_sq(start), w))
            .collect();
        near.sort_unstable();
        // Nearest first, so the danger map is asked only until enough safe ones are found.
        let near: Vec<(Fx, usize)> = near
            .into_iter()
            .filter(|&(_, w)| !intel.danger.hot(wrecks.pos[w]))
            .take(MOST_WRECKS)
            .collect();
        let reach = Fx::from_int(FIELD_RADIUS);
        let mut around: Vec<(Fx, usize)> = near
            .iter()
            .map(|&(_, w)| {
                let at = wrecks.pos[w];
                let mass = near
                    .iter()
                    .filter(|&&(_, o)| wrecks.pos[o].distance(at) <= reach)
                    .fold(Fx::ZERO, |m, &(_, o)| m + wrecks.mass[o]);
                (mass, w)
            })
            .collect();
        // Richest first; the wreck row settles ties.
        around.sort_by_key(|&(mass, w)| (std::cmp::Reverse(mass), w));
        let mut fields: Vec<Field> = Vec::new();
        for (mass, w) in around {
            let at = wrecks.pos[w];
            if fields.iter().all(|f| f.at.distance(at) > reach * 2) {
                fields.push(Field { at, mass });
                if fields.len() == MOST_FIELDS {
                    break;
                }
            }
        }
        fields
    }

    /// The reclaim tower a builder can put up: the first in its list.
    pub(super) fn pick_reclaimer(&self, builder_row: usize) -> Option<BlueprintId> {
        let builder = self.bp(builder_row).builder.as_ref()?;
        builder.builds.iter().copied().find(|b| {
            let bp = self.blueprints.unit(*b);
            bp.reclaimer.is_some() && bp.is_structure()
        })
    }

    /// A reclaim tower by the richest field no tower reaches yet.
    pub(super) fn salvage_job(
        &self,
        row: usize,
        planned: &Planned,
        allow: &dyn Fn(FxVec2) -> bool,
    ) -> Option<Job> {
        if planned.towers.len() >= MOST_TOWERS {
            return None;
        }
        let blueprint = self.pick_reclaimer(row)?;
        let reach = self.blueprints.unit(blueprint).reclaimer?.range;
        let field = planned.salvage.iter().find(|f| {
            f.mass >= Fx::from_int(TOWER_FIELD)
                && allow(f.at)
                && planned
                    .towers
                    .iter()
                    .all(|&(at, r)| at.distance(f.at) > r * Fx::ratio(3, 4))
        })?;
        // Back from the wrecks toward home a little, so the tower's lot is clear of them.
        let near = super::offset_toward(
            field.at,
            self.state.players[self.state.units.owner[row] as usize].start,
            reach / 4,
        );
        Some(Job {
            blueprint,
            near,
            heading: super::AI_BUILD_HEADING,
            min_r: Fx::from_int(24),
            keep_off_deposits: true,
            place: Place::Around,
        })
    }

    /// What an idle factory makes for salvage, if the side wants another salvage unit:
    /// its cheapest one (a Magpie from the air, a Gleaner from the land).
    pub(super) fn salvage_product(
        &self,
        factory: usize,
        census: &Census,
        planned_salvagers: usize,
        salvage: &[Field],
    ) -> Option<BlueprintId> {
        let lying = salvage.iter().fold(Fx::ZERO, |m, f| m + f.mass);
        let mut want = (1 + (lying / Fx::from_int(MASS_PER_SALVAGER)).floor_int() as usize)
            .min(MOST_SALVAGERS);
        // One early and more as its income grows: three in the
        // first hundred seconds held back its engineers and its power.
        let owner = self.state.units.owner[factory];
        let income = self.state.players[owner as usize].mass_income;
        want = want.min(1 + (income / Fx::from_int(15)).floor_int().max(0) as usize);
        if salvage.is_empty() || census.salvagers + planned_salvagers >= want {
            return None;
        }
        let builder = self.bp(factory).builder.as_ref()?;
        builder
            .builds
            .iter()
            .copied()
            .filter(|b| {
                let bp = self.blueprints.unit(*b);
                bp.is_salvager() && !bp.has(cat::NAVAL)
            })
            .min_by_key(|b| (self.blueprints.unit(*b).cost_mass, b.0))
    }

    /// Idle salvage units with nothing in their reach go to the richest safe field.
    pub(super) fn direct_salvagers(
        &self,
        census: &Census,
        salvage: &[Field],
        out: &mut Vec<crate::Command>,
    ) {
        let wrecks = &self.state.wrecks;
        for &row in &census.salvagers_idle {
            let bp = self.bp(row);
            let reach = bp
                .reclaimer
                .map_or(bp.drone_radius, |r| r.range)
                .max(bp.drone_radius);
            let pos = self.state.units.pos[row];
            let busy = wrecks
                .slots
                .iter()
                .any(|w| wrecks.mass[w] > Fx::ZERO && wrecks.pos[w].distance(pos) <= reach);
            if busy {
                continue;
            }
            // The richest field, the nearer the better among those alike.
            let Some(field) = salvage
                .iter()
                .enumerate()
                .max_by_key(|(i, f)| (f.mass - f.at.distance(pos) / 4, std::cmp::Reverse(*i)))
                .map(|(_, f)| f)
            else {
                continue;
            };
            out.push(crate::Command::Move {
                units: vec![self.state.units.id(row)],
                target: field.at,
                queue: false,
            });
        }
    }
}

#[cfg(test)]
#[path = "salvage_tests.rs"]
mod tests;
