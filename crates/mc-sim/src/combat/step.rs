//! A gun's tick: turning, pitching, charging and firing (`World::step_weapon`), out of
//! `combat.rs`, whose private helpers it shares.

use super::*;

impl World {
    /// The torso gun that turns the torso this tick: the hardest-hitting one with something
    /// to shoot, so a main gun is aimed with the whole body and the others fire where it
    /// points. `None`: none of them has anything, and the torso comes home.
    fn torso_lead(&self, row: usize) -> Option<usize> {
        let weapons = &self.bp(row).weapons;
        (0..weapons.len())
            .filter(|&i| on_torso(weapons, i) && self.weapon_mark(row, i, &weapons[i]).is_some())
            .max_by_key(|&i| (weapons[i].damage, std::cmp::Reverse(i)))
    }

    /// One tick of weapon `w` of `row`: each gun of a group in turn (`mounts.rs`), else
    /// the weapon itself.
    pub(super) fn step_weapon(&mut self, row: usize, w: usize) -> Result<(), SimError> {
        if self.bp(row).weapons[w].is_group() {
            return self.step_group(row, w);
        }
        self.step_gun(row, w, None)
    }

    /// One gun's tick: weapon `w` of `row`, or gun `m` of its group, whose state
    /// `step_group` has laid into the weapon's slot.
    pub(crate) fn step_gun(
        &mut self,
        row: usize,
        w: usize,
        m: Option<usize>,
    ) -> Result<(), SimError> {
        let bp = self.blueprints.clone();
        let group = &bp.unit(self.state.units.blueprint[row]).weapons[w];
        let weapon = m.map_or(group, |m| group.gun(m));
        let gun = (bp.unit(self.state.units.blueprint[row]).gun_slot(w) + m.unwrap_or(0)) as u16;
        // Interceptor tubes are fired and reloaded by `run_torpedo_defence`, and a
        // `dash_only` weapon goes off only as its unit lands from a dash (`dash.rs`).
        if weapon.intercepts || weapon.dash_only {
            return Ok(());
        }
        if let Some((centre, radius)) = self.ground_mark(row).filter(|m| m.1 > Fx::ZERO) {
            // First shot of a bombardment, or the circle has moved: choose a point in it.
            // A point this gun cannot reach from here is given up for another.
            // A bomber's point is where it flies to, so reach does not matter.
            // A gun riding another's turret lays on that gun's point, which it picks.
            let aim = self.state.units.ground_aim[row][w];
            let gap = aim.distance(self.state.units.pos[row]);
            let bomber = self.bombard_lead(row).is_some();
            if takes_ground(self.bp(row), weapon, false)
                && self.bombard_slot(row, w) == w
                && (aim.distance(centre) > radius
                    || (!bomber && (gap > weapon.range_max || gap < weapon.range_min)))
            {
                self.pick_bombard_aim(row, w);
            }
        }
        let mut mark = self.weapon_mark(row, w, weapon);
        // A ship spooling its warp drive is turning onto the jump (`step_spool`): its hull is
        // the drive's, and no gun may turn it off the mark, or the nose never lines up and
        // the jump never goes. Turrets keep firing.
        let spooling = self.state.units.warp[row].phase == WarpPhase::Spool;
        // A mark in a spinal gun's dead zone, too far in under the hull to dive onto, is
        // not its to take: the hull stays level and the turrets have it. Nor is any mark
        // while the ship is set down or spooling: the hull cannot turn or pitch for it.
        if spinal_gun(bp.unit(self.state.units.blueprint[row]), w, weapon) {
            let from = self.state.units.pos[row].extend(self.state.units.z[row]);
            let grounded = self.grounded_hull(row) || spooling;
            mark = mark.filter(|t| {
                !grounded && spinal_bears(from, weapon.muzzle.z, t.pos.extend(t.z + t.height / 2))
            });
        }
        if weapon.depression.0 > 0 {
            let u = &self.state.units;
            let hull = self.hull_pitch(row);
            mark = mark.filter(|t| {
                let to = t.pos.extend(t.z + t.height / 2);
                let (_, elevation) =
                    house_lay(u.pos[row], u.z[row], u.heading[row], hull, weapon, to);
                dips_to(weapon, elevation)
            });
        }
        // A drone launcher lays nothing: it lets a drone go at the mark (`strike_drones.rs`).
        if weapon.launches {
            let mark = mark.map(|m| (m.unit, m.pos, m.radius));
            return self.step_launcher(row, w, mark);
        }
        let on_body = on_torso(&bp.unit(self.state.units.blueprint[row]).weapons, w);
        let lead = if on_body { self.torso_lead(row) } else { None };
        // Whether this gun turns the torso and pitches the arm, or rides where they point.
        let drives = if weapon.mount {
            true
        } else if on_body {
            lead == Some(w)
        } else {
            w == 0
        };
        // A shoulder gun under water is silent (a torpedo launcher's house is meant to be
        // under it, as the Breakwater's is); so is every gun of a unit that is building,
        // except one on a turret of its own.
        let drowned = (weapon.mount
            && !weapon.torpedo
            && self.state.units.z[row] + weapon.pivot.unwrap_or(weapon.muzzle).z
                < self.terrain.water_level())
            || self.drowned_on_seabed(row, weapon);
        let units = &mut self.state.units;
        let held = drowned || (units.has_flag(row, flag::WORKING) && !weapon.mount);
        // A rotary gun spins up while it has something to shoot, and down again after. A
        // beam that runs up (the Edict Lance) runs down here too, and up again only once
        // its shot is laid (`run_up_beam`), so it lights as it fills, not long after.
        if weapon.spin_ticks > 0 {
            let spin = &mut units.spin[row][weapon.rotary as usize];
            spin[3] = spin[0];
            spin[0] = if mark.is_some() && !held && !weapon.beam {
                (spin[0] + 1).min(weapon.spin_ticks)
            } else {
                spin[0].saturating_sub(1)
            };
            let step = if weapon.barrels > 0 {
                barrel_step(weapon, spin[0], spin[1])
            } else {
                (SPIN_TOP as u32 * spin[0] as u32 / weapon.spin_ticks as u32) as u16
            };
            spin[1] = spin[1].wrapping_add(step);
            spin[2] = step;
        }
        // The stream ends once the barrels have run down, or at once for a gun without any.
        if weapon.sweep > 0
            && (mark.is_none() || held)
            && (weapon.spin_ticks == 0 || units.spin[row][weapon.rotary as usize][0] == 0)
        {
            units.streaming[row] = false;
        }
        // True if this tick used up a countdown (reload or a first-shot charge).
        // A weapon that sat ready (cooldown already 0) still has to charge before it fires.
        let cooling = units.weapon_cooldown[row][w] > 0;
        // A reload fed from the grid is counted down by the economy (`fed_reload.rs`).
        if cooling
            && !crate::fed_reload::fed(
                weapon,
                units.weapon_cooldown[row][w],
                units.weapon_salvo_left[row][w],
            )
        {
            units.weapon_cooldown[row][w] -= 1;
        }
        if held && weapon.mount {
            units.weapon_salvo_left[row][w] = 0;
            units.arm_pitch[row][2 + w] =
                units.arm_pitch[row][2 + w].turn_toward(Angle::ZERO, weapon.turret_turn / 2);
            return Ok(());
        }
        if held {
            // The torso is turned to its work: the orders phase aims it, and the guns wait.
            units.weapon_salvo_left[row][w] = 0;
            // The Thunderhead's slot 0 is its hull's flight pitch (`movement.rs`), not a gun's.
            if w == 0 && bp.unit(units.blueprint[row]).visual.mesh != "assault_air" {
                let rate = hull_pitch_rate(bp.unit(units.blueprint[row]), w, weapon);
                units.arm_pitch[row][0] = units.arm_pitch[row][0].turn_toward(Angle::ZERO, rate);
            }
            return Ok(());
        }
        // A siege gun only slews once the spade is down. While it is packing,
        // rolling, or still planting, the turret and tube come back to rest.
        // A lift ship's `deploy` is its ramp, not a spade.
        let planted = bp.unit(units.blueprint[row]).transport.is_some()
            || bp
                .unit(units.blueprint[row])
                .motion
                .is_none_or(|m| m.deploy_ticks == 0 || units.deploy[row] >= m.deploy_ticks);
        let Some(t) = mark.filter(|_| planted) else {
            // Nothing to shoot, or the gun is not planted: turrets drift back
            // to centre, arms come level.
            // A shoulder gun faces where the torso does.
            // A torso gun with nothing leaves the torso to the one that has something. One
            // that sways comes back in line with it, and level.
            if on_body && w != 0 {
                let torso = units.weapon_yaw[row][0];
                units.weapon_yaw[row][w] = if weapon.sway.0 > 0 {
                    let rate = hull_pitch_rate(bp.unit(units.blueprint[row]), w, weapon);
                    units.arm_pitch[row][2 + w] =
                        units.arm_pitch[row][2 + w].turn_toward(Angle::ZERO, rate);
                    within_sway(
                        torso,
                        units.weapon_yaw[row][w].turn_toward(torso, weapon.turret_turn),
                        weapon.sway,
                    )
                } else {
                    torso
                };
                units.weapon_salvo_left[row][w] = 0;
                return Ok(());
            }
            if (on_body && lead.is_some()) || weapon.keeps_aim {
                // A gun that keeps its aim (`Weapon::keeps_aim`) stays laid where it was.
                units.weapon_salvo_left[row][w] = 0;
                return Ok(());
            }
            // A ship's mount stands on the hull, not on another turret: it comes home to the bow.
            // So does a gun house on a land hull built like one (`hull_mounts`).
            let naval = bp
                .unit(units.blueprint[row])
                .motion
                .is_some_and(|m| m.layer == mc_data::MoveLayer::Naval)
                || (bp.unit(units.blueprint[row]).hull_mounts && weapon.mount);
            // A mount that faces off the nose (`facing`, or aft for `rear`) rests that way.
            let home = if weapon.mount && !naval {
                units.weapon_yaw[row][0]
            } else if weapon.mount {
                weapon.facing
            } else {
                Angle::ZERO
            };
            units.weapon_yaw[row][w] =
                units.weapon_yaw[row][w].turn_toward(home, weapon.turret_turn / 2);
            if weapon.mount
                || (w == 0 && bp.unit(units.blueprint[row]).visual.mesh != "assault_air")
            {
                let slot = pitch_slot(weapon, w);
                // An AA gun on the ground or afloat waits pointed at the sky, not at the
                // horizon: on its turret, on a structure, or in a house of its own on a hull
                // (the Fulgur's rotary gun, a frigate's AA mount), not on a shoulder. Its
                // model is drawn level; this is what holds it up.
                let sky_gun = (!weapon.mount || naval)
                    && !weapon.missile
                    && weapon.pivot.is_some()
                    && weapon.target_mask & !cat::AIR == 0
                    && bp
                        .unit(units.blueprint[row])
                        .motion
                        .is_none_or(|m| m.layer != mc_data::MoveLayer::Air);
                let rest = if weapon.rest_pitch != Angle::ZERO {
                    weapon.rest_pitch
                } else if sky_gun {
                    IDLE_SKY_PITCH
                } else {
                    Angle::ZERO
                };
                let rate = hull_pitch_rate(bp.unit(units.blueprint[row]), w, weapon);
                units.arm_pitch[row][slot] = units.arm_pitch[row][slot].turn_toward(rest, rate);
            }
            units.weapon_salvo_left[row][w] = 0;
            return Ok(());
        };
        // How far it may traverse and pitch this tick: a gun that nothing visible turns
        // (`Weapon::lays_at_once`) lays at once until it is lit, then drags at its rate.
        let traverse = if weapon.lays_at_once && !units.streaming[row] {
            0x8000
        } else {
            weapon.turret_turn
        };

        // Heard charging a fixed time before the salvo is due. During a reload that
        // is presentation only: the countdown is already the wait. The first shot,
        // and a shot after sitting idle, start a real charge below once the tube is on.
        // Skip the tick that just began an idle charge (`!cooling` and cooldown
        // already set would be a second start, not this path).
        // A lens with its front out is not charging (`lens.rs`).
        let lens_out = weapon.lens.is_some()
            && self
                .state
                .lenses
                .iter()
                .any(|t| t.source == units.id(row) && t.weapon as usize == w);
        // A fed reload is heard charging from the economy (`fed_reload.rs`).
        if weapon.charge_ticks > 0
            && weapon.reload_energy <= Fx::ZERO
            && cooling
            && !lens_out
            && units.weapon_cooldown[row][w] == weapon.charge_ticks
            && units.weapon_salvo_left[row][w] == 0
        {
            let at = charging_point(units, row, weapon, m.is_some());
            self.events.push(SimEvent::WeaponCharging {
                unit: units.id(row),
                pos: at,
                owner: units.owner[row],
                blueprint: units.blueprint[row],
                weapon: w as u8,
                gun,
            });
            if let (Some(storm), Some(m)) = (weapon.bore.and_then(|b| b.storm), mark.as_ref()) {
                self.events.push(SimEvent::StormCharging {
                    unit: units.id(row),
                    muzzle: at,
                    target: m.pos.extend(m.z),
                    on: m.unit.map(|t| units.id(t)),
                    radius: storm.radius,
                    ticks: weapon.charge_ticks,
                    owner: units.owner[row],
                    blueprint: units.blueprint[row],
                    weapon: w as u8,
                });
            }
        }

        let pos = units.pos[row];
        // A ship's turrets stand off the hull's origin, on the deck: each traverses about
        // its own pivot, carried round by the hull alone, and aims from there. A land hull
        // with gun houses (`hull_mounts`) carries them the same way.
        let naval = bp
            .unit(units.blueprint[row])
            .motion
            .is_some_and(|m| m.layer == mc_data::MoveLayer::Naval)
            || (bp.unit(units.blueprint[row]).hull_mounts && weapon.mount);
        // A turret that swings about a point off the middle (`turret_at`, a scorpion's
        // tail root) aims from there. So does a shoulder gun, from its trunnion on the
        // torso: well off the middle on a giant, it would lay wide of its own shells.
        let yaw_origin = match (weapon.pivot, bp.unit(units.blueprint[row]).turret_at) {
            (Some(p), _) if naval => pos + p.xy().rotate(units.heading[row]),
            (Some(p), _) if weapon.mount => {
                pos + p.xy().rotate(units.heading[row] + units.weapon_yaw[row][0])
            }
            (_, Some(at)) if w == 0 => pos + at.rotate(units.heading[row]),
            _ => pos,
        };
        let aircraft = bp
            .unit(units.blueprint[row])
            .motion
            .filter(|m| m.layer == mc_data::MoveLayer::Air);
        let bomb =
            aircraft.is_some() && weapon.trajectory == Trajectory::Ballistic && !weapon.missile;
        // The bomb bursts part-way through its last tick: the sight works from
        // the unrounded fall, or every stick lands most of a tick short.
        let fall = if bomb {
            bomb_fall(units.z[row] + weapon.muzzle.z - t.z - t.height / 2)
        } else {
            Fx::ZERO
        };
        let fall_ticks = fall.ceil_int();
        if let Some(motion) = aircraft {
            // No firing from a parked aircraft or during a low-speed launch.
            if (!motion.hover && !units.has_flag(row, flag::AIR_RUN) && weapon.half_arc < 0x8000)
                || (!motion.hover
                    && weapon.half_arc < 0x8000
                    && units.speed[row]
                        < motion.speed * if bomb { Fx::ratio(3, 4) } else { Fx::HALF })
                || (bomb
                    && (!units.has_flag(row, flag::AIR_RUN)
                        || units.speed[row] < motion.speed * Fx::ratio(3, 4)))
                || units.z[row]
                    < self.terrain.height_at(pos)
                        + if bp.unit(units.blueprint[row]).is_capital_ship() {
                            // A capital ship's guns fire all the way down, and from the ground.
                            -Fx::ONE
                        } else if bp.unit(units.blueprint[row]).visual.mesh == "assault_air" {
                            // Keep the strafing cannon live below cruise altitude.
                            crate::movement::ASSAULT_PASS_CLEARANCE / 2
                        } else if weapon.torpedo {
                            // A torpedo is let go low over the water, on the run in.
                            crate::movement::TORPEDO_RUN_HEIGHT / 2
                        } else if bomb {
                            // The sight works from the true fall, and `AIR_RUN` at speed
                            // already rules out a launch: a run in off low ground onto a
                            // cliff top, still climbing, drops rather than going round.
                            motion.altitude / 4
                        } else if !motion.hover && bp.unit(units.blueprint[row]).has(cat::ANTI_AIR)
                        {
                            // Pursuit can take fighters well below their cruise band.
                            crate::movement::FIGHTER_ATTACK_CLEARANCE / 2
                        } else {
                            motion.altitude * Fx::ratio(4, 5)
                        }
                // Nor from high up: it would fall too far and go in anyhow.
                || (weapon.torpedo
                    && units.z[row]
                        > self.terrain.height_at(pos).max(self.terrain.water_level())
                            + crate::movement::TORPEDO_RUN_HEIGHT * 2)
            {
                units.weapon_salvo_left[row][w] = 0;
                return Ok(());
            }
        }
        let step = weapon.projectile_speed / DT;
        let target_center = t.pos.extend(t.z + t.height / 2);
        let direct_air =
            weapon.trajectory == Trajectory::Direct && !weapon.missile && !weapon.guided && t.air;
        let target_velocity = t.moved;
        // A ship's house leads its mark from its own pivot, which on a giant hull stands a
        // kilometre off the middle and as much nearer or further from the mark.
        let lead_from = if naval { yaw_origin } else { pos };
        let air_aim = direct_air.then(|| {
            direct_air_aim(
                lead_from.extend(units.z[row] + weapon.pivot.unwrap_or(weapon.muzzle).z),
                target_center,
                target_velocity,
                t.turn,
                step,
            )
        });
        let ground_z = if aircraft.is_some() {
            air_strike_z(t.z, t.height)
        } else {
            target_center.z
        };
        let aim_z = air_aim.map_or(ground_z, |aim| aim.z);
        // Rockets aim at the current position; guided missiles update their intercept in flight.
        let aim = if let Some(aim) = air_aim {
            aim.xy()
        } else if bomb {
            let half_salvo = ((weapon.salvo.saturating_sub(1) / weapon.salvo_batch) as i32
                * weapon.salvo_delay_ticks as i32)
                / 2;
            t.pos + t.moved.xy() * Fx::from_int(fall_ticks + half_salvo)
        } else if weapon.missile {
            t.pos
        } else {
            let flight_ticks = match weapon.trajectory {
                Trajectory::Direct => lead_from.distance(t.pos) / step,
                Trajectory::Ballistic => {
                    Fx::from_int(ballistic_ticks(lead_from.distance(t.pos), weapon))
                }
            };
            t.pos + t.lead * flight_ticks
        };
        // A gun house on a pitched hull turns and elevates in the deck's frame.
        let deck = (weapon.mount && hull_pitched(bp.unit(units.blueprint[row]))).then(|| {
            house_lay(
                pos,
                units.z[row],
                units.heading[row],
                units.arm_pitch[row][0],
                weapon,
                aim.extend(aim_z),
            )
        });
        // A torso gun that sways lays its own yaw and pitch within its sway of the torso,
        // and lays its bore, which hangs beside the torso's axis, through the mark.
        let swaying = on_body && w != 0 && weapon.sway.0 > 0;
        let bearing = match deck {
            Some((yaw, _)) => units.heading[row] + yaw,
            None => (aim - yaw_origin).angle(),
        };
        // How far the bore's lay is off the bearing: the torso itself still turns to the
        // bearing, or a leading arm would hold it off the mark and its pods never fire.
        let lay_off = if swaying && deck.is_none() {
            crate::bore_lay::yaw(weapon, bearing, yaw_origin.distance(aim)) - bearing
        } else {
            Angle::ZERO
        };
        let bearing = bearing + lay_off;

        // An arm points up or down at its target as well as round to it.
        // Howitzers elevate to the lob, not the line of sight, and wait until the tube is there.
        let mut pitched_on = true;
        let slot = pitch_slot(weapon, w);
        // A vertical launcher's cells stand as they are, and a rack built at its rake
        // (`fixed_rack`): they never pitch.
        let laid = weapon
            .pivot
            .filter(|_| !weapon.vertical_launch && !weapon.fixed_rack);
        if let (true, Some(pivot)) = (drives || swaying, laid) {
            let want = if weapon.missile {
                missile_rack_pitch(weapon)
            } else if weapon.trajectory == Trajectory::Ballistic {
                ballistic_tube_pitch(
                    yaw_origin.distance(aim),
                    t.z + t.height / 2,
                    units.z[row],
                    weapon,
                    pivot,
                )
            } else if weapon.bore.is_some() && aircraft.is_none() {
                // Elevation is relative to the tilted hull, while the target is in world
                // space. The central turret yaws about the hull, independent houses
                // about their own pivots. Use the same terrain basis as the muzzle.
                let facing = units.heading[row] + units.weapon_yaw[row][w];
                let pivot_yaw = if weapon.mount {
                    units.heading[row]
                } else {
                    facing
                };
                let local_pivot = pivot.xy().rotate(pivot_yaw).extend(pivot.z);
                let lean = |v| {
                    crate::world::leaned(
                        &self.terrain,
                        pos,
                        bp.unit(units.blueprint[row]).radius,
                        units.heading[row],
                        v,
                    )
                };
                let forward = lean(FxVec2::from_angle(facing).extend(Fx::ZERO));
                let up = lean(FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE));
                let delta = (aim - pos).extend(aim_z - units.z[row]) - lean(local_pivot);
                if swaying {
                    // Already measured from the pivot along the laid bore.
                    crate::bore_lay::pitch(weapon, pivot, delta.dot(forward), delta.dot(up))
                } else {
                    crate::world::pitch_to(delta.dot(forward), delta.dot(up))
                }
            } else if swaying {
                let rise = aim_z - (units.z[row] + pivot.z);
                let run = crate::bore_lay::run(weapon, pivot, yaw_origin.distance(aim));
                crate::bore_lay::pitch(weapon, pivot, run, rise)
            } else {
                let rise = aim_z - (units.z[row] + pivot.z);
                if (direct_air && aircraft.is_none())
                    || bp.unit(units.blueprint[row]).is_capital_ship()
                {
                    // AA mounts track overhead aircraft beyond a working arm's 35-degree limit,
                    // and a capital ship's turrets look straight down at the ground.
                    let world = FxVec2::new(yaw_origin.distance(aim), rise).angle();
                    if let Some((_, elevation)) = deck {
                        // The house rides a pitched hull: it elevates off the deck.
                        let lay = Angle::ZERO.delta_to(elevation) as i32;
                        // No lower than the house may dip before its rails meet the deck.
                        let floor = if weapon.depression.0 > 0 {
                            -(weapon.depression.0 as i32)
                        } else {
                            i32::MIN
                        };
                        Angle(lay.max(floor) as i16 as u16)
                    } else {
                        world
                    }
                } else {
                    crate::world::pitch_to(yaw_origin.distance(aim), rise)
                }
            };
            let rate = if weapon.trajectory == Trajectory::Ballistic
                || weapon.missile
                || traverse != weapon.turret_turn
            {
                traverse
            } else {
                traverse / 2
            };
            units.arm_pitch[row][slot] = units.arm_pitch[row][slot].turn_toward(want, rate);
            if weapon.trajectory == Trajectory::Ballistic || weapon.missile {
                pitched_on = units.arm_pitch[row][slot] == want;
            } else if (direct_air && aircraft.is_none()) || weapon.bore.is_some() {
                pitched_on =
                    units.arm_pitch[row][slot].delta_to(want).unsigned_abs() <= AIM_TOLERANCE;
            }
        }

