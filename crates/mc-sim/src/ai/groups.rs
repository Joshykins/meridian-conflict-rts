//! Air and sea strike groups.
//!
//! Aircraft and ships used to be sent the moment each one was idle: every
//! bomber off the line flew at the enemy alone, and so did every ship, a
//! stream the enemy's defenses picked off one at a time. Now each gathers at
//! home and goes out as a group, the way the land army's waves do, and one
//! that comes back idle from a strike regroups at home before the next.
use super::*;
use mc_data::MoveLayer;

/// Aircraft this close to the staging point have gathered.
const AIR_HOME: Fx = Fx::from_int(320);
/// Enemy aircraft this close to the start are hunted at once, with whatever
/// fighters are ready.
const AIR_DEFENSE: Fx = Fx::from_int(1500);
/// Ships this close to the fleet's anchorage have gathered.
const FLEET_HOME: Fx = Fx::from_int(360);
/// A contact this close to the anchorage is fought at once, whatever the
/// fleet's size.
const FLEET_DEFENSE: Fx = Fx::from_int(800);
/// Ships a fleet waits for before it sails.
const FLEET_WAVE: usize = 4;
/// Spaceships this close to the staging point have gathered: their hulls are
/// up to half a kilometre long.
const CAPITAL_HOME: Fx = Fx::from_int(600);
/// Mass of armed spaceships a strike waits for: a lone corvette waits for a
/// second, a heavy frigate goes on its own.
const CAPITAL_WAVE_MASS: i32 = 2000;

/// Idle ships of one hull size and the water they reach.
struct Fleet {
    sea: super::sea::SeaReach,
    idle: Vec<usize>,
    /// Its water nearest home, where it gathers.
    anchorage: FxVec2,
    at_home: Vec<usize>,
    away: Vec<usize>,
}

/// `rows` grouped around seeds: each row joins the first group whose seed is
/// within `radius`, else starts its own.
pub(super) fn clusters(pos: &[FxVec2], rows: &[usize], radius: Fx) -> Vec<(FxVec2, Vec<usize>)> {
    let mut groups: Vec<(FxVec2, Vec<usize>)> = Vec::new();
    for &row in rows {
        let at = pos[row];
        match groups
            .iter_mut()
            .find(|(seed, _)| seed.distance(at) <= radius)
        {
            Some((_, members)) => members.push(row),
            None => groups.push((at, vec![row])),
        }
    }
    groups
}

impl World {
    /// Aircraft a strike waits for: four at first, more as the match goes on.
    fn air_wave(&self, player: u8) -> usize {
        (4 + self.state.ai[player as usize].waves as usize / 2).min(10)
    }

    /// Every weapon of `row` hits ships and nothing on land.
    fn hits_ships_only(&self, row: usize) -> bool {
        let weapons = &self.bp(row).weapons;
        !weapons.is_empty()
            && weapons.iter().all(|w| {
                w.target_mask & cat::NAVAL != 0 && w.target_mask & (cat::LAND | cat::STRUCTURE) == 0
            })
    }

    fn ids_of(&self, rows: &[usize]) -> Vec<UnitId> {
        rows.iter()
            .take(MAX_COMMAND_UNITS)
            .map(|&r| self.state.units.id(r))
            .collect()
    }

