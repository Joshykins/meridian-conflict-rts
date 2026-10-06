//! Orb batteries (`Weapon::orbs`, the Regency's Carillon): a gun that makes plasma orbs a
//! batch at a time while it stands idle and holds them, up to one for each of its
//! muzzles, until something comes into reach. The orbs drift round the spire like
//! fireflies, each about its muzzle's place (`place`). Each shot spends an orb and leaves
//! from where that orb floats, so the stored orbs are stored damage: a battery that has
//! waited long enough empties its whole store onto the field in one salvo.
//!
//! A shot is a fleck of plasma that flies straight and homes on its mark. When the mark
//! dies it takes the nearest foe the spire could still shoot, and when there is none it
//! flies home and is taken back into the store (`steer_orb_shots`).
//!
//! The store is `Units::orb_stock`, filled from slot 0 up and spent from the top slot
//! down, so a batch is the next `orbs` slots and the renderer is told which slots are
//! held (`UnitInstance::status[2]`, `World::held_orbs`).

use crate::spatial::kind;
use crate::tables::{flag, UnitId};
use crate::{SimEvent, World};
use mc_core::{Angle, Fx, FxVec2, FxVec3, TICKS_PER_SECOND};
use mc_data::Weapon;

const DT: i32 = TICKS_PER_SECOND as i32;
/// Ticks a fleck stays up (25 s) before it gutters out: a cap on a shot that can neither
/// reach a mark nor find its way home (its spire is gone).
pub(crate) const FLIGHT_TICKS: i32 = 25 * DT;
/// Share of the way to its wanted heading a fleck turns each tick, as a fraction.
const TURN: Fx = Fx::ratio(2, 5);
/// Strides from its aim point inside which a fleck turns straight onto it.
const CLOSE_STRIDES: i32 = 5;
/// Metres ahead a fleck reads the ground, how far apart, the clearance it keeps over it,
/// the margin it climbs with over what it reads (as a fraction), and the steepest climb.
const GROUND_LOOK: i32 = 480;
const GROUND_SAMPLE: i32 = 12;
const GROUND_CLEARANCE: Fx = Fx::from_int(8);
const GROUND_MARGIN: Fx = Fx::ratio(5, 4);
const MAX_CLIMB: Fx = Fx::from_int(3);
/// Metres from its spire's axis inside which a returning fleck is taken back in.
const HOME_REACH: Fx = Fx::from_int(14);
/// How far past the gun's range a fleck chases a mark before it gives it up, as a
/// fraction of the range.
const LEASH: Fx = Fx::ratio(5, 4);

/// Where the orb in slot `slot`, whose place is `home` (the weapon's muzzle, in the
/// spire's own frame), floats at sim tick `tick`: round the axis at a pace of its own, a
/// little off its path and bobbing. Integer maths only; the renderer draws the same place
/// (`orb_fx.rs`), so a shot leaves from the orb the player sees.
pub fn place(home: FxVec3, slot: usize, tick: u32) -> FxVec3 {
    let slot = slot as u32;
    let pace = 30 + slot * 13 % 50;
    let swing = tick.wrapping_mul(pace) as u16;
    let start = (slot as u16).wrapping_mul(2039);
    let round = Angle(if slot.is_multiple_of(2) {
        start.wrapping_add(swing)
    } else {
        start.wrapping_sub(swing)
    });
    let wander = Angle(
        (tick.wrapping_mul(23 + slot * 5 % 17) as u16)
            .wrapping_add((slot as u16).wrapping_mul(5003)),
    );
    let bob = Angle(
        (tick.wrapping_mul(17 + slot % 7 * 3) as u16)
            .wrapping_add((slot as u16).wrapping_mul(3001)),
    );
    let xy = home.xy().rotate(round)
        + FxVec2::new(wander.sin() * 3, wander.cos() * 3)
        + home.xy().rotate(round) * (bob.cos() / 6);
    xy.extend(home.z + bob.sin() * 4)
}

