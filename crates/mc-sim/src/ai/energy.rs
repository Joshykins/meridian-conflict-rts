//! What a side's factories and builders will draw: energy, so its power goes
//! up before a stall and not after one, and mass, so it has the factories to
//! spend its income. A stall slows every factory and builder at once; the AI
//! used to notice only once demand passed income, then built one plant at a
//! time while everything waited.
use super::*;
use crate::focus::{Focus, Priority};

/// Energy per unit of build power a mobile builder is counted at: structures
/// cost 3.6 (a plant) to 6 (a mine) energy per unit of build time, and builders
/// spend part of their time walking between sites.
const BUILDER_DRAW: Fx = Fx::from_int(2);

impl World {
    /// Energy a second the side draws with every factory and builder at work,
    /// plus the upkeep of everything it has standing or going up and the draw
    /// of the upgrades running, and never less than what it really spends right now
    /// (not what it asks for: in a mass stall that is far more than it is paid).
    pub(super) fn energy_need(&self, player: u8) -> Fx {
        let units = &self.state.units;
        let mut need = Fx::ZERO;
        // A Commander counts what builds at the speed its materials pay for: at
        // full draw, a side building at a fifth of full speed put up 27 plants to
        // the classic AI's 18 and claimed no mine from minute five to eight.
        // Only a materials stall with energy to spare: in an energy stall the slow
        // build speed is the want of power itself, and counting it so asked for no
        // plants at all.
        let pl = &self.state.players[player as usize];
        let energy_stalled = pl.energy < pl.energy_capacity / 2;
        let paid = if self.commander_directives(player).is_some() && !energy_stalled {
            pl.build_speed.clamp(Fx::ratio(1, 3), Fx::ONE)
        } else {
            Fx::ONE
        };
        for row in units.slots.iter() {
            if units.owner[row] != player {
                continue;
            }
            let bp = self.bp(row);
            need += bp.economy.energy_upkeep;
            if let Some(next) = bp.upgrades_to.filter(|_| self.upgrading(row)) {
                need += self.upgrade_draw(row, self.blueprints.unit(next)) * paid;
            }
            let Some(builder) = bp.builder.as_ref().filter(|_| units.is_active(row)) else {
                continue;
            };
            need += if bp.is_mobile() {
                builder.power * BUILDER_DRAW * paid
            } else {
                builder.power * self.product_draw(bp) * paid
            };
        }
        need.max(self.state.players[player as usize].energy_spent)
    }

    /// Mass a second `player`'s finished factories would spend with every
    /// one of them at work.
    pub(super) fn factory_mass_draw(&self, player: u8) -> Fx {
        let units = &self.state.units;
        units
            .slots
            .iter()
            .filter(|&r| units.owner[r] == player && units.is_active(r))
            .filter_map(|r| {
                let bp = self.bp(r);
                let power = bp.builder.as_ref().filter(|_| bp.has(cat::FACTORY))?.power;
                Some(power * self.product_rate(bp, |u| u.cost_mass))
            })
            .fold(Fx::ZERO, |a, b| a + b)
    }

    /// Energy per unit of build time of the mobile units a factory makes, on average.
    fn product_draw(&self, factory: &UnitBlueprint) -> Fx {
        self.product_rate(factory, |u| u.cost_energy)
    }

    /// `cost` per unit of build time of the mobile units a factory makes, on average.
    fn product_rate(&self, factory: &UnitBlueprint, cost: impl Fn(&UnitBlueprint) -> Fx) -> Fx {
        let Some(builder) = &factory.builder else {
            return Fx::ZERO;
        };
        let (mut sum, mut n) = (Fx::ZERO, 0);
        for &id in &builder.builds {
            let bp = self.blueprints.unit(id);
            if bp.is_mobile() && bp.build_time > Fx::ZERO {
                sum += cost(bp) / bp.build_time;
                n += 1;
            }
        }
        if n == 0 {
            Fx::ZERO
        } else {
            sum / Fx::from_int(n)
        }
    }

    /// Energy a second an upgrade of `row` into `next` draws while it runs.
    pub(super) fn upgrade_draw(&self, row: usize, next: &UnitBlueprint) -> Fx {
        let power = self
            .bp(row)
            .builder
            .as_ref()
            .map_or(crate::orders::SELF_UPGRADE_POWER, |b| b.power);
        if next.build_time > Fx::ZERO {
            power * self.blueprints.upgrade_cost(next).1 / next.build_time
        } else {
            Fx::ZERO
        }
    }

    /// Whether the side has the energy to spare for an upgrade drawing `draw`
    /// on top of what it draws now, with its store not run down. (Measured
    /// against the full `energy_need` no upgrade ever started: builders are
    /// seldom all at work at once.) Energy only: a side spending all its
    /// materials is short of mass nearly all the time, and judged by the whole
    /// stall it upgraded six mines in half an hour.
    pub(super) fn can_fund(&self, player: u8, draw: Fx) -> bool {
        let pl = &self.state.players[player as usize];
        pl.upkeep_efficiency >= Fx::ratio(9, 10)
            && pl.energy > pl.energy_capacity * Fx::ratio(3, 10)
            && pl.energy_income >= pl.energy_demand + draw
    }

    /// Puts first (`focus.rs`) whatever the side is running out of: new power while
    /// energy stalls, new mines while mass does. Energy first, since a side out of
    /// energy digs less mass too. Mines go last while more upgrades run than the
    /// budget (`mine_upgrade_budget`): the extra ones were started from spare
    /// materials and take only what the factories leave, energy too.
    pub(super) fn direct_focus(&self, player: u8, census: &Census, out: &mut Vec<Command>) {
        let pl = &self.state.players[player as usize];
        let short = |have: Fx, capacity: Fx, income: Fx, demand: Fx| {
            income < demand && have < capacity * Fx::ratio(1, 4)
        };
        let mut focus = Focus::default();
        let energy_short = short(
            pl.energy,
            pl.energy_capacity,
            pl.energy_income,
            pl.energy_demand,
        );
        if energy_short {
            focus.power = Priority::First;
        }
        if self.mine_upgrades_running(census) > self.mine_upgrade_budget(player) {
            focus.mines = Priority::Last;
        } else if !energy_short && short(pl.mass, pl.mass_capacity, pl.mass_income, pl.mass_demand)
        {
            focus.mines = Priority::First;
        }
        if focus != pl.focus {
            out.push(Command::SetFocus { focus });
        }
    }
}
