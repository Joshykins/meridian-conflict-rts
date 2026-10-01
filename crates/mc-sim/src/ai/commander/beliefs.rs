//! What the side thinks about its enemies (`docs/AI_COMMANDER.md`, "Beliefs"): each
//! a guess with a confidence 0..=100, from what it has seen, what it has not seen
//! where it looked, and the time and income that make a thing likely.
use super::profile::{role, Domain, Profiles};
use super::state::Sticky;
use crate::World;
use mc_core::{Fx, FxVec2};
use mc_data::cat;

/// A confidence, 0..=100.
pub(in crate::ai) type Sure = i32;

#[derive(Clone, Debug, Default)]
pub(in crate::ai) struct Beliefs {
    /// Their mass income, estimated.
    pub income: Fx,
    /// Highest tier seen, and the tier their time and income make likely.
    pub tech: u8,
    /// Enemy armed mobile mass seen lately, by domain (`Domain as usize`).
    pub army: [Fx; 7],
    /// Mass of what holds their ground seen: turrets, map guns, artillery.
    pub fortified: Fx,
    /// Strength of their anti-air seen.
    pub anti_air: Fx,
    pub air: Sure,
    pub navy: Sure,
    pub subs: Sure,
    pub space: Sure,
    /// They have, or are building, a strategic launcher.
    pub nukes: Sure,
    /// Strategic launchers of theirs we know of.
    pub silos: usize,
    /// Interceptor rounds they hold that we know of, and where (with coverage).
    pub interceptors: Vec<(FxVec2, Fx, u32)>,
    /// Sonar coverage seen (structures and ships with sonar).
    pub sonar: usize,
    /// Where their main army was last seen, and its mass.
    pub army_at: Option<(FxVec2, Fx)>,
    /// Ticks since we last had eyes on their start.
    pub base_unseen: u32,
    /// Their army seen lately is small for their income: they spend it elsewhere.
    pub quiet: bool,
}

impl Beliefs {
    pub(in crate::ai) fn army_mass(&self) -> Fx {
        self.army.iter().copied().sum()
    }
}

/// Confidence from evidence seen `age` ticks ago, fading over `fade` ticks to `floor`.
fn fading(age: u32, fade: u32, floor: Sure) -> Sure {
    if age >= fade {
        floor
    } else {
        floor + (100 - floor) * (fade - age) as i32 / fade as i32
    }
}

