//! What the Commander has built (`docs/AI_COMMANDER.md`, "Production"): factories
//! make what the plans need, picked from the menu by how well each unit fares
//! against the enemy the side believes in, per mass; builders put up what the
//! plans want (`wants`); and the classic economy code is steered by the plans.
use super::matchup::edge;
use super::profile::{role, Domain, Profile, Profiles, Target};
use super::state::{PlanKind, Stake};
use crate::ai::{Job, Place};
use crate::{Brain, World};
use mc_core::{Angle, Fx, FxVec2};
use mc_data::BlueprintId;
use std::collections::BTreeMap;

/// Enemy types, the heaviest by mass seen, a unit is judged against.
const MATCHED_AGAINST: usize = 8;

/// How the plans steer the classic builders and factories.
#[derive(Clone, Copy, Default)]
pub(in crate::ai) struct Directives {
    /// Engineers kept over the classic count.
    pub engineers: usize,
    /// Factories kept over the classic count.
    pub factories: usize,
}

fn s(stake: Stake) -> i64 {
    stake as i64
}

fn points(stake: Stake) -> i64 {
    [0, 1, 3, 6][stake as usize]
}

/// Forces a factory makes for: land (hover too), air, surface ships, submarines.
pub(in crate::ai) const FORCES: usize = 4;

/// The force a unit of profile `p` belongs to; none for spacecraft and structures.
pub(in crate::ai) fn force(p: &Profile) -> Option<usize> {
    match p.domain? {
        Domain::Land | Domain::Hover => Some(0),
        Domain::Air => Some(1),
        Domain::Naval => Some(2),
        Domain::Sub => Some(3),
        Domain::Static | Domain::Space => None,
    }
}

impl World {
    /// `player`'s plans as directives for the classic code; nothing for a classic AI.
    pub(in crate::ai) fn commander_directives(&self, player: u8) -> Option<Directives> {
        let ai = &self.state.ai[player as usize];
        if ai.config.brain != Brain::Commander {
            return None;
        }
        let c = &ai.commander;
        let boom = c.plan(PlanKind::Boom);
        let military = [
            PlanKind::Pressure,
            PlanKind::AirPower,
            PlanKind::SeaControl,
            PlanKind::SubWar,
            PlanKind::Raid,
        ]
        .iter()
        .map(|&k| c.plan(k) as usize)
        .sum::<usize>();
        Some(Directives {
            engineers: [0, 1, 3, 6][boom as usize],
            factories: (military / 3).min(3),
        })
    }

    /// The share of new combat units each force should get, from the stakes of the
    /// plans that use it: land, air, surface ships, submarines.
    pub(in crate::ai) fn force_shares(&self, player: u8) -> [i64; FORCES] {
        let c = &self.state.ai[player as usize].commander;
        let p = |k: PlanKind| points(c.plan(k));
        let land_route = self.land_route_to_enemy(player);
        [
            p(PlanKind::Pressure)
                + p(PlanKind::Raid)
                + p(PlanKind::Landing)
                + p(PlanKind::Siege)
                + if land_route { 2 } else { 1 },
            p(PlanKind::AirPower) + p(PlanKind::AirDefense) / 2,
            p(PlanKind::SeaControl) + if land_route { 0 } else { 2 },
            p(PlanKind::SubWar),
        ]
    }

