//! What the player chooses on the set-up screen (`SurvivalRules`) and the
//! schedule it sets: the tech each round prints at, what the round is worth,
//! its heavy wave, the Shapers raised, and how a Shaper grows (its levels, the
//! catch-up of a reseeded one, its height and visual tier). All of it is a
//! pure function of the rules, the round and the level, so the set-up screen
//! draws the same forecast the match plays. What a level is worth in hit
//! points, shield, Lance and interceptors is the Shaper blueprint's `growth`
//! block (data).
//!
//! Nothing here reacts to the defenders. The engine spends what the schedule
//! says, and what it spends is most of what the defenders can reclaim from
//! the wrecks, so a defender who reclaims well grows with it.

use super::HZ;
use mc_core::Fx;
use mc_data::survival::{Domain, SiteKind};
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
    /// A debug run's, never the set-up screen's: the engine plays on after the defenders
    /// fall, its rounds coming (printing nothing) and its Shapers growing as before, so a
    /// headless run can watch them grow (mc-game `MERIDIAN_SURVIVAL_PLAY_ON`).
    pub play_on: bool,
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
            play_on: false,
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

    /// The level a Shaper that has stood since the first round has reached as
    /// `round` begins.
    pub fn round_level(&self, round: u16) -> u16 {
        (2..=round).fold(1u16, |l, r| {
            l.saturating_add(self.shaper_levels_up(r, 1) as u16)
        })
    }

    /// The level a Shaper raised as `round` begins quickly evolves to
    /// (`CATCH_UP_TICKS` a level): three quarters of one that stood from the
    /// start, so a reseeded Shaper is a match for the round, and one that was
    /// never killed is still the bigger.
    pub fn catch_up_level(&self, round: u16) -> u16 {
        let l = self.round_level(round);
        (l - l / 4).max(1)
    }
}

/// Ticks between levels while a newly raised Shaper evolves to its catch-up level.
pub(super) const CATCH_UP_TICKS: u32 = 15 * HZ;

/// The first round a site whose Shaper died in round `died` may be reseeded in:
/// it stands empty for the rest of that round and the next.
pub(super) fn reseed_round(died: u16) -> u16 {
    died.saturating_add(2)
}

/// Metres a Shaper of `level` stands when fully grown: 60 at level 1, about
/// 700 by level 10 and 1000 by 20, toward 1160.
/// `60 + 1100 * (1 - 0.9^(level - 1))`.
pub fn shaper_height(level: u16) -> Fx {
    let mut left = Fx::ONE;
    // 0.9^200 is below what Fx resolves: a deliberate bound, not a ceiling on the level.
    for _ in 1..level.clamp(1, 200) {
        left *= Fx::ratio(9, 10);
    }
    Fx::from_int(60) + Fx::from_int(1100) * (Fx::ONE - left)
}

/// Metres a Shaper's built height climbs a tick toward its level's height:
/// 3 m/s, so it is seen to build.
pub(super) const CLIMB_PER_TICK: Fx = Fx(3 * Fx::ONE.0 / HZ as i64);

/// Print platforms a Shaper of `level` raises in front of itself: (ordinary,
/// great). One more every third level; great ones, for the big units, from
/// level 8 and one more every sixth. The site's room holds the arcs: past it
/// they stop (`base::slot_at`).
pub fn shaper_platforms(level: u16) -> (usize, usize) {
    let small = 1 + level.saturating_sub(1) as usize / 3;
    let great = if level >= 8 {
        1 + (level - 8) as usize / 6
    } else {
        0
    };
    (small, great)
}

/// Pylons a Shaper of `level` raises at its sides and behind: one every third level.
pub fn shaper_pylons(level: u16) -> usize {
    level as usize / 3
}

/// Ticks a piece of a Shaper's base takes to print.
pub(super) const PIECE_RAISE_TICKS: u32 = 12 * HZ;
/// Ticks between one piece done and the next started.
pub(super) const PIECE_GAP_TICKS: u32 = 4 * HZ;
/// Ticks after a piece is destroyed before the Shaper raises its next.
pub(super) const PIECE_LOST_TICKS: u32 = 45 * HZ;

/// Shield hit points a second `pylons` standing pylons feed their Shaper's dome
/// of `regen`: a quarter of its own regen each.
pub(super) fn pylon_feed(regen: Fx, pylons: usize) -> Fx {
    regen * pylons as i32 / 4
}

/// Metres from its middle to the edge of the foot of a Shaper of `kind` standing
/// `height` metres tall: it grows broad as it grows tall. With grown = `(height - 60) /
/// 1100` (0 at level 1, about 0.61 at level 10 and 0.86 at 20), a printer's is
/// `30 + 170 * grown` (30 m at level 1, 134 m at 10, 177 m at 20), never past 200 m; an
/// auxiliary Shaper's `28 + 150 * grown` (28, 120, 158 m), never past 180 m. Its base's
/// pieces stand out past it (`base::slot_at`). The model is drawn to match
/// (mc-models `shaper::reach`, a test in mc-render holds the two equal).
pub fn shaper_foot(kind: SiteKind, height: Fx) -> Fx {
    let grown = ((height - Fx::from_int(60)) / 1100).clamp(Fx::ZERO, Fx::ONE);
    let (least, spread, most) = if kind.is_auxiliary() {
        (28, 150, 180)
    } else {
        (30, 170, 200)
    };
    (Fx::from_int(least) + grown * spread).min(Fx::from_int(most))
}

