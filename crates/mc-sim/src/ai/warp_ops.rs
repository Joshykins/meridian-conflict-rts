//! Spaceships and their warp drives.
//!
//! The AI flew every spaceship the long way: a strike crossed the map at a
//! warship's crawl, the Vigil trailed the army, and no drive was ever spooled.
//! Now:
//!
//! - A strike of armed spaceships jumps (`jump_then_attack`): it comes out
//!   `STANDOFF` short of its target on the side it came from, out of any
//!   remembered dampener's field (`safe_mark`), and attack-moves in.
//! - A ship hurt out at the front jumps home (`JUMP_HOME_HEALTH`).
//! - With `Gambit::WarpRaid` held, warships go one by one, jumping at the
//!   softest target on the enemy's outskirts (`soft_target`) and on to the
//!   next while healthy, not waiting for a strike's worth of mass.
//! - Sensor ships (the Vigil) jump about the enemy's side of the map, each
//!   sweep to the next place on a round of starts, mines and ore, standing off
//!   from anti-air, and jump home when hurt or shot at.
use super::strategy::Gambit;
use super::*;
use crate::tables::WarpPhase;

/// Spaceships this close to their gathering point have gathered: their hulls
/// are up to half a kilometre long.
const CAPITAL_HOME: Fx = Fx::from_int(600);
/// Mass of armed spaceships a strike waits for: a lone corvette waits for a
/// second, a heavy frigate goes on its own.
const CAPITAL_WAVE_MASS: i32 = 2000;
/// A strike comes out this far short of its target.
const STANDOFF: Fx = Fx::from_int(260);
/// A mark nearer than this is flown to, not jumped.
const WORTH_A_JUMP: Fx = Fx::from_int(1400);
/// A ship out at the front with less of its health than this jumps home.
const JUMP_HOME_HEALTH: Fx = Fx::ratio(1, 2);
/// A raiding warship with less than this goes home rather than on.
const RAID_HEALTH: Fx = Fx::ratio(3, 4);
/// A jump comes out at least this far outside a remembered dampener's field.
const DAMPER_MARGIN: Fx = Fx::from_int(150);
/// Anti-air within this of a target counts against it.
const AA_REACH: Fx = Fx::from_int(700);
/// A sensor ship sweeps from this far short of where it looks: its eye
/// reaches a kilometre and a half, and anti-air waits at the place itself.
const SWEEP_STANDOFF: Fx = Fx::from_int(900);
/// An engineer seen within this many ticks is still near where it was seen.
const FRESH_ENGINEER: u32 = 300;

impl World {
    /// Whether `row`'s drive is ready to spool and its side can pay the charge.
    pub(super) fn can_jump(&self, row: usize) -> bool {
        let Some(drive) = self.bp(row).warp else {
            return false;
        };
        let units = &self.state.units;
        let w = &units.warp[row];
        let pl = &self.state.players[units.owner[row] as usize];
        w.phase == WarpPhase::Idle
            && w.recharge == 0
            && pl.energy + pl.energy_income * Fx::from_int(drive.spool_ticks as i32 / 10)
                >= drive.energy
    }

    /// `want`, moved out of every remembered enemy dampener's field, toward `from`.
    pub(super) fn safe_mark(&self, player: u8, want: FxVec2, from: FxVec2) -> FxVec2 {
        let mut mark = want;
        for c in &self.state.ai[player as usize].contacts {
            let Some(d) = self.blueprints.unit(c.blueprint).warp_damper else {
                continue;
            };
            let clear = d.radius + DAMPER_MARGIN;
            if mark.distance(c.pos) >= clear {
                continue;
            }
            let away = if mark == c.pos {
                from - c.pos
            } else {
                mark - c.pos
            };
            let away = if away.length() == Fx::ZERO {
                FxVec2::from_angle(Angle::ZERO)
            } else {
                away.normalize()
            };
            mark = c.pos + away * clear;
        }
        self.clamp_to_map(mark)
    }

    /// `rows` go at `target`: those whose drives are ready jump to a mark short
    /// of it and attack-move in from there, the rest fly.
    pub(super) fn jump_then_attack(
        &self,
        player: u8,
        rows: &[usize],
        target: FxVec2,
        from: FxVec2,
        out: &mut Vec<Command>,
    ) {
        let units = &self.state.units;
        let (jump, fly): (Vec<usize>, Vec<usize>) = rows
            .iter()
            .partition(|&&r| self.can_jump(r) && units.pos[r].distance(target) >= WORTH_A_JUMP);
        if !jump.is_empty() {
            let mark = self.safe_mark(player, offset_toward(target, from, STANDOFF), from);
            let ids = self.ids_of(&jump);
            out.push(Command::Warp {
                units: ids.clone(),
                pos: mark,
                queue: false,
            });
            out.push(Command::AttackMove {
                units: ids,
                target,
                queue: true,
            });
        }
        if !fly.is_empty() {
            out.push(Command::AttackMove {
                units: self.ids_of(&fly),
                target,
                queue: false,
            });
        }
    }

