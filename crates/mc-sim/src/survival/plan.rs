//! What a round is made of: formations, each printed on one bay and sent
//! together, and the round's heavy wave (squads of experimentals, one kind of
//! unit to a squad).

use super::rules::HeavyWave;
use super::{domain_of, Survival};
use mc_core::Rng;
use mc_data::survival::Domain;
use mc_data::{cat, BlueprintId, FactionId};
use serde::{Deserialize, Serialize};

/// Most units of a round's ordinary formations, and separately of its heavy
/// wave: the field never holds more than this at once (`FIELD_CAP`), so a
/// bigger plan could never come at once anyway. A deliberate guard on how much
/// a late endless round plans, not a limit a round reaches before about round
/// 40; the HUD shows the field cap holding the printing back.
const PLAN_CAP: usize = super::FIELD_CAP;

/// `Formation::front` of one no front leads from: only a Shaper's platform takes it.
pub(super) const NO_FRONT: u8 = u8::MAX;

/// One formation: printed on one bay at once, mustered and sent as one block.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct Formation {
    pub(super) units: Vec<BlueprintId>,
    /// Index into `SurvivalConfig::fronts`, or `NO_FRONT`.
    pub(super) front: u8,
    /// A squad of the round's heavy wave.
    pub(super) heavy: bool,
}

/// A unit the engine can send: blueprint, faction, tech, materials.
#[derive(Clone, Copy)]
struct Fighter {
    id: BlueprintId,
    faction: FactionId,
    tech: u8,
    cost: i64,
}

/// Shares of a round's budget: land carries most, then air, then the sea.
pub(super) fn domain_weight(d: Domain) -> i64 {
    match d {
        Domain::Land => 60,
        Domain::Air => 25,
        Domain::Naval => 20,
    }
}

impl crate::World {
    /// The formations of `round`, thrown with `rng`: the engine's own dice for
    /// the round it prints, dice of their own for a forecast.
    pub(super) fn plan_round(&self, s: &Survival, round: u16, rng: &mut Rng) -> Vec<Formation> {
        let rules = s.config.rules;
        let tier = rules.tier_at(round);
        let lanes = |d: Domain| -> Vec<u8> {
            s.config
                .fronts
                .iter()
                .enumerate()
                .filter(|(_, f)| f.domain == d && rules.has(d))
                .map(|(i, _)| i as u8)
                .collect()
        };
        // Land and air formations print in the facility's bays and go down a
        // front. Ships print on the sea gate's slips (down a sea front) or a
        // naval Shaper's: while neither stands, a round has no naval share.
        let naval_pads = s.nodes.iter().any(|n| {
            s.config
                .node_sites
                .get(n.site as usize)
                .is_some_and(|site| site.domain == Domain::Naval)
                && !self.shaper_pads(n).is_empty()
        });
        let naval_bays = s.config.bays.iter().any(|b| b.domain == Domain::Naval);
        let open = |d: Domain| match d {
            Domain::Naval => rules.has(d) && ((naval_bays && !lanes(d).is_empty()) || naval_pads),
            _ => !lanes(d).is_empty(),
        };
        // A formation with no front of its domain is one only a Shaper's platform takes.
        let pick_lane = |d: Domain, rng: &mut Rng| {
            let lane = lanes(d);
            if lane.is_empty() {
                NO_FRONT
            } else {
                lane[rng.below(lane.len() as u32) as usize]
            }
        };
        let domains: Vec<Domain> = Domain::ALL
            .into_iter()
            .filter(|d| open(*d) && !self.roster(*d, tier).is_empty())
            .collect();
        let mut out = Vec::new();
        if let Some(wave) = rules.heavy_wave(round) {
            let open: Vec<Domain> = Domain::ALL.into_iter().filter(|d| open(*d)).collect();
            for squad in self.plan_heavy_wave(&wave, &open, rng) {
                let front = pick_lane(domain_of_bp(self, squad.0), rng);
                out.push(Formation {
                    units: vec![squad.0; squad.1],
                    front,
                    heavy: true,
                });
            }
        }
        let budget = rules.formation_budget(round);
        let total: i64 = domains.iter().map(|d| domain_weight(*d)).sum();
        let size = rules.formation_size(round);
        let mut planned = 0;
        for d in &domains {
            let mut left = budget * domain_weight(*d) / total.max(1);
            let roster = self.roster(*d, tier);
            let mut factions: Vec<FactionId> = roster.iter().map(|u| u.faction).collect();
            factions.sort_by_key(|f| f.0);
            factions.dedup();
            let mut first = true;
            while left > 0 && planned < PLAN_CAP {
                // Most formations are one faction's; one in four mixes them.
                let pool: Vec<Fighter> = if factions.len() > 1 && rng.below(4) == 0 {
                    roster.clone()
                } else {
                    let f = factions[rng.below(factions.len() as u32) as usize];
                    roster.iter().copied().filter(|u| u.faction == f).collect()
                };
                let kinds = pick_kinds(&pool, rng);
                let mut units = Vec::new();
                let mut k = 0;
                while units.len() < size && planned + units.len() < PLAN_CAP {
                    let u = kinds[k % kinds.len()];
                    k += 1;
                    // The domain's first formation always gets its first unit,
                    // so a lean budget still sends something.
                    if u.cost > left && !(first && units.is_empty()) {
                        break;
                    }
                    left -= u.cost;
                    units.push(u.id);
                }
                if units.is_empty() {
                    break;
                }
                first = false;
                planned += units.len();
                out.push(Formation {
                    units,
                    front: pick_lane(*d, rng),
                    heavy: false,
                });
            }
        }
        out
    }

