//! The AI's use of adjacency (`crate::adjacency`): providers go where they save
//! the most, and material fabricators are built, upgraded and paused as the
//! economy wants.
//!
//! Power used to go only into farms behind the base, and the lane every building
//! keeps (`lots.rs`) kept anything from standing against a factory, so no plant
//! of the AI's ever saved a thing, and it never built a fabricator. Now a plant
//! or a fabricator takes the free lot flush against the side's buildings where it
//! saves the most a second: what it saves the neighbours that use what it
//! provides, and what they save it. It goes to a farm only where it would save
//! nothing. A fabricator and a plant that would be bound (go down together) are
//! never set against each other.
use super::*;
use crate::adjacency::{self, Resource};

/// The side's buildings this far from home are the ones a provider is set against.
const ADJACENT_REACH: Fx = HOME_RADIUS;
/// Deliberate cap on the lots tried, best first: past these a base is packed and a
/// farm lot saves nearly as much.
const ADJACENT_TRIES: usize = 48;
/// Fabricators' upkeep held to this share of the side's energy income, in percent:
/// past it a stall would leave the mines unpaid as well (deliberate cap).
const FABRICATOR_ENERGY: i32 = 50;

/// A lot as metres, `(x0, y0, x1, y1)` (`adjacency::lot`).
type Lot = (i32, i32, i32, i32);

/// One of the side's buildings, standing, going up or planned, as adjacency sees it.
struct Neighbour<'a> {
    bp: &'a UnitBlueprint,
    lot: Lot,
}

/// Whether one of `a` and `b` saves the other something, and they are not bound:
/// a provider may stand flush against such a neighbour (`lots.rs`).
pub(super) fn feeds(a: &UnitBlueprint, b: &UnitBlueprint) -> bool {
    let any = |o: [Option<(Resource, Fx)>; 2]| o.iter().any(Option::is_some);
    (any(adjacency::offers(a, b)) || any(adjacency::offers(b, a))) && !adjacency::bound(a, b)
}

/// Energy a unit of `resource` is worth: materials at `ENERGY_PER_MASS`.
fn worth_of(resource: Resource) -> Fx {
    match resource {
        Resource::Mass => Fx::from_int(ENERGY_PER_MASS),
        Resource::Energy => Fx::ONE,
    }
}

impl World {
    /// What a building of `bp` uses a second of `resource` that a neighbour can
    /// save: its energy upkeep, and what a factory spends at full speed (its build
    /// power at the average rate of what it makes).
    fn adjacency_use(&self, bp: &UnitBlueprint, resource: Resource) -> Fx {
        let building = bp
            .builder
            .as_ref()
            .filter(|b| adjacency::uses(bp, Resource::Mass) && !b.builds.is_empty())
            .map_or(Fx::ZERO, |b| {
                let per_time: Fx = b
                    .builds
                    .iter()
                    .map(|&id| {
                        let u = self.blueprints.unit(id);
                        let cost = match resource {
                            Resource::Mass => u.cost_mass,
                            Resource::Energy => u.cost_energy,
                        };
                        if u.build_time > Fx::ZERO {
                            cost / u.build_time
                        } else {
                            Fx::ZERO
                        }
                    })
                    .sum();
                b.power * per_time / Fx::from_int(b.builds.len() as i32)
            });
        match resource {
            Resource::Mass => building,
            Resource::Energy => building + bp.economy.energy_upkeep,
        }
    }

    /// What a building of `bp` on `lot` saves its side a second, in energy, against
    /// `neighbours`: what it saves those it touches and what they save it. `None`
    /// where it would touch one it is bound to.
    fn adjacency_worth(
        &self,
        bp: &UnitBlueprint,
        lot: Lot,
        neighbours: &[Neighbour],
    ) -> Option<Fx> {
        let mut worth = Fx::ZERO;
        for n in neighbours {
            let Some(edge) = adjacency::shared_edge(lot, n.lot) else {
                continue;
            };
            if adjacency::bound(bp, n.bp) {
                return None;
            }
            for (r, full) in adjacency::offers(bp, n.bp).into_iter().flatten() {
                worth += adjacency::edge_share(full, edge, n.lot)
                    * self.adjacency_use(n.bp, r)
                    * worth_of(r);
            }
            for (r, full) in adjacency::offers(n.bp, bp).into_iter().flatten() {
                worth += adjacency::edge_share(full, edge, lot)
                    * self.adjacency_use(bp, r)
                    * worth_of(r);
            }
        }
        Some(worth)
    }

