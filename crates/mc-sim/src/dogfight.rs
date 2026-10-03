//! Fighters on air targets: a dogfight against anything they can turn with, gun runs
//! against a hull too wide to turn inside.
use crate::tables::flag;
use crate::{SimError, World};
use mc_core::{Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::{cat, Motion};

const DT: i32 = TICKS_PER_SECOND as i32;

impl World {
    /// Pursue the predicted intercept: attack, bank, attack (a big hull gets gun runs
    /// instead, `air_strafe`). The nose is always on the target or coming round onto it
    /// at the hardest turn the fighter has (movement slows it to its corner speed while
    /// the target is off the nose). Two things make
    /// it fly straight instead. A target inside the turning circle on its side cannot
    /// be brought onto the nose by turning, only circled, so it flies on until the
    /// target falls outside the circle. And a turning fight that has not brought the
    /// guns to bear for a few seconds (two fighters chasing each other round one
    /// circle) is broken off for under a second, at a moment staggered per fighter so
    /// matched opponents do not mirror each other forever.
    pub(crate) fn air_dogfight(&mut self, row: usize, target: usize) -> Result<(), SimError> {
        let units = &self.state.units;
        let pos = units.pos[row];
        let motion = self.bp(row).motion.expect("air");
        let tvel = units.air_velocity[target].xy();
        let intercept_time = (pos.distance(units.pos[target]) / motion.speed)
            .clamp(Fx::ratio(1, 5), Fx::ratio(3, 2));
        let lead = units.pos[target] + tvel * Fx::from_int(DT) * intercept_time;
        if self.air_gun_run(row, target) {
            return self.air_strafe(row, target, lead);
        }
        let nose = FxVec2::from_angle(units.heading[row]);
        // The guns bear inside this much of the nose.
        let bears = self.air_gun_arc(row);
        let to = lead - pos;
        let off = units.heading[row].delta_to(to.angle()).unsigned_abs();
        // The turning circle at the present speed, on the side the target lies.
        let speed = units.speed[row].max(motion.speed * Fx::ratio(5, 9));
        let yaw = self.air_turn_rate(row, &motion).max(1);
        let radius = speed.mul_div(10430, yaw as i64 * DT as i64);
        let side = if nose.perp().dot(to) < Fx::ZERO {
            -nose.perp()
        } else {
            nose.perp()
        };
        let inside = lead.distance(pos + side * radius) < radius;
        let mut turn_ticks = if off > bears {
            units.air_turn_ticks[row].saturating_add(1)
        } else {
            0
        };
        let mut break_ticks = units.air_break_ticks[row];
        let stagger = (row as u16).wrapping_mul(7919) % 15;
        let goal = if break_ticks > 0 {
            break_ticks -= 1;
            turn_ticks = 0;
            pos + nose * motion.speed
        } else if turn_ticks >= 25 + stagger {
            turn_ticks = 0;
            break_ticks = 6 + stagger / 2;
            pos + nose * motion.speed
        } else if inside || to.length() < Fx::from_int(16) {
            // Finish the crossing before reversing; never pivot on the target.
            pos + nose * motion.speed
        } else {
            lead
        };
        let goal = self.clamp_to_map(goal);
        self.ensure_moving(row, goal, goal)?;
        // Changing a movement goal clears old flight state; retain these
        // counters only for this continuing engagement.
        self.state.units.air_turn_ticks[row] = turn_ticks;
        self.state.units.air_break_ticks[row] = break_ticks;
        self.state.units.flags[row] |= flag::AIR_RUN;
        Ok(())
    }

    /// Whether `target` is fought in gun runs. A fixed gun has to be pointed: on a target
    /// much slower than the fighter, chasing puts it over the mark at once, overshooting
    /// and turning with the guns off it. Guided missiles turn onto the mark themselves,
    /// so a missile fighter circles even a big hull (from over its deck,
    /// `air_attack_height`) and fires more that way than in runs.
    pub(crate) fn air_gun_run(&self, row: usize, target: usize) -> bool {
        let Some(m) = self.bp(row).motion else {
            return false;
        };
        let slow = self.bp(target).motion.is_none_or(|t| t.speed * 2 < m.speed);
        let guns = self
            .bp(row)
            .weapons
            .iter()
            .any(|w| w.target_mask & cat::AIR != 0 && !w.guided);
        guns && slow
    }

    /// The height a fighter fights `target` at: its own, or over the deck of a hull it
    /// would otherwise fly through. That is one wider than its turning circle, or, on a
    /// gun run, one wider than the room it breaks off in (`air_break_off`).
    pub(crate) fn air_attack_height(&self, row: usize, target: usize) -> Fx {
        let z = self.state.units.prev_z[target];
        let hull = self.bp(target).radius;
        let over = self.bp(row).motion.is_some_and(|m| {
            hull > air_turn_radius(&m)
                || (hull > self.air_break_off(row) && self.air_gun_run(row, target))
        });
        if over {
            z + self.bp(target).height + crate::movement::FIGHTER_ATTACK_CLEARANCE
        } else {
            z
        }
    }

    /// How far short of a hull's side a gun run breaks off.
    fn air_break_off(&self, row: usize) -> Fx {
        self.bp(row).max_weapon_range() / 4
    }

    /// A gun run on a big hull: straight in on the guns, over the deck, then out at full
    /// throttle until there is room to come round and be lined up again by the time it
    /// is back in reach (`air_break_ticks` counts the run out). A pass ends once the
    /// fighter is over the hull and the mark has left the gun arc: turning there is
    /// what wound it round inside the ship, or past a small one.
    fn air_strafe(&mut self, row: usize, target: usize, lead: FxVec2) -> Result<(), SimError> {
        let units = &self.state.units;
        let pos = units.pos[row];
        let motion = self.bp(row).motion.expect("air");
        let nose = FxVec2::from_angle(units.heading[row]);
        let hull = self.bp(target).radius;
        let to = units.pos[target] - pos;
        let gap = to.length();
        let bears = self.air_gun_arc(row);
        let off = units.heading[row].delta_to(to.angle()).unsigned_abs();
        let mut extend = units.air_break_ticks[row];
        // Breaking off a little short of a small mark, never through it.
        let close = hull + self.air_break_off(row);
        if extend == 0 && gap < close && (off > bears || nose.dot(to) <= Fx::ZERO) {
            // Out past the far side to the guns' reach and a little more: the
            // reversal is flown out there, and it comes back in already on the line.
            let room = hull + self.bp(row).max_weapon_range() * Fx::ratio(6, 5) - gap;
            let step = (motion.speed / DT).max(Fx::ONE);
            extend = (room / step).ceil_int().clamp(1, u16::MAX as i32) as u16;
        }
        let goal = if extend > 0 {
            extend -= 1;
            pos + nose * motion.speed
        } else {
            lead
        };
        let goal = self.clamp_to_map(goal);
        self.ensure_moving(row, goal, goal)?;
        let units = &mut self.state.units;
        units.air_turn_ticks[row] = 0;
        units.air_break_ticks[row] = extend;
        if extend > 0 {
            // Off the mark on the way out, which would otherwise hold it to its
            // corner speed: the run out is flown at full throttle.
            units.air_aim[row] = goal;
        }
        units.flags[row] |= flag::AIR_RUN;
        Ok(())
    }

    /// Half the widest gun arc a fighter has on air targets.
    fn air_gun_arc(&self, row: usize) -> u16 {
        self.bp(row)
            .weapons
            .iter()
            .filter(|w| w.target_mask & cat::AIR != 0)
            .map(|w| w.half_arc)
            .max()
            .unwrap_or(0x2000)
            .clamp(0x0800, 0x2000)
    }
}

/// The turning radius at cruise speed and the steady turn rate.
fn air_turn_radius(motion: &Motion) -> Fx {
    motion
        .speed
        .mul_div(10430, (motion.turn_rate as i64 * DT as i64).max(1))
}