    /// The combat unit an idle factory of `player` should make, from `menu`: first
    /// the force furthest under its share (`force_shares`), then the unit of that
    /// force that fills the plans' needs best and fares best against the enemy the
    /// side believes in, per mass.
    pub(in crate::ai) fn solve_production(
        &self,
        player: u8,
        menu: &[BlueprintId],
        counts: &BTreeMap<BlueprintId, usize>,
        serial: u32,
    ) -> Option<BlueprintId> {
        let profiles = Profiles::build(&self.blueprints);
        let c = &self.state.ai[player as usize].commander;
        let land_route = self.land_route_to_enemy(player);
        let stake = |k: PlanKind| s(c.plan(k));
        let air_seen = c.sticky.air_seen > 0;
        let landing = stake(PlanKind::Landing);
        // Which force each candidate is, and the shares.
        let shares = self.force_shares(player);
        let mut have = [Fx::ZERO; FORCES];
        let units = &self.state.units;
        for r in units.slots.iter() {
            if units.owner[r] == player {
                let p = profiles.get(units.blueprint[r]);
                if let Some(f) = force(p).filter(|_| p.armed()) {
                    have[f] += p.mass;
                }
            }
        }
        let total_share: i64 = shares.iter().sum::<i64>().max(1);
        let total_have: Fx = have.iter().copied().sum::<Fx>().max(Fx::ONE);
        // How far under its share each force is, in thousandths.
        let deficit = |f: usize| -> i64 {
            shares[f] * 1000 / total_share - (have[f] * 1000 / total_have).floor_int() as i64
        };
        let usable = |p: &Profile| {
            force(p).is_some()
                && (p.armed() || p.has(role::SHIELD))
                && (land_route || landing > 0 || !matches!(p.domain, Some(Domain::Land)))
        };
        let best_force = menu
            .iter()
            .map(|&id| profiles.get(id))
            .filter(|p| usable(p))
            .filter_map(force)
            .filter(|&f| shares[f] > 0)
            .max_by_key(|&f| (deficit(f), std::cmp::Reverse(f)));
        // Needs within a force: (does this unit fill it, weight).
        let ground = |p: &Profile| matches!(p.domain, Some(Domain::Land | Domain::Hover));
        let needs: Vec<(Box<dyn Fn(&Profile) -> bool>, i64)> = vec![
            (
                Box::new(move |p: &Profile| ground(p) && p.has(role::LINE)),
                3 + 2 * stake(PlanKind::Pressure) + landing,
            ),
            (
                Box::new(move |p: &Profile| ground(p) && p.has(role::RAIDER)),
                2 * stake(PlanKind::Raid),
            ),
            (
                Box::new(move |p: &Profile| ground(p) && p.has(role::ANTI_AIR)),
                (air_seen as i64) * (1 + stake(PlanKind::AirDefense)),
            ),
            (
                Box::new(move |p: &Profile| ground(p) && p.has(role::ARTILLERY)),
                1 + stake(PlanKind::Siege) * 2,
            ),
            (
                Box::new(|p: &Profile| p.domain == Some(Domain::Air) && p.has(role::STRIKE)),
                2 + 2 * stake(PlanKind::AirPower),
            ),
            (
                Box::new(|p: &Profile| p.domain == Some(Domain::Air) && p.has(role::ANTI_AIR)),
                1 + (air_seen as i64) * (1 + stake(PlanKind::AirDefense)),
            ),
            (
                Box::new(|p: &Profile| p.domain == Some(Domain::Naval) && p.armed()),
                3,
            ),
            (
                Box::new(|p: &Profile| p.domain == Some(Domain::Sub) && p.armed()),
                3,
            ),
        ];
        // The enemy each unit is judged against: what has been seen, heaviest first;
        // nothing seen, the side's own menu stands in.
        let mut enemy: BTreeMap<BlueprintId, Fx> = BTreeMap::new();
        for contact in &self.state.ai[player as usize].contacts {
            let p = profiles.get(contact.blueprint);
            if p.armed() {
                *enemy.entry(contact.blueprint).or_insert(Fx::ZERO) += p.mass;
            }
        }
        let mut enemy: Vec<(BlueprintId, Fx)> = enemy.into_iter().collect();
        if enemy.is_empty() {
            enemy = menu.iter().map(|&id| (id, Fx::ONE)).collect();
        }
        enemy.sort_by_key(|&(id, m)| (std::cmp::Reverse(m), id));
        enemy.truncate(MATCHED_AGAINST);
        let pl = &self.state.players[player as usize];
        menu.iter()
            .copied()
            .filter_map(|id| {
                let p = profiles.get(id);
                if !usable(p) || best_force.is_some_and(|f| force(p) != Some(f)) {
                    return None;
                }
                let need: i64 = needs.iter().filter(|(f, _)| f(p)).map(|(_, w)| *w).sum();
                if need <= 0 {
                    return None;
                }
                // How it fares, per mass, against what it would meet: units that
                // cannot meet it (a tank and a ship) do not count either way.
                let meets = |e: &Profile| {
                    let reach = |a: &Profile, b: &Profile| b.is.is_some_and(|t| a.hits(t));
                    reach(p, e) || reach(e, p)
                };
                let (mut sum, mut weight) = (Fx::ZERO, Fx::ZERO);
                for &(eid, m) in &enemy {
                    let e = profiles.get(eid);
                    if meets(e) {
                        sum += edge(p, e) * m;
                        weight += m;
                    }
                }
                let fare = if weight > Fx::ZERO {
                    sum / weight
                } else {
                    Fx::ZERO
                };
                // 0..200: an even unit 100.
                let mut score = ((Fx::ONE + fare) * 100).floor_int() as i64 * need;
                let existing = counts.get(&id).copied().unwrap_or(0) as i64;
                score = score * 8 / (8 + existing);
                let bp = self.blueprints.unit(id);
                if bp.cost_energy > (pl.energy + pl.energy_income * 20).max(Fx::from_int(200)) {
                    score /= 3;
                }
                if bp.cost_mass > (pl.mass + pl.mass_income * 20).max(Fx::from_int(100)) {
                    score /= 3;
                }
                // A tier above is worth a little more: it is where the game goes.
                score += p.tech as i64 * 10;
                let tie = (id.0 as u32)
                    .wrapping_mul(1664525)
                    .wrapping_add(serial.wrapping_mul(1013904223))
                    .wrapping_add(player as u32 * 97)
                    % 13;
                Some((id, score + tie as i64))
            })
            .max_by_key(|&(id, score)| (score, std::cmp::Reverse(id.0)))
            .map(|(id, _)| id)
    }

