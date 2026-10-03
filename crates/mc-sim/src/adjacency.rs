//! Adjacency: providers make the buildings they stand against cheaper to run.
//!
//! A provider is a finished building whose blueprint carries an `adjacency` saving
//! (`mc_data::Adjacency`): reactors save energy, material fabricators save materials.
//! Each finished building of the same owner whose lot shares an edge with the
//! provider's (a corner is not enough) is its neighbour, and a neighbour that uses
//! what the provider saves gets the saving:
//!
//! - energy: its standing upkeep (a fabricator, mine, shield or radar), and the energy
//!   a factory spends on what it builds;
//! - materials: the materials a factory spends on what it builds.
//!
//! Savings from every provider touching a building add up, to a cap per resource
//! (`MAX_SAVING`). A fabricator and a power plant of its tech that touch are bound
//! (`World::bound_partners`): when one is destroyed the other goes up with it.
//!
//! Nothing here is state. The links are worked out from the units table each economy
//! tick and kept for the mirror (the ground links and the interface), and the bound
//! partners of a dying unit are worked out when it dies.

use crate::tables::{flag, UnitId};
use crate::World;
use mc_core::{Fx, FxVec2};
use mc_data::UnitBlueprint;

/// Most of a building's use of each resource its neighbours can save: `[mass, energy]`.
pub const MAX_SAVING: [Fx; 2] = [Fx::ratio(1, 3), Fx::ratio(1, 2)];

/// What a provider saves its neighbour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    Mass = 0,
    Energy = 1,
}

/// One provider saving one neighbour one resource.
#[derive(Clone, Copy, Debug)]
pub struct Link {
    pub provider: UnitId,
    pub consumer: UnitId,
    pub resource: Resource,
    /// Share of the neighbour's use this provider saves on its own (before the cap).
    pub share: Fx,
    /// The stretch of lot edge the two share, end to end, in metres.
    pub edge: (FxVec2, FxVec2),
}

/// This tick's links and what they come to per unit. Not state.
#[derive(Default)]
pub struct Links {
    pub links: Vec<Link>,
    /// Share of its use each unit is saved, capped, by row: `[mass, energy]`.
    saving: Vec<[Fx; 2]>,
}

impl Links {
    /// Share of its use of `resource` the unit in `row` is saved this tick.
    pub fn saving(&self, row: usize, resource: Resource) -> Fx {
        self.saving
            .get(row)
            .map_or(Fx::ZERO, |s| s[resource as usize])
    }
}

/// A lot as metres: `(x0, y0, x1, y1)`.
type Lot = (i32, i32, i32, i32);

/// The lot a building of `bp` at `pos` stands on, in metres.
pub fn lot(bp: &UnitBlueprint, pos: FxVec2) -> Lot {
    let half_w = bp.footprint.0 as i32 * mc_map::BUILD_CELL_M / 2;
    let half_h = bp.footprint.1 as i32 * mc_map::BUILD_CELL_M / 2;
    let (cx, cy) = (pos.x.round_int(), pos.y.round_int());
    (cx - half_w, cy - half_h, cx + half_w, cy + half_h)
}

/// The stretch of edge two lots share, if they meet along a side (not just a corner)
/// without overlapping.
pub fn shared_edge(a: Lot, b: Lot) -> Option<(FxVec2, FxVec2)> {
    let point = |x: i32, y: i32| FxVec2::new(Fx::from_int(x), Fx::from_int(y));
    let span = |a0: i32, a1: i32, b0: i32, b1: i32| (a0.max(b0), a1.min(b1));
    if a.2 == b.0 || b.2 == a.0 {
        let x = if a.2 == b.0 { a.2 } else { a.0 };
        let (y0, y1) = span(a.1, a.3, b.1, b.3);
        (y1 > y0).then(|| (point(x, y0), point(x, y1)))
    } else if a.3 == b.1 || b.3 == a.1 {
        let y = if a.3 == b.1 { a.3 } else { a.1 };
        let (x0, x1) = span(a.0, a.2, b.0, b.2);
        (x1 > x0).then(|| (point(x0, y), point(x1, y)))
    } else {
        None
    }
}

/// Whether a building of `bp` spends `resource` in a way a neighbour can save:
/// energy upkeep, or a factory's building.
pub fn uses(bp: &UnitBlueprint, resource: Resource) -> bool {
    let factory = bp.is_structure() && bp.builder.is_some();
    match resource {
        Resource::Mass => factory,
        Resource::Energy => factory || bp.economy.energy_upkeep > Fx::ZERO,
    }
}

/// What a `provider` saves a `consumer` standing against it: one link per resource.
pub fn offers(provider: &UnitBlueprint, consumer: &UnitBlueprint) -> [Option<(Resource, Fx)>; 2] {
    let Some(a) = provider.adjacency else {
        return [None, None];
    };
    let one = |resource, share: Fx| {
        (share > Fx::ZERO && uses(consumer, resource)).then_some((resource, share))
    };
    [one(Resource::Mass, a.mass), one(Resource::Energy, a.energy)]
}

