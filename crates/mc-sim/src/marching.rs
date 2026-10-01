//! Blocks on the march: each tick a formation's anchor moves on along its
//! way, and every member is steered to its rank round it (`movement.rs` then
//! drives them there).

use crate::formations::Group;
use crate::movement::{patrol_lead, FormationMotion, PHASE_NEXT_LEG};
use crate::nav::Steer;
use crate::tables::*;
use crate::World;
use mc_core::{Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::{Motion, MoveLayer};

const DT: i32 = TICKS_PER_SECOND as i32;
/// Members handed to one worker at a time.
const CHUNK: usize = 128;
/// Share of its pace a block keeps to while forming up, wheeling or edging
/// round ground: the rest is speed in hand for its ranks to keep up.
const RESERVE: Fx = Fx::ratio(7, 10);
/// The least share of its pace a block forming up keeps to, waiting for a slow
/// member far off its rank.
const FORMING: Fx = Fx::ratio(2, 5);
/// How far a block's heading may lie off its way before it counts as wheeling.
const WHEELING: u16 = 0x0400;
/// Speed a formed-up block gives up per metre a member is off its rank (per
/// second), down to its `RESERVE` pace: small slips close in a couple of
/// seconds, and a member far off costs no more than forming up does. A member
/// with speed of its own to spare makes up its ground itself.
const LAG_GAIN: Fx = Fx::ONE;
/// Members a worker takes at a time in a big block's own loops.
const ROWS_CHUNK: usize = 256;

/// A block's heading and way this tick (`block_head`), for its anchor to move on by.
struct Head {
    id: u64,
    rows: Vec<usize>,
    group: Group,
    first: Order,
    target: FxVec2,
    air: bool,
    sweep: bool,
    mean: FxVec2,
    pace: Fx,
    accel: Fx,
    radius: Fx,
    yaw: i32,
    delta: FxVec2,
    m: Motion,
    clear_ahead: bool,
    route: FxVec2,
    /// The block is still wheeling onto its way: its outside ranks need speed in hand.
    wheeling: bool,
}

/// A marching block's decisions for this tick, for its members to keep rank by.
struct Ranks {
    rows: Vec<usize>,
    anchor: FxVec2,
    heading: mc_core::Angle,
    speed: Fx,
    route: FxVec2,
    pace: Fx,
    radius: Fx,
    side: FxVec2,
    target: FxVec2,
    sweep: bool,
    air: bool,
}

impl World {
    /// Advance a real group anchor, then steer every member to its moving slot.
    /// Members converge during travel; a disordered group never waits to assemble.
    pub(crate) fn formation_motion(&mut self) -> Vec<Option<FormationMotion>> {
        let units = &self.state.units;
        let mut out = vec![None; units.slots.rows()];
        let mut ranks: Vec<Ranks> = Vec::new();
        let mut active = std::collections::BTreeMap::<u64, Vec<usize>>::new();
        let mut circling = std::collections::BTreeMap::<u64, Vec<usize>>::new();
        let mut live = std::collections::BTreeSet::new();
        for row in units.slots.iter() {
            for o in self.state.orders.iter(units, row) {
                if o.formation != 0 {
                    live.insert(o.formation);
                }
            }
            if !units.is_active(row) || units.flags[row] & (flag::AIR_RUN | flag::HOLD) != 0 {
                continue;
            }
            if let Some(o) = self.state.orders.front(units, row) {
                if o.formation != 0 && o.kind == OrderKind::Guard {
                    circling.entry(o.formation).or_default().push(row);
                } else if o.formation != 0 {
                    active.entry(o.formation).or_default().push(row);
                }
            }
        }
        self.state.formations.retain(|id, _| live.contains(id));
        self.orbit_formation_motion(circling, &mut out);
        // Each block's heading and way, side by side on the pool; then the
        // ranks of those that reform are handed round (it moves their orders'
        // slots, one block at a time), and each anchor moves on, on the pool.
        let blocks: Vec<(u64, Vec<usize>, Group)> = active
            .into_iter()
            .filter_map(|(id, rows)| Some((id, rows, self.state.formations.get(&id)?.clone())))
            .collect();
        let this = &*self;
        let heads: Vec<Head> = self
            .pool
            .parallel_map_chunks(blocks.len(), 1, |_, range| {
                blocks[range]
                    .iter()
                    .map(|(id, rows, group)| this.block_head(*id, rows.clone(), group.clone()))
                    .collect::<Vec<_>>()
            })
            .into_iter()
            .flatten()
            .collect();
        for head in &heads {
            // Out of order after a pass, or wheeling through a turn: hand slots
            // to whoever is nearest them instead of swinging the whole block.
            let group = &head.group;
            if !head.air
                && (group.phase == 1
                    || (group.phase == 2 && (self.state.tick as u64 + head.id).is_multiple_of(4)))
            {
                self.regroup_ranks(&head.rows, group.anchor, group.heading);
            }
        }
        let this = &*self;
        let marched: Vec<(u64, Group, Option<Ranks>)> = self
            .pool
            .parallel_map_chunks(heads.len(), 1, |_, range| {
                heads[range]
                    .iter()
                    .map(|head| {
                        let (group, block) = this.block_march(head);
                        (head.id, group, block)
                    })
                    .collect::<Vec<_>>()
            })
            .into_iter()
            .flatten()
            .collect();
        for (id, group, block) in marched {
            self.state.formations.insert(id, group);
            ranks.extend(block);
        }
        // Each member's way to its rank reads the ground round it: the members
        // of every block, side by side on the pool.
        let members: Vec<(usize, usize)> = ranks
            .iter()
            .enumerate()
            .flat_map(|(g, r)| r.rows.iter().map(move |&row| (g, row)))
            .collect();
        let this = &*self;
        let chunks: Vec<Vec<(usize, FormationMotion)>> =
            self.pool
                .parallel_map_chunks(members.len(), CHUNK, |_, range| {
                    members[range]
                        .iter()
                        .map(|&(g, row)| (row, this.rank_motion(row, &ranks[g])))
                        .collect()
                });
        for (row, motion) in chunks.into_iter().flatten() {
            out[row] = Some(motion);
        }
        out
    }

    /// A block's heading and way this tick, before its ranks are handed round.
    fn block_head(&self, id: u64, rows: Vec<usize>, mut group: Group) -> Head {
        let first = *self.state.orders.front(&self.state.units, rows[0]).unwrap();
        let target = first.pos;
        let air = self.bp(rows[0]).motion.unwrap().layer == MoveLayer::Air;
        let hover = self.bp(rows[0]).motion.unwrap().hover;
        // A flight on patrol sweeps round its loop like one aircraft: it
        // never stops on a post, and turns no faster than its wings follow.
        // So does a flight through a waypoint with another queued behind it.
        let sweep = air
            && ((first.kind == OrderKind::Patrol && !hover)
                || self.air_waypoint_after(rows[0]).is_some());
        let mut mean = FxVec2::ZERO;
        let mut pace = Fx::MAX;
        let mut accel = Fx::MAX;
        let mut radius = Fx::ZERO;
        let mut yaw = i32::MAX;
        for &row in &rows {
            let o = self.state.orders.front(&self.state.units, row).unwrap();
            mean += self.state.units.pos[row] - o.offset;
            let m = self.bp(row).motion.unwrap();
            pace = pace.min(m.speed);
            accel = accel.min(m.accel);
            yaw = yaw.min(m.turn_rate as i32);
            radius = radius.max(self.bp(row).radius);
        }
        mean = FxVec2::new(mean.x / rows.len() as i32, mean.y / rows.len() as i32);
        // The anchor turns at half the slowest wing's rate, so the outside
        // of the flight has room to keep station through the turn.
        let yaw = (yaw / 2).max(1);
        // Carry on the way the flight is already going, at its airspeed: the
        // slots do not swing round to the new leg, and nobody brakes. On patrol
        // always; on a move, when it is flying on from the last waypoint.
        let units = &self.state.units;
        let (mut dir, mut speed) = (FxVec2::ZERO, Fx::ZERO);
        if air && group.phase == 0 {
            for &row in &rows {
                // A hovering airframe may face off its way: go by where it is going.
                dir += if hover {
                    units.air_velocity[row].xy()
                } else {
                    FxVec2::from_angle(units.heading[row])
                };
                speed += units.speed[row];
            }
        }
        let flying_on = speed * 2 >= pace * rows.len() as i32;
        if group.phase == 0 && (sweep || flying_on) {
            group.heading = if dir == FxVec2::ZERO {
                first.heading
            } else {
                dir.angle()
            };
            let mut anchor = FxVec2::ZERO;
            for &row in &rows {
                let o = self.state.orders.front(units, row).unwrap();
                anchor += units.pos[row] - o.offset.rotate(group.heading - o.heading);
            }
            let n = rows.len() as i32;
            group.anchor = FxVec2::new(anchor.x / n, anchor.y / n);
            group.speed = (speed / n).min(pace);
            group.phase = 1;
        }
        if group.phase == 0 {
            group.anchor = mean;
            group.heading = first.heading;
            group.phase = 1;
        }
        let delta = target - group.anchor;
        let m = self.bp(rows[0]).motion.unwrap();
        let probe = group.anchor + delta.clamp_length(Fx::from_int(128));
        // Straight on only while the block fits that way, not just its
        // middle: a gap the anchor alone threads is for the route to find.
        let clear_ahead = air
            || self
                .nav
                .clear_segment(m.layer, m.size_class, group.anchor, probe)
                && {
                    let shut = self.count_rows(&rows, |row| {
                        let o = self.state.orders.front(&self.state.units, row).unwrap();
                        let slot = group.anchor + o.offset.rotate(group.heading - o.heading);
                        let mo = self.bp(row).motion.unwrap();
                        !self.nav.clear_segment(
                            mo.layer,
                            mo.size_class,
                            slot,
                            slot + (probe - group.anchor),
                        )
                    });
                    shut * 3 <= rows.len()
                };
        let mut on_field = false;
        let route = if sweep {
            if delta != FxVec2::ZERO {
                group.heading = group.heading.turn_toward(delta.angle(), yaw as u16);
            }
            FxVec2::from_angle(group.heading)
        } else {
            // Straight at the goal while the way is open and the field agrees
            // with it; the probe sees only so far, and open ground that ends
            // in a ridge's pocket is for the field to lead the block round.
            let straight = delta.normalize();
            match self
                .nav
                .sample(self.state.units.field[rows[0]], group.anchor)
            {
                Steer::Direction(d) if !(clear_ahead && d.dot(straight) > Fx::HALF) => {
                    on_field = true;
                    d
                }
                _ => straight,
            }
        };
        // Slots turn with the route, returning to the requested facing on arrival.
        let facing = if delta.length() < pace {
            first.heading
        } else if on_field && !air {
            // A block faces where its way goes over the next stretch, not
            // each kink the field makes under its anchor.
            self.way_ahead(
                self.state.units.field[rows[0]],
                group.anchor,
                route,
                delta.length(),
            )
            .angle()
        } else {
            route.angle()
        };
        let mut wheeling = false;
        if !sweep && (group.phase == 1 || group.phase == 2) {
            // A block wheels no faster than its outside ranks can keep up
            // with speed in hand, or its inside ranks crowd together.
            let wheel = if air {
                m.turn_rate / 2
            } else {
                let mut reach = Fx::ONE;
                for &row in &rows {
                    reach = reach.max(
                        self.state
                            .orders
                            .front(&self.state.units, row)
                            .unwrap()
                            .offset
                            .length(),
                    );
                }
                let spare = pace * Fx::ratio(3, 10);
                let cap = (spare * 10430 / (reach * DT))
                    .floor_int()
                    .clamp(1, (m.turn_rate / 2) as i32) as u16;
                // Beside a slope it turns with its way, to keep its ranks
                // off it. Only worth asking when the cap holds it back.
                let look = route
                    * (pace * 3)
                        .clamp(Fx::from_int(24), Fx::from_int(64))
                        .min(delta.length());
                let hemmed = || {
                    rows.iter().any(|&row| {
                        let o = self.state.orders.front(&self.state.units, row).unwrap();
                        let slot = group.anchor + o.offset.rotate(group.heading - o.heading);
                        let mo = self.bp(row).motion.unwrap();
                        !self
                            .nav
                            .clear_segment(mo.layer, mo.size_class, slot, slot + look)
                    })
                };
                if group.heading.delta_to(facing).unsigned_abs() > cap && hemmed() {
                    m.turn_rate / 2
                } else {
                    cap
                }
            };
            group.heading = group.heading.turn_toward(facing, wheel);
            wheeling = group.heading.delta_to(facing).unsigned_abs() > WHEELING;
        }
        Head {
            id,
            rows,
            group,
            first,
            target,
            air,
            sweep,
            mean,
            pace,
            accel,
            radius,
            yaw,
            delta,
            m,
            clear_ahead,
            route,
            wheeling,
        }
    }

    /// Where a block's anchor moves this tick, and what its members keep rank
    /// by (`None` when it stops to file through a pass).
    fn block_march(&self, head: &Head) -> (Group, Option<Ranks>) {
        let Head {
            id: _,
            ref rows,
            ref group,
            first,
            target,
            air,
            sweep,
            mean,
            pace,
            accel,
            radius,
            yaw,
            delta,
            m,
            clear_ahead,
            route,
            wheeling,
        } = *head;
        let rows = rows.clone();
        let mut group = group.clone();
        let offset = |o: &Order| o.offset.rotate(group.heading - o.heading);
        // Whether a member's rank, moved from `from` to `to` with the anchor, crosses ground it cannot.
        let cut_off = |row: usize, from: FxVec2, to: FxVec2| {
            let m = self.bp(row).motion.unwrap();
            !self.nav.clear_segment(m.layer, m.size_class, from, to)
        };
        let n = rows.len();
        // A spur or a boulder may cut off a few hulls: they go round it on
        // their own and the block marches on. It files through only where
        // most of its ranks cannot pass.
        let crowded = |cut: usize| cut * 3 > n;
        // Members off their ranks (reachable ones): out of rank at all, and far out.
        let cut_off_rank = radius.max(Fx::from_int(5));
        let far_off_rank = radius * 2 + Fx::from_int(14);
        // (astray, cut off, worst, off rank, far off)
        let (astray, cut, worst, off_rank, far_off) = self.fold_rows(
            &rows,
            (0usize, 0usize, Fx::ZERO, 0usize, 0usize),
            |(astray, cut, worst, off_rank, far_off), row| {
                let o = self.state.orders.front(&self.state.units, row).unwrap();
                let slot = group.anchor + offset(o);
                let pos = self.state.units.pos[row];
                let astray = astray + (pos.distance(slot) > pace * 2) as usize;
                if cut_off(row, pos, slot) {
                    (astray, cut + 1, worst, off_rank, far_off)
                } else {
                    // The march paces itself on the members that can reach their ranks.
                    let d = pos.distance(slot);
                    (
                        astray,
                        cut,
                        worst.max(d),
                        off_rank + (d > cut_off_rank) as usize,
                        far_off + (d >= far_off_rank) as usize,
                    )
                }
            },
            |a, b| (a.0 + b.0, a.1 + b.1, a.2.max(b.2), a.3 + b.3, a.4 + b.4),
        );
        // Clear of a pass once the ranks fit again and the way on is open,
        // straight at the goal or along the route the next stretch.
        let reopened = || {
            let look = route
                * (pace * 3)
                    .clamp(Fx::from_int(24), Fx::from_int(64))
                    .min(delta.length());
            let ahead = rows
                .iter()
                .filter(|&&row| {
                    let o = self.state.orders.front(&self.state.units, row).unwrap();
                    let slot = group.anchor + offset(o);
                    cut_off(row, slot, slot + look)
                })
                .count();
            cut * 4 <= n && (clear_ahead || ahead * 4 <= n)
        };
        if crowded(cut) || (group.phase == 3 && !reopened()) {
            // A narrow passage may not fit the whole block. Let the existing
            // per-hull navigation cross it, then recover ranks while moving on open ground.
            group.anchor = mean;
            group.phase = 3;
            group.speed = Fx::ZERO;
            return (group, None);
        }
        if group.phase == 3 {
            group.phase = 1;
            group.anchor = mean;
        }
        if air && astray * 2 > rows.len() {
            // A flight coming off its attack runs is strewn over the sky
            // round an anchor nobody was moving. Form up where the aircraft
            // are and on the way, not back at slots left behind.
            group.anchor = mean;
        }
        // A block on the ground or at sea is formed up with one in eight still
        // out: one hull that cannot find its rank must not hold a big block
        // at a crawl for good. Formed up, it paces itself on them.
        let stragglers = if air { 0 } else { n / 8 };
        if group.phase == 1 && off_rank <= stragglers {
            group.phase = 2;
        }
        // Leave immediately, keeping speed in reserve for members catching up.
        // Formation error can slow the march, but must never create a rally pause.
        let speed = if air {
            // A flight keeps speed in reserve for its wings to close up. On
            // patrol it holds its pace: its wings have speed in hand to close
            // up, and an anchor that slows leaves them ahead of it.
            let share = if sweep || far_off == 0 {
                RESERVE
            } else {
                FORMING
            };
            pace * share
        } else {
            // A block on the ground or at sea keeps speed in hand for its ranks
            // while it forms up, wheels, or has ground ahead of a rank to edge
            // off or go round (it looks further than it steers, to be at that
            // pace by then). Formed up on open ground or water it goes at its
            // slowest member's own speed.
            let warn = route
                * (pace * 12)
                    .clamp(Fx::from_int(96), Fx::from_int(256))
                    .min(delta.length());
            let ground_ahead = || {
                self.count_rows(&rows, |row| {
                    let o = self.state.orders.front(&self.state.units, row).unwrap();
                    let slot = group.anchor + offset(o);
                    cut_off(row, slot, slot + warn)
                }) > 0
            };
            let forming = group.phase != 2;
            let full = if forming || wheeling || ground_ahead() {
                pace * RESERVE
            } else {
                pace
            };
            // It gives up only what a member off its rank needs to close up at
            // its own speed: a member with speed to spare makes up its ground
            // itself. Forming up it may wait longer than on the march.
            let most = pace - pace * if forming { FORMING } else { RESERVE };
            self.fold_rows(
                &rows,
                full,
                |keep_up, row| {
                    let o = self.state.orders.front(&self.state.units, row).unwrap();
                    let slot = group.anchor + offset(o);
                    let pos = self.state.units.pos[row];
                    if cut_off(row, pos, slot) {
                        return keep_up;
                    }
                    // Ahead of its rank a member eases off by itself; behind or
                    // beside it, it needs speed in hand.
                    let error = slot - pos;
                    let lag = error.dot(route);
                    let off = (error - route * lag).length().max(lag);
                    if off > Fx::ZERO {
                        let own = self.bp(row).motion.unwrap().speed;
                        keep_up.min(own - (off * LAG_GAIN).min(most))
                    } else {
                        keep_up
                    }
                },
                Fx::min,
            )
        };
        let speed = if sweep {
            speed
        } else {
            speed.min(delta.length() * 2)
        };
        group.speed = group.speed.approach(speed, accel / DT);
        let advance = if sweep {
            route * (group.speed / DT)
        } else {
            route * (group.speed / DT).min(delta.length())
        };
        let mut candidate = group.anchor + advance;
        let side = route.perp();
        let blocked_by = |to: FxVec2| {
            self.count_rows(&rows, |row| {
                let o = self.state.orders.front(&self.state.units, row).unwrap();
                cut_off(row, group.anchor + offset(o), to + offset(o))
            })
        };
        let mut cut = blocked_by(candidate);
        if !air {
            // Ranks the next stretch runs into ground: side the block away from
            // them, so it skirts a mountain's flank a rank's width off instead
            // of scraping along it. A pass pinching from both sides nets out.
            // Ground past the goal is none of the block's business.
            let look = route
                * (pace * 3)
                    .clamp(Fx::from_int(24), Fx::from_int(64))
                    .min(delta.length());
            let mut lean = 0i32;
            let mut facing_ground = false;
            let mut width = Fx::ZERO;
            for &row in &rows {
                let o = self.state.orders.front(&self.state.units, row).unwrap();
                let slot = group.anchor + offset(o);
                let across = offset(o).dot(side);
                width = width.max(across.abs());
                if cut_off(row, slot, slot + look) {
                    facing_ground = true;
                    if across.abs() > radius {
                        lean -= across.signum() as i32;
                    }
                }
            }
            let mut dodge = false;
            let far = route
                * (pace * 8)
                    .clamp(Fx::from_int(48), Fx::from_int(128))
                    .min(delta.length());
            if lean == 0 && !facing_ground {
                // Look further down the road for ground square across the front.
                facing_ground = rows.iter().all(|&row| {
                    let o = self.state.orders.front(&self.state.units, row).unwrap();
                    let slot = group.anchor + offset(o);
                    cut_off(row, slot, slot + far)
                });
            }
            if lean == 0 && facing_ground {
                // Ground square across the whole front, a mountain dead ahead:
                // the block goes round it on one side rather than splitting
                // either side of it. Take the side with more open ground,
                // then the one the route already bends to.
                let shut = |shift: FxVec2| {
                    rows.iter()
                        .filter(|&&row| {
                            let o = self.state.orders.front(&self.state.units, row).unwrap();
                            let slot = group.anchor + shift + offset(o);
                            !self.nav.passable(m.layer, m.size_class, slot + far)
                        })
                        .count()
                };
                let step = side * (width + radius * 2);
                let left = shut(step) + shut(step * Fx::from_int(2));
                let right = shut(-step) + shut(-step * Fx::from_int(2));
                let bend = delta.normalize().cross(route);
                lean = match left.cmp(&right) {
                    std::cmp::Ordering::Less => 1,
                    std::cmp::Ordering::Greater => -1,
                    _ if bend < Fx::ZERO => -1,
                    _ => 1,
                };
                dodge = true;
            }
            if lean != 0 {
                // Edge off only as fast as the ranks can follow.
                let rate = if dodge {
                    pace / DT
                } else if worst < radius * 2 + Fx::from_int(14) {
                    pace / DT / 2
                } else {
                    pace / DT / 4
                };
                let slid = candidate + if lean > 0 { side } else { -side } * rate;
                let slid_cut = blocked_by(slid);
                if slid_cut <= cut && self.nav.passable(m.layer, m.size_class, slid) {
                    candidate = slid;
                    cut = slid_cut;
                }
            }
        }
        // The anchor keeps to ground the block can drive, so a ridge too
        // thin to cut off many ranks at once is not walked straight over.
        let anchor_clear = !self.nav.passable(m.layer, m.size_class, group.anchor)
            || self
                .nav
                .clear_segment(m.layer, m.size_class, group.anchor, candidate);
        if anchor_clear && !crowded(cut) {
            group.anchor = candidate;
        } else {
            group.phase = 3;
            group.speed = Fx::ZERO;
            group.anchor = mean;
            return (group, None);
        }
        if sweep {
            if group.phase != 3 && self.turn_onto_next_leg(rows[0], &group, yaw) {
                group.phase = PHASE_NEXT_LEG;
            }
        } else if group.anchor.distance(target) <= Fx::HALF {
            group.anchor = target;
            group.heading = first.heading;
            group.speed = Fx::ZERO;
        }
        let block = Ranks {
            rows,
            anchor: group.anchor,
            heading: group.heading,
            speed: group.speed,
            route,
            pace,
            radius,
            side,
            target,
            sweep,
            air,
        };
        (group, Some(block))
    }

    /// Where member `row` of a block drives this tick to keep its rank.
    fn rank_motion(&self, row: usize, block: &Ranks) -> FormationMotion {
        let Ranks {
            anchor,
            heading,
            speed: block_speed,
            route,
            pace,
            radius,
            side,
            target,
            sweep,
            air,
            ..
        } = *block;
        // Whether a member's rank, moved from `from` to `to` with the anchor, crosses ground it cannot.
        let cut_off = |from: FxVec2, to: FxVec2| {
            let m = self.bp(row).motion.unwrap();
            !self.nav.clear_segment(m.layer, m.size_class, from, to)
        };
        let o = self.state.orders.front(&self.state.units, row).unwrap();
        let slot = anchor + o.offset.rotate(heading - o.heading);
        let own_speed = self.bp(row).motion.unwrap().speed;
        let settling = !sweep && anchor.distance(target) <= pace;
        let (goal, speed) = if !settling {
            let error = slot - self.state.units.pos[row];
            let lag = error.dot(route);
            let look = (pace / 2).max(Fx::from_int(12));
            // Look ahead of each hull: even members ahead of their rank
            // move forward as they ease into it, rather than reversing
            // toward a slot behind them.
            let mo = self.bp(row).motion.unwrap();
            let forward = if mo.layer == MoveLayer::Air && !mo.hover {
                // A wing takes seconds to roll into a turn. Chasing a
                // point a few lengths ahead, it is still rolling when
                // the point has crossed its nose, and it weaves all the
                // way. Two turn radii out it eases onto its station.
                let turn_radius = mo
                    .speed
                    .mul_div(10430, (mo.turn_rate as i64 * DT as i64).max(1));
                (turn_radius * 2).max(look * 2)
            } else {
                (look + lag).clamp(look / 4, look * 2)
            };
            let lateral = (error - route * lag).clamp_length(forward * Fx::ratio(3, 4));
            let pos = self.state.units.pos[row];
            let spacing = radius * 2 + Fx::from_int(6);
            // Far from its rank, a hull makes for it by the way from where it
            // is: the anchor's heading may lead it into a slope it is beside.
            let astray = !air && error.length() > spacing * 3;
            let mut goal = if astray {
                slot
            } else {
                pos + route * forward + lateral
            };
            // Only close by: a member walled off from a rank far away takes
            // the field round, not a sidestep along the wrong side of a ridge.
            if !air && !astray && cut_off(pos, goal) {
                // A knoll stands in this rank's way. Tuck in toward the
                // middle of the block, which has found its way past,
                // rather than going round on a route of its own.
                let inward = side * (anchor - pos).dot(side);
                // Failing that, edge across behind it before going round alone.
                if let Some(tucked) = [2, 1, 0]
                    .into_iter()
                    .flat_map(|k| {
                        (1..=4).map(move |j| {
                            pos + route * forward * Fx::ratio(k, 2) + inward * Fx::ratio(j, 4)
                        })
                    })
                    .find(|&g| g != pos && !cut_off(pos, g))
                {
                    goal = tucked;
                }
            }
            // A wing on patrol eases back onto station instead of
            // throttling down to a crawl; a stall reads as a fault.
            let floor = if sweep { pace / 2 } else { pace / 5 };
            let speed = (block_speed + lag * 2).clamp(floor, own_speed);
            (goal, speed)
        } else {
            (slot, own_speed)
        };
        FormationMotion {
            goal,
            speed,
            facing: heading,
            cruise: sweep,
            settling,
        }
    }

    /// Whether a flight on patrol or through a waypoint, its anchor where
    /// `group` has it, should begin its turn onto the leg after its post now.
    /// It turns in early by the lead its turn radius needs for the corner, so
    /// it rolls out on the
    /// new leg instead of overshooting the post and weaving back; a post it
    /// has already passed abeam is taken as reached.
    fn turn_onto_next_leg(&self, row: usize, group: &crate::formations::Group, yaw: i32) -> bool {
        let units = &self.state.units;
        let mut queue = self.state.orders.iter(units, row);
        let Some(first) = queue.next() else {
            return false;
        };
        // Round a patrol loop, the next post; otherwise the next waypoint.
        let next = if first.kind == OrderKind::Patrol {
            queue.find(|o| o.kind == OrderKind::Patrol)
        } else {
            queue.next()
        };
        let (post, Some(next)) = (first.pos, next.map(|o| o.pos)) else {
            return false;
        };
        let to_post = post - group.anchor;
        let dist = to_post.length();
        let turn_radius = group.speed.mul_div(10430, (yaw as i64 * DT as i64).max(1));
        if to_post.dot(FxVec2::from_angle(group.heading)) <= Fx::ZERO && dist <= turn_radius * 2 {
            return true;
        }
        dist <= patrol_lead(group.heading, post, next, turn_radius).max(Fx::from_int(4))
    }
}

impl World {
    /// `rows` folded by `step` from `zero`. A big block's members are taken in
    /// pieces on the pool and the pieces' results joined by `join`: every use
    /// is a count, a sum, a least or a most, which no order changes.
    fn fold_rows<T: Copy + Send + Sync>(
        &self,
        rows: &[usize],
        zero: T,
        step: impl Fn(T, usize) -> T + Sync,
        join: impl Fn(T, T) -> T,
    ) -> T {
        if rows.len() <= ROWS_CHUNK {
            return rows.iter().fold(zero, |acc, &row| step(acc, row));
        }
        self.pool
            .parallel_map_chunks(rows.len(), ROWS_CHUNK, |_, range| {
                rows[range].iter().fold(zero, |acc, &row| step(acc, row))
            })
            .into_iter()
            .fold(zero, join)
    }

    /// How many of `rows` `test` holds for, as [`fold_rows`](Self::fold_rows).
    fn count_rows(&self, rows: &[usize], test: impl Fn(usize) -> bool + Sync) -> usize {
        self.fold_rows(rows, 0, |n, row| n + test(row) as usize, |a, b| a + b)
    }
}
