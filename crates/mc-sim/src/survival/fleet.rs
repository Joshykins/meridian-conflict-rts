//! A Gatewright's fleets (Shapers raised as `SiteKind::Fleet`). A Gatewright prints a
//! fleet of warships beside itself, hanging in the air in front of it under its beam
//! (`Spot::Berth`): every warship of the roster with a warp drive up to the tier the
//! rounds print at (`fleet_roster`), more of them and bigger classes as it levels, and
//! capital ships from level 10 once the rounds reach T4 (`rules::fleet_size`,
//! `fleet_classes`). Built, the fleet is given a mark over the defenders (one of their
//! structures, else an army on the ground), announced to every side with its countdown
//! (`SimEvent::FleetMarked`, `RenderFrame::warp_ins`); `FLEET_LEAD_TICKS` later its ships
//! are given a real warp jump there (`Command::Warp`, `warp.rs`), in the formation they
//! were built in. Out of the warp they attack-move at the defenders, and the Gatewright
//! rests (`fleet_gap_ticks`) before printing the next.
//!
//! A fleet may warp into a defender's dampening field, unlike a Seedcaster's squads: the
//! field is its counter, and drags, hurts and stuns it as it does any jump. It prefers a
//! mark clear of every field when one is near its target.

use super::print::{Job, Route};
use super::rules::{fleet_classes, fleet_gap_ticks, fleet_size, shaper_foot, FLEET_LEAD_TICKS};
use super::{Hold, Spot, HZ};
use crate::mirror::SimEvent;
use crate::tables::{OrderKind, WarpPhase};
use crate::{Command, SimError, UnitId};
use mc_core::{Angle, Fx, FxVec2, StateHasher};
use mc_data::survival::SiteKind;
use mc_data::BlueprintId;
use serde::{Deserialize, Serialize};

/// Ticks a Gatewright with nothing to build or nothing to aim at waits to look again.
const RETRY_TICKS: u32 = 5 * HZ;
/// Targets tried for a mark clear of every field before the fleet takes the first. A
/// bound on the search.
const TRIES: usize = 6;
/// Ticks after its ships were ordered to jump that a fleet is sent on regardless: a
/// ship held up (stunned, called off) is not waited on for ever. A deliberate bound.
const WARP_TIMEOUT_TICKS: u32 = 120 * HZ;
/// Metres between the edge of a Gatewright's widest foot and its fleet's berth.
const BERTH_GAP: i32 = 80;

/// Where a Gatewright's fleet is. The numbers may reach the interface: never renumber one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FleetStage {
    /// Resting after its last fleet (or not yet started), until its next is due.
    #[default]
    Resting = 0,
    /// Printing its fleet beside it.
    Building = 1,
    /// Built, its mark announced; its ships start to spool at `Fleet::jump_at`.
    Marked = 2,
    /// Ordered to jump: spooling, in warp or coming out.
    Warping = 3,
}

/// One Gatewright's fleet (`Survival::fleets`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Fleet {
    pub(super) shaper: UnitId,
    pub(super) site: u8,
    pub(super) stage: FleetStage,
    /// The ships of the fleet being built or about to warp in (none while it rests).
    pub(super) ships: Vec<UnitId>,
    /// Its mark's serial (`WarpIns::next_serial`) and the mark; the tick it was given and
    /// the tick its ships spool.
    pub(super) serial: u32,
    pub(super) mark: FxVec2,
    pub(super) marked: u32,
    pub(super) jump_at: u32,
    /// Tick its build started, and the ticks it takes.
    pub(super) started: u32,
    pub(super) build_ticks: u32,
    /// Its ships sent at the defenders and still alive.
    pub(super) field: Vec<UnitId>,
}

impl Fleet {
    pub(super) fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.shaper.0 as u64 | (self.site as u64) << 32 | (self.stage as u64) << 40);
        h.write_u64(self.serial as u64 | (self.marked as u64) << 32);
        h.write_u64(self.jump_at as u64 | (self.started as u64) << 32);
        h.write_u64(self.build_ticks as u64 | (self.ships.len() as u64) << 32);
        h.write_i64(self.mark.x.0 ^ self.mark.y.0.rotate_left(32));
        for u in self.ships.iter().chain(&self.field) {
            h.write_u64(u.0 as u64);
        }
    }

    /// Whether `unit` is one of its ships not yet sent at the defenders: the Gatewright
    /// gives it its orders.
    pub(super) fn holds(&self, unit: UnitId) -> bool {
        self.ships.contains(&unit)
    }
}

