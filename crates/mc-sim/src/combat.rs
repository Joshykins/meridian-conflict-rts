//! Targeting, weapons, projectiles, damage and death.
//!
//! The read-only searches (target acquisition, projectile sweeps) run in
//! parallel over fixed-size row chunks and return per-chunk results that are
//! applied in chunk order. Everything that mutates state or draws random
//! numbers runs sequentially in row order.

use crate::mirror::SimEvent;
use crate::spatial::kind;
use crate::tables::*;
use crate::world::footprint_cells;
use crate::{SimError, World};
use mc_core::{Angle, Fx, FxVec2, FxVec3, TICKS_PER_SECOND};
use mc_data::{cat, Trajectory, Weapon, MAX_WEAPONS};

const DT: i32 = TICKS_PER_SECOND as i32;
const CHUNK: usize = 256;
/// A weapon keeps its target this many ticks before looking for a closer one.
const RETARGET_PERIOD: u32 = 8;
/// Turret must be within this of the firing solution to shoot (~2 degrees).
const AIM_TOLERANCE: u16 = 364;
/// Ballistic gravity, metres per tick squared. Stronger than Earth's so shells arc visibly.
const GRAVITY: Fx = Fx::ratio(40, (DT * DT) as i64);
/// Blast when a commander dies.
const COMMANDER_BLAST_RADIUS: Fx = Fx::from_int(140);
const COMMANDER_BLAST_DAMAGE: Fx = Fx::from_int(2500);

struct Hit {
    projectile: usize,
    point: FxVec3,
    unit: Option<usize>,
}

impl World {
    fn is_valid_target(&self, shooter: usize, target: usize, weapon: &Weapon) -> bool {
        let units = &self.state.units;
        if !units.slots.is_alive(target) || units.has_flag(target, flag::IN_FACTORY) {
            return false;
        }
        let owner = units.owner[shooter];
        if !self.are_enemies(owner, units.owner[target]) || self.bp(target).categories & weapon.target_mask == 0 {
            return false;
        }
        let gap = units.pos[shooter].distance(units.pos[target]) - self.bp(target).radius;
        gap <= weapon.range_max && gap >= weapon.range_min - self.bp(target).radius * 2 && self.detects(owner, target)
    }

    pub(crate) fn run_targeting(&mut self) {
        let rows = self.state.units.slots.rows();
        let tick = self.state.tick;
        let this = &*self;
        let picks: Vec<Vec<(usize, [UnitId; MAX_WEAPONS])>> = self.pool.parallel_map_chunks(rows, CHUNK, |_, range| {
            let units = &this.state.units;
            let mut out = Vec::new();
            for row in range {
                if !units.slots.is_alive(row) || !units.is_active(row) {
                    continue;
                }
                let bp = this.bp(row);
                if bp.weapons.is_empty() {
                    continue;
                }
                let ordered = this
                    .state
                    .orders
                    .front(units, row)
                    .filter(|o| o.kind == OrderKind::Attack)
                    .and_then(|o| units.row(o.target));
                let refresh = (row as u32).wrapping_add(tick) % RETARGET_PERIOD == 0;
                let mut targets = units.weapon_target[row];
                for (w, weapon) in bp.weapons.iter().enumerate() {
                    if let Some(t) = ordered.filter(|t| this.is_valid_target(row, *t, weapon)) {
                        targets[w] = units.id(t);
                        continue;
                    }
                    let current = units.row(targets[w]).filter(|t| this.is_valid_target(row, *t, weapon));
                    if current.is_some() && !refresh {
                        continue;
                    }
                    let found = this.index.nearest(units.pos[row], weapon.range_max, kind::UNIT, |e| {
                        this.unit_entry_is_current(e) && this.is_valid_target(row, e.row as usize, weapon)
                    });
                    targets[w] = found.map_or(Handle::NONE, |e| units.id(e.row as usize));
                }
                if targets != units.weapon_target[row] {
                    out.push((row, targets));
                }
            }
            out
        });
        for (row, targets) in picks.into_iter().flatten() {
            self.state.units.weapon_target[row] = targets;
        }
    }

    pub(crate) fn run_weapons(&mut self) -> Result<(), SimError> {
        let rows = self.state.units.slots.rows();
        for row in 0..rows {
            if !self.state.units.slots.is_alive(row) || !self.state.units.is_active(row) {
                continue;
            }
            let weapon_count = self.bp(row).weapons.len();
            for w in 0..weapon_count {
                self.step_weapon(row, w)?;
            }
        }
        Ok(())
    }

