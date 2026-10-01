//! What the Commander has built (`docs/AI_COMMANDER.md`, "Production"): factories
//! make what the plans need, picked from the menu by how well each unit fares
//! against the enemy the side believes in, per mass; builders put up what the
//! plans want (`wants`); and the classic economy code is steered by the plans.
use super::matchup::edge;
use super::profile::{role, Domain, Profile, Profiles};
use super::state::{Hurt, PlanKind, Stake};
use crate::ai::{Job, Place};
use crate::{Brain, World};
use mc_core::{Angle, Fx, FxVec2};
use mc_data::BlueprintId;
use std::collections::BTreeMap;

/// Enemy types, the heaviest by mass seen, a unit is judged against.
const MATCHED_AGAINST: usize = 8;

/// How the Commander steers the classic builders, factories and upgrades: its
/// economy's reading (`economy.rs`).
#[derive(Clone, Copy)]
pub(in crate::ai) struct Directives {
    /// Engineers and factories the side should have.
    pub engineers: usize,
    pub factories: usize,
    pub power: super::economy::Power,
    /// Energy a second short, for how many plants to start at once.
    pub power_short: Fx,
    /// Mine upgrades at once, and the most seconds one may take to pay back.
    pub upgrades: i32,
    pub payback: u32,
    /// Metres from home the commander may work out to (zero: home).
    pub roam: Fx,
    /// How far out builders claim mines.
    pub reach: Fx,
    /// The store is filling: sinks before anything else.
    pub floating: bool,
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
        let e = &ai.commander.eco;
        Some(Directives {
            engineers: e.engineers as usize,
            factories: e.factories as usize,
            power: e.power,
            power_short: e.power_short,
            upgrades: e.upgrades as i32,
            payback: e.payback,
            roam: e.roam,
            reach: e.reach,
            floating: e.floating,
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
            p(PlanKind::AirPower)
                + (p(PlanKind::AirDefense) + 1) / 2
                // Hit from above: fighters to hunt what the guns cannot reach.
                + (c.hurt[Hurt::Air as usize] + c.hurt[Hurt::Space as usize])
                    .floor_int() as i64
                    / 1500,
            p(PlanKind::SeaControl) + if land_route { 0 } else { 2 },
            p(PlanKind::SubWar),
        ]
    }

