//! Reclaim: taking wrecks and live units apart for their mass.
//!
//! A wreck gives up its mass at the reclaimer's power. A live unit is unbuilt
//! at the same power it would take to build: it loses health as it goes, and
//! when the last of it is taken it is simply gone (`flag::RECLAIMED`). Reclaimers
//! with nothing to do clear the wrecks within their reach; a live unit is only
//! ever reclaimed on an order.

pub use crate::mirror::BeamInstance;
use crate::mirror::SimEvent;
use crate::orders::CHASE_REPATH_DISTANCE;
use crate::reclaim_heads::HeadWork;
use crate::spatial::kind;
use crate::tables::*;
use crate::{SimError, World};
use mc_core::{Fx, FxVec2, FxVec3, TICKS_PER_SECOND};

const DT: i32 = TICKS_PER_SECOND as i32;
/// Share of a live unit's mass that comes back. Less than a repair costs, so
/// mending a unit and taking it apart again never turns a profit.
const UNIT_YIELD: Fx = Fx::ratio(1, 5);
/// An armed builder leaves wrecks alone while an enemy is within this many times its guns' reach.
const GUN_RANGE_MARGIN: Fx = Fx::ratio(5, 4);
/// No wreck is wider than this: how far past its reach a reclaimer looks for one.
pub(crate) const WIDEST_TARGET: Fx = Fx::from_int(48);

/// One reclaimer working on one thing this tick.
#[derive(Clone, Copy, Debug)]
pub struct ReclaimWork {
    pub source: UnitId,
    /// The unit being taken apart, `Handle::NONE` for a wreck.
    pub unit: UnitId,
    /// Where the target stood when it was worked on, on the ground.
    pub at: FxVec3,
    pub radius: Fx,
    pub height: Fx,
    /// Mass riding from this drone back into its carrier's belly.
    pub relay: bool,
    /// Which of a reclaimer's heads the beam leaves (`mc_data::Reclaimer::heads`).
    pub head: u8,
}

/// `BeamInstance::kind` of a reclaim beam.
pub const BEAM_RECLAIM: u32 = 0;
/// A Regency builder's nanite stream (`mc_data::Construction::Nanite`): from the
/// builder's emitter to the weld, `to` and `to_prev` both the weld, `height` zero.
pub const BEAM_NANITE: u32 = 1;
/// A Regency site being fed by nanite streams: one per site, however many feed it. `from`
/// is the site's foot, `to` (and `to_prev`) the middle of its build front, `radius` and
/// `height` the hull's. The renderer draws the rings and the filaments rising round it.
pub const BEAM_NANITE_SITE: u32 = 6;
/// Salvage riding from a drone into the underside of its carrier: particles, no ribbon.
pub const BEAM_RELAY: u32 = 3;
// retired: 7 (a scavenger tower's dim sweep beam)
/// A reclaim beam of a faction that builds with nanites (`mc_data::Construction::Nanite`):
/// drawn as their nanite stream, its strands reaching out to the target and the matter
/// riding home down them. Laid out as `BEAM_RECLAIM`.
pub const BEAM_NANITE_RECLAIM: u32 = 8;

/// Whether a beam of `kind` takes something apart: a reclaim beam of either look.
pub fn is_reclaim(kind: u32) -> bool {
    kind == BEAM_RECLAIM || kind == BEAM_NANITE_RECLAIM
}

impl World {
    /// Whether `player` knows of wreck `w`: a settled wreck is salvage to plan around, so
    /// it counts anywhere the player's team has explored, not only in sight, and the
    /// map's own wreckage from the start. What the player is shown (`push_wrecks`, the
    /// Ctrl survey) and every wreck a reclaimer picks for itself (area and guard work,
    /// idle salvage, reclaim heads, drone carriers) follow this one rule.
    pub(crate) fn wreck_known(&self, w: usize, player: u8) -> bool {
        let s = &self.state;
        !s.fog_enabled
            || s.wrecks.from_map[w]
            || self
                .fog
                .is_explored(s.wrecks.pos[w], self.team_mask(player))
    }

