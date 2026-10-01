//! The land army: answering raids on home, the home guard, landings and
//! hunts taking their units, waves gathering at the staging point and leaving
//! as one group, and waves out in the field moving on together.
use super::*;

impl World {
    pub(super) fn direct_army(
        &mut self,
        player: u8,
        census: &Census,
        intel: &Intel,
        stance: Stance,
        persona: Personality,
        start: FxVec2,
        facing: Angle,
        firebase: Option<FxVec2>,
        out: &mut Vec<Command>,
    ) {
        self.direct_fleet(player, census, out);
        self.direct_capital(player, census, intel, start, out);
        for &row in &census.support_idle {
            let escort = census
                .combat_rows
                .iter()
                .copied()
                .filter(|&r| r != row)
                .min_by_key(|&r| self.state.units.pos[r].distance_sq(self.state.units.pos[row]));
            if let Some(escort) = escort {
                out.push(Command::Move {
                    units: vec![self.state.units.id(row)],
                    target: offset_toward(self.state.units.pos[escort], start, Fx::from_int(100)),
                    queue: false,
                });
            }
        }
        if census.army_idle.is_empty()
            && census.artillery_idle.is_empty()
            && census.bombers_idle.is_empty()
            && census.interceptors_idle.is_empty()
            && census.home_guard.is_empty()
        {
            return;
        }
        let staging = firebase
            .filter(|_| matches!(stance, Stance::Firebase | Stance::Push | Stance::Raid))
            .unwrap_or_else(|| {
                offset_toward(
                    start,
                    intel
                        .enemy_start
                        .unwrap_or(start + FxVec2::from_angle(facing) * Fx::from_int(400)),
                    Fx::from_int(240),
                )
            });

        // Land units that cannot walk to any enemy answer raids on their own
        // ground with the rest (`theatre.rs`).
        let mut army_idle: Vec<usize> = census
            .army_idle
            .iter()
            .chain(&census.home_guard)
            .copied()
            .collect();
        army_idle.sort_unstable();
        if let Some(&(at, enemy)) = intel.threats.first().filter(|(_, enemy)| {
            stance == Stance::Defend || enemy.distance(start) < Fx::from_int(420)
        }) {
            let target = if at.distance(start) < HOME_RADIUS {
                enemy
            } else {
                at
            };
            if enemy.distance(start) < Fx::from_int(420) {
                // The base itself: everything goes.
                let ids: Vec<UnitId> = army_idle
                    .iter()
                    .chain(&census.artillery_idle)
                    .chain(&census.bombers_idle)
                    .take(MAX_COMMAND_UNITS)
                    .map(|&r| self.state.units.id(r))
                    .collect();
                if !ids.is_empty() {
                    out.push(Command::AttackMove {
                        units: ids,
                        target,
                        queue: false,
                    });
                    return;
                }
            } else if !army_idle.is_empty() {
                // A raid on an outlying mine: the closest few answer it and the
                // rest carry on. Returning here held the whole army at home, a
                // squad at a time, for as long as any raider stayed near a mine.
                let units = &self.state.units;
                army_idle.sort_by_key(|&r| (units.pos[r].distance_sq(target), r));
                let squad: Vec<usize> = army_idle.drain(..army_idle.len().min(6)).collect();
                out.push(Command::AttackMove {
                    units: squad.iter().map(|&r| units.id(r)).collect(),
                    target,
                    queue: false,
                });
                army_idle.sort_unstable();
            }
        }

        self.direct_air(player, census, intel, start, staging, out);
        let wave = self.wave_size(player, persona);
        // A landing takes its cargo first, the home guard too (`landing.rs`).
        self.direct_landing(
            player,
            census,
            intel,
            start,
            staging,
            wave,
            &mut army_idle,
            out,
        );
        // The home guard never goes out with a wave: one that went to meet a
        // raid comes back to wait at the staging point.
        let guard_home = staging.distance(start) + Fx::from_int(400);
        let guard_back: Vec<usize> = army_idle
            .iter()
            .copied()
            .filter(|r| census.home_guard.contains(r))
            .filter(|&r| self.state.units.pos[r].distance(start) > guard_home)
            .collect();
        if !guard_back.is_empty() {
            out.push(Command::Move {
                units: guard_back
                    .iter()
                    .take(MAX_COMMAND_UNITS)
                    .map(|&r| self.state.units.id(r))
                    .collect(),
                target: staging,
                queue: false,
            });
        }
        army_idle.retain(|r| !census.home_guard.contains(r));

        // Units only ever leave in one group: from the staging point, or, out
        // in the field after a wave, together with the rest of it.
        // A staging point across a cliff or a river from home is never reached.
        let size = army_idle
            .iter()
            .filter_map(|&r| self.bp(r).motion)
            .map(|m| m.size_class)
            .max()
            .unwrap_or(0);
        let ground = self.home_ground(start, size);
        let staging = ground
            .as_ref()
            .map_or(staging, |g| self.reachable_staging(start, staging, g));
        let mut at_stage = Vec::new();
        let mut gathering = Vec::new();
        let mut forward = Vec::new();
        let stage_reach = staging.distance(start) + Fx::from_int(400);
        // The gathered blob grows with the army: its edge is past a fixed radius
        // once a hundred units stand there, and those were sent back in every think.
        let near_stage = army_idle
            .iter()
            .filter(|&&r| self.state.units.pos[r].distance(staging) <= STAGING_RADIUS * 3)
            .count() as i32;
        let stage_radius = STAGING_RADIUS + Fx::from_int(12 * near_stage.isqrt());
        for &row in &army_idle {
            let pos = self.state.units.pos[row];
            if pos.distance(staging) <= stage_radius {
                at_stage.push(row);
            } else if pos.distance(start) > stage_reach {
                forward.push(row);
            } else if ground.as_ref().is_some_and(|g| !g.reaches(pos)) {
                // Made by a factory on another shelf: it cannot walk to the
                // staging point, so it goes with the wave from where it is.
                at_stage.push(row);
            } else {
                gathering.push(row);
            }
        }
        self.direct_hunt(player, &mut at_stage, staging, out);
        let ids = |rows: &[usize]| -> Vec<UnitId> {
            rows.iter()
                .take(MAX_COMMAND_UNITS)
                .map(|&r| self.state.units.id(r))
                .collect()
        };
        if !forward.is_empty() {
            // A wave that took its target moves on only as a wave: enough of it
            // left presses on to the next, a remnant falls back to join the
            // next wave. Each group of the army out there (600 m clusters,
            // busy units counted) waits until most of it is done fighting.
            // Sending each unit on as it went idle strung the wave out into a
            // stream that arrived, and was beaten, a unit at a time.
            let out_there: Vec<usize> = census
                .combat_rows
                .iter()
                .copied()
                .filter(|&r| {
                    self.state.units.pos[r].distance(start) > stage_reach
                        && adaptive::domain(self.bp(r)) == 0
                })
                .collect();
            for (seed, members) in clusters(&self.state.units.pos, &out_there, Fx::from_int(600)) {
                let idle: Vec<usize> = members
                    .iter()
                    .copied()
                    .filter(|r| forward.contains(r))
                    .collect();
                if idle.is_empty() || idle.len() * 3 < members.len() * 2 {
                    continue;
                }
                // Put ashore where home cannot be walked to, it fights on.
                let press = members.len() >= (wave / 2).max(4) || !census.land_route;
                let target = press
                    .then(|| self.attack_target(player, seed, intel, stance))
                    .flatten()
                    .unwrap_or(staging);
                out.push(Command::AttackMove {
                    units: ids(&idle),
                    target,
                    queue: false,
                });
            }
        }
        if stance == Stance::Raid && !intel.enemy_extractors.is_empty() {
            let raiders: Vec<UnitId> = at_stage
                .iter()
                .copied()
                .take(12)
                .map(|r| self.state.units.id(r))
                .collect();
            if raiders.len() >= 6 {
                if let Some(mex) = intel.enemy_extractors.get(
                    self.state.ai[player as usize].waves as usize % intel.enemy_extractors.len(),
                ) {
                    self.state.ai[player as usize].raids += 1;
                    out.push(Command::AttackMove {
                        units: raiders,
                        target: *mex,
                        queue: false,
                    });
                    return;
                }
            }
        }
        if !gathering.is_empty() {
            out.push(Command::AttackMove {
                units: ids(&gathering),
                target: staging,
                queue: false,
            });
        }
        // Only what has gathered goes: a wave that leaves while half of it is
        // still on the road arrives strung out and is beaten a few at a time.
        let ready = at_stage.len() + census.artillery_idle.len() >= wave
            || (stance == Stance::Push && at_stage.len() * 3 >= wave * 2);
        if ready {
            if let Some(target) = self.attack_target(player, staging, intel, stance) {
                self.state.ai[player as usize].waves += 1;
                // Those nearly there go too, idle or still on their way to the
                // staging point: left behind, a handful waited for the next,
                // bigger wave on their own.
                let units = &self.state.units;
                let on_the_way = |r: usize| {
                    let head = units.order_head[r];
                    head != NO_ORDER
                        && self.state.orders.order[head as usize].pos == staging
                        && adaptive::domain(self.bp(r)) == 0
                        && !self.bp(r).has(cat::ARTILLERY)
                };
                let mut wave_rows: Vec<usize> = at_stage.clone();
                wave_rows.extend(
                    gathering
                        .iter()
                        .copied()
                        .chain(
                            census
                                .combat_rows
                                .iter()
                                .copied()
                                .filter(|&r| on_the_way(r)),
                        )
                        .filter(|&r| units.pos[r].distance(staging) <= stage_radius * 3),
                );
                wave_rows.sort_unstable();
                wave_rows.dedup();
                if !wave_rows.is_empty() {
                    out.push(Command::AttackMove {
                        units: ids(&wave_rows),
                        target,
                        queue: false,
                    });
                }
                if !census.artillery_idle.is_empty() {
                    let range = census
                        .artillery_idle
                        .iter()
                        .flat_map(|&r| self.bp(r).weapons.iter().map(|w| w.range_max))
                        .min()
                        .unwrap_or(Fx::from_int(350));
                    let siege = offset_toward(target, staging, range * Fx::ratio(9, 10));
                    let ids: Vec<UnitId> = census
                        .artillery_idle
                        .iter()
                        .take(MAX_COMMAND_UNITS)
                        .map(|&r| self.state.units.id(r))
                        .collect();
                    out.push(Command::AttackMove {
                        units: ids,
                        target: siege,
                        queue: false,
                    });
                }
            }
        } else if !census.artillery_idle.is_empty() {
            let hold = offset_toward(staging, start, Fx::from_int(90));
            let ids: Vec<UnitId> = census
                .artillery_idle
                .iter()
                .take(MAX_COMMAND_UNITS)
                .map(|&r| self.state.units.id(r))
                .collect();
            out.push(Command::AttackMove {
                units: ids,
                target: hold,
                queue: false,
            });
        }
    }
}
