//! Target acquisition: every unit's guns choose what they lay on (`World::run_targeting`),
//! out of `combat.rs`, whose private helpers it shares.

use super::*;

impl World {
    pub(crate) fn run_targeting(&mut self) {
        let rows = self.state.units.slots.rows();
        let incoming = self.incoming_damage(rows);
        let this = &*self;
        let picks: Vec<Vec<(usize, [UnitId; MAX_WEAPONS], u8)>> =
            self.pool.parallel_map_chunks(rows, CHUNK, |_, range| {
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
                    // Holding fire, or stunned: let go of whatever it was aiming at.
                    if units.has_flag(row, flag::PASSIVE) || this.stunned(row) {
                        if units.weapon_target[row] != [Handle::NONE; MAX_WEAPONS] {
                            out.push((row, [Handle::NONE; MAX_WEAPONS], 0));
                        }
                        continue;
                    }
                    let ordered = this
                        .state
                        .orders
                        .front(units, row)
                        .filter(|o| {
                            o.kind == OrderKind::Attack
                                // Attack-move aircraft retain a flight target through
                                // their return leg. Aim the weapon at that same target;
                                // a nearer enemy can otherwise steal the bomb bay while
                                // the pilot keeps lining up on the original target.
                                || (matches!(o.kind, OrderKind::AttackMove | OrderKind::Patrol)
                                    && units.has_flag(row, flag::AIR_RUN)
                                    && bp
                                        .motion
                                        .is_some_and(|m| m.layer == mc_data::MoveLayer::Air))
                        })
                        .and_then(|o| units.row(o.target));
                    // A fighter's guns leave an enemy the shots already on their way
                    // will kill, unless an order names it: a pack otherwise pours its
                    // first pass into the lead fighter and lets the rest fly free.
                    let fighter = bp.has(cat::ANTI_AIR)
                        && bp
                            .motion
                            .is_some_and(|m| m.layer == mc_data::MoveLayer::Air && !m.hover);
                    let doomed = |t: usize| fighter && incoming[t] >= units.health[t];
                    let ordered = ordered.filter(|&t| {
                        !doomed(t)
                            || this
                                .state
                                .orders
                                .front(units, row)
                                .is_some_and(|o| o.kind == OrderKind::Attack)
                    });
                    let ground = this.ground_mark(row).is_some();
                    let strike = this.striking(row);
                    let mut targets = units.weapon_target[row];
                    for (w, weapon) in bp.weapons.iter().enumerate() {
                        // A volley that spreads under an order (`Weapon::split_ordered`),
                        // once begun on the mark, goes on over what is round it, unless it
                        // holds its fire but for the mark.
                        let spreading = units.weapon_salvo_left[row][w] > 0
                            && weapon.split_ordered
                            && this.fires_at_will(row);
                        if let Some(t) =
                            ordered.filter(|t| !spreading && this.is_valid_target(row, *t, weapon))
                        {
                            targets[w] = units.id(t);
                            continue;
                        }
                        // Ordered onto the ground: the weapons phase aims these at it.
                        if ground && takes_ground(bp, weapon, strike) {
                            targets[w] = Handle::NONE;
                            continue;
                        }
                        // Stick with the current target while it lives and stays in
                        // range. A closer enemy is not a reason to turn the gun.
                        let current = units.row(targets[w]).filter(|t| {
                            this.is_valid_target(row, *t, weapon)
                                && this.fires_at_will(row)
                                && (!doomed(*t) || ordered == Some(*t))
                        });
                        // An arm that sways off the torso (`Weapon::sway`) looks for the foe
                        // nearest its own side of the body, so two arms split two marks
                        // either side of where the torso points instead of sharing one.
                        let side = if w != 0 && weapon.sway.0 > 0 {
                            weapon.muzzle.y
                        } else {
                            Fx::ZERO
                        };
                        // A held beam still streaming takes the foe nearest where it lands,
                        // so it drags the shortest way on, not back across the field to
                        // whatever stands nearest the hull.
                        let dragging = weapon.beam && units.streaming[row];
                        // A gun that keeps its aim (`Weapon::keep_aim`) takes the foe nearest
                        // a point half its reach out along where it points, so it swings as
                        // little as it can; searched round that point, its reach the gun's.
                        let reach_half = weapon.range_max / 2;
                        let aim_at = units.pos[row]
                            + FxVec2::from_angle(units.heading[row] + units.weapon_yaw[row][w])
                                * reach_half;
                        let from = if dragging {
                            units.beam_at[row]
                        } else if weapon.keep_aim {
                            aim_at
                        } else {
                            units.pos[row]
                                + FxVec2::new(Fx::ZERO, side)
                                    .rotate(units.heading[row] + units.weapon_yaw[row][0])
                        };
                        // Searched round the stroke, its reach still the gun's
                        // (`is_valid_target`): anything in reach lies within twice it.
                        let around = if dragging {
                            from.distance(units.pos[row])
                        } else if weapon.keep_aim {
                            reach_half
                        } else {
                            Fx::ZERO
                        };
                        // Walls are left for last (`World::is_wall`): a search that passes
                        // one over notes it, so the search for walls runs only when one
                        // was there to find.
                        let wall_seen = std::cell::Cell::new(false);
                        let in_grid = |prefer: u32, walls: bool| {
                            this.index
                                .nearest_foe(
                                    from,
                                    weapon.range_max
                                        + this.gun_offset(row, weapon)
                                        + side.abs()
                                        + around,
                                    // A torpedo also takes a walker on the seabed.
                                    if weapon.torpedo {
                                        kind::UNIT
                                    } else {
                                        target_kinds(weapon.target_mask)
                                    },
                                    this.team_mask(units.owner[row]),
                                    |e| {
                                        let t = e.row as usize;
                                        if !this.unit_entry_is_current(e)
                                            || doomed(t)
                                            || (prefer != 0 && !this.hittable(t, prefer))
                                            || !this.is_valid_target(row, t, weapon)
                                        {
                                            return false;
                                        }
                                        if !walls && this.is_wall(t) {
                                            wall_seen.set(true);
                                            return false;
                                        }
                                        true
                                    },
                                )
                                .map(|e| e.row as usize)
                        };
                        let nearest = |prefer: u32, walls: bool| {
                            if weapon.range_max > MAP_GUN_REACH {
                                this.nearest_on_map(row, weapon, prefer, walls)
                            } else {
                                in_grid(prefer, walls)
                            }
                        };
                        // A weapon with a preference (`Weapon::prefer_mask`) leaves what it
                        // is on for one of those as soon as one is in range.
                        let preferred = (weapon.prefer_mask != 0 && this.fires_at_will(row))
                            .then(|| nearest(weapon.prefer_mask, false))
                            .flatten();
                        if let Some(t) = current {
                            // A gun on a wall turns to anything else that comes into reach,
                            // unless an order names the wall.
                            let other = if this.is_wall(t) && ordered != Some(t) {
                                preferred.or_else(|| nearest(0, false))
                            } else {
                                preferred.filter(|_| !this.hittable(t, weapon.prefer_mask))
                            };
                            if let Some(p) = other {
                                targets[w] = units.id(p);
                            }
                            continue;
                        }
                        targets[w] = if this.fires_at_will(row) {
                            preferred
                                .or_else(|| nearest(0, false))
                                .or_else(|| {
                                    let map_gun = weapon.range_max > MAP_GUN_REACH;
                                    (wall_seen.get() || map_gun)
                                        .then(|| nearest(0, true))
                                        .flatten()
                                })
                                .map_or(Handle::NONE, |t| units.id(t))
                        } else {
                            Handle::NONE
                        };
                    }
                    // Line of fire (`line_of_fire.rs`): looked at on the unit's turn, and
                    // at once for a gun on a new target. A gun that cannot see its target
                    // turns to one it can, unless an order names the one it is on.
                    let due = this.line_check_due(row);
                    let mut blocked = units.shot_blocked[row];
                    for (w, weapon) in bp.weapons.iter().enumerate() {
                        let bit = 1u8 << w;
                        let Some(t) = units
                            .row(targets[w])
                            .filter(|_| this.needs_line(row, weapon))
                        else {
                            blocked &= !bit;
                            continue;
                        };
                        if !due && targets[w] == units.weapon_target[row][w] {
                            continue;
                        }
                        if this.clear_shot(row, weapon, t) {
                            blocked &= !bit;
                            continue;
                        }
                        blocked |= bit;
                        if ordered != Some(t) && this.fires_at_will(row) {
                            if let Some(seen) = this.visible_alternative(row, weapon, t) {
                                targets[w] = units.id(seen);
                                blocked &= !bit;
                            }
                        }
                    }
                    if targets != units.weapon_target[row] || blocked != units.shot_blocked[row] {
                        out.push((row, targets, blocked));
                    }
                }
                out
            });
        for (row, targets, blocked) in picks.into_iter().flatten() {
            let before = self.state.units.weapon_target[row];
            self.state.units.weapon_target[row] = targets;
            self.state.units.shot_blocked[row] = blocked;
            self.storm_retargets(row, before);
        }
    }
}