    /// The combat unit an idle factory of `player` should make, from `menu`: first
    /// the force furthest under its share (`force_shares`), then the role in that
    /// force furthest under its share of the force's mass (anti-air by the enemy's
    /// air, artillery by its defences and the siege plan, raiders by the raid plan,
    /// line units for the rest), then the unit for that role that fares best against
    /// the enemy the side believes in, per mass. Picking the single best need every
    /// time built no anti-air at all: line units always scored higher, and one
    /// enemy warship killed a 40-tank army.
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
        let air_seen = c.sticky.air_seen > 0 || c.sticky.space_seen > 0;
        let landing = stake(PlanKind::Landing);
        // The enemy seen, by mass: air (and warships), defences, all.
        let (mut air, mut forts, mut all) = (Fx::ZERO, Fx::ZERO, Fx::ZERO);
        let mut enemy: BTreeMap<BlueprintId, Fx> = BTreeMap::new();
        for contact in &self.state.ai[player as usize].contacts {
            let p = profiles.get(contact.blueprint);
            if !p.armed() {
                continue;
            }
            *enemy.entry(contact.blueprint).or_insert(Fx::ZERO) += p.mass;
            all += p.mass;
            if matches!(p.domain, Some(Domain::Air | Domain::Space)) {
                air += p.mass;
            }
            if p.has(role::DEFENSE) {
                forts += p.mass;
            }
        }
        let share = |m: Fx| (m * 1000 / all.max(Fx::ONE)).floor_int() as i64;
        let (air_share, fort_share) = (share(air), share(forts));
        // What has been killing it, in thousandths of its recent losses: the side
        // answers what hurts it, not only what it has seen.
        let hurt = c.hurt;
        let lost: Fx = hurt.iter().copied().sum::<Fx>().max(Fx::from_int(300));
        let hurt_by = |k: Hurt| (hurt[k as usize] * 1000 / lost).floor_int() as i64;
        let from_above = hurt_by(Hurt::Air) + hurt_by(Hurt::Space);
        let air_share = air_share.max(from_above);
        let fort_share = fort_share.max(hurt_by(Hurt::Artillery) + hurt_by(Hurt::Static));
        // Roles within each force and the share (per mille) of its mass each
        // should hold; the first role a unit fits is the one it counts toward.
        type Fits = Box<dyn Fn(&Profile) -> bool>;
        let aa_land = (air_share * 7 / 10).min(400)
            + if air_seen { 80 } else { 0 }
            + 60 * stake(PlanKind::AirDefense);
        let roles: [Vec<(Fits, i64)>; FORCES] = [
            vec![
                (Box::new(|p: &Profile| p.has(role::ANTI_AIR)), aa_land),
                (
                    Box::new(|p: &Profile| p.has(role::ARTILLERY)),
                    120 + 100 * stake(PlanKind::Siege) + (fort_share * 3 / 10).min(200),
                ),
                (
                    Box::new(|p: &Profile| p.has(role::RAIDER) && !p.has(role::LINE)),
                    100 * stake(PlanKind::Raid),
                ),
                (Box::new(|p: &Profile| p.has(role::LINE)), 300),
            ],
            vec![
                (
                    Box::new(|p: &Profile| p.has(role::ANTI_AIR)),
                    (air_share * 8 / 10).clamp(150, 700) + 100 * stake(PlanKind::AirDefense),
                ),
                (
                    Box::new(|p: &Profile| p.has(role::STRIKE)),
                    400 + 150 * stake(PlanKind::AirPower),
                ),
            ],
            vec![
                (
                    Box::new(|p: &Profile| p.has(role::ANTI_AIR)),
                    (air_share * 6 / 10).min(400),
                ),
                (Box::new(|p: &Profile| p.armed()), 600),
            ],
            vec![(Box::new(|p: &Profile| p.armed()), 1000)],
        ];
        let usable = |p: &Profile| {
            force(p).is_some()
                && p.armed()
                && (land_route || landing > 0 || !matches!(p.domain, Some(Domain::Land)))
        };
        // Which force: the furthest under its share of the plans' stakes.
        let shares = self.force_shares(player);
        let mut have = [Fx::ZERO; FORCES];
        let mut have_role: [Vec<Fx>; FORCES] =
            std::array::from_fn(|f| vec![Fx::ZERO; roles[f].len()]);
        let units = &self.state.units;
        for r in units.slots.iter() {
            if units.owner[r] != player {
                continue;
            }
            let p = profiles.get(units.blueprint[r]);
            let Some(f) = force(p).filter(|_| p.armed()) else {
                continue;
            };
            have[f] += p.mass;
            if let Some(i) = roles[f].iter().position(|(fits, _)| fits(p)) {
                have_role[f][i] += p.mass;
            }
        }
        let total_share: i64 = shares.iter().sum::<i64>().max(1);
        let total_have: Fx = have.iter().copied().sum::<Fx>().max(Fx::ONE);
        let deficit = |f: usize| -> i64 {
            shares[f] * 1000 / total_share - (have[f] * 1000 / total_have).floor_int() as i64
        };
        let cands: Vec<&Profile> = menu
            .iter()
            .map(|&id| profiles.get(id))
            .filter(|p| usable(p))
            .collect();
        let f = cands
            .iter()
            .filter_map(|p| force(p))
            .filter(|&f| shares[f] > 0)
            .max_by_key(|&f| (deficit(f), std::cmp::Reverse(f)))?;
        // Which role in it: the furthest under its share that this factory makes.
        let target: i64 = roles[f].iter().map(|(_, t)| *t).sum::<i64>().max(1);
        let in_force = have[f].max(Fx::ONE);
        let role_at = |p: &Profile| roles[f].iter().position(|(fits, _)| fits(p));
        let slot = (0..roles[f].len())
            .filter(|&i| {
                roles[f][i].1 > 0
                    && cands
                        .iter()
                        .any(|p| force(p) == Some(f) && role_at(p) == Some(i))
            })
            .max_by_key(|&i| {
                let want = roles[f][i].1 * 1000 / target;
                let got = (have_role[f][i] * 1000 / in_force).floor_int() as i64;
                (want - got, std::cmp::Reverse(i))
            });
        // The enemy each unit is judged against: the heaviest seen; nothing seen,
        // the side's own menu stands in.
        let mut enemy: Vec<(BlueprintId, Fx)> = enemy.into_iter().collect();
        if enemy.is_empty() {
            enemy = menu.iter().map(|&id| (id, Fx::ONE)).collect();
        }
        enemy.sort_by_key(|&(id, m)| (std::cmp::Reverse(m), id));
        enemy.truncate(MATCHED_AGAINST);
        let pl = &self.state.players[player as usize];
        cands
            .iter()
            .filter(|p| force(p) == Some(f) && (slot.is_none() || role_at(p) == slot))
            .map(|p| {
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
                let mut score = ((Fx::ONE + fare) * 100).floor_int() as i64;
                let existing = counts.get(&p.id).copied().unwrap_or(0) as i64;
                score = score * 8 / (8 + existing);
                let bp = self.blueprints.unit(p.id);
                if bp.cost_energy > (pl.energy + pl.energy_income * 20).max(Fx::from_int(200)) {
                    score /= 3;
                }
                if bp.cost_mass > (pl.mass + pl.mass_income * 20).max(Fx::from_int(100)) {
                    score /= 3;
                }
                // A tier above is worth more: it is where the game goes.
                score += p.tech as i64 * 25;
                let tie = (p.id.0 as u32)
                    .wrapping_mul(1664525)
                    .wrapping_add(serial.wrapping_mul(1013904223))
                    .wrapping_add(player as u32 * 97)
                    % 13;
                (p.id, score + tie as i64)
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
            // A coastal gun by the side's sea mine nearest the enemy; other guns
            // toward the enemy; the rest behind the factories.
            let sea_mine = || {
                units
                    .slots
                    .iter()
                    .filter(|&r| {
                        units.owner[r] == player
                            && self.bp(r).mine.is_some()
                            && self.ore.at_sea(units.pos[r])
                    })
                    .map(|r| units.pos[r])
                    .min_by_key(|m| (m.distance_sq(start + toward * Fx::from_int(4000)), m.x, m.y))
            };
            let near = if p.has(role::ANTI_SHIP) && p.has(role::DEFENSE) {
                match sea_mine() {
                    Some(m) => m + toward * Fx::from_int(80),
                    None => continue,
                }
            } else if p.has(role::ANTI_AIR) && !p.mobile() {
                // Where the bombers last hit, else by the factories.
                match ai.commander.hit_from_above {
                    Some((at, t)) if self.state.tick < t + 1800 => at + toward * Fx::from_int(40),
                    _ => start - toward * Fx::from_int(120),
                }
            } else if p.has(role::DEFENSE) {
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