    /// This tick's reclaim beams. Left out when the viewer can see neither end.
    #[expect(
        clippy::disallowed_methods,
        reason = "presentation: fills the render frame's beam instances"
    )]
    pub(crate) fn write_reclaim_beams(
        &self,
        viewer: Option<u8>,
        out: &mut Vec<BeamInstance>,
        sources: &mut Vec<u32>,
    ) {
        let s = &self.state;
        out.clear();
        sources.clear();
        let seen = |p: FxVec2| {
            viewer.is_none_or(|v| !s.fog_enabled || self.fog.is_detected(p, self.team_mask(v)))
        };
        for work in &self.reclaims {
            let Some(row) = s.units.row(work.source) else {
                continue;
            };
            // Where it is now if it is still there; where it was when the last of it went.
            let (to_prev, to) = match s.units.row(work.unit) {
                Some(t) if !work.relay => (
                    s.units.prev_pos[t].extend(s.units.prev_z[t]),
                    s.units.pos[t].extend(s.units.z[t]),
                ),
                _ => (work.at, work.at),
            };
            if !seen(s.units.pos[row]) && !seen(to.xy()) {
                continue;
            }
            let bp = self.bp(row);
            let mut from = match (bp.reclaimer, bp.builder.as_ref().and_then(|b| b.arm)) {
                (Some(_), _) => self.head_emitter(row, work.head as usize),
                // The build arm is pitched at its work: the beam leaves from where its tip has swung to.
                (None, Some(arm)) => {
                    let emitter = crate::world::pose_build_arm(
                        arm,
                        s.units.arm_pitch[row][0],
                        s.units.arm_pitch[row][1],
                    );
                    let facing = s.units.heading[row] + s.units.weapon_yaw[row][0];
                    (s.units.pos[row]
                        + bp.turret_point(
                            FxVec2::new(emitter.x, emitter.y),
                            s.units.heading[row],
                            facing,
                        ))
                    .extend(s.units.z[row] + emitter.z)
                }
                (None, None) => s.units.pos[row].extend(s.units.z[row] + bp.height),
            };
            // Nanite factions take things apart as they build: with a nanite stream.
            let mut kind = if self.uses_nanites(row) {
                BEAM_NANITE_RECLAIM
            } else {
                BEAM_RECLAIM
            };
            let (mut to_prev, mut to, mut height) = (to_prev, to, work.height);
            // Bits travel from the grip into the emitter. A relay parks the emitter
            // on the carrier's belly so the stream arrives underneath it.
            if work.relay {
                if let Some(parent) = s.units.row(work.unit) {
                    let belly = Fx::ratio(3, 5);
                    from = s.units.pos[parent].extend(s.units.z[parent] + belly);
                    to_prev = s.units.prev_pos[row].extend(s.units.prev_z[row]);
                    to = s.units.pos[row].extend(s.units.z[row]);
                    height = Fx::ratio(4, 5);
                    kind = BEAM_RELAY;
                }
            }
            // The drone also ferries mass home. A second key, or the renderer
            // treats the jump from the wreck to the carrier as a new beam and
            // the one that left hangs in the air.
            let key = if work.relay { 1 << 31 } else { 0 };
            sources.push(work.source.0 | key);
            out.push(BeamInstance {
                from: from.to_f32(),
                kind,
                to_prev: to_prev.to_f32(),
                radius: work.radius.to_f32(),
                to: to.to_f32(),
                height: height.to_f32(),
            });
        }
    }

    /// How far this unit's tools reach: a builder's, or else a reclaimer's.
    pub(crate) fn work_range(&self, row: usize) -> Fx {
        let bp = self.bp(row);
        bp.builder
            .as_ref()
            .map(|b| b.range)
            .or(bp.reclaimer.map(|r| r.range))
            .unwrap_or(Fx::ZERO)
    }

    /// Whether the unit in `row` may turn a reclaim beam on the unit in `t`:
    /// its owner's own units, and enemies it can see. Never an ally's.
    pub(crate) fn can_reclaim_unit(&self, row: usize, t: usize) -> bool {
        let units = &self.state.units;
        if t == row
            || !units.slots.is_alive(t)
            || units.health[t] <= Fx::ZERO
            || units.flags[t] & (flag::IN_FACTORY | flag::INVULNERABLE) != 0
        {
            return false;
        }
        let (owner, theirs) = (units.owner[row], units.owner[t]);
        owner == theirs || (self.are_enemies(owner, theirs) && self.detects(owner, t))
    }

    /// Whether `player` has nowhere to put salvage, so reclaimers leave wrecks alone. A
    /// side that builds for free (the test range) never waits for room, as `drain_wreck`
    /// never does for it: its full store once left every idle tower aimed and dark.
    /// Energy never matters: reclaim takes none, even in a stall.
    pub(crate) fn no_room_for_salvage(&self, player: u8) -> bool {
        let p = &self.state.players[player as usize];
        !p.free_build && p.mass >= p.mass_capacity
    }

    /// One tick of pulling mass out of a wreck. True once there is none left.
    /// A wreck gives up no more than its owner-to-be has room to store: what does
    /// not fit stays in the wreck for later, never lost.
    pub(crate) fn drain_wreck(&mut self, row: usize, w: usize, power: Fx, head: u8) -> bool {
        let player = &self.state.players[self.state.units.owner[row] as usize];
        let wrecks = &mut self.state.wrecks;
        // Free building (the test range) keeps no books: it never waits for room.
        let room = if player.free_build {
            wrecks.mass[w]
        } else {
            (player.mass_capacity - player.mass).max(Fx::ZERO)
        };
        let take = (power * World::reclaim_rate())
            .min(wrecks.mass[w])
            .min(room);
        if take <= Fx::ZERO {
            return false;
        }
        wrecks.mass[w] -= take;
        wrecks.reclaimed[w] += take;
        wrecks.drained[w] = self.state.tick;
        let bp = self.blueprints.unit(wrecks.blueprint[w]);
        let at = wrecks.pos[w].extend(wrecks.z[w]);
        let gone = wrecks.mass[w] <= Fx::ZERO;
        if gone {
            wrecks.slots.free(w);
            self.events.push(SimEvent::Reclaimed {
                pos: at,
                blueprint: bp.id,
                wreck: true,
            });
        }
        self.reclaims.push(ReclaimWork {
            source: self.state.units.id(row),
            unit: Handle::NONE,
            at,
            radius: bp.radius,
            height: bp.height,
            relay: false,
            head,
        });
        if let Some(parent) = self
            .state
            .units
            .row(self.state.units.drone_parent[row])
            .filter(|&p| self.state.units.health[p] > Fx::ZERO)
        {
            let units = &self.state.units;
            self.reclaims.push(ReclaimWork {
                source: units.id(row),
                unit: units.id(parent),
                at: units.pos[parent].extend(units.z[parent]),
                radius: Fx::ONE,
                height: Fx::ZERO,
                relay: true,
                head,
            });
        }
        self.credit(row, take);
        gone
    }

    /// One tick of unbuilding a live unit. True once it has nothing left to give.
    pub(crate) fn drain_unit(&mut self, row: usize, t: usize, power: Fx, head: u8) -> bool {
        let tbp = self.bp(t);
        let (full, time, cost, radius, height) = (
            tbp.health,
            tbp.build_time,
            tbp.cost_mass,
            tbp.radius,
            tbp.height,
        );
        let (by, of, source) = (
            self.state.units.owner[row],
            self.state.units.owner[t],
            self.state.units.id(row),
        );
        let enemies = self.are_enemies(by, of);
        let taken = (full * (power / DT) / time)
            .max(Fx::EPSILON)
            .min(self.state.units.health[t]);
        self.state.units.health[t] -= taken;
        self.state.units.flags[t] |= flag::HURT;
        // A site is worth what has been put into it: a fresh one gives nothing back.
        let built = self.state.units.build_progress[t] / time;
        let mass = cost * taken / full * built * UNIT_YIELD;
        let gone = self.state.units.health[t] <= Fx::ZERO;
        if enemies {
            self.record_damage(t, source, taken);
        }
        if gone {
            self.state.units.flags[t] |= flag::RECLAIMED;
            if enemies {
                self.settle_kill(t, source, by);
            }
        }
        let units = &self.state.units;
        self.reclaims.push(ReclaimWork {
            source: units.id(row),
            unit: units.id(t),
            at: units.pos[t].extend(units.z[t]),
            radius,
            height,
            relay: false,
            head,
        });
        self.credit(row, mass);
        gone
    }

    fn credit(&mut self, row: usize, mass: Fx) {
        let player = &mut self.state.players[self.state.units.owner[row] as usize];
        player.mass += mass;
        player.reclaimed_mass += mass;
        let units = &mut self.state.units;
        units.reclaimed[row] += mass;
        if let Some(parent) = units.row(units.drone_parent[row]) {
            units.reclaimed[parent] += mass;
        }
        units.flags[row] |= flag::RECLAIMING;
        self.flow(row).made[0] += mass;
    }

    pub(crate) fn run_reclaim_unit(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let units = &self.state.units;
        let Some(t) = units
            .row(o.target)
            .filter(|&t| self.can_reclaim_unit(row, t))
        else {
            self.finish_order(row);
            return Ok(());
        };
        let (pos, radius) = (units.pos[t], self.bp(t).radius);
        let reach = self.work_range(row) + radius;
        if units.pos[row].distance(pos) > reach
            && self.bp(row).motion.is_some()
            && units.stuck_ticks[row] != u16::MAX
        {
            // After something that may be driving off: the way there is only asked for again once it has got far from it.
            let goal = if units.has_flag(row, flag::HAS_FIELD)
                && units.field_goal[row].distance(pos) <= CHASE_REPATH_DISTANCE
            {
                units.field_goal[row]
            } else {
                pos
            };
            self.ensure_moving(row, goal, pos)?;
            return self.reclaim_on_the_way(row);
        }
        if !self.approach(row, pos, radius)? {
            if self.state.units.stuck_ticks[row] == u16::MAX {
                self.finish_order(row);
                return Ok(());
            }
            return self.reclaim_on_the_way(row);
        }
        let done = if self.bp(row).reclaimer.is_some() {
            self.heads_work(row, HeadWork::Unit(t))
        } else {
            let power = self.tool_power(row);
            self.reclaim_ready(row, pos) && self.drain_unit(row, t, power, 0)
        };
        if done {
            self.finish_order(row);
        }
        Ok(())
    }

    /// What a builder or a drone pulls a second with its tools.
    pub(crate) fn tool_power(&self, row: usize) -> Fx {
        self.bp(row).reclaims().map_or(Fx::ZERO, |(power, _)| power)
    }

    /// Whether this builder turns an arm onto its work (see [`Self::face_work`]).
    fn aims_to_work(&self, row: usize) -> bool {
        self.bp(row)
            .builder
            .as_ref()
            .is_some_and(|b| b.arm.is_some())
    }

    /// Turns a builder's arm onto `pos`. True once the beam may come on. A reclaimer
    /// aims its heads instead (`reclaim_heads.rs`).
    pub(crate) fn reclaim_ready(&mut self, row: usize, pos: FxVec2) -> bool {
        self.face_work(row, pos)
    }

    /// On its way to what it was ordered to reclaim, a unit takes the wrecks it passes
    /// within its reach, as it would idle, without stopping for them.
    pub(crate) fn reclaim_on_the_way(&mut self, row: usize) -> Result<(), SimError> {
        self.idle_reclaim(row)
    }

    /// What a reclaimer with no orders does by itself: it clears the wrecks within
    /// its reach while there is room to store the mass. It never leaves where it
    /// stands, and it never turns its beam on a live unit unasked: that takes an
    /// order. A builder that carries weapons leaves wrecks alone while there is
    /// anything about to shoot at, so its torso is never on a wreck between two targets.
    pub(crate) fn idle_reclaim(&mut self, row: usize) -> Result<(), SimError> {
        let bp = self.bp(row);
        let Some((_, range)) = bp.reclaims() else {
            return Ok(());
        };
        let units = &self.state.units;
        if units.has_flag(row, flag::PASSIVE)
            || self.has_live_target(row)
            || self.enemy_in_gun_range(row)
        {
            return Ok(());
        }
        let (pos, owner) = (units.pos[row], units.owner[row]);
        if self.no_room_for_salvage(owner) {
            return Ok(());
        }
        if bp.reclaimer.is_some() {
            self.heads_clear_wrecks(row);
            return Ok(());
        }
        let wrecks = &self.state.wrecks;
        let usable = |e: &crate::spatial::Entry| {
            let w = e.row as usize;
            wrecks.slots.is_alive(w)
                && wrecks.pos[w] == e.pos
                && e.pos.distance(pos) <= range + e.radius
                && self.wreck_known(w, owner)
        };
        let found = if self.aims_to_work(row) {
            // Something that has to turn onto its work takes the wreck it is nearest to
            // pointing at, the nearer the better among those alike, so it sweeps a field
            // in order instead of swinging across its reach and back.
            let aim = units.heading[row] + units.weapon_yaw[row][0];
            let mut best: Option<(i64, crate::spatial::Entry)> = None;
            self.index
                .query(pos, range + WIDEST_TARGET, kind::WRECK, |e| {
                    if usable(e) {
                        let turn = aim.delta_to((e.pos - pos).angle()).unsigned_abs() as i64;
                        // A degree of turn (182 of a u16 turn) weighs as much as 18 m.
                        let cost = turn + (e.pos.distance(pos) * 10).floor_int() as i64;
                        if best.as_ref().is_none_or(|(c, _)| cost < *c) {
                            best = Some((cost, *e));
                        }
                    }
                    true
                });
            best.map(|(_, e)| e)
        } else {
            self.index
                .nearest(pos, range + WIDEST_TARGET, kind::WRECK, |e| usable(e))
        };
        if let Some(e) = found {
            if self.reclaim_ready(row, e.pos) {
                let power = self.tool_power(row);
                self.drain_wreck(row, e.row as usize, power, 0);
            }
        }
        Ok(())
    }

    /// Whether an enemy this unit can see stands within its guns' reach, or near enough to be there soon.
    pub(crate) fn enemy_in_gun_range(&self, row: usize) -> bool {
        let reach = self.bp(row).max_weapon_range();
        if reach <= Fx::ZERO {
            return false;
        }
        let units = &self.state.units;
        let (pos, owner, reach) = (units.pos[row], units.owner[row], reach * GUN_RANGE_MARGIN);
        let mut found = false;
        self.index.query(pos, reach, kind::UNIT, |e| {
            let t = e.row as usize;
            found = self.unit_entry_is_current(e)
                && self.are_enemies(owner, units.owner[t])
                && !units.has_flag(t, flag::IN_FACTORY)
                && self.detects(owner, t);
            !found
        });
        found
    }
}