/// Whether two buildings that touch go down together: a material fabricator and a
/// power plant (an energy provider) of the same tech.
pub fn bound(a: &UnitBlueprint, b: &UnitBlueprint) -> bool {
    let powers = |p: &UnitBlueprint| p.adjacency.is_some_and(|a| a.energy > Fx::ZERO);
    a.tech == b.tech
        && ((a.fabricator.is_some() && powers(b)) || (b.fabricator.is_some() && powers(a)))
}

impl World {
    /// Works out this tick's links from the finished structures: run by the economy
    /// before anything is paid.
    pub(crate) fn refresh_adjacency(&mut self) {
        let units = &self.state.units;
        // Every finished structure that provides or uses, by owner then row, so the
        // pairs are visited in one order on every machine.
        let mut lots: Vec<(u8, usize, Lot)> = Vec::new();
        for row in units.slots.iter() {
            if !units.is_active(row) {
                continue;
            }
            let bp = self.bp(row);
            if !bp.is_structure()
                || (bp.adjacency.is_none()
                    && !uses(bp, Resource::Mass)
                    && !uses(bp, Resource::Energy))
            {
                continue;
            }
            lots.push((units.owner[row], row, lot(bp, units.pos[row])));
        }
        lots.sort_by_key(|&(owner, row, _)| (owner, row));
        let links = &mut self.adjacency;
        links.links.clear();
        links.saving.clear();
        links.saving.resize(units.slots.rows(), [Fx::ZERO; 2]);
        let mut start = 0;
        while start < lots.len() {
            let owner = lots[start].0;
            let end = start + lots[start..].iter().take_while(|l| l.0 == owner).count();
            let side = &lots[start..end];
            for &(_, p, p_lot) in side {
                let pbp = self.blueprints.unit(units.blueprint[p]);
                if pbp.adjacency.is_none() {
                    continue;
                }
                for &(_, c, c_lot) in side {
                    if c == p {
                        continue;
                    }
                    let cbp = self.blueprints.unit(units.blueprint[c]);
                    let offered = offers(pbp, cbp);
                    if offered.iter().all(Option::is_none) {
                        continue;
                    }
                    let Some(edge) = shared_edge(p_lot, c_lot) else {
                        continue;
                    };
                    for (resource, share) in offered.into_iter().flatten() {
                        let s = &mut links.saving[c][resource as usize];
                        *s = (*s + share).min(MAX_SAVING[resource as usize]);
                        links.links.push(Link {
                            provider: units.id(p),
                            consumer: units.id(c),
                            resource,
                            share,
                            edge,
                        });
                    }
                }
            }
            start = end;
        }
    }

    /// Energy upkeep the unit in `row` draws this tick, per second: none while it is
    /// powered down, less what its neighbours save it.
    pub(crate) fn upkeep(&self, row: usize) -> Fx {
        if self.powered_down(row) {
            return Fx::ZERO;
        }
        let full = self.bp(row).economy.energy_upkeep;
        full - full * self.adjacency.saving(row, Resource::Energy)
    }

    /// What the unit in `row` pays of the full `(mass, energy)` cost of building the
    /// one in `target`: a factory pays less for its products by what its neighbours
    /// save it.
    pub(crate) fn builder_pays(&self, row: usize, target: usize, cost: (Fx, Fx)) -> (Fx, Fx) {
        if !self.state.units.has_flag(target, flag::IN_FACTORY)
            || !uses(self.bp(row), Resource::Mass)
        {
            return cost;
        }
        let less = |v: Fx, r| v - v * self.adjacency.saving(row, r);
        (less(cost.0, Resource::Mass), less(cost.1, Resource::Energy))
    }

    /// The finished buildings bound to the one in `row` (`bound`) that touch it: they
    /// go up when it is destroyed. Worked out from the table, so it holds on the tick
    /// a snapshot is restored as on any other.
    pub(crate) fn bound_partners(&self, row: usize) -> Vec<usize> {
        let units = &self.state.units;
        let bp = self.bp(row);
        if bp.fabricator.is_none() && bp.adjacency.is_none_or(|a| a.energy <= Fx::ZERO) {
            return Vec::new();
        }
        let own = lot(bp, units.pos[row]);
        units
            .slots
            .iter()
            .filter(|&r| {
                r != row
                    && units.owner[r] == units.owner[row]
                    && units.is_active(r)
                    && self.bp(r).is_structure()
                    && bound(bp, self.bp(r))
                    && shared_edge(own, lot(self.bp(r), units.pos[r])).is_some()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lots_share_an_edge_but_not_a_corner() {
        let a = (0, 0, 24, 24);
        assert_eq!(
            shared_edge(a, (24, 12, 72, 60)),
            Some((
                FxVec2::new(Fx::from_int(24), Fx::from_int(12)),
                FxVec2::new(Fx::from_int(24), Fx::from_int(24))
            ))
        );
        assert!(shared_edge(a, (0, -48, 24, 0)).is_some());
        assert_eq!(shared_edge(a, (24, 24, 48, 48)), None, "a corner only");
        assert_eq!(shared_edge(a, (36, 0, 60, 24)), None, "a gap");
    }
}