/// What one fleck does this tick.
struct Steer {
    shot: usize,
    target: UnitId,
    /// Where it turns toward; `None` flies straight on.
    wants: Option<FxVec3>,
    /// Home and taken back in.
    absorbed: bool,
}

impl World {
    /// The orb battery among `row`'s weapons, with its slot.
    pub(crate) fn orb_battery(&self, row: usize) -> Option<(usize, &Weapon)> {
        self.bp(row)
            .weapons
            .iter()
            .enumerate()
            .find(|(_, w)| w.orbs > 0)
    }

    /// Makes `row`'s next batch once its clock runs out, while the store has room for it.
    /// A full store stops the clock, so the first batch after a salvo takes a whole period.
    pub(crate) fn step_orbs(&mut self, row: usize) {
        let Some((slot, weapon)) = self.orb_battery(row) else {
            return;
        };
        let (batch, period, cap) = (weapon.orbs, weapon.orb_ticks, weapon.muzzles.len());
        let at = self.state.units.pos[row].extend(self.state.units.z[row]);
        let blueprint = self.state.units.blueprint[row];
        let units = &mut self.state.units;
        if units.orb_stock[row] as usize + batch as usize > cap {
            units.orb_clock[row] = 0;
            return;
        }
        units.orb_clock[row] += 1;
        if units.orb_clock[row] >= period {
            units.orb_clock[row] = 0;
            units.orb_stock[row] += batch;
            let held = units.orb_stock[row];
            self.events.push(SimEvent::OrbsMade {
                pos: at,
                blueprint,
                weapon: slot as u8,
                held,
            });
        }
    }