impl crate::World {
    /// Every warship a Gatewright may build at `tier`: listed spacecraft with a warp drive
    /// and guns, no transports, the cheapest class first.
    pub(super) fn fleet_roster(&self, tier: u8) -> Vec<BlueprintId> {
        let bps = self.blueprints.as_ref();
        let mut ships: Vec<(Fx, BlueprintId)> = bps
            .units
            .iter()
            .filter(|u| {
                bps.is_listed(u.id)
                    && u.has(mc_data::cat::SPACE)
                    && u.warp.is_some()
                    && u.transport.is_none()
                    && !u.weapons.is_empty()
                    && u.tech <= tier
            })
            .map(|u| (u.cost_materials, u.id))
            .collect();
        // Cost then id: a total order.
        ships.sort_unstable();
        ships.into_iter().map(|s| s.1).collect()
    }

    /// The classes a Gatewright of `level` builds from at `tier`: its light classes, then
    /// its capital ones (`rules::fleet_classes`).
    pub(super) fn fleet_classes_of(&self, level: u16, tier: u8) -> Vec<BlueprintId> {
        let bps = self.blueprints.as_ref();
        let (light, capital): (Vec<BlueprintId>, Vec<BlueprintId>) = self
            .fleet_roster(tier)
            .into_iter()
            .partition(|b| bps.unit(*b).tech < 4);
        let (cl, cc) = fleet_classes(level);
        let mut open: Vec<BlueprintId> = light.iter().copied().take(cl).collect();
        open.extend(capital.iter().copied().take(cc));
        open
    }

    /// One fleet of a Gatewright of `level` at `tier`: its capital ships, then its light
    /// ones, each the biggest classes open to it first.
    pub(super) fn fleet_of(&self, level: u16, tier: u8) -> Vec<BlueprintId> {
        let bps = self.blueprints.as_ref();
        let (light, capital): (Vec<BlueprintId>, Vec<BlueprintId>) = self
            .fleet_classes_of(level, tier)
            .into_iter()
            .partition(|b| bps.unit(*b).tech < 4);
        let (nl, nc) = fleet_size(level);
        let pick = |set: &[BlueprintId], n: usize| -> Vec<BlueprintId> {
            if set.is_empty() {
                return Vec::new();
            }
            (0..n).map(|k| set[set.len() - 1 - k % set.len()]).collect()
        };
        let mut fleet = pick(&capital, nc);
        fleet.extend(pick(&light, nl));
        fleet
    }

    /// A tick of the Gatewrights' fleets.
    pub(super) fn survival_fleets(&mut self, now: u32) -> Result<(), SimError> {
        self.survival_fleets_tally();
        let s = self.state.survival.as_ref().unwrap();
        for k in 0..s.fleets.len() {
            let f = &self.state.survival.as_ref().unwrap().fleets[k];
            match f.stage {
                FleetStage::Resting => self.fleet_rest(k, now)?,
                FleetStage::Building => self.fleet_built(k, now),
                FleetStage::Marked if now >= f.jump_at => self.fleet_jump(k)?,
                FleetStage::Marked => {}
                FleetStage::Warping => self.fleet_arrived(k, now)?,
            }
        }
        Ok(())
    }

    /// A fleet record for every Gatewright standing; the dead struck off the fleets, and
    /// a fleet whose Gatewright is gone and that has nothing left to send let go.
    fn survival_fleets_tally(&mut self) {
        let Some(s) = self.state.survival.as_mut() else {
            return;
        };
        let units = &self.state.units;
        for n in s.nodes.iter().filter(|n| n.kind == SiteKind::Fleet) {
            if !s.fleets.iter().any(|f| f.shaper == n.unit) {
                s.fleets.push(Fleet {
                    shaper: n.unit,
                    site: n.site,
                    stage: FleetStage::Resting,
                    ships: Vec::new(),
                    serial: 0,
                    mark: FxVec2::ZERO,
                    marked: 0,
                    jump_at: 0,
                    started: 0,
                    build_ticks: 0,
                    field: Vec::new(),
                });
            }
        }
        for f in s.fleets.iter_mut() {
            f.ships.retain(|u| units.row(*u).is_some());
            f.field.retain(|u| units.row(*u).is_some());
            if f.ships.is_empty() && matches!(f.stage, FleetStage::Marked | FleetStage::Warping) {
                f.stage = FleetStage::Resting;
            }
        }
        let nodes = &s.nodes;
        s.fleets.retain(|f| {
            nodes.iter().any(|n| n.unit == f.shaper) || !f.ships.is_empty() || !f.field.is_empty()
        });
    }

