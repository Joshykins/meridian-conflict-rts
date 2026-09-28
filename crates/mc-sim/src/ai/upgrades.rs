//! Upgrades the AI starts: mines by payback, then its commander, radar and factories.
use super::*;

impl World {
    pub(super) fn direct_upgrades(&self, player: u8, census: &Census, out: &mut Vec<Command>) {
        let pl = &self.state.players[player as usize];
        let income = pl.mass_income;
        let mass_rich = pl.mass > pl.mass_capacity * Fx::ratio(6, 10);
        if census.power.len() < 2 {
            return;
        }
        // A mine upgrade is paid first, energy too: started without the energy
        // to spare for its own draw, it stalled every factory for minutes.
        if let Some((row, next)) = self.mine_to_upgrade(player, census, mass_rich) {
            if self.can_fund(player, self.upgrade_draw(row, next)) {
                out.push(Command::Upgrade {
                    units: vec![self.state.units.id(row)],
                });
                return;
            }
        }
        let skill = self.state.ai[player as usize].config.skill();
        let mut candidates: Vec<(u8, usize)> = Vec::new();
        // The next tier is a step taken on purpose, not only once materials
        // pile up: an army spending all it had kept the side at tech 1 for
        // thirty minutes. One factory goes up a tier once income reaches the
        // skill's mark for it (tech 2 at the mark, tech 3 at three times it).
        let best = census
            .factories
            .iter()
            .map(|&r| self.bp(r).tech)
            .max()
            .unwrap_or(0);
        let step = Fx::from_int(skill.tech_income * (2 * best as i32 - 1).max(1));
        if (1..3).contains(&best)
            && income >= step
            && !census.factories.iter().any(|&r| self.upgrading(r))
        {
            if let Some(&row) = census
                .factories_idle
                .iter()
                .filter(|&&r| self.bp(r).tech == best && self.bp(r).upgrades_to.is_some())
                .min_by_key(|&&r| (!self.factory_trains_engineers(r), r))
            {
                candidates.push((0, row));
            }
        }
        if income >= Fx::from_int(10) && pl.mass > Fx::from_int(400) {
            if let Some(cmd) = self.state.units.row(pl.commander) {
                if (self.bp(cmd).upgrades_to.is_some() || self.ai_next_refit(cmd).is_some())
                    && self.state.units.order_head[cmd] == NO_ORDER
                {
                    candidates.push((1, cmd));
                }
            }
        }
        if income >= Fx::from_int(8) {
            for row in self.state.units.slots.iter() {
                if self.state.units.owner[row] != player {
                    continue;
                }
                let bp = self.bp(row);
                if bp.has(cat::INTEL)
                    && bp.upgrades_to.is_some()
                    && self.state.units.order_head[row] == NO_ORDER
                    && self.state.units.is_active(row)
                {
                    candidates.push((2, row));
                }
            }
        }
        let surplus = mass_rich || (skill.eager_tech && pl.mass > Fx::from_int(400));
        // A factory being upgraded builds nothing: a few at a time, however
        // often this AI thinks, so the army keeps coming.
        let upgrading = census
            .factories
            .iter()
            .filter(|&&row| self.upgrading(row))
            .count();
        if income >= Fx::from_int(skill.tech_income)
            && surplus
            && upgrading < 1 + census.factories.len() / 4
        {
            for &row in &census.factories_idle {
                if self.bp(row).upgrades_to.is_some() {
                    candidates.push((3, row));
                }
            }
        }
        candidates.sort_by_key(|(p, r)| (*p, *r));
        // The first that the side's energy can carry: one it cannot does not
        // hold back a cheaper one behind it.
        for (_, row) in candidates {
            let kit = self.ai_next_refit(row);
            let Some(next) = kit.or(self.bp(row).upgrades_to) else {
                continue;
            };
            if !self.can_fund(player, self.upgrade_draw(row, self.blueprints.unit(next))) {
                continue;
            }
            let units = vec![self.state.units.id(row)];
            out.push(match kit {
                Some(kit) => Command::Refit { units, kit },
                None => Command::Upgrade { units },
            });
            return;
        }
    }

    pub(super) fn upgrading(&self, row: usize) -> bool {
        let head = self.state.units.order_head[row];
        head != NO_ORDER
            && matches!(
                self.state.orders.order[head as usize].kind,
                OrderKind::Upgrade
            )
    }

    /// The mine whose next tier pays back its cost soonest, if soon enough for this AI.
    pub(super) fn mine_to_upgrade(
        &self,
        player: u8,
        census: &Census,
        mass_rich: bool,
    ) -> Option<(usize, &UnitBlueprint)> {
        let pl = &self.state.players[player as usize];
        let skill = self.state.ai[player as usize].config.skill();
        let units = &self.state.units;
        if pl.mass_income < Fx::from_int(8) {
            return None;
        }
        let upgrading = census
            .extractors
            .iter()
            .filter(|&&row| self.upgrading(row))
            .count() as i32;
        // One at a time while the side is small, more as it grows.
        if upgrading > (pl.mass_income / Fx::from_int(60)).floor_int() {
            return None;
        }
        let side_tech = self.side_tech(player);
        let open = census.extractors.iter().copied().filter_map(|row| {
            let next = self.blueprints.unit(self.bp(row).upgrades_to?);
            (self.blueprints.upgrade_needs(next) <= side_tech && units.order_head[row] == NO_ORDER)
                .then_some((row, next))
        });
        let horizon = Fx::from_int(skill.upgrade_payback * if mass_rich { 2 } else { 1 });
        // No saving up first: a mine upgrade is paid before the factories in a
        // stall, and a good one is the best thing the side can spend on.
        // Energy counts at what a tech 1 mine costs in it for each unit of mass.
        open.filter_map(|(row, next)| {
            let state = self.state.mines.by_unit.get(&units.id(row))?;
            let gain = state.full_rate(&next.mine?) - state.full_rate(&self.bp(row).mine?);
            let cost = next.cost_mass + next.cost_energy / ENERGY_PER_MASS;
            (gain > Fx::ZERO).then(|| (row, next, cost / gain))
        })
        .filter(|&(_, _, payback)| payback <= horizon)
        .min_by_key(|&(row, _, payback)| (payback, row))
        .map(|(row, next, _)| (row, next))
    }

    /// The cheapest module that goes on this unit without taking another off.
    pub(super) fn ai_next_refit(&self, row: usize) -> Option<mc_data::BlueprintId> {
        let (set, loadout) = self.blueprints.loadout(self.bp(row).id)?;
        set.slots
            .iter()
            .enumerate()
            .flat_map(|(s, slot)| (0..slot.modules.len() as u8).map(move |m| (s, m)))
            .filter(|&(s, m)| {
                set.fit(&loadout.fitted, s, m).is_ok()
                    && set.replaces(&loadout.fitted, s, m).is_none()
            })
            .min_by_key(|&(s, m)| (set.module(s, m).cost_mass, s, m))
            .map(|(s, m)| set.module(s, m).kit)
    }
}