        // A strafing beam lays its stroke ahead of the hull, not on the mark (`strafe.rs`).
        let stroke = weapon.strafe.and_then(|strafe| {
            crate::strafe::stroke(
                pos,
                units.heading[row],
                strafe,
                weapon.half_arc,
                t.pos,
                t.radius,
            )
        });
        let aligned = if weapon.strafe.is_some() {
            stroke.is_some()
        } else if bomb {
            let nose = FxVec2::from_angle(units.heading[row]);
            let to = aim - pos;
            let half_salvo = Fx::ratio(
                (weapon.salvo.saturating_sub(1) / weapon.salvo_batch) as i64
                    * weapon.salvo_delay_ticks as i64,
                2,
            );
            let travel = units.speed[row] / DT;
            let release = travel * (fall + half_salvo);
            let along = to.dot(nose);
            let cross = to.cross(nose).abs();
            // Open the bay on the tick nearest the release line, so the centre
            // of the carpet lands on the target instead of two ticks short of
            // it. A bay that was not ready on the line may still open just past.
            // Once pickled, finish the rack while flying on past the release line.
            units.weapon_salvo_left[row][w] > 0
                || (along > Fx::ZERO
                    && along - release <= travel / 2
                    && along - release >= -travel * 2
                    && cross <= t.radius + weapon.splash.max(Fx::from_int(4)))
        } else if weapon.orbs > 0 {
            // An orb battery throws from the orbs round it, whichever way its mark lies.
            true
        } else if weapon.turret_turn == 0
            && (aircraft.is_none() || bp.unit(units.blueprint[row]).is_capital_ship())
            && (weapon.guided || weapon.vertical_launch)
        {
            // Launch cells on a hull (a capital ship's too), or a missile that homes: no
            // need to point the hull.
            true
        } else if weapon.turret_turn == 0 && weapon.torpedo && aircraft.is_none() {
            // Fixed tubes on a hull or a structure (the Fathom's batteries): a torpedo
            // steers itself onto the mark once it is out of the tube, so the tubes fire at
            // anything inside their arc, bow tubes all round, astern too. A hull that is
            // not driving somewhere still comes round onto the mark.
            if units.flags[row] & flag::MOVING == 0 && !spooling {
                if let Some(m) = bp.unit(units.blueprint[row]).motion {
                    units.heading[row] = units.heading[row].turn_toward(bearing, m.turn_rate);
                }
            }
            units.heading[row]
                .delta_to(bearing - weapon.facing)
                .unsigned_abs()
                <= weapon.half_arc
        } else if weapon.turret_turn == 0 {
            // Hull-mounted: the unit turns itself when it is not driving somewhere.
            if units.flags[row] & flag::MOVING == 0 && !spooling {
                if let Some(m) = bp.unit(units.blueprint[row]).motion {
                    units.heading[row] = units.heading[row].turn_toward(bearing, m.turn_rate);
                }
            }
            // A warship's spinal gun lies along the keel: the hull pitches onto the mark as
            // well, nose down onto the ground, and fires only once the bore is on it.
            if spinal_gun(bp.unit(units.blueprint[row]), w, weapon) {
                let want =
                    spinal_pitch(pos.extend(units.z[row]), weapon.muzzle.z, aim.extend(aim_z));
                let rate = hull_pitch_rate(bp.unit(units.blueprint[row]), w, weapon);
                units.arm_pitch[row][0] = units.arm_pitch[row][0].turn_toward(want, rate);
                pitched_on = units.arm_pitch[row][0].delta_to(want).unsigned_abs() <= AIM_TOLERANCE;
            }
            // A started salvo stays pickled through a bombing run as the target
            // slides aft; the first drop still needs a tight heading.
            // An aerial torpedo steers itself onto the mark once it is in the water.
            let slack = if weapon.torpedo {
                Angle::from_degrees(40).0
            } else if units.weapon_salvo_left[row][w] > 0 {
                AIM_TOLERANCE * 16
            } else {
                AIM_TOLERANCE * 4
            };
            units.heading[row].delta_to(bearing).unsigned_abs() <= weapon.half_arc.min(slack)
        } else {
            // The arc is centred where the weapon faces (`facing`; aft for `rear`).
            let base = weapon.facing;
            let mut want = bearing - units.heading[row] - base;
            // A main turret that reaches only so far across the nose (`Motion::aim_arc`).
            let body_arc = bp
                .unit(units.blueprint[row])
                .motion
                .map_or(0x8000, |m| m.aim_arc);
            // A broadside hull (`Motion::broadside`), whatever layer it flies or sails in.
            let broadside = bp
                .unit(units.blueprint[row])
                .motion
                .is_some_and(|m| m.broadside.0 > 0);
            let half_arc = if w == 0 {
                weapon.half_arc.min(body_arc)
            } else {
                weapon.half_arc
            };
            // Whether the mark is inside the arc at all: at the end of its traverse a gun
            // is not on a mark that lies beyond it.
            let mut in_arc = true;
            // How far the hull turned under the gun this tick (below), which it takes out
            // of its own traverse to stay laid where it was.
            let facing_before = units.heading[row];
            if half_arc < 0x8000 {
                let d = Angle::ZERO.delta_to(want);
                // A ship's main gun cannot bear astern: a stopped ship turns its hull to bring
                // the mark into the arc. Its other mounts keep tracking by themselves.
                // A ship that fights broadside on (`Motion::broadside`) lays its beam to the
                // mark instead, on whichever side is nearer, so every battery bears.
                // A land unit with an `aim_arc` turns its body the same way.
                if (naval || broadside || body_arc < 0x8000)
                    && w == 0
                    && units.flags[row] & flag::MOVING == 0
                    && !spooling
                {
                    if let Some(m) = bp.unit(units.blueprint[row]).motion {
                        if m.broadside.0 > 0 {
                            let to = (t.pos - pos).angle();
                            let lay = if units.heading[row].delta_to(to) >= 0 {
                                to - m.broadside
                            } else {
                                to + m.broadside
                            };
                            units.heading[row] = units.heading[row].turn_toward(lay, m.turn_rate);
                        } else {
                            // A held beam's mount (the Harrow's tail) handles small aim
                            // changes on its own, so the stream drags across the ground from
                            // mark to mark; the hull steps round only for the last quarter
                            // of its traverse. Other limited guns square up.
                            let tail = weapon.beam;
                            let free = if tail {
                                ((half_arc as u32 * 3) / 4) as u16
                            } else {
                                half_arc
                            };
                            if d.unsigned_abs() > free || (body_arc < 0x8000 && !tail) {
                                units.heading[row] =
                                    units.heading[row].turn_toward(bearing - base, m.turn_rate);
                            }
                        }
                    }
                }
                // Off the nose as it stands now the hull has turned this tick, or the gun
                // lays past its mark while the body comes round and swings back once it
                // stops (the Harrow's tail wagging over its target).
                let d = Angle::ZERO.delta_to(bearing - units.heading[row] - base);
                in_arc = d.unsigned_abs() <= half_arc;
                want = Angle(d.clamp(-(half_arc as i32) as i16, half_arc as i16) as u16);
            }
            // Combat can turn a beam walker's body after movement has run.
            // Count that ground too, so its feet step rather than swivel on the spot.
            if w == 0 && weapon.beam {
                let turn = facing_before.delta_to(units.heading[row]).unsigned_abs() as i32;
                let radius = bp.unit(units.blueprint[row]).radius;
                let ground = (radius * turn).mul_div(355, 113 * 0x10000);
                let step = (ground * 256).floor_int().clamp(0, u16::MAX as i32) as u16;
                units.gait[row] = units.gait[row].wrapping_add(step as u32);
                units.gait_step[row][0] = units.gait_step[row][0].saturating_add(step);
            }
            // Guns sharing a torso (`on_torso`) turn it together: the lead turns it, the
            // others ride where it points, or swing onto their own marks within their sway.
            let torso = if on_body && drives {
                units.weapon_yaw[row][0].turn_toward(want + base - lay_off, weapon.turret_turn)
            } else {
                units.weapon_yaw[row][0]
            };
            let yaw = if swaying {
                let lay = within_sway(torso, want + base, weapon.sway);
                within_sway(
                    torso,
                    units.weapon_yaw[row][w].turn_toward(lay, weapon.turret_turn),
                    weapon.sway,
                )
            } else if on_body {
                torso
            } else {
                // Stabilised: the hull's turn this tick is taken out first (within the
                // gun's arc), so a turret whose body comes round under it holds its lay
                // and traverses only for the rest.
                let turned = facing_before.delta_to(units.heading[row]);
                let held = units.weapon_yaw[row][w] - Angle(turned as u16);
                if half_arc < 0x8000 {
                    // Within its arc it traverses inside the arc, never across the dead
                    // zone: the short way round from over one bow to the other runs through
                    // the bow, so the aft battery goes the long way, by the stern.
                    let limit = half_arc as i32;
                    let from = (Angle::ZERO.delta_to(held - base) as i32).clamp(-limit, limit);
                    let to = Angle::ZERO.delta_to(want) as i32;
                    let step = traverse as i32;
                    base + Angle((from + (to - from).clamp(-step, step)) as i16 as u16)
                } else {
                    held.turn_toward(want + base, traverse)
                }
            };
            if on_body {
                units.weapon_yaw[row][0] = torso;
                // An arm that laid earlier this tick is carried with the torso as it
                // turns: never past its sway of where the torso now points.
                let weapons = &bp.unit(units.blueprint[row]).weapons;
                for (i, other) in weapons.iter().enumerate().skip(1) {
                    if i != w && other.sway.0 > 0 && on_torso(weapons, i) {
                        units.weapon_yaw[row][i] =
                            within_sway(torso, units.weapon_yaw[row][i], other.sway);
                    }
                }
            }
            units.weapon_yaw[row][w] = yaw;
            if weapon.sweep > 0 && !weapon.missile {
                // A sweeping gun fires while its mark is anywhere in the cone.
                (units.heading[row] + yaw).delta_to(bearing).unsigned_abs() <= weapon.sweep
            } else if weapon.missile {
                let off = (bearing - units.heading[row] - base)
                    .delta_to(Angle::ZERO)
                    .unsigned_abs();
                // An unguided rocket flies where its rack points, its arc solved along the
                // rack, so the rack (or the torso it rides) waits until it is on the mark:
                // fired while still coming round, the rockets flew off to the side and short,
                // a walker's down at its own feet.
                let on_mark =
                    (units.heading[row] + yaw).delta_to(bearing).unsigned_abs() <= AIM_TOLERANCE;
                if aircraft.is_none() {
                    weapon.guided || weapon.vertical_launch || on_mark
                } else {
                    off <= weapon.half_arc && (weapon.guided || on_mark)
                }
            } else if weapon.trajectory == Trajectory::Ballistic {
                // `want` is off the weapon's facing; the turret's yaw is off the nose.
                in_arc && yaw == want + base
            } else {
                // Held at the end of its traverse, it is not on a mark just beyond it.
                in_arc
                    && (units.heading[row] + yaw).delta_to(bearing).unsigned_abs() <= AIM_TOLERANCE
            }
        };