    /// The Gatewright of fleet `k`, if it stands: its index among the nodes.
    fn fleet_node(&self, k: usize) -> Option<usize> {
        let s = self.state.survival.as_ref()?;
        let shaper = s.fleets[k].shaper;
        s.nodes
            .iter()
            .position(|n| n.unit == shaper && n.raised)
            .filter(|_| self.state.units.row(shaper).is_some())
    }

    /// Resting: when its next fleet is due, the Gatewright starts printing it at its berth.
    fn fleet_rest(&mut self, k: usize, now: u32) -> Result<(), SimError> {
        let Some(i) = self.fleet_node(k) else {
            return Ok(());
        };
        let s = self.state.survival.as_ref().unwrap();
        let n = s.nodes[i].clone();
        if now < n.next_fire {
            return Ok(());
        }
        let tier = s.config.rules.tier_at(s.round.max(1));
        let fleet = self.fleet_of(n.level, tier);
        if fleet.is_empty() {
            self.state.survival.as_mut().unwrap().nodes[i].next_fire = now + RETRY_TICKS;
            return Ok(());
        }
        let Some(row) = self.state.units.row(n.unit) else {
            return Ok(());
        };
        // In front of it, clear of the widest its foot grows, the block's middle there.
        let heading = self.state.units.heading[row];
        let bps = self.blueprints.as_ref();
        let radius = fleet
            .iter()
            .map(|b| bps.unit(*b).radius)
            .max()
            .unwrap_or(Fx::ZERO);
        let out = shaper_foot(SiteKind::Fleet, Fx::from_int(100_000))
            + radius * 2
            + Fx::from_int(BERTH_GAP);
        let at = self.clamp_to_map(n.pos + FxVec2::from_angle(heading) * out);
        let ticks = super::print::formation_ticks(bps, &fleet);
        let job = Job {
            spot: Spot::Berth { at, heading },
            source: n.unit,
            route: Route::Held,
            round: false,
            ticks,
            done: 0,
        };
        let places = self.formation_places(&fleet, at, heading);
        let ships = self.survival_print_at(&fleet, &places, heading, job, now)?;
        let s = self.state.survival.as_mut().unwrap();
        s.nodes[i].hold = Hold::None;
        s.nodes[i].printed += ships.len() as u32;
        let f = &mut s.fleets[k];
        f.ships = ships;
        f.stage = FleetStage::Building;
        f.started = now;
        f.build_ticks = ticks;
        Ok(())
    }

    /// Building: once every ship left is finished, the fleet is given its mark.
    fn fleet_built(&mut self, k: usize, now: u32) {
        let s = self.state.survival.as_ref().unwrap();
        let f = &s.fleets[k];
        if f.ships.is_empty() {
            // Shot down on the berth: it rests, and starts again.
            let shaper = f.shaper;
            let s = self.state.survival.as_mut().unwrap();
            s.fleets[k].stage = FleetStage::Resting;
            if let Some(n) = s.nodes.iter_mut().find(|n| n.unit == shaper) {
                n.next_fire = now + fleet_gap_ticks(n.level) / 2;
            }
            return;
        }
        if s.printing.iter().any(|p| f.ships.contains(&p.unit)) {
            return;
        }
        let ships = f.ships.clone();
        let mut rng = s.warp_ins.rng;
        let mark = self.fleet_mark(&mut rng, &ships);
        let s = self.state.survival.as_mut().unwrap();
        s.warp_ins.rng = rng;
        let Some(mark) = mark else {
            // Nothing of the defenders' to aim at: it holds, ready, and looks again.
            s.fleets[k].started = now;
            return;
        };
        let serial = s.warp_ins.next_serial();
        let f = &mut s.fleets[k];
        f.stage = FleetStage::Marked;
        f.serial = serial;
        f.mark = mark;
        f.marked = now;
        f.jump_at = now + FLEET_LEAD_TICKS;
        let (source, site) = (f.shaper, f.site);
        let ground = self.terrain.height_at(mark).max(self.terrain.water_level());
        self.events.push(SimEvent::FleetMarked {
            serial,
            source,
            site,
            at: mark.extend(ground),
            ships: ships.len().min(u16::MAX as usize) as u16,
            ticks: FLEET_LEAD_TICKS,
        });
    }