    /// Turns every orb shot onto its mark: a unit it homes on with a lead while that
    /// stands and is in reach, else the nearest foe the spire could shoot, else the spire
    /// itself, where it is taken back into the store. Shots of a spire that is gone fly on
    /// until they gutter out.
    pub(crate) fn steer_orb_shots(&mut self) {
        let blueprints = self.blueprints.clone();
        let mut steers: Vec<Steer> = Vec::new();
        let units = &self.state.units;
        let p = &self.state.projectiles;
        for i in 0..p.len() {
            let weapon = &blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize];
            if weapon.orbs == 0 {
                continue;
            }
            let step = weapon.projectile_speed / DT;
            let owner = p.owner[i];
            let at = p.pos[i];
            let spire = units
                .row(p.source[i])
                .filter(|&s| units.health[s] > Fx::ZERO);
            // The gun's own place, or the shot's if the gun is gone, is what reach is from.
            let centre = spire.map_or(at.xy(), |s| units.pos[s]);
            let leash = weapon.range_max * LEASH;
            let holds = |t: usize| {
                units.health[t] > Fx::ZERO
                    && !units.has_flag(t, flag::IN_FACTORY)
                    && self.weapon_reaches(t, weapon)
                    && units.pos[t].distance_sq(centre) <= leash * leash
            };
            let mut target = units.row(p.target[i]).filter(|&t| holds(t));
            if target.is_none() {
                let mut best: Option<(Fx, usize)> = None;
                self.index.query_foes(
                    centre,
                    weapon.range_max,
                    kind::UNIT,
                    self.team_mask(owner),
                    |e| {
                        let t = e.row as usize;
                        if !self.unit_entry_is_current(e)
                            || !holds(t)
                            || !self.are_enemies(owner, units.owner[t])
                            || !self.detects(owner, t)
                        {
                            return true;
                        }
                        let off = units.pos[t].distance_sq(at.xy());
                        if best.is_none_or(|(s, _)| off < s) {
                            best = Some((off, t));
                        }
                        true
                    },
                );
                target = best.map(|(_, t)| t);
            }
            let (wants, absorbed) = if let Some(t) = target {
                let aim = units.pos[t].extend(units.z[t] + self.bp(t).height / 2);
                let moving =
                    (units.pos[t] - units.prev_pos[t]).extend(units.z[t] - units.prev_z[t]);
                let lead = ((aim - at).length() / step).min(Fx::from_int(20));
                (Some(aim + moving * lead - at), false)
            } else if let Some(s) = spire {
                let home = units.pos[s].extend(units.z[s] + self.bp(s).height / 2);
                let near = units.pos[s].distance_sq(at.xy()) <= HOME_REACH * HOME_REACH;
                (Some(home - at), near)
            } else {
                (None, false)
            };
            steers.push(Steer {
                shot: i,
                target: target.map_or(UnitId::NONE, |t| units.id(t)),
                wants,
                absorbed,
            });
        }
        let mut gone = Vec::new();
        for s in steers {
            let i = s.shot;
            self.state.projectiles.target[i] = s.target;
            if s.absorbed {
                gone.push(i);
                let source = self.state.projectiles.source[i];
                if let Some(row) = self.state.units.row(source) {
                    let cap = self.orb_battery(row).map_or(0, |(_, w)| w.muzzles.len());
                    // A store with no room takes nothing back: the fleck dissolves.
                    if (self.state.units.orb_stock[row] as usize) < cap {
                        self.state.units.orb_stock[row] += 1;
                    }
                }
                continue;
            }
            let Some(wants) = s.wants else { continue };
            let vel = self.state.projectiles.vel[i];
            let speed = vel.length();
            // Its turning circle is wider than what it hits: close in it turns straight onto
            // the aim point, or it would circle a mark (or the spire) it keeps passing.
            let turn = if wants.length() <= speed * CLOSE_STRIDES {
                Fx::ONE
            } else {
                TURN
            };
            let want = wants.normalize();
            let level = want.xy().length();
            // Over rising ground it flies at least the slope that clears it; it never
            // levels off short of its aim point, and straight above it just drops.
            let wanted = match self.ground_slope(i, wants.length()) {
                Some(ground) if level > Fx::ratio(1, 10) && ground > want.z / level => {
                    want.xy().normalize().extend(ground).normalize()
                }
                _ => want,
            };
            let dir = (vel.normalize() * (Fx::ONE - turn) + wanted * turn).normalize();
            if dir != FxVec3::ZERO {
                self.state.projectiles.aim[i] = dir;
                self.state.projectiles.vel[i] = dir * speed;
            }
        }
        for i in gone.into_iter().rev() {
            self.state.projectiles.swap_remove(i);
        }
    }

    /// The steepest rise over run fleck `i` needs from where it is to clear the ground
    /// along the way it is heading, read every `GROUND_SAMPLE` metres out to
    /// `GROUND_LOOK`, with a margin. The stretch round its aim point (`reach` metres off)
    /// is left out: it goes down onto it. `None` when nothing is read.
    fn ground_slope(&self, i: usize, reach: Fx) -> Option<Fx> {
        let p = &self.state.projectiles;
        let (pos, vel) = (p.pos[i], p.vel[i]);
        let ahead = vel.xy().normalize();
        let stride = vel.length();
        let mut slope: Option<Fx> = None;
        for j in 1..=GROUND_LOOK / GROUND_SAMPLE {
            let along = Fx::from_int(j * GROUND_SAMPLE);
            if along + stride * 2 >= reach {
                break;
            }
            let rise = self.terrain.height_at(pos.xy() + ahead * along) + GROUND_CLEARANCE - pos.z;
            slope = Some(slope.map_or(rise / along, |s| s.max(rise / along)));
        }
        slope.map(|s| if s > Fx::ZERO { s * GROUND_MARGIN } else { s }.min(MAX_CLIMB))
    }

    /// `UnitInstance::status[2]` for an orb battery: a bit for each slot holding an orb.
    /// `None` for a unit without one.
    pub(crate) fn held_orbs(&self, row: usize) -> Option<u32> {
        self.orb_battery(row)?;
        let held = self.state.units.orb_stock[row] as u32;
        Some(if held >= 32 {
            u32::MAX
        } else {
            (1u32 << held) - 1
        })
    }
}
