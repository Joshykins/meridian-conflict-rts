//! What the AI's factories make: engineers while the side is short of them,
//! a couple of scouts, salvagers where wrecks lie, and otherwise the combat
//! unit `choose_combat_unit` ranks best, never a land unit past the home guard
//! where the land army cannot walk to the enemy (`theatre.rs`).
use super::*;

impl World {
    pub(super) fn direct_factories(
        &mut self,
        player: u8,
        census: &Census,
        salvage: &[salvage::Field],
        stance: Stance,
        persona: Personality,
        out: &mut Vec<Command>,
    ) {
        let mut counter = self.state.ai[player as usize].production_counter;
        let mut composition = self.ai_composition(player);
        let mut planned_engineers = 0;
        let mut planned_scouts = 0;
        let mut planned_salvagers = 0;
        // Scouting as a plan keeps more eyes out (`strategy.rs`).
        let want_scouts = if self.holds(player, super::strategy::Gambit::Scouting) {
            4
        } else {
            2
        };
        // No rally point: a finished unit rolls out idle and the army sends it
        // to the staging point with the rest. A rally among the base's buildings
        // jammed: units stuck a few metres short of it in the crowd never
        // finished the move, never counted as idle and never joined a wave.
        for &row in &census.factories_idle {
            let Some(builder) = &self.bp(row).builder else {
                continue;
            };
            // A Commander's economy says how many (`commander/economy.rs`).
            let want_engineers = match self.commander_directives(player) {
                Some(d) => d.engineers,
                None => {
                    let n = 2 + census.factories.len() * 2;
                    match persona {
                        Personality::Expander => n + 2,
                        Personality::Turtle => n + 1,
                        Personality::Aggressive => n,
                    }
                }
            };
            let commander = self.commander_directives(player).is_some();
            let engineer = builder
                .builds
                .iter()
                .copied()
                .filter(|b| {
                    let u = self.blueprints.unit(*b);
                    u.has(cat::ENGINEER) && !u.has(cat::COMMANDER) && u.builder.is_some()
                })
                .max_by_key(|b| (self.blueprints.unit(*b).tech, std::cmp::Reverse(b.0)));
            // More than one of the best tier: they put up the big plants and
            // factories, and one alone was always busy elsewhere.
            let missing_tech_builder = engineer.is_some_and(|id| {
                self.blueprints.unit(id).tech > 1
                    && composition.get(&id).copied().unwrap_or(0) < 1 + census.factories.len() / 2
            });
            let scout = builder.builds.iter().copied().find(|b| {
                let u = self.blueprints.unit(*b);
                u.has(cat::SCOUT) && (census.land_route || !theatre::land_bound(u))
            });
            // With no land route to the enemy a land-only unit never reaches it:
            // past a home guard, none are made (`theatre.rs`).
            let land_guard = !census.land_route
                && composition
                    .iter()
                    .filter(|(id, _)| {
                        let u = self.blueprints.unit(**id);
                        theatre::land_bound(u) && !u.weapons.is_empty() && !u.has(cat::ENGINEER)
                    })
                    .map(|(_, n)| *n)
                    .sum::<usize>()
                    >= self.land_guard_cap(player, census);
            let fighters: Vec<BlueprintId> = builder
                .builds
                .iter()
                .copied()
                .filter(|b| {
                    let u = self.blueprints.unit(*b);
                    !(land_guard && theatre::land_bound(u))
                        && u.is_mobile()
                        && (!u.weapons.is_empty()
                            || u.shield.is_some()
                            || u.radar > Fx::ZERO
                            || u.anti_missile > Fx::ZERO
                            || u.drone.is_some())
                        && !u.has(cat::COMMANDER)
                        && !u.has(cat::ENGINEER)
                        && !u.is_salvager()
                })
                .collect();
            // The best tier missing comes at once, not on every fourth product: the
            // count runs over all factories, and a tech 2 factory that always fell
            // on the wrong turn made one Mason II in five minutes.
            // A Commander makes an engineer whenever it is short of them: the
            // first one out claims mines, the rest follow as the income allows.
            let blueprint = if engineer.is_some()
                && (missing_tech_builder
                    || (census.engineers + planned_engineers < want_engineers
                        && (commander || counter.is_multiple_of(4) || stance == Stance::Firebase)))
            {
                planned_engineers += 1;
                engineer
            } else if let Some(salvager) = commander
                .then(|| self.salvage_product(row, census, planned_salvagers, salvage))
                .flatten()
            {
                // Then something to fetch the reclaim lying about, before scouts.
                planned_salvagers += 1;
                Some(salvager)
            } else if census.scouts + planned_scouts < want_scouts
                && scout.is_some()
                && counter % 5 == 1
            {
                planned_scouts += 1;
                scout
            } else if let Some(salvager) = (counter % 3 == 2)
                .then(|| self.salvage_product(row, census, planned_salvagers, salvage))
                .flatten()
            {
                planned_salvagers += 1;
                Some(salvager)
            } else {
                match self.commander_directives(player) {
                    Some(_) => self.solve_production(player, &fighters, &composition, counter),
                    None => {
                        self.choose_combat_unit(player, &fighters, &composition, stance, counter)
                    }
                }
            };
            if let Some(id) = blueprint {
                *composition.entry(id).or_insert(0) += 1;
            }
            counter = counter.wrapping_add(1);
            if let Some(blueprint) = blueprint {
                out.push(Command::Produce {
                    factories: vec![self.state.units.id(row)],
                    blueprint,
                    count: 1,
                });
            }
        }
        self.state.ai[player as usize].production_counter = counter;
    }
}