        if w == 0 && bp.unit(units.blueprint[row]).visual.mesh == "assault_air" {
            // The rotary cannon is fixed to the nose. Heading alone permits
            // near-vertical shots as the aircraft passes over a ground target.
            // Match the rendered muzzle's bank and hull pitch before measuring
            // the complete firing solution, and require an inbound flight path.
            let local = weapon.muzzle;
            let rolled = crate::world::pitched(
                FxVec3::new(local.y, local.x, local.z),
                Some(FxVec3::ZERO),
                Angle(units.bank[row] as u16),
            );
            let offset = crate::world::pitched(
                FxVec3::new(rolled.y, rolled.x, rolled.z),
                Some(FxVec3::ZERO),
                units.arm_pitch[row][0],
            );
            // The gun swivels a little in its mount (`turret_turn`, `arc`), about its breech.
            let facing = units.heading[row] + units.weapon_yaw[row][0];
            let breech = FxVec2::new(
                assault_gun_pivot_x(bp.unit(units.blueprint[row]).radius),
                Fx::ZERO,
            );
            let muzzle =
                (pos + breech.rotate(units.heading[row]) + (offset.xy() - breech).rotate(facing))
                    .extend(units.z[row] + offset.z);
            let elevation = FxVec2::from_angle(units.arm_pitch[row][0]);
            let gun = (FxVec2::from_angle(facing) * elevation.x).extend(elevation.y);
            let solution = aim.extend(aim_z) - muzzle;
            if (t.pos - pos).dot(units.air_velocity[row].xy()) <= Fx::ZERO
                || gun.dot(solution.normalize()) < FxVec2::from_angle(ASSAULT_GUN_CONE).x
            {
                return Ok(());
            }
        }

