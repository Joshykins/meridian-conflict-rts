//! What a side sets out to do: the gambits it commits to for a few minutes at
//! a time, chosen from what it has seen of the enemy and what it can build.
//!
//! Without them every AI played the same game: mines, power, factories and
//! waves of whatever ranked best. Now each side also holds a few plans, one to
//! three by difficulty (`Skill::gambits`), and its builders, factories and
//! army serve them: a landing by lift ship jumped through warp, a warship
//! raiding the outskirts, bombers kept back until a big wing can go at the
//! enemy's unguarded economy, a squad hunting engineers, map guns under
//! shields, a nuke when the enemy has no interceptor, submarines when it has no
//! sonar, or more scouting when it knows too little.
//!
//! The first plans are chosen once the scouts have had time to look
//! (`FIRST_REVIEW`). A plan is held at least `REVIEW` ticks and dropped on a
//! later review once what it needs is gone or the enemy has answered it
//! (`score` falls to zero). A held plan gets a bonus for its first
//! `COMMIT_REVIEWS` reviews, so a side commits rather than flipping, and a
//! growing penalty after that, so a long game sees it try something else. A
//! small per-match salt makes sides with the same doctrine pick differently.
use super::*;

/// Ticks between reviews of a side's plans: two minutes.
const REVIEW: u32 = 1200;
/// Tick of the first review: four minutes in, once the scouts have looked.
const FIRST_REVIEW: u32 = 2400;
/// Score a plan already held keeps over a new one, for its first reviews.
const HOLD_BONUS: i32 = 60;
const COMMIT_REVIEWS: u8 = 3;
/// Score a plan loses for each review it has been held past those.
const STALE: i32 = 25;
/// Least a plan must score on what the side has seen, before its hold bonus and
/// salt, to be taken up: a slot left empty costs nothing, a weak plan held
/// for want of a better one cost the first air strikes and a factory.
const MIN_SCORE: i32 = 40;
/// Most a match's salt adds to a plan's score: enough to break near ties
/// between sides, not to pick a plan the situation argues against.
const SALT_SPREAD: u32 = 45;
/// Enemy anti-air (by count of contacts) at or below which the enemy is open
/// to air and to warships.
const FEW_AA: usize = 3;
/// A contact this far from its owner's start is out on the edges, where a
/// raid finds it alone.
const OUTSKIRTS: Fx = Fx::from_int(900);

/// A plan a side can commit to. The number is its bit in `Strategy::active`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub(super) enum Gambit {
    /// Land units carried by lift ship, jumped beside an enemy weak spot through
    /// warp and let out there (`landing.rs`).
    Landing = 0,
    /// An armed spaceship jumps onto the enemy's unguarded outskirts and jumps
    /// home when hurt (`warp_ops.rs`).
    WarpRaid = 1,
    /// Bombers held back behind the base until a big wing is ready, then sent
    /// at the enemy's unguarded economy (`groups.rs`).
    AirFleet = 2,
    /// A fast squad hunts the enemy's engineers and outlying mines.
    Hunt = 3,
    /// Map guns first among the projects, two at a time, under shields.
    Siege = 4,
    /// A nuke first among the projects: the enemy has no interceptor.
    NukeRace = 5,
    /// Submarines: the enemy has no sonar to find them.
    Submarines = 6,
    /// More scouts, and the sensor ship jumping about the map.
    Scouting = 7,
}

const ALL: [Gambit; 8] = [
    Gambit::Landing,
    Gambit::WarpRaid,
    Gambit::AirFleet,
    Gambit::Hunt,
    Gambit::Siege,
    Gambit::NukeRace,
    Gambit::Submarines,
    Gambit::Scouting,
];

