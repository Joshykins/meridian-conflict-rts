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
        // A mine upgrade within the budget waits for the energy to spare for its
        // draw: started without it, it stalled every factory for minutes. One
        // past the budget is built from spare materials, put last (`direct_focus`),
        // so it takes only what nothing else wants, energy included.
        // An engineer goes up a tier where it stands, alongside whatever else starts.
        if let Some((row, next)) = self.engineer_to_upgrade(player) {
            if self.can_fund(player, self.upgrade_draw(row, next)) {
                let units = vec![self.state.units.id(row)];
                // An idle one may be helping a finished factory, which never
                // ends: it stops, or the upgrade would wait behind it for good.
                if census.builders_idle.contains(&row) {
                    out.push(Command::Stop {
                        units: units.clone(),
                    });
                }
                out.push(Command::Upgrade { units });
            }
        }
        if let Some((row, next)) = self.mine_to_upgrade(player, census) {
            // A side whose store fills upgrades while its energy holds: the
            // upgrade is the sink that pays, and its power follows (`economy.rs`).
            // Only while its energy holds: three begun at once on a draining store
            // left the mines unpaid.
            let pl = &self.state.players[player as usize];
            let floating = self.state.ai[player as usize].commander.eco.floating
                && pl.energy > pl.energy_capacity / 2
                && pl.energy_income >= pl.energy_spent;
            if floating || self.can_fund(player, self.upgrade_draw(row, next)) {
                out.push(Command::Upgrade {
                    units: vec![self.state.units.id(row)],
                });
                return;
            }
        }
        let skill = self.state.ai[player as usize].config.skill();
        let mut candidates: Vec<(u8, usize)> = Vec::new();
        if let Some(row) = self.tech_step(player, census) {
            candidates.push((0, row));
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
        if let Some(row) = self.fabricator_to_upgrade(player, census) {
            candidates.push((2, row));
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
            let behind = self.state.ai[player as usize].commander.behind;
            for &row in &census.factories_idle {
                if self.bp(row).upgrades_to.is_some() && !(behind && self.bp(row).tech >= 2) {
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

    /// The factory to take the side's next tier, when it is due. The next tier
    /// is a step taken on purpose, not only once materials pile up: an army
    /// spending all it had kept the side at tech 1 for thirty minutes. One
    /// factory goes up a tier once income reaches the skill's mark for it (tech
    /// 2 at the mark, tech 3 at three times it). A busy one too: the upgrade
    /// waits behind the unit it is building, and a factory kept busy was never
    /// idle when the AI looked, which held the side at tech 2 for twenty minutes.
    pub(super) fn tech_step(&self, player: u8, census: &Census) -> Option<usize> {
        let skill = self.state.ai[player as usize].config.skill();
        let income = self.state.players[player as usize].mass_income;
        let best = census
            .factories
            .iter()
            .map(|&r| self.bp(r).tech)
            .max()
            .unwrap_or(0);
        let step = Fx::from_int(skill.tech_income * (2 * best as i32 - 1).max(1));
        // A Commander behind on army takes tech 3 later: a tech 3 factory begun at
        // half the enemy's army left it nothing to hold its base with.
        let held = best >= 2 && self.state.ai[player as usize].commander.behind;
        if !(1..3).contains(&best)
            || income < step
            || held
            || census.factories.iter().any(|&r| self.upgrading(r))
        {
            return None;
        }
        census
            .factories
            .iter()
            .copied()
            .filter(|&r| self.bp(r).tech == best && self.bp(r).upgrades_to.is_some())
            .min_by_key(|&r| {
                (
                    !self.factory_trains_engineers(r),
                    !census.factories_idle.contains(&r),
                    r,
                )
            })
    }

    /// The engineer to take up a tier, lowest tier first, once the side's
    /// tech allows it. A third of them at most at once, so the rest keep
    /// building: an upgrade takes a Mason out of work for about three minutes,
    /// and gives it four times the build power and the side's bigger plants.
    pub(super) fn engineer_to_upgrade(&self, player: u8) -> Option<(usize, &UnitBlueprint)> {
        let units = &self.state.units;
        let side_tech = self.side_tech(player);
        let engineers: Vec<usize> = units
            .slots
            .iter()
            .filter(|&r| {
                let bp = self.bp(r);
                units.owner[r] == player
                    && units.is_active(r)
                    && bp.is_mobile()
                    && bp.has(cat::ENGINEER)
                    && !bp.has(cat::COMMANDER)
            })
            .collect();
        let running = engineers.iter().filter(|&&r| self.upgrading(r)).count();
        if running >= (engineers.len() / 3).max(1) {
            return None;
        }
        // A busy one too: the upgrade waits behind its job. They were seldom
        // idle, every one on a site crawling for want of materials, and ten
        // Masons stayed tech 1 for ten minutes after the side reached tech 2.
        engineers
            .iter()
            .copied()
            .filter(|&r| !self.upgrading(r))
            .filter_map(|r| {
                let next = self.blueprints.unit(self.bp(r).upgrades_to?);
                (self.blueprints.upgrade_needs(next) <= side_tech).then_some((r, next))
            })
            .min_by_key(|&(r, _)| (self.bp(r).tech, r))
    }

    /// Whether `row` has an upgrade under way or queued behind what it is building.
    pub(super) fn upgrading(&self, row: usize) -> bool {
        self.state
            .orders
            .iter(&self.state.units, row)
            .any(|o| matches!(o.kind, OrderKind::Upgrade))
    }

    /// Mine upgrades the side runs at once, as its economy sets them
    /// (`commander/economy.rs`): one more for every 30 a second, more while its
    /// store fills. One at a time kept the income flat for twenty minutes.
    pub(super) fn mine_upgrade_budget(&self, player: u8) -> i32 {
        self.commander_directives(player).upgrades
    }

    pub(super) fn mine_upgrades_running(&self, census: &Census) -> i32 {
        census
            .extractors
            .iter()
            .filter(|&&row| self.upgrading(row))
            .count() as i32
    }

    /// The mine whose next tier pays back its cost soonest, if soon enough for
    /// this AI: twice as long with materials to `spare`.
    pub(super) fn mine_to_upgrade(
        &self,
        player: u8,
        census: &Census,
    ) -> Option<(usize, &UnitBlueprint)> {
        let pl = &self.state.players[player as usize];
        let units = &self.state.units;
        if pl.mass_income < Fx::from_int(3) {
            return None;
        }
        let directives = self.commander_directives(player);
        // Short of power, plants come before mines that draw more: a tech 2
        // mine's upkeep is six times a tech 1's.
        if directives.power != super::commander::economy::Power::Enough && !directives.floating {
            return None;
        }
        if self.mine_upgrades_running(census) >= self.mine_upgrade_budget(player) {
            return None;
        }
        let side_tech = self.side_tech(player);
        let open = census.extractors.iter().copied().filter_map(|row| {
            let next = self.blueprints.unit(self.bp(row).upgrades_to?);
            (self.blueprints.upgrade_needs(next) <= side_tech && units.order_head[row] == NO_ORDER)
                .then_some((row, next))
        });
        let horizon = Fx::from_int(directives.payback as i32);
        // No saving up first: a mine upgrade is paid before the factories in a
        // stall, and a good one is the best thing the side can spend on.
        // Energy counts at what a tech 1 mine costs in it for each unit of mass.
        open.filter_map(|(row, next)| {
            let state = self.state.mines.by_unit.get(&units.id(row))?;
            let gain = state.full_rate(&next.mine?) - state.full_rate(&self.bp(row).mine?);
            let (mass, energy) = self.blueprints.upgrade_cost(next);
            let cost = mass + energy / ENERGY_PER_MASS;
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