        // A sweeping gun fires while anything it may shoot is in the cone ahead of it,
        // its mark or not. Once it is streaming it keeps firing as it swings onto a new
        // mark outside the cone, walking the rounds across to it.
        let aligned = aligned
            || (weapon.sweep > 0
                && !weapon.missile
                && (self.state.units.streaming[row] || self.target_in_cone(row, w, weapon)));
        // The ground is in the way: keep the gun on it, and wait for a shot.
        let hidden = t.unit.is_some() && self.shot_blocked(row, w);
        // A lift ship's gun reaches the ground only along the line of sight (`Weapon::slant`).
        let origin = self.gun_origin(row, weapon);
        let gap = match t.unit {
            Some(u) => self.hull_gap(origin, u),
            None => origin.distance(t.pos) - t.radius,
        };
        let slant_out = t
            .unit
            .is_some_and(|u| !self.slant_reaches(row, u, weapon, gap));
        let in_reach = gap
            <= self.reach_onto(row, weapon, t.z + t.height / 2, t.moved.xy().length())
            && gap >= weapon.range_min - t.radius * 2;
        // A lens fires nothing more while its front is out (`lens.rs`).
        if weapon.lens.is_some() && self.lens_out(row, w) {
            return Ok(());
        }
        let units = &mut self.state.units;
        if !(aligned && pitched_on && !hidden) || units.weapon_cooldown[row][w] > 0 {
            return Ok(());
        }
        if weapon.beam && weapon.spin_ticks > 0 {
            if !in_reach || slant_out {
                return Ok(());
            }
            run_up_beam(units, row, weapon);
        }
        let spun = if weapon.spin_ramp > 0 {
            weapon.spin_ticks.div_ceil(3)
        } else {
            weapon.spin_ticks
        };
        let spin = units.spin[row][weapon.rotary as usize];
        if weapon.spin_ticks > 0 && spin[0] < spun {
            return Ok(());
        }
        // A barrel-timed rotary gun fires only as a barrel comes up to the top.
        if weapon.barrels > 0 && !barrel_topped(weapon.barrels, spin[1], spin[2]) {
            return Ok(());
        }
        if let Some(motion) = bp.unit(units.blueprint[row]).motion {
            if motion.deploy_ticks > 0
                && units.deploy[row] < motion.deploy_ticks
                && bp.unit(units.blueprint[row]).transport.is_none()
            {
                return Ok(());
            }
        }
        // A cell launcher waits for its hatches to open (`launch_cells.rs`).
        if weapon.hatch_ticks > 0 && units.deploy[row] < weapon.hatch_ticks {
            return Ok(());
        }
        // An orb battery with nothing in store has nothing to throw (`orbs.rs`).
        if weapon.orbs > 0 && units.orb_stock[row] == 0 {
            units.weapon_salvo_left[row][w] = 0;
            return Ok(());
        }
        if !in_reach || slant_out {
            return Ok(());
        }
        // On target and ready, but it has not just finished a countdown: charge first.
        // A twin waits its turn and is seen charging at the end of the wait (`twins.rs`).
        if weapon.charge_ticks > 0 && !cooling && units.weapon_salvo_left[row][w] == 0 {
            // A fed charge runs at its reload's pace, set out so it takes as long as it is
            // heard (`fed_reload.rs`).
            let charge_at = self.charge_mark(row, weapon);
            let countdown = self.twin_charge(row, w, weapon) - weapon.charge_ticks + charge_at;
            let units = &mut self.state.units;
            units.weapon_cooldown[row][w] = countdown;
            if countdown > charge_at {
                return Ok(());
            }
            let at = charging_point(units, row, weapon, m.is_some());
            self.events.push(SimEvent::WeaponCharging {
                unit: units.id(row),
                pos: at,
                owner: units.owner[row],
                blueprint: units.blueprint[row],
                weapon: w as u8,
                gun,
            });
            if let (Some(storm), Some(m)) = (weapon.bore.and_then(|b| b.storm), mark.as_ref()) {
                self.events.push(SimEvent::StormCharging {
                    unit: units.id(row),
                    muzzle: at,
                    target: m.pos.extend(m.z),
                    on: m.unit.map(|t| units.id(t)),
                    radius: storm.radius,
                    ticks: weapon.charge_ticks,
                    owner: units.owner[row],
                    blueprint: units.blueprint[row],
                    weapon: w as u8,
                });
            }
            return Ok(());
        }