impl Gambit {
    pub(super) fn name(self) -> &'static str {
        match self {
            Gambit::Landing => "landing",
            Gambit::WarpRaid => "warp_raid",
            Gambit::AirFleet => "air_fleet",
            Gambit::Hunt => "hunt",
            Gambit::Siege => "siege",
            Gambit::NukeRace => "nuke_race",
            Gambit::Submarines => "submarines",
            Gambit::Scouting => "scouting",
        }
    }
}

/// The plans a side holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Strategy {
    /// Bit `g` set: the side holds `Gambit` number `g`.
    pub active: u16,
    /// Tick of the next review.
    pub next_review: u32,
    /// Reviews so far.
    pub reviews: u32,
    /// Drawn once per match: sides with the same doctrine pick differently.
    pub salt: u32,
    /// Reviews each plan has been held in a row.
    pub held: [u8; 8],
}

impl Strategy {
    pub(super) fn holds(&self, g: Gambit) -> bool {
        self.active >> g as u8 & 1 != 0
    }

    pub(super) fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.active as u64 | (self.reviews as u64) << 16 | (self.salt as u64) << 32);
        h.write_u64(self.next_review as u64);
        h.write_u64(u64::from_le_bytes(self.held));
    }

    /// The names of the plans held, for reports.
    pub(super) fn names(&self) -> String {
        let names: Vec<&str> = ALL
            .iter()
            .filter(|g| self.holds(**g))
            .map(|g| g.name())
            .collect();
        if names.is_empty() {
            "-".into()
        } else {
            names.join("+")
        }
    }
}

/// What the side can build now, from every builder and factory it has.
pub(super) struct Menu {
    pub ids: Vec<BlueprintId>,
}

impl Menu {
    pub(super) fn any(&self, w: &World, f: impl Fn(&UnitBlueprint) -> bool) -> bool {
        self.ids.iter().any(|&id| f(w.blueprints.unit(id)))
    }
}

/// An armed spaceship with a warp drive.
pub(super) fn warship(bp: &UnitBlueprint) -> bool {
    bp.has(cat::SPACE)
        && bp.is_mobile()
        && bp.transport.is_none()
        && bp.warp.is_some()
        && !bp.weapons.is_empty()
}

/// An unarmed spaceship with radar and a warp drive (the Vigil).
pub(super) fn sensor_ship(bp: &UnitBlueprint) -> bool {
    bp.has(cat::SPACE)
        && bp.is_mobile()
        && bp.weapons.is_empty()
        && bp.radar > Fx::ZERO
        && bp.warp.is_some()
}

/// Whether some weapon of `bp` reaches aircraft.
pub(super) fn shoots_air(bp: &UnitBlueprint) -> bool {
    bp.weapons.iter().any(|w| w.target_mask & cat::AIR != 0)
}

impl World {
    /// Every blueprint some builder or factory of `player` has in its menu.
    pub(super) fn side_menu(&self, player: u8) -> Menu {
        let units = &self.state.units;
        let mut ids: Vec<BlueprintId> = units
            .slots
            .iter()
            .filter(|&r| units.owner[r] == player && units.is_active(r))
            .filter_map(|r| self.bp(r).builder.as_ref())
            .flat_map(|b| b.builds.iter().copied())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        Menu { ids }
    }

    /// Whether `player` holds `g`.
    pub(super) fn holds(&self, player: u8, g: Gambit) -> bool {
        self.state.ai[player as usize].strategy.holds(g)
    }

    /// Enemy contacts `player` remembers that shoot at aircraft.
    pub(super) fn enemy_anti_air(&self, player: u8) -> usize {
        self.state.ai[player as usize]
            .contacts
            .iter()
            .filter(|c| shoots_air(self.blueprints.unit(c.blueprint)))
            .count()
    }

    /// Strength of the remembered enemy anti-air within `r` of `at`.
    pub(super) fn anti_air_near(&self, player: u8, at: FxVec2, r: Fx) -> i64 {
        self.state.ai[player as usize]
            .contacts
            .iter()
            .filter(|c| c.pos.distance(at) <= r)
            .map(|c| self.blueprints.unit(c.blueprint))
            .filter(|bp| shoots_air(bp))
            .map(adaptive::strength)
            .sum()
    }