    /// The side's buildings near home that a building of `bp` would trade with:
    /// standing, going up, or planned by a builder.
    fn adjacency_neighbours(
        &self,
        bp: &UnitBlueprint,
        player: u8,
        start: FxVec2,
    ) -> Vec<Neighbour<'_>> {
        let units = &self.state.units;
        let near = |p: FxVec2| p.distance(start) <= ADJACENT_REACH;
        let standing = units
            .slots
            .iter()
            .filter(|&r| units.owner[r] == player && near(units.pos[r]))
            .map(|r| (self.bp(r), units.pos[r]));
        let planned = self
            .planned_sites(player)
            .filter(|(_, o)| near(o.pos))
            .map(|(_, o)| (self.blueprints.unit(o.blueprint), o.pos));
        let mut out: Vec<Neighbour> = standing
            .chain(planned)
            .filter(|(n, _)| n.is_structure() && (feeds(bp, n) || adjacency::bound(bp, n)))
            .map(|(n, pos)| Neighbour {
                bp: n,
                lot: adjacency::lot(n, pos),
            })
            .collect();
        // The same plan in two builders' queues is one building.
        out.sort_by_key(|n| n.lot);
        out.dedup_by_key(|n| n.lot);
        out
    }

    /// The free lot flush against the side's buildings where a provider of `bp`
    /// saves the most, if it saves anything anywhere.
    pub(super) fn adjacent_site(
        &self,
        bp: &UnitBlueprint,
        player: u8,
        start: FxVec2,
        claimed: &[Claim],
        home: Option<&staging::HomeGround>,
    ) -> Option<FxVec2> {
        bp.adjacency?;
        let neighbours = self.adjacency_neighbours(bp, player, start);
        let cell = mc_map::BUILD_CELL_M;
        let (w, h) = (bp.footprint.0 as i32 * cell, bp.footprint.1 as i32 * cell);
        // (worth, metres off a corner of the building it stands against, distance
        // from home, site): from the corners in, so a side takes as many as fit.
        let mut spots: Vec<(Fx, i32, Fx, FxVec2)> = Vec::new();
        for n in neighbours.iter().filter(|n| feeds(bp, n.bp)) {
            let (x0, y0, x1, y1) = n.lot;
            // Lower corners of lots against each side, a cell of edge shared at least.
            let mut corners: Vec<(i32, i32)> = Vec::new();
            for y in ((y0 - h + cell)..=(y1 - cell)).step_by(cell as usize) {
                corners.push((x0 - w, y));
                corners.push((x1, y));
            }
            for x in ((x0 - w + cell)..=(x1 - cell)).step_by(cell as usize) {
                corners.push((x, y0 - h));
                corners.push((x, y1));
            }
            for (x, y) in corners {
                let site = FxVec2::from_ints(x + w / 2, y + h / 2);
                let lot = adjacency::lot(bp, site);
                if let Some(worth) = self
                    .adjacency_worth(bp, lot, &neighbours)
                    .filter(|&v| v > Fx::ZERO)
                {
                    let corner = (lot.0 - x0)
                        .abs()
                        .min((lot.2 - x1).abs())
                        .min((lot.1 - y0).abs())
                        .min((lot.3 - y1).abs());
                    spots.push((worth, corner, site.distance(start), site));
                }
            }
        }
        spots.sort_by_key(|&(worth, corner, d, p)| (-worth, corner, d, p.x, p.y));
        let ore = self.ore_centres();
        spots
            .into_iter()
            .take(ADJACENT_TRIES)
            .map(|(_, _, _, p)| p)
            .find(|&p| self.lot_free(bp, p, claimed, &ore, home))
    }

    /// Whether the side's economy has room for fabricators: energy to spare, the
    /// materials not piling up, past the opening, and their upkeep within
    /// `FABRICATOR_ENERGY` of the income.
    fn fabricate(&self, player: u8) -> bool {
        let pl = &self.state.players[player as usize];
        let directives = self.commander_directives(player);
        let skill = self.state.ai[player as usize].config.skill();
        let units = &self.state.units;
        let upkeep: Fx = units
            .slots
            .iter()
            .filter(|&r| units.owner[r] == player && self.bp(r).fabricator.is_some())
            .map(|r| self.bp(r).economy.energy_upkeep)
            .sum();
        directives.power == super::commander::economy::Power::Enough
            && !directives.floating
            && pl.energy >= pl.energy_capacity * Fx::ratio(3, 4)
            && pl.mass_income >= Fx::from_int(3 * skill.tech_income)
            && upkeep * 100 < pl.energy_income * Fx::from_int(FABRICATOR_ENERGY)
    }

    /// Seconds a fabricator `step` (a new one, or an upgrade) takes to pay back
    /// `cost` and the power for its upkeep, `extra_upkeep` a second, from the
    /// `extra_mass` a second it makes: power at what the best plant `row` can raise
    /// costs, energy at `ENERGY_PER_MASS`.
    fn fabricator_payback(
        &self,
        row: usize,
        cost: (Fx, Fx),
        extra_mass: Fx,
        extra_upkeep: Fx,
    ) -> Fx {
        let tech = self
            .builder_tech(row)
            .max(self.best_builder_tech(self.state.units.owner[row]));
        let per_energy = self
            .best_plant(self.state.units.owner[row], tech)
            .map_or(Fx::ONE / ENERGY_PER_MASS, |p| {
                p.cost_mass / p.economy.energy_income
            });
        let total = cost.0 + cost.1 / ENERGY_PER_MASS + extra_upkeep * per_energy;
        if extra_mass > Fx::ZERO {
            total / extra_mass
        } else {
            Fx::MAX
        }
    }

    /// The side's cheapest plant per unit of energy up to `tech`, from any of its
    /// builders' lists.
    fn best_plant(&self, player: u8, tech: u8) -> Option<&UnitBlueprint> {
        let menu = self.side_menu(player);
        menu.ids
            .iter()
            .map(|&id| self.blueprints.unit(id))
            .filter(|b| {
                b.has(cat::POWER)
                    && b.is_structure()
                    && b.tech <= tech
                    && b.economy.energy_income > Fx::ZERO
            })
            .min_by_key(|b| (b.cost_mass / b.economy.energy_income, b.id.0))
    }

    /// A fabricator for builder `row`, set where it saves the most: when the
    /// economy has room for one (`fabricate`), none is going up, and it pays back
    /// within the economy's horizon.
    pub(super) fn fabricator_job(&self, row: usize, planned: &Planned) -> Option<Job> {
        let player = self.state.units.owner[row];
        let start = self.state.players[player as usize].start;
        // A builder far out leaves the base's economy to those at home.
        if planned.fabricators_rising > 0
            || self.state.units.pos[row].distance(start) > FAR_FROM_HOME
            || !self.fabricate(player)
        {
            return None;
        }
        let horizon = Fx::from_int(self.commander_directives(player).payback as i32);
        let builder = self.bp(row).builder.as_ref()?;
        let (blueprint, _) = builder
            .builds
            .iter()
            .filter_map(|&id| {
                let bp = self.blueprints.unit(id);
                let f = bp.fabricator.filter(|_| bp.is_structure())?;
                let payback = self.fabricator_payback(
                    row,
                    (bp.cost_mass, bp.cost_energy),
                    f.mass,
                    bp.economy.energy_upkeep,
                );
                (payback <= horizon).then_some((id, payback))
            })
            .min_by_key(|&(id, payback)| (payback, id.0))?;
        Some(Job {
            blueprint,
            near: start,
            heading: AI_BUILD_HEADING,
            min_r: Fx::ZERO,
            keep_off_deposits: true,
            place: Place::Adjacent,
        })
    }

    /// A fabricator to take up a tier: when the economy has room for one, the
    /// side has the tech, it pays back within the horizon, and the upgrade would
    /// not bind it to a plant it touches.
    pub(super) fn fabricator_to_upgrade(&self, player: u8, census: &Census) -> Option<usize> {
        if !self.fabricate(player) {
            return None;
        }
        let units = &self.state.units;
        let horizon = Fx::from_int(self.commander_directives(player).payback as i32);
        let side_tech = self.side_tech(player);
        census
            .fabricators
            .iter()
            .copied()
            .filter(|&row| units.order_head[row] == NO_ORDER && !units.paused[row])
            .filter_map(|row| {
                let bp = self.bp(row);
                let next = self.blueprints.unit(bp.upgrades_to?);
                if self.blueprints.upgrade_needs(next) > side_tech {
                    return None;
                }
                let gain = next.fabricator?.mass - bp.fabricator?.mass;
                let payback = self.fabricator_payback(
                    row,
                    self.blueprints.upgrade_cost(next),
                    gain,
                    next.economy.energy_upkeep - bp.economy.energy_upkeep,
                );
                let lot = adjacency::lot(next, units.pos[row]);
                let binds = units.slots.iter().any(|r| {
                    units.owner[r] == player
                        && self.bp(r).is_structure()
                        && adjacency::bound(next, self.bp(r))
                        && adjacency::shared_edge(lot, adjacency::lot(self.bp(r), units.pos[r]))
                            .is_some()
                });
                (payback <= horizon && !binds).then_some((row, payback))
            })
            .min_by_key(|&(row, payback)| (payback, row))
            .map(|(row, _)| row)
    }

    /// Fabricators off while the side's power has run out, so the mines are paid
    /// first, and on again once the store holds and the income carries them.
    pub(super) fn direct_fabricators(&self, player: u8, census: &Census, out: &mut Vec<Command>) {
        let units = &self.state.units;
        let pl = &self.state.players[player as usize];
        let power = self.commander_directives(player).power;
        let (paused, running): (Vec<usize>, Vec<usize>) =
            census.fabricators.iter().partition(|&&r| units.paused[r]);
        if power == super::commander::economy::Power::Urgent {
            if !running.is_empty() {
                out.push(Command::SetPaused {
                    units: running.iter().map(|&r| units.id(r)).collect(),
                    paused: true,
                });
            }
            return;
        }
        let upkeep: Fx = paused
            .iter()
            .map(|&r| self.bp(r).economy.energy_upkeep)
            .sum();
        if !paused.is_empty()
            && pl.energy >= pl.energy_capacity / 2
            && pl.energy_income >= pl.energy_spent + upkeep
        {
            out.push(Command::SetPaused {
                units: paused.iter().map(|&r| units.id(r)).collect(),
                paused: false,
            });
        }
    }
}

#[cfg(test)]
#[path = "adjacent_tests.rs"]
mod tests;