        if units.weapon_salvo_left[row][w] == 0 {
            units.weapon_salvo_left[row][w] = weapon.salvo;
        }
        // Whether it was already streaming before this shot: a held beam walks on from
        // where it last landed (`Units::beam_at`).
        let streamed = units.streaming[row];
        if weapon.sweep > 0 && !weapon.missile {
            units.streaming[row] = true;
        }
        // Zero delay dumps the rest of the salvo on this tick.
        let volley = if weapon.salvo_delay_ticks == 0 {
            units.weapon_salvo_left[row][w]
        } else {
            weapon.salvo_batch.min(units.weapon_salvo_left[row][w])
        };
        units.weapon_salvo_left[row][w] -= volley;
        units.weapon_cooldown[row][w] = if units.weapon_salvo_left[row][w] > 0 {
            weapon.salvo_delay_ticks.max(1) as u16
        } else if weapon.spin_ramp > 100 && weapon.spin_ticks > 0 {
            // Slower while it is still spinning up: `spin_ramp` times at a standstill, its
            // own reload at full spin.
            let (full, spin) = (
                weapon.spin_ticks as u32,
                units.spin[row][weapon.rotary as usize][0].min(weapon.spin_ticks) as u32,
            );
            let stretch = 100 * full + (weapon.spin_ramp as u32 - 100) * (full - spin);
            ((weapon.reload_ticks as u32 * stretch).div_ceil(100 * full)).clamp(1, u16::MAX as u32)
                as u16
        } else {
            weapon.reload_ticks
        };
        // Twin barrels of the same gun take turns (`twins.rs`).
        if units.weapon_salvo_left[row][w] == 0 {
            self.stagger_twins(row, w, weapon);
        }
        let units = &mut self.state.units;