    /// The start of the enemy nearest `at`.
    pub(super) fn enemy_start_near(&self, player: u8, at: FxVec2) -> Option<FxVec2> {
        self.state
            .players
            .iter()
            .enumerate()
            .filter(|(i, p)| !p.defeated && self.are_enemies(player, *i as u8))
            .map(|(_, p)| p.start)
            .min_by_key(|s| (s.distance_sq(at), s.x, s.y))
    }

    /// Reviews `player`'s plans when one is due.
    pub(super) fn review_strategy(&mut self, player: u8, census: &Census, intel: &Intel) {
        let tick = self.state.tick;
        let s = self.state.ai[player as usize].strategy;
        if tick < s.next_review.max(FIRST_REVIEW) {
            return;
        }
        let salt = if s.reviews == 0 {
            self.state.rng.next_u32()
        } else {
            s.salt
        };
        let menu = self.side_menu(player);
        let persona = self.ai_personality(player, census, intel);
        let mut scored: Vec<(i32, Gambit)> = ALL
            .iter()
            .map(|&g| {
                let score = self.gambit_score(player, g, census, intel, &menu, persona);
                let n = s.held[g as usize];
                let held = match n {
                    0 => 0,
                    n if n <= COMMIT_REVIEWS => HOLD_BONUS,
                    n => -STALE * (n - COMMIT_REVIEWS) as i32,
                };
                let jitter = (salt ^ (g as u32).wrapping_mul(0x9E37_79B9))
                    .wrapping_mul(0x85EB_CA6B)
                    .rotate_left(s.reviews / 3 % 32)
                    % SALT_SPREAD;
                let total = if score >= MIN_SCORE {
                    score + held + jitter as i32
                } else {
                    0
                };
                (total, g)
            })
            .collect();
        // Score first; the gambit number breaks ties.
        scored.sort_by_key(|&(score, g)| (std::cmp::Reverse(score), g as u8));
        let n = self.state.ai[player as usize].config.skill().gambits;
        let active = scored
            .iter()
            .filter(|(score, _)| *score > 0)
            .take(n)
            .fold(0u16, |bits, (_, g)| bits | 1 << *g as u8);
        let mut held = [0u8; 8];
        for g in ALL {
            if active >> g as u8 & 1 != 0 {
                held[g as usize] = s.held[g as usize].saturating_add(1);
            }
        }
        self.state.ai[player as usize].strategy = Strategy {
            active,
            next_review: tick + REVIEW,
            reviews: s.reviews + 1,
            salt,
            held,
        };
    }

