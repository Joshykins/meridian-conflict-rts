//! What the player chooses on the set-up screen (`SurvivalRules`) and the
//! schedule it sets: the tech each round prints at, what the round is worth,
//! its heavy wave, the Shapers raised, and how a Shaper grows. All of it is a
//! pure function of the rules and the round, so the set-up screen draws the
//! same forecast the match plays.
//!
//! Nothing here reacts to the defenders. The engine spends what the schedule
//! says, and what it spends is most of what the defenders can reclaim from
//! the wrecks, so a defender who reclaims well grows with it.

use mc_data::survival::Domain;
use serde::{Deserialize, Serialize};

/// What the player chooses on the survival set-up screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SurvivalRules {
    /// Rounds to survive; zero is endless.
    pub rounds: u16,
    /// Seconds before the first round starts printing.
    pub grace_secs: u16,
    /// Seconds from one round's launch to the next round's printing.
    pub interval_secs: u16,
    /// Round size, in thousandths of standard.
    pub intensity: u16,
    /// Which fronts attack: `Domain::bit`s.
    pub fronts: u8,
    /// The highest tech the engine replicates.
    pub tier_cap: u8,
    /// Replication nodes: 0 none, 1 rare, 2 regular, 3 frequent.
    pub nodes: u8,
}

impl Default for SurvivalRules {
    fn default() -> Self {
        SurvivalRules {
            rounds: 15,
            grace_secs: 240,
            interval_secs: 150,
            intensity: 1000,
            fronts: 7,
            tier_cap: 3,
            nodes: 2,
        }
    }
}

/// A round's heavy wave: squads of T4 and T5 experimentals, each squad one
/// kind of unit, printed with the round and sent with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeavyWave {
    /// Materials the wave is worth, taken out of the round's budget.
    pub value: i64,
    /// Fewest heavies the wave holds however lean `value` is, by intensity.
    pub floor: u16,
    /// The highest tech it takes: 4, or 5 once the rounds print at T5.
    pub top: u8,
    /// Chance in percent that a squad is of T5s (0 below T5).
    pub t5_share: u32,
}

/// Endless rounds the schedule looks ahead through for the first round of a tier.
const LOOK_AHEAD: u16 = 1000;

impl SurvivalRules {
    pub fn has(&self, domain: Domain) -> bool {
        self.fronts & domain.bit() != 0
    }

    /// The tech the engine prints at in `round` (1-based).
    pub fn tier_at(&self, round: u16) -> u8 {
        let cap = self.tier_cap.clamp(1, mc_data::MAX_TECH) as u32;
        let r = round.max(1) as u32 - 1;
        // Endless climbs a tier every six rounds.
        let tier = if self.rounds == 0 {
            1 + r / 6
        } else {
            1 + r * cap / self.rounds as u32
        };
        tier.min(cap) as u8
    }

    /// The first round that prints at `tier` or above, if one does.
    pub fn first_round_of(&self, tier: u8) -> Option<u16> {
        let last = if self.rounds == 0 {
            LOOK_AHEAD
        } else {
            self.rounds
        };
        (1..=last).find(|r| self.tier_at(*r) >= tier)
    }

    /// Materials the engine spends on `round`: 800 at the first, a quarter
    /// more each round after, with no ceiling. Tiers cost about four times the
    /// one below, so this keeps the rounds' numbers up as the tiers climb.
    pub fn budget(&self, round: u16) -> i64 {
        let mut materials: i64 = 800;
        for _ in 1..round.clamp(1, 200) {
            materials = (materials * 5 / 4).min(1 << 40);
        }
        materials * self.intensity as i64 / 1000
    }