    /// The squads of a heavy wave: (blueprint, how many). The wave splits into
    /// one to a few squads, more as it grows, each one kind of experimental of
    /// one faction; each squad is T5s at `t5_share` once T5 is open.
    fn plan_heavy_wave(
        &self,
        wave: &HeavyWave,
        domains: &[Domain],
        rng: &mut Rng,
    ) -> Vec<(BlueprintId, usize)> {
        let pool: Vec<Fighter> = domains
            .iter()
            .flat_map(|d| self.heavies(*d))
            .filter(|u| u.tech <= wave.top)
            .collect();
        if pool.is_empty() {
            return Vec::new();
        }
        // Heavies are counted at no less than the middle T4's price, so a cheap
        // experimental comes in the same numbers as the rest.
        let mut t4: Vec<i64> = pool
            .iter()
            .filter(|u| u.tech == 4)
            .map(|u| u.cost)
            .collect();
        t4.sort_unstable();
        let par = t4
            .get(t4.len() / 2)
            .copied()
            .unwrap_or_else(|| pool.iter().map(|u| u.cost).min().unwrap_or(1))
            .max(1);
        let floor = wave.floor as usize;
        let worth = (wave.value / par) as usize;
        let count = floor.max(worth).min(PLAN_CAP);
        let squads = (1 + rng.below(1 + count as u32 / 4) as usize).min(count);
        let per_value = wave.value / squads as i64;
        let per_floor = floor.div_ceil(squads);
        let mut out = Vec::new();
        let mut sent = 0;
        for _ in 0..squads {
            let t5 = wave.t5_share > 0 && rng.below(100) < wave.t5_share;
            let want = if t5 { 5 } else { 4 };
            let pick: Vec<&Fighter> = pool.iter().filter(|u| u.tech == want).collect();
            let pick = if pick.is_empty() {
                pool.iter().collect()
            } else {
                pick
            };
            let u = pick[rng.below(pick.len() as u32) as usize];
            let n = (per_value / u.cost.max(par)) as usize;
            // A squad of T4s meets the floor; a T5 counts for more and comes as it is paid for.
            let n = if u.tech >= 5 {
                n.max(1)
            } else {
                n.max(per_floor)
            };
            let n = n.min(PLAN_CAP - sent);
            if n == 0 {
                break;
            }
            sent += n;
            out.push((u.id, n));
        }
        out
    }

    /// Fighting units of `domain` up to `tier`, of any faction. Experimentals
    /// are not in it: they come in the heavy waves (`heavies`).
    pub(super) fn roster_ids(&self, domain: Domain, tier: u8) -> Vec<(BlueprintId, u8)> {
        self.roster(domain, tier)
            .into_iter()
            .map(|u| (u.id, u.tech))
            .collect()
    }

    /// The T4 and T5 experimentals of `domain` up to `tier`.
    pub(super) fn heavy_ids(&self, domain: Domain, tier: u8) -> Vec<BlueprintId> {
        self.heavies(domain)
            .into_iter()
            .filter(|u| u.tech <= tier)
            .map(|u| u.id)
            .collect()
    }

    fn roster(&self, domain: Domain, tier: u8) -> Vec<Fighter> {
        self.fighters(domain, false)
            .into_iter()
            .filter(|u| u.tech <= tier)
            .collect()
    }

    fn heavies(&self, domain: Domain) -> Vec<Fighter> {
        self.fighters(domain, true)
            .into_iter()
            .filter(|u| u.tech >= 4)
            .collect()
    }

    fn fighters(&self, domain: Domain, experimental: bool) -> Vec<Fighter> {
        let bps = self.blueprints.as_ref();
        let skip = cat::COMMANDER | cat::ENGINEER | cat::SCOUT | cat::REPLICATOR;
        bps.units
            .iter()
            .filter(|u| {
                bps.is_listed(u.id)
                    && (u.categories & cat::EXPERIMENTAL != 0) == experimental
                    && u.categories & skip == 0
                    && !u.weapons.is_empty()
                    // Transports carry, they do not fight: the engine never sends one.
                    && u.transport.is_none()
                    && u.motion.is_some_and(|m| domain_of(m.layer) == domain)
                    && !bps.units.iter().any(|o| o.drone == Some(u.id))
            })
            .map(|u| Fighter {
                id: u.id,
                faction: u.faction,
                tech: u.tech,
                cost: u.cost_materials.round_int().max(1) as i64,
            })
            .collect()
    }
}

/// The kinds of unit one formation is made of: a core of the newest tier the
/// pool has, and up to two more, mostly of that tier, some of the ones below.
fn pick_kinds(pool: &[Fighter], rng: &mut Rng) -> Vec<Fighter> {
    let top = pool.iter().map(|u| u.tech).max().unwrap_or(1);
    let newest: Vec<Fighter> = pool.iter().copied().filter(|u| u.tech == top).collect();
    let older: Vec<Fighter> = pool.iter().copied().filter(|u| u.tech < top).collect();
    let mut kinds = vec![newest[rng.below(newest.len() as u32) as usize]];
    for _ in 0..rng.below(3) {
        let from = if older.is_empty() || rng.below(10) < 7 {
            &newest
        } else {
            &older
        };
        let u = from[rng.below(from.len() as u32) as usize];
        if !kinds.iter().any(|k| k.id == u.id) {
            kinds.push(u);
        }
    }
    kinds
}

/// The domain a unit fights in.
pub(super) fn domain_of_bp(w: &crate::World, bp: BlueprintId) -> Domain {
    w.blueprints
        .unit(bp)
        .motion
        .map_or(Domain::Land, |m| domain_of(m.layer))
}