        let facing = units.heading[row]
            + if weapon.vertical_launch {
                Angle::ZERO
            } else {
                units.weapon_yaw[row][w]
            };
        // A sweeping gun fires down its barrel, not at its target: the stream walks onto
        // what is in the cone as the torso comes round. A held beam off its mark lays onto
        // the surface under the barrel, so as it slews from one mark to the next it drags a
        // line across the ground; units in its path intercept it.
        let (aim, aim_z) = if let Some(at) = stroke {
            (at, Fx::ZERO)
        } else if weapon.sweep > 0 && !weapon.missile {
            // Down the barrel from where the gun turns (`turret_at`: the Harrow's tail root).
            let root = pos
                + bp.unit(units.blueprint[row])
                    .turret_at
                    .map_or(FxVec2::ZERO, |at| at.rotate(units.heading[row]));
            let down = root + FxVec2::from_angle(facing) * root.distance(aim);
            // A held beam's stroke walks over the ground in a straight line from where it
            // last landed onto its mark, as fast as its gun's traverse would carry it at
            // that reach (never under a third of its range): turning the barrel at the
            // mark's own distance swung it round in a circle, a tight one under the hull
            // for a mark close below, where it seemed to fire straight down.
            let down = if weapon.beam && streamed {
                let from = units.beam_at[row];
                let reach = root.distance(from).max(weapon.range_max / 3);
                // The traverse a tick, as ground at that reach: turns of 0x10000 steps.
                let step = (reach * weapon.turret_turn as i32).mul_div(710, 113 * 0x10000);
                from + (aim - from).clamp_length(step)
            } else {
                down
            };
            let off_mark = weapon.beam && down.distance(aim) > t.radius;
            let z = if off_mark {
                self.terrain.height_at(down).max(self.terrain.water_level())
            } else {
                aim_z
            };
            if weapon.beam {
                units.beam_at[row] = if off_mark { down } else { aim };
            }
            (down, z)
        } else {
            (aim, aim_z)
        };
        let unit_z = units.z[row];
        let heading = units.heading[row];
        let travel = (units.pos[row] - units.prev_pos[row]).extend(unit_z - units.prev_z[row]);
        let arm_pitch = units.arm_pitch[row][slot];
        let torso = units.heading[row] + units.weapon_yaw[row][0];
        let owner = units.owner[row];
        let id = units.id(row);
        let blueprint = units.blueprint[row];
        let left = units.weapon_salvo_left[row][w];
        let first_tube =
            weapon.salvo.saturating_sub(1) as usize - left as usize - (volley as usize - 1);
        // Weapons on the first weapon's arm (the same elbow) are carried up and down with it.
        let arm = bp
            .unit(blueprint)
            .weapons
            .first()
            .and_then(|first| first.pivot);
        let pivot = weapon
            .pivot
            .filter(|p| !weapon.fixed_rack && (w == 0 || weapon.mount || Some(*p) == arm));