    /// The first structure or site-built unit the plans want (`wants`) that the
    /// builder in `row` can make and the side can fund, and where it goes: anti-air
    /// and guns toward the enemy, the rest behind the factories.
    pub(in crate::ai) fn commander_job(
        &self,
        row: usize,
        start: FxVec2,
        facing: Angle,
    ) -> Option<Job> {
        let player = self.state.units.owner[row];
        let ai = &self.state.ai[player as usize];
        if ai.config.brain != Brain::Commander {
            return None;
        }
        let builds = &self.bp(row).builder.as_ref()?.builds;
        let pl = &self.state.players[player as usize];
        let units = &self.state.units;
        for &id in &ai.commander.wants {
            if !builds.contains(&id) {
                continue;
            }
            let bp = self.blueprints.unit(id);
            // One of each going up at a time.
            let rising = units.slots.iter().any(|r| {
                units.owner[r] == player && units.blueprint[r] == id && !units.is_active(r)
            }) || self.planned_sites(player).any(|(_, o)| o.blueprint == id);
            if rising {
                continue;
            }
            // Paid for over a few minutes of income: a site starts once the side
            // could finish it in that time.
            let seconds = Fx::from_int(240);
            if bp.cost_mass > pl.mass + pl.mass_income * seconds
                || bp.cost_energy > pl.energy + pl.energy_income * seconds
            {
                continue;
            }
            let p = super::profile::Profile::of(bp);
            let toward = FxVec2::from_angle(facing);
            let near = if p.has(role::DEFENSE) || p.hits(Target::Air) && !p.mobile() {
                start + toward * Fx::from_int(160)
            } else {
                start - toward * Fx::from_int(260)
            };
            return Some(Job {
                blueprint: id,
                near,
                heading: bp.build_heading(),
                min_r: Fx::from_int(24),
                keep_off_deposits: true,
                place: Place::Around,
            });
        }
        None
    }

    /// Warheads: the classic launch rule already salvoes past interceptors it can
    /// outnumber and goes round those it cannot (`projects.rs`); the plans decide
    /// whether silos are built, and strikes go for interceptors while warheads wait.
    pub(in crate::ai) fn direct_strategic(
        &mut self,
        ctx: &super::Ctx,
        out: &mut Vec<crate::command::Command>,
    ) {
        self.direct_nukes(ctx.player, out);
    }
}