    /// The enemy's economy least covered by anti-air: mines, power and
    /// engineers (seen lately), nearest `from` after the anti-air near each.
    /// With `outskirts`, only what stands well away from an enemy start.
    pub(super) fn soft_target(&self, player: u8, from: FxVec2, outskirts: bool) -> Option<FxVec2> {
        let tick = self.state.tick;
        self.state.ai[player as usize]
            .contacts
            .iter()
            .filter(|c| {
                let bp = self.blueprints.unit(c.blueprint);
                let economy = bp.has(cat::EXTRACTOR) || bp.has(cat::POWER);
                let engineer = bp.has(cat::ENGINEER)
                    && !bp.has(cat::COMMANDER)
                    && tick.saturating_sub(c.seen) <= FRESH_ENGINEER;
                (economy || engineer)
                    && (!outskirts
                        || self
                            .enemy_start_near(player, c.pos)
                            .is_some_and(|s| s.distance(c.pos) > Fx::from_int(900)))
            })
            .map(|c| {
                let aa = self.anti_air_near(player, c.pos, AA_REACH);
                let cost = c.pos.distance(from).floor_int() as i64 / 4 + aa * 3;
                (cost, c.pos)
            })
            .min_by_key(|&(cost, p)| (cost, p.x, p.y))
            .map(|(_, p)| p)
    }