    /// Where fleet `ships` warps in: over one of the defenders' structures (else an army
    /// on the ground), the first of a few tried whose ground round it keeps the whole
    /// fleet clear of the defenders' fields, else the first tried, field and all.
    fn fleet_mark(&self, rng: &mut mc_core::Rng, ships: &[UnitId]) -> Option<FxVec2> {
        let (structures, armies) = self.survival_targets();
        let pool = if structures.is_empty() {
            armies
        } else {
            structures
        };
        if pool.is_empty() {
            return None;
        }
        let at: Vec<(FxVec2, Fx)> = ships
            .iter()
            .filter_map(|u| self.state.units.row(*u))
            .map(|r| (self.state.units.pos[r], self.bp(r).radius))
            .collect();
        let n = at.len().max(1) as i32;
        let sum = at.iter().fold(FxVec2::ZERO, |a, b| a + b.0);
        let middle = FxVec2::new(sum.x / n, sum.y / n);
        let reach = at
            .iter()
            .map(|(p, r)| p.distance(middle) + *r)
            .max()
            .unwrap_or(Fx::ZERO);
        let fields = self.survival_fields();
        let mut first = None;
        for _ in 0..TRIES {
            let target = pool[rng.below(pool.len() as u32) as usize];
            first.get_or_insert(target);
            let turn = Angle(rng.below(1 << 16) as u16);
            if let Some(clear) = self.clear_ground(target, target, reach, &fields, turn) {
                return Some(clear);
            }
        }
        first
    }

    /// Marked, and its lead run out: its ships are given the jump, in their formation.
    fn fleet_jump(&mut self, k: usize) -> Result<(), SimError> {
        let s = self.state.survival.as_mut().unwrap();
        let side = s.config.engine_player;
        let f = &mut s.fleets[k];
        f.stage = FleetStage::Warping;
        let (ships, mark) = (f.ships.clone(), f.mark);
        for chunk in ships.chunks(crate::command::MAX_COMMAND_UNITS) {
            self.apply_as(
                side,
                &Command::Warp {
                    units: chunk.to_vec(),
                    pos: mark,
                    queue: false,
                },
            )?;
        }
        Ok(())
    }

    /// Warping: once every ship is out of the warp (or the jump has run too long), the
    /// fleet attack-moves at the defenders and the Gatewright rests.
    fn fleet_arrived(&mut self, k: usize, now: u32) -> Result<(), SimError> {
        let s = self.state.survival.as_ref().unwrap();
        let f = &s.fleets[k];
        let u = &self.state.units;
        let out = f.ships.iter().all(|id| {
            u.row(*id).is_none_or(|r| {
                u.warp[r].phase == WarpPhase::Idle
                    && self
                        .state
                        .orders
                        .front(u, r)
                        .is_none_or(|o| o.kind != OrderKind::Warp)
            })
        });
        if !out && now < f.jump_at + WARP_TIMEOUT_TICKS {
            return Ok(());
        }
        let ships = f.ships.clone();
        let shaper = f.shaper;
        self.survival_send(&ships, Route::Free, false)?;
        let s = self.state.survival.as_mut().unwrap();
        let f = &mut s.fleets[k];
        f.field.extend(ships);
        f.ships.clear();
        f.stage = FleetStage::Resting;
        if let Some(n) = s.nodes.iter_mut().find(|n| n.unit == shaper) {
            n.next_fire = now + fleet_gap_ticks(n.level);
        }
        Ok(())
    }
}