    /// The heavy wave of `round`, if it has one. Heavy waves start with the
    /// first T4 round and come every third round, then every other from six
    /// rounds on and every round from twelve (all of it six rounds sooner at
    /// Onslaught and above). A wave takes a quarter of the round's budget,
    /// two points more each round after the first T4 one, up to half: it grows
    /// with the rounds and has no count of its own.
    pub fn heavy_wave(&self, round: u16) -> Option<HeavyWave> {
        let round = round.max(1);
        let first = self.first_round_of(4)?;
        if round < first {
            return None;
        }
        let since = round - first;
        let pace = since + if self.intensity >= 2200 { 6 } else { 0 };
        let every = match pace {
            0..=5 => 3,
            6..=11 => 2,
            _ => 1,
        };
        if !since.is_multiple_of(every) {
            return None;
        }
        let share = (25 + 2 * since as i64).min(50);
        let top = self.tier_at(round);
        let t5_share = match self.first_round_of(5) {
            Some(f5) if top >= 5 => (30 + 5 * (round - f5) as u32).min(80),
            _ => 0,
        };
        Some(HeavyWave {
            value: self.budget(round) * share / 100,
            floor: (self.intensity as u32 * 2 / 1000).max(1) as u16,
            top,
            t5_share,
        })
    }

    /// What the round's ordinary formations share: its budget less its heavy wave.
    pub fn formation_budget(&self, round: u16) -> i64 {
        self.budget(round) - self.heavy_wave(round).map_or(0, |w| w.value)
    }

    /// Units in one of the round's ordinary formations: six at the first, one
    /// more every other round, up to 24. One formation is printed on one bay's
    /// apron, so a bigger round brings more formations, not bigger ones.
    pub fn formation_size(&self, round: u16) -> usize {
        (6 + round as usize / 2).min(24)
    }

    /// Shapers the facility starts raising as `round` begins: it wakes as the
    /// rounds climb, so more come each round (up to `node_limit` standing).
    pub fn nodes_raised(&self, round: u16) -> usize {
        let r = round as usize;
        match self.nodes {
            0 => 0,
            // One every third round from the third.
            1 => (r >= 3 && (r - 3).is_multiple_of(3)) as usize,
            // One a round from the second, two from the seventh, three from the twelfth.
            2 if r >= 2 => 1 + (r >= 7) as usize + (r >= 12) as usize,
            2 => 0,
            // From the first: one, two from the fourth, three from the eighth, four from the twelfth.
            _ if r >= 1 => 1 + (r >= 4) as usize + (r >= 8) as usize + (r >= 12) as usize,
            _ => 0,
        }
    }

    /// Most Shapers standing at once.
    pub fn node_limit(&self) -> usize {
        match self.nodes {
            0 => 0,
            1 => 6,
            2 => 12,
            _ => 18,
        }
    }

    /// Whether a Shaper that came online in round `born` gains a level as
    /// `round` begins: every second round it stands, every round from the
    /// twentieth. Its level has no ceiling.
    pub fn shaper_levels_up(&self, round: u16, born: u16) -> bool {
        let age = round.saturating_sub(born);
        age > 0 && (round >= 20 || age.is_multiple_of(2))
    }
}

/// What a Shaper of `level` prints in one go: light units (T1-T3), then heavies.
/// Two light units and one more a level (one more again when they are T1);
/// from level 8, with a heavy product, one heavy and one more every four levels.
pub(super) fn shaper_batch(level: u16, light_tech: u8, has_heavy: bool) -> (usize, usize) {
    let light = 2 + level as usize + (light_tech <= 1) as usize;
    let heavy = if has_heavy && level >= 8 {
        1 + (level as usize - 8) / 4
    } else {
        0
    };
    (light, heavy)
}

/// Whether a Shaper reaching `level` takes on a new light product (every other level from 3).
pub(super) fn shaper_gains_product(level: u16) -> bool {
    level >= 3 && level % 2 == 1
}

/// Whether a Shaper reaching `level` takes on a heavy product (8, then every four levels).
pub(super) fn shaper_gains_heavy(level: u16) -> bool {
    level >= 8 && (level - 8).is_multiple_of(4)
}