    pub(super) fn direct_air(
        &mut self,
        player: u8,
        census: &Census,
        intel: &Intel,
        start: FxVec2,
        staging: FxVec2,
        out: &mut Vec<Command>,
    ) {
        let pos = &self.state.units.pos;
        let home = |r: &usize| pos[*r].distance(staging) <= AIR_HOME;
        let (bombers, bombers_away): (Vec<usize>, Vec<usize>) =
            census.bombers_idle.iter().partition(|r| home(r));
        // Torpedo bombers strike ships only, as a group of their own.
        let (sea_strike, bombers): (Vec<usize>, Vec<usize>) =
            bombers.iter().partition(|&&r| self.hits_ships_only(r));
        let (mut fighters, fighters_away): (Vec<usize>, Vec<usize>) =
            census.interceptors_idle.iter().partition(|r| home(r));
        let wave = self.air_wave(player);

        // Fighters: enemy aircraft over home are hunted by all that are ready.
        let intruder = self
            .visible_air_target(player, start)
            .filter(|p| p.distance(start) <= AIR_DEFENSE);
        if let Some(target) = intruder {
            let all: Vec<usize> = fighters.iter().chain(&fighters_away).copied().collect();
            if !all.is_empty() {
                out.push(Command::AttackMove {
                    units: self.ids_of(&all),
                    target,
                    queue: false,
                });
            }
            fighters.clear();
        } else if !fighters_away.is_empty() {
            out.push(Command::Move {
                units: self.ids_of(&fighters_away),
                target: staging,
                queue: false,
            });
        }

        if !bombers_away.is_empty() {
            out.push(Command::Move {
                units: self.ids_of(&bombers_away),
                target: staging,
                queue: false,
            });
        }
        if bombers.len() >= wave {
            let target = intel
                .enemy_extractors
                .iter()
                .min_by_key(|m| (m.distance_sq(staging), m.x, m.y))
                .copied()
                .or_else(|| self.attack_target(player, staging, intel, Stance::Raid));
            if let Some(target) = target {
                self.state.ai[player as usize].raids += 1;
                out.push(Command::AttackMove {
                    units: self.ids_of(&bombers),
                    target,
                    queue: false,
                });
                // Gathered fighters fly with the strike as its escort.
                if !fighters.is_empty() {
                    out.push(Command::AttackMove {
                        units: self.ids_of(&fighters),
                        target,
                        queue: false,
                    });
                    fighters.clear();
                }
            }
        }
        if sea_strike.len() >= wave {
            let from = self.state.units.pos[sea_strike[0]];
            let target = self.state.ai[player as usize]
                .contacts
                .iter()
                .filter(|c| {
                    self.bp(sea_strike[0])
                        .can_attack(self.blueprints.unit(c.blueprint))
                })
                .map(|c| c.pos)
                .min_by_key(|p| (p.distance_sq(from), p.x, p.y));
            if let Some(target) = target {
                self.state.ai[player as usize].raids += 1;
                out.push(Command::AttackMove {
                    units: self.ids_of(&sea_strike),
                    target,
                    queue: false,
                });
            }
        }
        // A full wing of fighters goes hunting enemy aircraft wherever seen.
        if fighters.len() >= wave {
            if let Some(target) = self.visible_air_target(player, staging) {
                out.push(Command::AttackMove {
                    units: self.ids_of(&fighters),
                    target,
                    queue: false,
                });
            }
        }
    }

    /// Armed spaceships gather at the staging point and strike together once
    /// they carry `CAPITAL_WAVE_MASS`, at the base itself when it is raided.
    /// They were counted as bombers: a wing of four 1100-mass corvettes was
    /// waited for, and they went after mines.
    pub(super) fn direct_capital(
        &mut self,
        player: u8,
        census: &Census,
        intel: &Intel,
        start: FxVec2,
        out: &mut Vec<Command>,
    ) {
        if census.capital_idle.is_empty() {
            return;
        }
        let units = &self.state.units;
        let staging = offset_toward(start, intel.enemy_start.unwrap_or(start), Fx::from_int(400));
        let mass = |rows: &[usize]| -> i32 {
            rows.iter().map(|&r| self.bp(r).cost_mass.floor_int()).sum()
        };
        // A raid on the base: every idle warship answers it.
        if let Some(&(_, enemy)) = intel
            .threats
            .iter()
            .find(|(_, e)| e.distance(start) < HOME_RADIUS * 2)
        {
            out.push(Command::AttackMove {
                units: self.ids_of(&census.capital_idle),
                target: enemy,
                queue: false,
            });
            return;
        }
        let (home, away): (Vec<usize>, Vec<usize>) = census
            .capital_idle
            .iter()
            .partition(|&&r| units.pos[r].distance(staging) <= CAPITAL_HOME);
        // Out at the front: a group strong enough moves on to the next target,
        // what is left of one comes home to wait for the next.
        let armed: Vec<usize> = census
            .combat_rows
            .iter()
            .copied()
            .filter(|&r| {
                self.bp(r).has(cat::SPACE) && units.pos[r].distance(staging) > CAPITAL_HOME
            })
            .collect();
        for (seed, members) in clusters(&units.pos, &armed, Fx::from_int(900)) {
            let idle: Vec<usize> = members
                .iter()
                .copied()
                .filter(|r| away.contains(r))
                .collect();
            if idle.is_empty() || idle.len() * 3 < members.len() * 2 {
                continue;
            }
            let target = (mass(&members) >= CAPITAL_WAVE_MASS)
                .then(|| self.attack_target(player, seed, intel, Stance::Expand))
                .flatten()
                .unwrap_or(staging);
            out.push(Command::AttackMove {
                units: self.ids_of(&idle),
                target,
                queue: false,
            });
        }
        if mass(&home) >= CAPITAL_WAVE_MASS {
            if let Some(target) = self.attack_target(player, staging, intel, Stance::Expand) {
                self.state.ai[player as usize].raids += 1;
                out.push(Command::AttackMove {
                    units: self.ids_of(&home),
                    target,
                    queue: false,
                });
            }
        }
    }