    /// Armed spaceships: strikes by warp, hurt ships home by warp, and raiders
    /// one at a time while `Gambit::WarpRaid` is held. They gather 400 m out
    /// from the start toward the enemy; a raid on the base calls them home.
    pub(super) fn direct_capital(
        &mut self,
        player: u8,
        census: &Census,
        intel: &Intel,
        start: FxVec2,
        out: &mut Vec<Command>,
    ) {
        let units = &self.state.units;
        let staging = offset_toward(start, intel.enemy_start.unwrap_or(start), Fx::from_int(400));
        // Hurt out at the front, busy or not: home through warp, out of the fight at once.
        let hurt: Vec<usize> = census
            .combat_rows
            .iter()
            .copied()
            .filter(|&r| {
                let bp = self.bp(r);
                bp.has(cat::SPACE)
                    && units.health[r] < bp.health * JUMP_HOME_HEALTH
                    && units.pos[r].distance(staging) > WORTH_A_JUMP
                    && self.can_jump(r)
            })
            .collect();
        if !hurt.is_empty() {
            let ids = self.ids_of(&hurt);
            out.push(Command::Warp {
                units: ids,
                pos: staging,
                queue: false,
            });
        }
        let idle: Vec<usize> = census
            .capital_idle
            .iter()
            .copied()
            .filter(|r| !hurt.contains(r))
            .collect();
        if idle.is_empty() {
            return;
        }
        // A raid on the base: every idle warship answers it.
        if let Some(&(_, enemy)) = intel
            .threats
            .iter()
            .find(|(_, e)| e.distance(start) < HOME_RADIUS * 2)
        {
            self.jump_then_attack(player, &idle, enemy, staging, out);
            return;
        }
        let (mut home, mut away): (Vec<usize>, Vec<usize>) = idle
            .iter()
            .partition(|&&r| units.pos[r].distance(staging) <= CAPITAL_HOME);
        // Raiders go one at a time at the softest target on the outskirts, and on
        // to the next while healthy; a worn one goes home.
        if self.holds(player, Gambit::WarpRaid) {
            let (healthy, worn): (Vec<usize>, Vec<usize>) = idle
                .iter()
                .partition(|&&r| units.health[r] >= self.bp(r).health * RAID_HEALTH);
            let mut raids = 0;
            for &row in &healthy {
                if let Some(target) = self.soft_target(player, units.pos[row], true) {
                    raids += 1;
                    self.jump_then_attack(player, &[row], target, staging, out);
                    home.retain(|&r| r != row);
                    away.retain(|&r| r != row);
                }
            }
            for &row in worn.iter().filter(|r| away.contains(r)) {
                self.go_home(row, staging, out);
            }
            away.retain(|r| !worn.contains(r));
            self.state.ai[player as usize].raids += raids;
            if home.is_empty() && away.is_empty() {
                return;
            }
        }
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
            match (self.mass_of(&members) >= CAPITAL_WAVE_MASS)
                .then(|| self.attack_target(player, seed, intel, Stance::Expand))
                .flatten()
            {
                Some(target) => self.jump_then_attack(player, &idle, target, seed, out),
                None => {
                    for row in idle {
                        self.go_home(row, staging, out);
                    }
                }
            }
        }
        if self.mass_of(&home) >= CAPITAL_WAVE_MASS {
            if let Some(target) = self.attack_target(player, staging, intel, Stance::Expand) {
                self.state.ai[player as usize].raids += 1;
                self.jump_then_attack(player, &home, target, staging, out);
            }
        }
    }

    fn mass_of(&self, rows: &[usize]) -> i32 {
        rows.iter().map(|&r| self.bp(r).cost_mass.floor_int()).sum()
    }

    /// `row` back to `home`: by warp when it is far and the drive is ready.
    fn go_home(&self, row: usize, home: FxVec2, out: &mut Vec<Command>) {
        let far = self.state.units.pos[row].distance(home) >= WORTH_A_JUMP;
        let units = vec![self.state.units.id(row)];
        out.push(if far && self.can_jump(row) {
            Command::Warp {
                units,
                pos: home,
                queue: false,
            }
        } else {
            Command::Move {
                units,
                target: home,
                queue: false,
            }
        });
    }

    /// Idle sensor ships sweep the enemy's side of the map by warp: each to the
    /// next place on a round of enemy starts, their mines and the map's ore,
    /// standing off from it, then drifting across it. Shot at or hurt, home.
    pub(super) fn direct_sensor_ships(
        &mut self,
        player: u8,
        census: &Census,
        start: FxVec2,
        out: &mut Vec<Command>,
    ) {
        for &row in &census.sensor_idle {
            let units = &self.state.units;
            let pos = units.pos[row];
            let hurt = units.health[row] < self.bp(row).health * RAID_HEALTH;
            let shot_at = self.anti_air_near(player, pos, Fx::from_int(1200)) > 0;
            if (hurt || shot_at) && pos.distance(start) > WORTH_A_JUMP {
                self.go_home(row, start, out);
                continue;
            }
            if !self.can_jump(row) {
                continue;
            }
            let sweeps = self.state.ai[player as usize].sweeps;
            let Some(spot) = self.sweep_spot(player, start, sweeps) else {
                continue;
            };
            self.state.ai[player as usize].sweeps = sweeps.wrapping_add(1);
            let mark = self.safe_mark(player, offset_toward(spot, start, SWEEP_STANDOFF), start);
            let across = (spot - mark).perp();
            let across = if across.length() == Fx::ZERO {
                FxVec2::ZERO
            } else {
                across.normalize() * Fx::from_int(700)
            };
            let id = vec![units.id(row)];
            out.push(Command::Warp {
                units: id.clone(),
                pos: mark,
                queue: false,
            });
            // Drift across what it came to see, then wait for the drive.
            out.push(Command::Move {
                units: id,
                target: self.clamp_to_map(mark + across),
                queue: true,
            });
        }
    }

    /// The `n`th place on a sensor ship's round: an enemy start, a remembered
    /// enemy mine, or ore on the map far from home, in turn.
    fn sweep_spot(&self, player: u8, start: FxVec2, n: u32) -> Option<FxVec2> {
        let enemies: Vec<FxVec2> = self
            .state
            .players
            .iter()
            .enumerate()
            .filter(|(i, p)| !p.defeated && self.are_enemies(player, *i as u8))
            .map(|(_, p)| p.start)
            .collect();
        if enemies.is_empty() {
            return None;
        }
        let pick = |list: &[FxVec2], k: u32| list.get(k as usize % list.len().max(1)).copied();
        match n % 3 {
            0 => pick(&enemies, n / 3),
            1 => {
                let mines: Vec<FxVec2> = self.state.ai[player as usize]
                    .contacts
                    .iter()
                    .filter(|c| self.blueprints.unit(c.blueprint).has(cat::EXTRACTOR))
                    .map(|c| c.pos)
                    .collect();
                pick(&mines, n / 3).or_else(|| pick(&enemies, n / 3 + 1))
            }
            _ => {
                let mut ore: Vec<FxVec2> = self
                    .ore_centres()
                    .into_iter()
                    .filter(|d| d.distance(start) > Fx::from_int(2500))
                    .collect();
                ore.sort_by_key(|d| (d.x, d.y));
                pick(&ore, n / 3).or_else(|| pick(&enemies, n / 3))
            }
        }
    }
}