        let mut wakes = Vec::new();
        let mut latch = None;
        for i in 0..volley {
            let tube = first_tube + i as usize;
            let (aim, aim_z) = if stroke.is_some() {
                // Each beam lands on the ground in its own line, beside the others.
                let local = weapon.muzzles[tube % weapon.muzzles.len()];
                let gap = weapon.strafe.map_or(Fx::ZERO, |s| s.gap);
                let at = crate::strafe::beam_at(aim, heading, local, gap);
                (
                    at,
                    self.terrain.height_at(at).max(self.terrain.water_level()),
                )
            } else if weapon.corkscrew > Fx::ZERO {
                let phase = Angle::from_degrees(
                    (tube % 6) as i32 * 60 + if w.is_multiple_of(2) { 30 } else { 0 },
                );
                let area = aim + FxVec2::from_angle(phase) * (weapon.splash * Fx::ratio(4, 5));
                (
                    area,
                    self.terrain.height_at(area).max(self.terrain.water_level()),
                )
            } else {
                (aim, aim_z)
            };
            // A salvo longer than the rack goes round its tubes again. An orb battery's shot
            // leaves from the top slot still holding an orb, and spends it (`orbs.rs`).
            let local = if weapon.orbs > 0 {
                units.orb_stock[row] = units.orb_stock[row].saturating_sub(1);
                if units.orb_stock[row] == 0 {
                    units.weapon_salvo_left[row][w] = 0;
                }
                let slot = units.orb_stock[row] as usize;
                crate::orbs::place(weapon.muzzles[slot], slot, self.state.tick)
            } else if weapon.muzzles.is_empty() {
                weapon.muzzle
            } else {
                weapon.muzzles[tube % weapon.muzzles.len()]
            };
            let local = if weapon.rear {
                FxVec3::new(-local.x, -local.y, local.z)
            } else {
                local
            };
            // A rail gun's slugs leave down their own rails and fly on side by side, the
            // rails' spacing apart, not crossed onto one point: each is laid on the mark
            // moved over by its rail's offset from the bore.
            let aim = if weapon.rail && !weapon.muzzles.is_empty() {
                let bore = if weapon.rear {
                    -weapon.muzzle.y
                } else {
                    weapon.muzzle.y
                };
                aim + FxVec2::new(Fx::ZERO, local.y - bore).rotate(facing)
            } else {
                aim
            };
            let at = crate::world::pitched(local, pivot, arm_pitch);
            // Aircraft turrets yaw around their chin mount, not the hull origin.
            let yaw_pivot = if bp.unit(blueprint).visual.mesh == "assault_air" && w == 0 {
                // The Thunderhead's cannon swivels about its breech (`assault_gun_pivot_x`).
                Some(FxVec3::new(
                    assault_gun_pivot_x(bp.unit(blueprint).radius),
                    Fx::ZERO,
                    Fx::ZERO,
                ))
            } else if aircraft.is_some() || naval {
                // A ship's turret or mount turns about its own pivot on the deck.
                pivot
            } else {
                None
            };
            let muzzle_xy = if let Some(p) = yaw_pivot {
                pos + p.xy().rotate(units.heading[row]) + (at.xy() - p.xy()).rotate(facing)
            } else if let (true, Some(p)) = (weapon.mount, pivot) {
                // A shoulder gun: its trunnion rides the torso, the barrel turns about it.
                // A vertical launcher on the turret (the Strider's) turns with it, cells and
                // all: it never turns on its own.
                let turn = if weapon.vertical_launch {
                    torso
                } else {
                    facing
                };
                pos + p.xy().rotate(torso) + (at.xy() - p.xy()).rotate(turn)
            } else {
                pos + bp.unit(blueprint).turret_point(
                    FxVec2::new(at.x, at.y),
                    units.heading[row],
                    facing,
                )
            };
            let muzzle = if aircraft.is_some() {
                let offset =
                    (muzzle_xy - pos).rotate(Angle(0u16.wrapping_sub(units.heading[row].0)));
                let bank = Angle(units.bank[row] as u16);
                let rolled = crate::world::pitched(
                    FxVec3::new(offset.y, offset.x, at.z),
                    Some(FxVec3::ZERO),
                    bank,
                );
                let hull_pitch = if bp.unit(blueprint).visual.mesh == "assault_air" {
                    arm_pitch
                } else if hull_pitched(bp.unit(blueprint)) {
                    // A warship laying its spinal gun pitches the whole hull (`spinal_gun`).
                    units.arm_pitch[row][0]
                } else if aircraft.is_some_and(|m| {
                    crate::hover_flight::flies(
                        &m,
                        bp.unit(blueprint).transport.is_some(),
                        bp.unit(blueprint).is_capital_ship(),
                    )
                }) {
                    // A hover aircraft leans with its lift (`hover_flight::lean`).
                    units.arm_pitch[row][crate::hover_flight::PITCH_SLOT]
                } else if aircraft.is_some_and(|m| m.hover) {
                    // Hovering hulls lean with forward travel rather than yaw.
                    let forward_step = (units.pos[row] - units.prev_pos[row])
                        .dot(FxVec2::from_angle(units.heading[row]));
                    Angle((-forward_step * 365).floor_int().clamp(-1252, 1252) as i16 as u16)
                } else {
                    crate::world::pitch_to(
                        (units.pos[row] - units.prev_pos[row])
                            .length()
                            .max(Fx::from_int(2)),
                        units.z[row] - units.prev_z[row],
                    )
                };
                let local = crate::world::pitched(
                    FxVec3::new(rolled.y, rolled.x, rolled.z),
                    Some(FxVec3::ZERO),
                    hull_pitch,
                );
                (pos + local.xy().rotate(units.heading[row])).extend(unit_z + local.z)
            } else if bp.unit(blueprint).is_mobile() {
                // The hull leans with the ground under it, and the turret with it.
                let offset = crate::world::leaned(
                    &self.terrain,
                    pos,
                    bp.unit(blueprint).radius,
                    units.heading[row],
                    (muzzle_xy - pos).extend(at.z),
                );
                (pos + offset.xy()).extend(unit_z + offset.z)
            } else {
                muzzle_xy.extend(unit_z + at.z)
            };
            // Use the same pitched muzzle for the cannon's launch solution
            // that was checked against its firing cone above.
            let muzzle_xy = if aircraft.is_none() || bp.unit(blueprint).visual.mesh == "assault_air"
            {
                muzzle.xy()
            } else {
                muzzle_xy
            };

            // Refine from the actual barrel tip after yaw and elevation are applied.
            let launch_aim = if direct_air {
                direct_air_aim(muzzle, target_center, target_velocity, t.turn, step)
            } else {
                aim.extend(aim_z)
            };
            let aim = launch_aim.xy();
            let aim_z = launch_aim.z;
            let (vel, ticks) = if bomb {
                // A dropped bomb inherits forward velocity. It never steers
                // sideways or launches upward to solve an artillery trajectory.
                let velocity = FxVec2::from_angle(units.heading[row]) * (units.speed[row] / DT);
                let velocity = if weapon.burn_ticks > 0 && weapon.spread > 0 {
                    let yaw = self.state.rng.below(weapon.spread as u32 * 2 + 1) as i32
                        - weapon.spread as i32;
                    velocity.rotate(Angle(yaw as i16 as u16))
                        * (Fx::ratio(85, 100) + Fx::ratio(self.state.rng.below(31) as i64, 100))
                } else {
                    velocity
                };
                (velocity.extend(Fx::ZERO), fall_ticks + DT * 3)
            } else if weapon.curve.0 > 0 {
                // A curving charge leaves fanned off the line to its mark (`curve.rs`).
                let dir =
                    crate::curve::launch_dir(weapon, muzzle, aim.extend(aim_z), tube, local.y);
                (dir * step, crate::curve::flight_ticks(weapon, step))
            } else if weapon.guided {
                let dir = if weapon.vertical_launch {
                    // Out along the cell: straight up, or leaning toward the bow (`cant`).
                    let lean = FxVec2::from_angle(Angle::from_degrees(90) - weapon.cant);
                    (FxVec2::from_angle(units.heading[row]) * lean.x).extend(lean.y)
                } else if weapon.skim > Fx::ZERO {
                    // A sea skimmer leaves level and settles to its height (`naval_arms.rs`).
                    (aim - muzzle_xy).extend(Fx::ZERO).normalize()
                } else {
                    (aim - muzzle_xy).extend(aim_z - muzzle.z).normalize()
                };
                let kick = if weapon.boost_ticks > 0 {
                    crate::launch_cells::boost_climb(weapon, 0, step)
                } else if weapon.cold_launch_ticks > 0 {
                    cold_lob_speed(0, weapon.cold_launch_ticks)
                } else if aircraft.is_some() {
                    // Off a rail under a wing it leaves at the aircraft's own speed, or the
                    // aircraft would fly into its own rockets.
                    (step * Fx::ratio(3, 20)).max(units.speed[row] / DT)
                } else {
                    step * Fx::ratio(3, 20)
                };
                (
                    dir * kick,
                    (weapon.range_max * 3 / step).ceil_int() + DT * 3,
                )
            } else if weapon.missile && weapon.trajectory == Trajectory::Ballistic {
                // Out the elevated tubes, along the hull. Speed is chosen so that
                // parabola comes down at the target's range; the shot does not turn
                // toward the aim point.
                let mut ahead = FxVec2::from_angle(facing);
                let mut range = (aim - muzzle_xy).dot(ahead).max(Fx::ONE);
                if weapon.spread > 0 {
                    let (yaw, along) = aim_error(&mut self.state.rng, weapon.spread);
                    ahead = ahead.rotate(Angle(yaw as i16 as u16));
                    // Short or long by the same angular budget as a shell.
                    range *= Fx::ONE + Angle(along as i16 as u16).sin();
                }
                let rise = aim_z - muzzle.z;
                let tan_rake = (weapon.rake.0 > 0)
                    .then(|| {
                        let r = FxVec2::from_angle(weapon.rake);
                        r.y / r.x.max(Fx::EPSILON)
                    })
                    .or(pivot.map(|p| {
                        let along = FxVec2::new(at.x - p.x, at.z - p.z);
                        along.y / along.x.max(Fx::ONE)
                    }))
                    .unwrap_or(Fx::ratio(1192, 1000));
                let need = (range * tan_rake - rise).max(GRAVITY);
                // Half of sqrt(1 + 8 need / g) - 1, rounded up, in fixed point (never negative).
                let v = (Fx::ONE + need * 8 / GRAVITY).sqrt() - Fx::ONE;
                let n = (v.raw().max(0) as u64).div_ceil(2 * Fx::ONE.raw() as u64);
                let n = (n as i32).max(1);
                let horiz = range / n;
                let vz = horiz * tan_rake;
                ((ahead * horiz).extend(vz), n + BALLISTIC_OVERRUN)
            } else {
                let mut delta = aim - muzzle_xy;
                let mut aim_z = aim_z;
                if weapon.spread > 0 {
                    let (yaw, off) = aim_error(&mut self.state.rng, weapon.spread);
                    let off = Angle(off as i16 as u16).sin();
                    delta = delta.rotate(Angle(yaw as i16 as u16));
                    match weapon.trajectory {
                        // A direct shot misses high or low as well as to the side. A bore's
                        // tracer strays only to the side: its channel still runs past the mark.
                        Trajectory::Direct if weapon.bore.is_some() => {}
                        Trajectory::Direct => {
                            aim_z += delta.extend(aim_z - muzzle.z).length() * off;
                        }
                        // A shell misses short or long: the same angular budget laid
                        // along the ground.
                        Trajectory::Ballistic => delta = delta * (Fx::ONE + off),
                    }
                }
                let dist = delta.length().max(Fx::ONE);
                match weapon.trajectory {
                    // A storm seed is steered out to its mark by `cloud_seed.rs`.
                    Trajectory::Direct if weapon.cloud.is_some() => (
                        FxVec3::ZERO,
                        weapon.cloud.map_or(1, |c| c.life_ticks() as i32),
                    ),
                    Trajectory::Direct => {
                        let dir = delta.extend(aim_z - muzzle.z).normalize();
                        (
                            dir * step,
                            if weapon.orbs > 0 {
                                // Homing flecks stay up until they strike or are home (`orbs.rs`).
                                crate::orbs::FLIGHT_TICKS
                            } else {
                                (delta
                                    .extend(aim_z - muzzle.z)
                                    .length()
                                    .max(weapon.range_max)
                                    * Fx::ratio(13, 10)
                                    / step)
                                    .ceil_int()
                                    + 2
                            },
                        )
                    }
                    Trajectory::Ballistic => {
                        let n = ballistic_ticks(dist, weapon);
                        let drop = GRAVITY * (n * (n + 1) / 2);
                        let vz = (aim_z - muzzle.z + drop) / n;
                        (
                            FxVec2::new(delta.x / n, delta.y / n).extend(vz),
                            n + BALLISTIC_OVERRUN,
                        )
                    }
                }
            };
            let (muzzle, vel, ticks) = if weapon.torpedo {
                // Dropped from an aircraft, it keeps the aircraft's way on it as it falls.
                let carried = if aircraft.is_some() {
                    FxVec2::from_angle(units.heading[row]) * (units.speed[row] / DT)
                } else {
                    FxVec2::ZERO
                };
                crate::naval::torpedo_launch(
                    muzzle,
                    facing,
                    weapon,
                    self.terrain.water_level(),
                    t.unit.is_none().then_some(aim),
                    carried,
                )
            } else if bomb && weapon.salvo == 1 {
                // The bay opens on the tick nearest the release line, up to half a
                // tick of flight either side of it; a lone bomb is let go from where
                // the line was crossed, or a fast bomber's bomb lands that much off.
                let nose = FxVec2::from_angle(units.heading[row]);
                let travel = units.speed[row] / DT;
                let off = ((aim - pos).dot(nose) - travel * fall).clamp(-travel, travel);
                (muzzle + (nose * off).extend(Fx::ZERO), vel, ticks)
            } else {
                (muzzle, vel, ticks)
            };
            if weapon.cone.is_some() {
                // A cone weapon's shot flies nowhere: its wake rolls out over the fan,
                // let go once the loop lets go of the unit tables (`wake.rs`).
                wakes.push((muzzle, facing));
            } else if weapon.lens.is_some() {
                // A lens fires no round: its front runs out to the unit it shot at, let
                // go once the loop lets go of the unit tables (`lens.rs`).
                latch = t.unit.map(|u| (muzzle, u));
            } else {
                self.state.projectiles.spawn(
                    muzzle,
                    vel,
                    owner,
                    id,
                    blueprint,
                    w as u8,
                    ticks.clamp(1, u16::MAX as i32) as u16,
                )?;
                let shot = self.state.projectiles.len() - 1;
                self.state.projectiles.target[shot] = t.unit.map_or(Handle::NONE, |t| units.id(t));
                self.state.projectiles.mark[shot] = aim.extend(aim_z);
                if let Some(cloud) = weapon.cloud {
                    // A storm seed is aimed at nothing: it hangs where it was let go, carried
                    // on a little by the aircraft's way as it slows (`cloud_seed.rs`), so a
                    // run lays a trail of them.
                    let carried = FxVec2::from_angle(units.heading[row]) * (units.speed[row] / DT);
                    let drift = carried * Fx::ratio(cloud.grow_ticks as i64, CLOUD_CARRY as i64);
                    self.state.projectiles.mark[shot] = (muzzle.xy() + drift).extend(muzzle.z);
                    self.state.projectiles.target[shot] = Handle::NONE;
                }
                if weapon.missile {
                    self.state.projectiles.hp[shot] = weapon.casing_hp();
                }
                // A missile out of a dived hull boils the surface and paints the boat.
                if weapon.missile && unit_z + bp.unit(blueprint).height < self.terrain.water_level()
                {
                    units.revealed[row] = crate::naval_arms::LAUNCH_REVEAL;
                    self.events.push(SimEvent::DivedLaunch {
                        pos: muzzle,
                        blueprint,
                        weapon: w as u8,
                    });
                }
            }
            self.muzzles.push(muzzle);
            self.events.push(SimEvent::ShotFired {
                pos: muzzle,
                vel,
                travel,
                color: weapon.color,
                owner,
                blueprint,
                weapon: w as u8,
                gun,
            });
            if let Some(sabot) = weapon.sabot {
                // The spent casing leaves by the gun's port (`titan.rs`): the port and the
                // throw are in the gun's frame, so they turn, pitch and lean with it the way
                // the muzzle does, and the unit's own way is carried into the throw.
                let unit = bp.unit(blueprint);
                let gun = |v: FxVec3| {
                    let v = crate::world::pitched(v, Some(FxVec3::ZERO), arm_pitch);
                    let v = v.xy().rotate(facing).extend(v.z);
                    if aircraft.is_none() && unit.is_mobile() {
                        crate::world::leaned(&self.terrain, pos, unit.radius, units.heading[row], v)
                    } else {
                        v
                    }
                };
                let seed = self.state.tick.wrapping_mul(2_654_435_761)
                    ^ id.0.wrapping_mul(40_503)
                    ^ w as u32;
                // Out of the port along the gun's `throw`, every case alike: they part
                // only in the air (`FallingSabot::drift`, `tumble`).
                let thrown = crate::titan::FallingSabot::thrown(
                    muzzle + gun(sabot.port - local),
                    gun(sabot.throw) * sabot.kick,
                    travel * Fx::from_int(DT),
                    facing,
                    owner,
                    id,
                    blueprint,
                    w as u8,
                    seed,
                );
                self.events.push(SimEvent::SabotThrown {
                    from: thrown.from,
                    vel: thrown.vel,
                    blueprint,
                    weapon: w as u8,
                });
                self.state.sabots.push(thrown);
            }
        }
        for (muzzle, facing) in wakes {
            self.wake(row, w, muzzle, facing);
        }
        if let Some((muzzle, target)) = latch {
            self.launch_lens(row, w, muzzle, target);
        }
        if weapon.split && t.unit.is_some() {
            self.split_target(row, w);
        }
        if t.scatter > Fx::ZERO && self.state.units.weapon_salvo_left[row][w] == 0 {
            // The salvo is away: the gun slews to its next point before it fires again,
            // and a bomber comes round for its next run somewhere else in the circle.
            self.pick_bombard_aim(row, self.bombard_slot(row, w));
        }
        Ok(())
    }
}