    fn step_weapon(&mut self, row: usize, w: usize) -> Result<(), SimError> {
        let bp = self.blueprints.clone();
        let weapon = &bp.unit(self.state.units.blueprint[row]).weapons[w];
        let units = &mut self.state.units;
        if units.weapon_cooldown[row][w] > 0 {
            units.weapon_cooldown[row][w] -= 1;
        }
        let Some(t) = units.row(units.weapon_target[row][w]) else {
            // Nothing to shoot: turrets drift back to centre.
            units.weapon_yaw[row][w] = units.weapon_yaw[row][w].turn_toward(Angle::ZERO, weapon.turret_turn / 2);
            units.weapon_salvo_left[row][w] = 0;
            return Ok(());
        };

        let target_bp = bp.unit(units.blueprint[t]);
        let pos = units.pos[row];
        let step = weapon.projectile_speed / DT;
        // Lead the target by its travel during the shell's flight.
        let flight_ticks = pos.distance(units.pos[t]) / step;
        let target_vel = FxVec2::from_angle(units.heading[t]) * (units.speed[t] / DT);
        let aim = units.pos[t] + target_vel * flight_ticks;
        let bearing = (aim - pos).angle();

        let aligned = if weapon.turret_turn == 0 {
            // Hull-mounted: the unit turns itself when it is not driving somewhere.
            if units.flags[row] & flag::MOVING == 0 {
                if let Some(m) = bp.unit(units.blueprint[row]).motion {
                    units.heading[row] = units.heading[row].turn_toward(bearing, m.turn_rate);
                }
            }
            units.heading[row].delta_to(bearing).unsigned_abs() <= weapon.half_arc.min(AIM_TOLERANCE * 4)
        } else {
            let mut want = bearing - units.heading[row];
            if weapon.half_arc < 0x8000 {
                let d = Angle::ZERO.delta_to(want);
                want = Angle(d.clamp(-(weapon.half_arc as i32) as i16, weapon.half_arc as i16) as u16);
            }
            let yaw = units.weapon_yaw[row][w].turn_toward(want, weapon.turret_turn);
            units.weapon_yaw[row][w] = yaw;
            (units.heading[row] + yaw).delta_to(bearing).unsigned_abs() <= AIM_TOLERANCE
        };

        if !aligned || units.weapon_cooldown[row][w] > 0 {
            return Ok(());
        }
        let gap = pos.distance(units.pos[t]) - target_bp.radius;
        if gap > weapon.range_max || gap < weapon.range_min - target_bp.radius * 2 {
            return Ok(());
        }

        if units.weapon_salvo_left[row][w] == 0 {
            units.weapon_salvo_left[row][w] = weapon.salvo;
        }
        units.weapon_salvo_left[row][w] -= 1;
        units.weapon_cooldown[row][w] = if units.weapon_salvo_left[row][w] > 0 { weapon.salvo_delay_ticks.max(1) as u16 } else { weapon.reload_ticks };

        let facing = units.heading[row] + units.weapon_yaw[row][w];
        let muzzle_xy = pos + FxVec2::new(weapon.muzzle.x, weapon.muzzle.y).rotate(facing);
        let muzzle = muzzle_xy.extend(units.z[row] + weapon.muzzle.z);
        let aim_z = units.z[t] + target_bp.height / 2;

        let mut delta = aim - muzzle_xy;
        if weapon.spread > 0 {
            let error = self.state.rng.below(weapon.spread as u32 * 2 + 1) as i32 - weapon.spread as i32;
            delta = delta.rotate(Angle(error as i16 as u16));
        }
        let dist = delta.length().max(Fx::ONE);
        let (vel, ticks) = match weapon.trajectory {
            Trajectory::Direct => {
                let dir = delta.extend(aim_z - muzzle.z).normalize();
                (dir * step, (weapon.range_max * Fx::ratio(13, 10) / step).ceil_int() + 2)
            }
            Trajectory::Ballistic => {
                // Land exactly on the aim point after n ticks of the integrator in `run_projectiles`.
                let n = (dist / step).ceil_int().max(1);
                let drop = GRAVITY * (n * (n + 1) / 2);
                let vz = (aim_z - muzzle.z + drop) / n;
                (FxVec2::new(delta.x / n, delta.y / n).extend(vz), n + 20)
            }
        };
        let units = &self.state.units;
        let (owner, id, blueprint) = (units.owner[row], units.id(row), units.blueprint[row]);
        self.state.projectiles.spawn(muzzle, vel, owner, id, blueprint, w as u8, ticks.clamp(1, u16::MAX as i32) as u16)?;
        self.events.push(SimEvent::ShotFired { pos: muzzle, color: weapon.color, owner });
        Ok(())
    }