    /// How much `g` suits `player` now; zero when it cannot be played or the
    /// enemy has answered it.
    fn gambit_score(
        &self,
        player: u8,
        g: Gambit,
        census: &Census,
        intel: &Intel,
        menu: &Menu,
        persona: Personality,
    ) -> i32 {
        let ai = &self.state.ai[player as usize];
        let contacts = &ai.contacts;
        let seen = |f: &dyn Fn(&UnitBlueprint) -> bool| {
            contacts
                .iter()
                .filter(|c| f(self.blueprints.unit(c.blueprint)))
                .count()
        };
        let enemy_aa = self.enemy_anti_air(player);
        // Little anti-air seen means little only once their base has been seen.
        let open_sky = !intel.enemy_factories.is_empty() && enemy_aa <= FEW_AA;
        let fortified = seen(&|bp| bp.has(cat::DEFENSE) && !bp.weapons.is_empty());
        let income = self.state.players[player as usize].mass_income;
        let aggressive = matches!(persona, Personality::Aggressive) as i32;
        match g {
            Gambit::Scouting => {
                let blind = intel.enemy_factories.is_empty() as i32;
                40 + 60 * blind + 20 * (contacts.len() < 12) as i32
            }
            Gambit::Hunt => {
                if !census.land_route || census.factories.is_empty() {
                    return 0;
                }
                let outskirts = |c: &&adaptive::Contact| {
                    self.enemy_start_near(player, c.pos)
                        .is_some_and(|s| s.distance(c.pos) > OUTSKIRTS)
                };
                let engineers = contacts
                    .iter()
                    .filter(|c| self.blueprints.unit(c.blueprint).has(cat::ENGINEER))
                    .filter(outskirts)
                    .count();
                let mines = contacts
                    .iter()
                    .filter(|c| self.blueprints.unit(c.blueprint).has(cat::EXTRACTOR))
                    .filter(outskirts)
                    .count();
                // Nothing seen out there: nothing to hunt.
                if engineers + mines == 0 {
                    return 0;
                }
                20 + 25 * engineers.min(3) as i32 + 10 * mines.min(4) as i32 + 30 * aggressive
            }
            Gambit::Landing => {
                if !menu.any(self, |bp| bp.transport.is_some() && bp.warp.is_some()) {
                    return 0;
                }
                // Off an island, a landing is the only way the land army ever fights.
                let island = (!census.land_route) as i32;
                25 + 140 * island + 40 * (fortified >= 4) as i32 + 20 * aggressive
            }
            Gambit::WarpRaid => {
                if !menu.any(self, warship) || income < Fx::from_int(12) {
                    return 0;
                }
                20 + 50 * open_sky as i32 + 30 * (!census.land_route) as i32
            }
            Gambit::AirFleet => {
                // With an air factory to fill it: holding bombers back before
                // there is one only slows the first strikes.
                if census.air_factories == 0 || ai.config.domain_weights[1] == 0 || enemy_aa >= 8 {
                    return 0;
                }
                20 + 60 * open_sky as i32 + 20 * (!census.land_route) as i32
            }
            Gambit::Siege => {
                let gun = |bp: &UnitBlueprint| {
                    projects::project_kind(bp) == Some(projects::Project::MapGun)
                };
                if !menu.any(self, gun) {
                    return 0;
                }
                let turtle = matches!(persona, Personality::Turtle) as i32;
                10 + 60 * turtle
                    + 50 * (fortified >= 6) as i32
                    + 30 * (income >= Fx::from_int(40)) as i32
            }
            Gambit::NukeRace => {
                let silo = |bp: &UnitBlueprint| {
                    projects::project_kind(bp) == Some(projects::Project::Nuke)
                };
                let interceptors =
                    seen(&|bp| projects::project_kind(bp) == Some(projects::Project::Interceptor));
                // Seen their base and no interceptor in it: the warhead gets through.
                let scouted = !intel.enemy_factories.is_empty();
                if !menu.any(self, silo) || interceptors > 0 || !scouted {
                    return 0;
                }
                60 + 20 * matches!(persona, Personality::Expander) as i32
                    + 30 * (income >= Fx::from_int(60)) as i32
            }
            Gambit::Submarines => {
                let subs = |bp: &UnitBlueprint| {
                    bp.dive.is_some() && !bp.weapons.is_empty() && bp.is_mobile()
                };
                // Ships or a yard: a mine standing in the sea is no fleet.
                let at_sea = seen(&|bp| {
                    adaptive::domain(bp) == 2 && (bp.is_mobile() || bp.has(cat::FACTORY))
                });
                let sonar = seen(&|bp| bp.sonar > Fx::ZERO);
                // A yard of ours makes them, and their own fleet or yard says
                // there is water to fight on.
                // and a sonar or two is little cover for a sea.
                if !menu.any(self, subs)
                    || at_sea == 0
                    || sonar >= 3
                    || ai.config.domain_weights[2] == 0
                {
                    return 0;
                }
                70 + 10 * at_sea.min(4) as i32 - 25 * sonar as i32
            }
        }
    }
}

#[cfg(test)]
#[path = "strategy_tests.rs"]
mod tests;
