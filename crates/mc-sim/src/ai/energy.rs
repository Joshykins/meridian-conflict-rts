//! What a side's factories and builders will draw: energy, so its power goes
//! up before a stall and not after one, and mass, so it has the factories to
//! spend its income. A stall slows every factory and builder at once; the AI
//! used to notice only once demand passed income, then built one plant at a
//! time while everything waited.
use super::*;

/// Energy per unit of build power a mobile builder is counted at: structures
/// cost 3.6 (a plant) to 6 (a mine) energy per unit of build time, and builders
/// spend part of their time walking between sites.
const BUILDER_DRAW: Fx = Fx::from_int(2);

impl World {
    /// Energy a second the side draws with every factory and builder at work,
    /// plus the upkeep of everything it has standing or going up, and never
    /// less than what it asks for right now.
    pub(super) fn energy_need(&self, player: u8) -> Fx {
        let units = &self.state.units;
        let mut need = Fx::ZERO;
        for row in units.slots.iter() {
            if units.owner[row] != player {
                continue;
            }
            let bp = self.bp(row);
            need += bp.economy.energy_upkeep;
            let Some(builder) = bp.builder.as_ref().filter(|_| units.is_active(row)) else {
                continue;
            };
            need += if bp.is_mobile() {
                builder.power * BUILDER_DRAW
            } else {
                builder.power * self.product_draw(bp)
            };
        }
        need.max(self.state.players[player as usize].energy_demand)
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
            power * next.cost_energy / next.build_time
        } else {
            Fx::ZERO
        }
    }

    /// Whether the side has the energy to spare for an upgrade drawing `draw`
    /// on top of what it draws now, with its store not run down. (Measured
    /// against the full `energy_need` no upgrade ever started: builders are
    /// seldom all at work at once.)
    pub(super) fn can_fund(&self, player: u8, draw: Fx) -> bool {
        let pl = &self.state.players[player as usize];
        pl.efficiency >= Fx::ratio(9, 10)
            && pl.energy > pl.energy_capacity * Fx::ratio(3, 10)
            && pl.energy_income >= pl.energy_demand + draw
    }
}