/// A Shaper's visual tier: one every four levels, each a stage, collar or ring more.
pub fn shaper_tier(level: u16) -> u8 {
    (1 + level.saturating_sub(1) / 4).min(u8::MAX as u16) as u8
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

/// The first round the ray may raise a Shaper of `kind`: printers from the first, the
/// auxiliary Seedcasters (warp-ins) from the third, Unmakers (nukes) from the sixth and
/// Gatewrights (fleets) from the eighth, so the defenders have a base to lose before the
/// squads come. An `Auxiliary` site may raise from the earliest of them.
pub fn aux_first_round(kind: SiteKind) -> u16 {
    match kind {
        SiteKind::Printer => 1,
        SiteKind::Artillery | SiteKind::Auxiliary => 3,
        SiteKind::Nuke => 6,
        SiteKind::Fleet => 8,
    }
}

/// Squads a Seedcaster of `level` warps in a volley: one, and one more every sixth level.
pub fn squads_per_volley(level: u16) -> usize {
    1 + level.max(1) as usize / 6
}

/// Ticks between a Seedcaster's volleys: 75 s at level 1, quicker as it levels (50 s at
/// 10, 37 s at 20), never under 20 s.
pub fn volley_ticks(level: u16) -> u32 {
    (75 * HZ * 20 / (20 + level.min(1000) as u32)).max(20 * HZ)
}

/// Ticks from a squad's mark being announced to its warping in: 16 s shortened as the
/// Seedcaster levels (15 s at level 1, 12 s at 7), never under 10 s, so the defenders
/// always have time to see it coming.
pub fn warp_in_ticks(level: u16) -> u32 {
    (16 * HZ * 20 / (20 + level.min(1000) as u32)).max(10 * HZ)
}

/// The units one warp-in brings: light units (T1-T3), then heavies. Three light units and
/// one more every other level (one more again when they are T1); from level 8, with a
/// heavy product, one heavy and one more every sixth level.
pub fn squad_size(level: u16, light_tech: u8, has_heavy: bool) -> (usize, usize) {
    let light = 3 + level as usize / 2 + (light_tech <= 1) as usize;
    let heavy = if has_heavy && level >= 8 {
        1 + (level as usize - 8) / 6
    } else {
        0
    };
    (light, heavy)
}

/// The shock where a squad warps in: (radius in metres, damage over it). Modest, the squad
/// is the threat: 40 m and 600 at level 1, two metres more a level up to 100 m, and a
/// tenth more damage a level.
pub fn arrival_shock(level: u16) -> (Fx, Fx) {
    let steps = level.max(1) - 1;
    let radius = Fx::from_int(40 + 2 * steps.min(30) as i32);
    let mut damage = Fx::from_int(600);
    // A bound on the compounding (1.1^200 is past any hull), not on the level.
    for _ in 0..steps.min(200) {
        damage = (damage * Fx::ratio(11, 10)).min(Fx::from_int(1_000_000));
    }
    (radius, damage)
}

/// Ticks a squad's print takes as it warps in, and the share of it already done as it
/// comes out: it arrives two thirds whole and is finished a second later.
pub(super) const WARP_IN_PRINT_TICKS: u32 = 3 * HZ;
pub(super) const WARP_IN_WHOLE: (u32, u32) = (2, 3);

/// Silos an Unmaker of `level` raises behind itself: one, and one more every eighth level.
pub fn shaper_silos(level: u16) -> usize {
    1 + level.max(1).saturating_sub(1) as usize / 8
}

/// Ticks between two warheads out of one Unmaker's silos: 30 s at level 1, quicker as it
/// levels, never under 6 s. Its silos' stock and round time grow by their `growth` block.
pub fn launch_gap_ticks(level: u16) -> u32 {
    (30 * HZ * 20 / (20 + level.min(1000) as u32)).max(6 * HZ)
}

/// Warships one fleet of a Gatewright of `level` holds: (light, capital). Two light ships
/// and one more every fourth level; from level 10 a capital ship, one more every eighth
/// level (capitals only once the rounds print at T4: none is in its roster before).
pub fn fleet_size(level: u16) -> (usize, usize) {
    let light = 2 + level as usize / 4;
    let capital = if level >= 10 {
        1 + (level as usize - 10) / 8
    } else {
        0
    };
    (light, capital)
}

/// Classes a Gatewright of `level` builds from, the cheapest first: (light, capital). The
/// cheapest light class, one bigger every fifth level; from level 10 the cheapest capital
/// class, one bigger every tenth.
pub fn fleet_classes(level: u16) -> (usize, usize) {
    let light = 1 + level as usize / 5;
    let capital = if level >= 10 {
        1 + (level as usize - 10) / 10
    } else {
        0
    };
    (light, capital)
}

/// Ticks a Gatewright rests between one fleet warping in and the next starting to print:
/// 90 s at level 1, quicker as it levels (60 s at 10), never under 40 s.
pub fn fleet_gap_ticks(level: u16) -> u32 {
    (90 * HZ * 20 / (20 + level.min(1000) as u32)).max(40 * HZ)
}

/// Ticks a fleet's mark is announced before its ships start to spool their drives. The
/// spool and the jump follow (`warp.rs`), so the whole warning is longer.
pub const FLEET_LEAD_TICKS: u32 = 20 * HZ;
