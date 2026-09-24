//! The capital ships' weapons (docs/NAVY.md): interceptor torpedoes that run at
//! torpedoes coming in, sea-skimming and high-arc missiles, and the missile launch
//! that gives a dived submarine away.
//!
//! An interceptor (`Weapon::intercepts`) never takes a unit for its target. This pass
//! fires it at the nearest enemy torpedo in reach that no other interceptor is
//! already after; the shot carries that torpedo's `serial` as its `quarry` and runs
//! at it with a lead. Both burst when they meet. Interceptors are a countermeasure:
//! they work on a passive hull or one holding its fire, and stop while the owner's
//! grid is dark, like the Manta's laser.

use crate::mirror::SimEvent;
use crate::{SimError, World};
use mc_core::{Fx, FxVec2, FxVec3, TICKS_PER_SECOND};
use mc_data::Weapon;

const DT: i32 = TICKS_PER_SECOND as i32;
/// Metres an interceptor must pass within of its quarry to burst it.
const INTERCEPT_REACH: Fx = Fx::from_int(4);
/// Longest lead an interceptor takes on its quarry, in ticks.
const INTERCEPT_LEAD: i32 = 10;
/// Shallowest an interceptor runs: a metre under the surface, as a torpedo does.
const CEILING: Fx = Fx::ONE;
/// Ticks a missile launch keeps a dived hull on enemy radar and vision: 8 s.
pub(crate) const LAUNCH_REVEAL: u16 = 8 * DT as u16;
/// Horizontal metres from its mark inside which a sea skimmer stops hugging the
/// surface and steers straight at it.
const SKIM_TERMINAL: Fx = Fx::from_int(120);
/// Ticks of flight ahead a sea skimmer looks for rising ground.
const SKIM_LOOK: i32 = 3;
/// How far off the vertical a high arc leans toward its mark while it climbs (tan).
const CLIMB_LEAN: Fx = Fx::ratio(3, 20);

impl World {
    /// Interceptor tubes fire at enemy torpedoes in reach. Runs before `run_weapons`,
    /// which leaves these weapons alone: the cooldown is kept here.
    pub(crate) fn run_torpedo_defence(&mut self) -> Result<(), SimError> {
        let blueprints = self.blueprints.clone();
        // Torpedoes some interceptor is already running at.
        let mut marked: Vec<u32> = self
            .state
            .projectiles
            .quarry
            .iter()
            .copied()
            .filter(|&q| q != 0)
            .collect();
        for row in 0..self.state.units.slots.rows() {
            let units = &self.state.units;
            if !units.slots.is_alive(row)
                || !units.is_active(row)
                || units.health[row] <= Fx::ZERO
            {
                continue;
            }
            let bp = blueprints.unit(units.blueprint[row]);
            for (w, weapon) in bp.weapons.iter().enumerate() {
                if !weapon.intercepts {
                    continue;
                }
                let cooldown = &mut self.state.units.weapon_cooldown[row][w];
                *cooldown = cooldown.saturating_sub(1);
                if *cooldown > 0 {
                    continue;
                }
                let owner = self.state.units.owner[row];
                if self.state.players[owner as usize].efficiency <= Fx::ZERO {
                    continue;
                }
                let hull = self.state.units.pos[row].extend(self.state.units.z[row]);
                let Some(i) = self.incoming_torpedo(owner, hull, weapon.range_max, &marked) else {
                    continue;
                };
                marked.push(self.state.projectiles.serial[i]);
                self.launch_interceptor(row, w, weapon, i)?;
            }
        }
        Ok(())
    }