impl World {
    pub(in crate::ai) fn beliefs(&mut self, player: u8, profiles: &Profiles) -> Beliefs {
        let tick = self.state.tick;
        let mut b = Beliefs::default();
        let mut sticky = self.state.ai[player as usize].commander.sticky;
        let contacts = &self.state.ai[player as usize].contacts;
        let mut mines = 0;
        let mut heaviest: Option<(FxVec2, Fx)> = None;
        let mut clusters: Vec<(FxVec2, Fx)> = Vec::new();
        for c in contacts {
            let bp = self.blueprints.unit(c.blueprint);
            let p = profiles.get(c.blueprint);
            sticky.max_tech = sticky.max_tech.max(bp.tech);
            mines += bp.has(cat::EXTRACTOR) as i32;
            // Guns that hold ground: turrets, map guns, and an artillery park.
            if p.has(role::DEFENSE) || p.has(role::MAP_GUN) || p.has(role::ARTILLERY) {
                b.fortified += bp.cost_mass;
            }
            if p.has(role::ANTI_AIR) {
                b.anti_air += p.dps[2];
            }
            if bp.sonar > Fx::ZERO {
                b.sonar += 1;
            }
            if p.has(role::STRATEGIC) {
                sticky.nukes_seen = tick.max(1);
                b.silos += 1;
            }
            if let Some(s) = bp.strategic.as_ref().filter(|_| p.has(role::INTERCEPTOR)) {
                b.interceptors.push((c.pos, s.coverage, s.stock as u32));
            }
            if !(p.armed() && p.mobile()) || bp.has(cat::COMMANDER | cat::ENGINEER) {
                continue;
            }
            let fresh = tick.saturating_sub(c.seen) <= 1200;
            if !fresh {
                continue;
            }
            if let Some(d) = p.domain {
                b.army[d as usize] += bp.cost_mass;
                match d {
                    Domain::Air => sticky.air_seen = tick,
                    Domain::Naval => sticky.navy_seen = tick,
                    Domain::Sub => sticky.subs_seen = tick,
                    Domain::Space => sticky.space_seen = tick,
                    _ => {}
                }
            }
            // Where most of their army stands: 1.5 km clusters.
            match clusters
                .iter_mut()
                .find(|(at, _)| at.distance(c.pos) < Fx::from_int(1500))
            {
                Some((_, m)) => *m += bp.cost_mass,
                None => clusters.push((c.pos, bp.cost_mass)),
            }
        }
        for &(at, m) in &clusters {
            if heaviest.is_none_or(|(_, h)| m > h) {
                heaviest = Some((at, m));
            }
        }
        b.army_at = heaviest;
        // Naval factories seen say a navy is coming.
        for c in contacts {
            let bp = self.blueprints.unit(c.blueprint);
            if bp.has(cat::FACTORY) && bp.water_build {
                sticky.navy_seen = sticky.navy_seen.max(c.seen);
            }
            if bp.has(cat::FACTORY | cat::AIR) {
                sticky.air_seen = sticky.air_seen.max(c.seen);
            }
        }
        let minutes = (tick / 600) as i32;
        // Income: mines seen at a typical yield, and never less than the time says.
        let by_mines = Fx::from_int(mines * 4 + 2);
        let by_time = Fx::from_int(2 + minutes * 3).min(Fx::from_int(150));
        b.income = by_mines.max(by_time * Fx::ratio(3, 4));
        let expected_tech = match minutes {
            0..=7 => 1,
            8..=15 => 2,
            16..=25 => 3,
            26..=35 => 4,
            _ => 5,
        };
        b.tech = sticky.max_tech.max(expected_tech.min(sticky.max_tech + 1));
        let seen_ago = |t: u32| {
            if t == 0 {
                u32::MAX
            } else {
                tick.saturating_sub(t)
            }
        };
        b.air = fading(seen_ago(sticky.air_seen), 6000, 15);
        b.navy = fading(seen_ago(sticky.navy_seen), 6000, 0);
        b.subs = fading(seen_ago(sticky.subs_seen), 6000, 0);
        b.space = fading(seen_ago(sticky.space_seen), 6000, 0);
        // Their base: how long since anyone looked.
        let start = self
            .state
            .players
            .iter()
            .enumerate()
            .filter(|(i, p)| !p.defeated && self.are_enemies(player, *i as u8))
            .map(|(_, p)| p.start)
            .next();
        let base_seen = start
            .and_then(|s| {
                let m = &self.state.ai[player as usize].commander;
                let (w, _) = super::world_model::WorldModel::dims(self.terrain.size_metres());
                let (x, y) = (
                    s.x.floor_int() / super::world_model::CELL,
                    s.y.floor_int() / super::world_model::CELL,
                );
                m.seen.get((y * w + x).max(0) as usize).copied()
            })
            .unwrap_or(0);
        b.base_unseen = seen_ago(base_seen);
        // Under fog part of an army is unseen: a fifth more than seen, and never
        // less than forty seconds of their income. At ninety seconds of income the
        // believed army outgrew every wave, and the side never attacked.
        let land = Domain::Land as usize;
        b.army[land] += b.army[land] / 5;
        let floor = b.income * Fx::from_int(40);
        if b.army[land] + b.army[Domain::Hover as usize] < floor {
            b.army[land] = floor - b.army[Domain::Hover as usize];
        }
        // Quiet: little army seen for what they earn.
        let spent = b.income * Fx::from_int(240);
        b.quiet = minutes >= 10 && b.army_mass() * 3 < spent;
        // Nukes: certain once seen. Unseen, likelier with time, income, tech and
        // quiet; less likely while we keep looking at their base and see none.
        b.nukes = if sticky.nukes_seen > 0 {
            100
        } else {
            let mut n: Sure = 0;
            if b.tech >= 3 {
                n += 15 + (minutes - 18).clamp(0, 20) * 2;
            }
            if b.income >= Fx::from_int(60) {
                n += 15;
            }
            if b.quiet {
                n += 15;
            }
            if b.base_unseen < 1800 {
                n -= 30;
            }
            n.clamp(0, 90)
        };
        self.state.ai[player as usize].commander.sticky = sticky;
        b
    }
}

impl Sticky {
    pub(in crate::ai) fn hash(&self, h: &mut mc_core::StateHasher) {
        h.write_u64(self.max_tech as u64 | (self.nukes_seen as u64) << 8);
        h.write_u64(self.air_seen as u64 | (self.navy_seen as u64) << 32);
        h.write_u64(self.subs_seen as u64 | (self.space_seen as u64) << 32);
    }
}