    pub(crate) fn run_projectiles(&mut self) -> Result<(), SimError> {
        let count = self.state.projectiles.len();
        let this = &*self;
        let hits: Vec<Vec<Hit>> = self.pool.parallel_map_chunks(count, CHUNK, |_, range| {
            let mut out = Vec::new();
            for i in range {
                if let Some(hit) = this.sweep_projectile(i) {
                    out.push(hit);
                }
            }
            out
        });
        let hits: Vec<Hit> = hits.into_iter().flatten().collect();

        let p = &mut self.state.projectiles;
        for i in 0..count {
            if self.blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize].trajectory == Trajectory::Ballistic {
                p.vel[i].z -= GRAVITY;
            }
            p.pos[i] += p.vel[i];
            p.ticks_left[i] = p.ticks_left[i].saturating_sub(1);
        }

        let mut remove: Vec<usize> = Vec::with_capacity(hits.len());
        for hit in &hits {
            let i = hit.projectile;
            self.state.projectiles.pos[i] = hit.point;
            self.apply_impact(i, hit)?;
            remove.push(i);
        }
        let p = &self.state.projectiles;
        remove.extend((0..count).filter(|&i| p.ticks_left[i] == 0));
        remove.sort_unstable();
        remove.dedup();
        for &i in remove.iter().rev() {
            self.state.projectiles.swap_remove(i);
        }
        Ok(())
    }

    /// Finds what projectile `i` runs into during this tick's step, if anything.
    fn sweep_projectile(&self, i: usize) -> Option<Hit> {
        let p = &self.state.projectiles;
        let weapon = &self.blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize];
        let mut vel = p.vel[i];
        if weapon.trajectory == Trajectory::Ballistic {
            vel.z -= GRAVITY;
        }
        let from = p.pos[i];
        let to = from + vel;
        let mut best_t = Fx::MAX;
        let mut best = None;
        if let Some(point) = self.terrain.raycast(from, to) {
            best_t = (point.xy() - from.xy()).length() / vel.xy().length().max(Fx::EPSILON);
            best = Some(Hit { projectile: i, point, unit: None });
        }
        // Lobbed shells pass over things on the way up.
        if weapon.trajectory == Trajectory::Direct || vel.z < Fx::ZERO {
            let seg = vel.xy();
            let len_sq = seg.length_sq().max(Fx::EPSILON);
            let mid = from.xy() + seg * Fx::HALF;
            self.index.query(mid, seg.length() / 2, kind::UNIT, |e| {
                let row = e.row as usize;
                let units = &self.state.units;
                if !self.unit_entry_is_current(e) || !self.are_enemies(p.owner[i], units.owner[row]) {
                    return true;
                }
                let t = ((e.pos - from.xy()).dot(seg) / len_sq).clamp(Fx::ZERO, Fx::ONE);
                let point = from + vel * t;
                let height = self.bp(row).height;
                let inside = point.xy().distance_sq(e.pos) <= e.radius * e.radius && point.z >= units.z[row] - Fx::ONE && point.z <= units.z[row] + height + Fx::ONE;
                if inside && t < best_t {
                    best_t = t;
                    best = Some(Hit { projectile: i, point, unit: Some(row) });
                }
                true
            });
        }
        best
    }

    fn apply_impact(&mut self, projectile: usize, hit: &Hit) -> Result<(), SimError> {
        let p = &self.state.projectiles;
        let (owner, source) = (p.owner[projectile], p.source[projectile]);
        let blueprints = self.blueprints.clone();
        let weapon = &blueprints.unit(p.blueprint[projectile]).weapons[p.weapon[projectile] as usize];
        self.events.push(SimEvent::Impact { pos: hit.point, splash: weapon.splash, color: weapon.color });
        if weapon.splash > Fx::ZERO {
            self.blast(hit.point.xy(), weapon.splash, weapon.damage, owner, source)?;
        } else {
            if let Some(row) = hit.unit {
                self.damage_unit(row, weapon.damage, owner);
            } else {
                self.add_stain(hit.point.xy(), Fx::from_int(2) + weapon.damage.sqrt() / 4, 40)?;
            }
        }
        Ok(())
    }

    /// Area damage to enemies of `owner`, a crater stain, and flattened trees.
    fn blast(&mut self, center: FxVec2, radius: Fx, damage: Fx, owner: u8, _source: UnitId) -> Result<(), SimError> {
        let mut victims = Vec::new();
        self.index.query(center, radius, kind::UNIT, |e| {
            let row = e.row as usize;
            if self.unit_entry_is_current(e) && self.are_enemies(owner, self.state.units.owner[row]) && !self.state.units.has_flag(row, flag::IN_FACTORY) {
                victims.push(row);
            }
            true
        });
        for row in victims {
            self.damage_unit(row, damage, owner);
        }
        let mut felled = Vec::new();
        self.prop_index.query(center, radius, kind::PROP, |e| {
            felled.push(e.row as usize);
            true
        });
        for prop in felled {
            if self.map.props[prop].kind.is_tree() {
                self.state.props_dead[prop / 64] |= 1 << (prop % 64);
            }
        }
        self.add_stain(center, radius, 96)
    }

    fn damage_unit(&mut self, row: usize, damage: Fx, by: u8) {
        let units = &mut self.state.units;
        if units.health[row] <= Fx::ZERO {
            return;
        }
        units.health[row] -= damage;
        if units.health[row] <= Fx::ZERO {
            self.state.players[by as usize].units_killed += 1;
        }
    }

    /// Adds a ground stain, or deepens one that is already there.
    pub(crate) fn add_stain(&mut self, pos: FxVec2, radius: Fx, strength: u8) -> Result<(), SimError> {
        let mut existing = None;
        self.index.query(pos, Fx::ZERO, kind::STAIN, |e| {
            if e.pos.distance_sq(pos) <= (e.radius / 2) * (e.radius / 2) && e.radius >= radius / 2 {
                existing = Some(e.row as usize);
                return false;
            }
            true
        });
        let stains = &mut self.state.stains;
        match existing {
            Some(i) if i < stains.len() => {
                stains.strength[i] = stains.strength[i].saturating_add(strength / 2);
                stains.radius[i] = stains.radius[i].max(radius);
            }
            _ => {
                let seed = self.state.rng.next_u32() as u16;
                stains.push(pos, radius, strength, seed)?;
            }
        }
        Ok(())
    }

    pub(crate) fn reap_dead(&mut self) -> Result<(), SimError> {
        let mut dead = std::mem::take(&mut self.scratch.dead);
        dead.clear();
        let units = &self.state.units;
        dead.extend(units.slots.iter().filter(|&row| units.health[row] <= Fx::ZERO));
        for &row in &dead {
            // A commander's blast or a defeat can take out rows later in this list.
            if self.state.units.slots.is_alive(row) {
                self.despawn_unit(row, true)?;
            }
        }
        self.scratch.dead = dead;
        Ok(())
    }

    /// Kills a unit: effects, wreck, scorch mark, and defeat if it was a commander.
    pub(crate) fn despawn_unit(&mut self, row: usize, leave_wreck: bool) -> Result<(), SimError> {
        let units = &self.state.units;
        let bp = self.blueprints.unit(units.blueprint[row]).clone();
        let (pos, z, heading, owner) = (units.pos[row], units.z[row], units.heading[row], units.owner[row]);
        let visible = !units.has_flag(row, flag::IN_FACTORY);
        let complete = !units.has_flag(row, flag::UNDER_CONSTRUCTION);
        self.remove_unit_row(row, false)?;

        if visible {
            self.events.push(SimEvent::UnitDied { pos: pos.extend(z), blueprint: bp.id, owner });
            self.state.players[owner as usize].units_lost += 1;
            if leave_wreck {
                self.add_stain(pos, bp.radius * Fx::ratio(3, 2), 72)?;
                let mass = bp.cost_mass * bp.wreck_fraction;
                if complete && mass > Fx::ZERO {
                    self.state.wrecks.spawn(bp.id, pos, z, heading, mass)?;
                }
            }
        }
        if complete && bp.has(cat::COMMANDER) {
            self.blast(pos, COMMANDER_BLAST_RADIUS, COMMANDER_BLAST_DAMAGE, owner, Handle::NONE)?;
            if self.state.players[owner as usize].commander.index() == row {
                self.defeat_player(owner);
            }
        }
        Ok(())
    }

    /// Frees a unit row and everything hanging off it, with no death effects.
    pub(crate) fn remove_unit_row(&mut self, row: usize, keep_blocked: bool) -> Result<(), SimError> {
        // Whatever it was assembling dies with it.
        let units = &self.state.units;
        if let Some(t) = units.row(units.build_target[row]) {
            if units.has_flag(t, flag::IN_FACTORY) {
                self.remove_unit_row(t, true)?;
            }
        }
        self.state.orders.clear(&mut self.state.units, row);
        self.stop_moving(row);
        let units = &self.state.units;
        let bp = self.blueprints.unit(units.blueprint[row]);
        if bp.is_structure() && !keep_blocked && !units.has_flag(row, flag::UPGRADE) {
            let (min, max) = footprint_cells(bp, units.pos[row]);
            self.nav.unblock_cells(min, max);
        }
        self.state.units.slots.free(row);
        Ok(())
    }

    /// Assassination rules: lose the commander, lose everything.
    pub(crate) fn defeat_player(&mut self, player: u8) {
        let p = &mut self.state.players[player as usize];
        if p.defeated {
            return;
        }
        p.defeated = true;
        self.events.push(SimEvent::PlayerDefeated { player });
        let units = &mut self.state.units;
        for row in 0..units.slots.rows() {
            if units.slots.is_alive(row) && units.owner[row] == player {
                units.health[row] = Fx::ZERO;
            }
        }
    }
}