    /// The nearest enemy torpedo in the water within `reach` of `hull` that no
    /// interceptor is after. Ties go to the earlier row.
    fn incoming_torpedo(&self, owner: u8, hull: FxVec3, reach: Fx, marked: &[u32]) -> Option<usize> {
        let p = &self.state.projectiles;
        let water = self.terrain.water_level();
        let mut best: Option<(Fx, usize)> = None;
        for i in 0..p.len() {
            let weapon = &self.blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize];
            if !weapon.torpedo
                || weapon.intercepts
                || p.pos[i].z > water
                || !self.are_enemies(owner, p.owner[i])
                || marked.contains(&p.serial[i])
            {
                continue;
            }
            let d2 = (p.pos[i] - hull).length_sq();
            if d2 <= reach * reach && best.is_none_or(|(b, _)| d2 < b) {
                best = Some((d2, i));
            }
        }
        best.map(|(_, i)| i)
    }

    /// Weapon `w` of `row` sends an interceptor at torpedo `i`, from its next tube.
    fn launch_interceptor(
        &mut self,
        row: usize,
        w: usize,
        weapon: &Weapon,
        i: usize,
    ) -> Result<(), SimError> {
        let water = self.terrain.water_level();
        let units = &self.state.units;
        let p = &self.state.projectiles;
        // The tubes take turns, by the number the shot is about to get.
        let next = p.next_serial.wrapping_add(1).max(1) as usize;
        let local = if weapon.muzzles.is_empty() {
            weapon.muzzle
        } else {
            weapon.muzzles[next % weapon.muzzles.len()]
        };
        let muzzle = (units.pos[row] + local.xy().rotate(units.heading[row]))
            .extend(units.z[row] + local.z);
        let facing = (p.pos[i].xy() - muzzle.xy()).angle();
        let quarry = p.serial[i];
        let (owner, id, blueprint) = (units.owner[row], units.id(row), units.blueprint[row]);
        let travel = (units.pos[row] - units.prev_pos[row]).extend(units.z[row] - units.prev_z[row]);
        let (at, vel, ticks) =
            crate::naval::torpedo_launch(muzzle, facing, weapon, water, None, FxVec2::ZERO);
        self.state.projectiles.spawn(
            at,
            vel,
            owner,
            id,
            blueprint,
            w as u8,
            ticks.clamp(1, u16::MAX as i32) as u16,
        )?;
        let shot = self.state.projectiles.len() - 1;
        self.state.projectiles.quarry[shot] = quarry;
        self.state.units.weapon_cooldown[row][w] = weapon.reload_ticks;
        self.muzzles.push(at);
        self.events.push(SimEvent::ShotFired {
            pos: at,
            vel,
            travel,
            color: weapon.color,
            owner,
            blueprint,
            weapon: w as u8,
        });
        Ok(())
    }

    /// Whether interceptor `i` has lost its quarry: burst or run out. It bursts at once
    /// (`torpedo_burst`).
    pub(crate) fn quarry_lost(&self, i: usize) -> bool {
        let p = &self.state.projectiles;
        p.quarry[i] != 0 && !p.serial.contains(&p.quarry[i])
    }

    /// Interceptors run at a lead on their quarry, 3D, under the surface. One that
    /// passes within `INTERCEPT_REACH` of it this tick bursts it: both go.
    /// Runs after `steer_torpedoes`, so the quarry's way this tick is known.
    pub(crate) fn steer_interceptors(&mut self) {
        let water = self.terrain.water_level();
        let mut gone: Vec<usize> = Vec::new();
        for i in 0..self.state.projectiles.len() {
            let p = &self.state.projectiles;
            if p.quarry[i] == 0 || gone.contains(&i) {
                continue;
            }
            let Some(q) = (0..p.len()).find(|&j| p.serial[j] == p.quarry[i]) else {
                continue;
            };
            if gone.contains(&q) {
                continue;
            }
            let weapon = &self.blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize];
            let step = weapon.projectile_speed / DT;
            let (pos, them, their_way) = (p.pos[i], p.pos[q], p.vel[q]);
            let closing = (step + their_way.length()).max(Fx::EPSILON);
            let lead = ((them - pos).length() / closing).min(Fx::from_int(INTERCEPT_LEAD));
            let aim = them + their_way * lead;
            let dir = (aim - pos).normalize();
            let dir = if dir == FxVec3::ZERO {
                p.vel[i].normalize()
            } else {
                dir
            };
            let mut vel = dir * step;
            vel.z = vel.z.min(water - CEILING - pos.z);
            // Closest the two come over this tick's steps.
            let rel = pos - them;
            let dv = vel - their_way;
            let t = if dv.length_sq() > Fx::EPSILON {
                (-rel.dot(dv) / dv.length_sq()).clamp(Fx::ZERO, Fx::ONE)
            } else {
                Fx::ZERO
            };
            let gap = rel + dv * t;
            let p = &mut self.state.projectiles;
            p.vel[i] = vel;
            p.aim[i] = vel.normalize();
            if gap.length_sq() <= INTERCEPT_REACH * INTERCEPT_REACH {
                self.events.push(SimEvent::TorpedoIntercepted {
                    pos: pos + vel * t,
                });
                gone.push(i);
                gone.push(q);
            }
        }
        gone.sort_unstable();
        gone.dedup();
        for &i in gone.iter().rev() {
            self.state.projectiles.swap_remove(i);
        }
    }

    /// Where guided missile `i` of `weapon` wants to fly this tick, for the two naval
    /// doctrines; `desired` is the plain homing direction. A missile with a unit to
    /// track flies at it; with none (fired at the ground) at the point it was fired at.
    ///
    /// - A sea skimmer (`Weapon::skim`) runs toward its mark at `skim` metres over
    ///   ground and water, looking `SKIM_LOOK` ticks ahead for rising ground, until it
    ///   is within `SKIM_TERMINAL` of the mark across; then it homes.
    /// - A high arc (`Weapon::apogee`) climbs nearly straight up, leaning toward its
    ///   mark, until it reaches `apogee` or has come within a quarter of its launch
    ///   distance of the mark across; once turned over (falling) it homes.
    pub(crate) fn naval_guidance(&self, i: usize, weapon: &Weapon, desired: FxVec3) -> FxVec3 {
        if weapon.skim <= Fx::ZERO && weapon.apogee <= Fx::ZERO {
            return desired;
        }
        let p = &self.state.projectiles;
        let units = &self.state.units;
        let pos = p.pos[i];
        let tracked = units
            .row(p.target[i])
            .filter(|&t| units.health[t] > Fx::ZERO);
        let mark = match tracked {
            Some(t) => units.pos[t].extend(units.z[t] + self.bp(t).height / 2),
            None => p.mark[i],
        };
        let desired = match tracked {
            Some(_) => desired,
            None => match (mark - pos).normalize() {
                d if d == FxVec3::ZERO => desired,
                d => d,
            },
        };
        let across = mark.xy() - pos.xy();
        let dist = across.length();
        if weapon.apogee > Fx::ZERO {
            let launch = p.origin[i].distance(mark.xy());
            let lit = p.age[i] <= weapon.cold_launch_ticks.saturating_add(1);
            let climbing = pos.z < weapon.apogee
                && dist * 4 > launch
                && (lit || p.vel[i].z > Fx::ZERO);
            if climbing {
                let lean = across.normalize() * CLIMB_LEAN;
                return lean.extend(Fx::ONE).normalize();
            }
            return desired;
        }
        if dist <= SKIM_TERMINAL {
            return desired;
        }
        let step = weapon.projectile_speed / DT;
        let way = match p.vel[i].xy().normalize() {
            w if w == FxVec2::ZERO => across.normalize(),
            w => w,
        };
        let water = self.terrain.water_level();
        let surface = |at: FxVec2| self.terrain.height_at(at).max(water);
        let mut floor = surface(pos.xy());
        for k in 1..=SKIM_LOOK {
            floor = floor.max(surface(pos.xy() + way * (step * k)));
        }
        let ahead = desired.xy().normalize();
        let ahead = if ahead == FxVec2::ZERO {
            across.normalize()
        } else {
            ahead
        };
        (ahead * (step * SKIM_LOOK))
            .extend(floor + weapon.skim - pos.z)
            .normalize()
    }
}