    /// Ships gather at the fleet's anchorage, the water it can reach nearest
    /// home, and sail as one fleet. A fleet out at sea that went idle presses on
    /// if enough of it is left, and falls back to the anchorage if not.
    pub(super) fn direct_fleet(&self, player: u8, census: &Census, out: &mut Vec<Command>) {
        let start = self.state.players[player as usize].start;
        let units = &self.state.units;
        // Idle ships by the water they can reach, flooded once per think.
        let mut seas: Vec<(MoveLayer, u8, super::sea::SeaReach, Vec<usize>)> = Vec::new();
        for &row in &census.naval_idle {
            let motion = self.bp(row).motion.unwrap();
            let from = units.pos[row];
            let known = seas.iter().position(|(l, s, sea, _)| {
                *l == motion.layer && *s == motion.size_class && sea.reaches(from)
            });
            match known {
                Some(i) => seas[i].3.push(row),
                None => {
                    if let Some(sea) = self.sea_reach(motion.layer, motion.size_class, from) {
                        seas.push((motion.layer, motion.size_class, sea, vec![row]));
                    }
                }
            }
        }
        // Each hull size floods its own water, but they all sail as one fleet:
        // split by size, boats, frigates and submarines each gathered two or
        // three at home, and none of those groups ever made a fleet of four.
        let fleets: Vec<Fleet> = seas
            .into_iter()
            .filter_map(|(_, _, sea, idle)| {
                let anchorage = sea.nearest(start)?;
                let (at_home, away) = idle
                    .iter()
                    .partition(|&&r| units.pos[r].distance(anchorage) <= FLEET_HOME);
                Some(Fleet {
                    sea,
                    idle,
                    anchorage,
                    at_home,
                    away,
                })
            })
            .collect();
        let gathered: usize = fleets.iter().map(|f| f.at_home.len()).sum();
        let all_away: Vec<usize> = fleets.iter().flat_map(|f| f.away.iter().copied()).collect();
        for f in &fleets {
            let (sea, anchorage) = (&f.sea, f.anchorage);
            let target_from = |from: FxVec2| self.fleet_target(player, &f.idle, sea, from);
            // The whole fleet, busy or not, out past the anchorage.
            let out_there: Vec<usize> = census
                .combat_rows
                .iter()
                .copied()
                .filter(|&r| {
                    adaptive::domain(self.bp(r)) == 2
                        && units.pos[r].distance(anchorage) > FLEET_HOME
                })
                .collect();
            for (seed, members) in clusters(&units.pos, &out_there, Fx::from_int(600)) {
                let idle = members.iter().filter(|r| all_away.contains(r)).count();
                // Most of it still fighting: the rest waits for it.
                if idle == 0 || idle * 3 < members.len() * 2 {
                    continue;
                }
                let ours: Vec<usize> = members
                    .iter()
                    .copied()
                    .filter(|r| f.away.contains(r))
                    .collect();
                if ours.is_empty() {
                    continue;
                }
                let target = (members.len() >= FLEET_WAVE)
                    .then(|| target_from(seed).or_else(|| self.enemy_water(player, sea, seed)))
                    .flatten()
                    .unwrap_or(anchorage);
                out.push(Command::AttackMove {
                    units: self.ids_of(&ours),
                    target,
                    queue: false,
                });
            }
            if f.at_home.is_empty() {
                continue;
            }
            let target = target_from(anchorage);
            let threatened = target.is_some_and(|t| t.distance(anchorage) <= FLEET_DEFENSE);
            let target = if gathered >= FLEET_WAVE || threatened {
                target.or_else(|| self.enemy_water(player, sea, anchorage))
            } else {
                None
            };
            if let Some(target) = target {
                out.push(Command::AttackMove {
                    units: self.ids_of(&f.at_home),
                    target,
                    queue: false,
                });
            }
        }
    }

    /// The nearest remembered contact some ship of `fleet` can reach within
    /// weapon range of, on water `sea` reaches.
    fn fleet_target(
        &self,
        player: u8,
        fleet: &[usize],
        sea: &super::sea::SeaReach,
        from: FxVec2,
    ) -> Option<FxVec2> {
        self.state.ai[player as usize]
            .contacts
            .iter()
            .filter_map(|c| {
                let enemy = self.blueprints.unit(c.blueprint);
                fleet.iter().find_map(|&row| {
                    let bp = self.bp(row);
                    let motion = bp.motion?;
                    let range = bp
                        .weapons
                        .iter()
                        .filter(|w| w.target_mask & enemy.target_categories() != 0)
                        .map(|w| w.range_max)
                        .max()?;
                    let goal = self
                        .nav
                        .nearest_passable(motion.layer, motion.size_class, c.pos)?;
                    (goal.distance(c.pos) <= range && sea.reaches(goal)).then_some(goal)
                })
            })
            .min_by_key(|p| (p.distance_sq(from), p.x, p.y))
    }

    /// With no contact, the fleet's own water nearest an enemy start.
    fn enemy_water(&self, player: u8, sea: &super::sea::SeaReach, from: FxVec2) -> Option<FxVec2> {
        self.state
            .players
            .iter()
            .enumerate()
            .filter(|(i, p)| !p.defeated && self.are_enemies(player, *i as u8))
            .map(|(_, p)| p.start)
            .min_by_key(|s| (s.distance_sq(from), s.x, s.y))
            .and_then(|s| sea.nearest(s))
    }
}
