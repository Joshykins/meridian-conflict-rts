//! A Seedcaster's warp-ins (Shapers raised as `SiteKind::Artillery`). On a beat that
//! quickens with its level it marks a volley of places beside something of the
//! defenders' (a structure, six in ten, or an army on the ground), each announced to
//! every side with its countdown from the moment it is chosen (`RenderFrame::warp_ins`).
//! When the countdown runs out the squad warps in at the mark: a modest shock where it
//! comes out (shields first), and its units come out of the warp all but whole, finished
//! in a moment in the replication violet, then attack-move at the defenders.
//!
//! A defender's warp dampening field (a finished, powered unit with a `warp_damper`,
//! `World::damper_live`) is the counter, with the Seedcaster itself:
//!
//! - No mark falls in a field, and no squad stands in one. A target inside a field has
//!   its mark moved to the nearest ground clear of every field, no further than
//!   `WARP_REACH` from it; with no such ground another target is tried, and with none
//!   left the volley is held (`Hold::Dampened`, "Dampened" on the HUD and the card).
//! - A field that comes up over a mark already announced cancels it
//!   (`SimEvent::WarpInCancelled`): that squad is lost.
//!
//! Squad size, heavies, squads a volley, the beat, the countdown and the shock grow with
//! the level (`rules.rs`); a squad is made of the Seedcaster's products, which it takes
//! on as it levels like a printing Shaper.

use super::print::{block, Job, Route};
use super::rules::{
    arrival_shock, squad_size, squads_per_volley, volley_ticks, warp_in_ticks, WARP_IN_PRINT_TICKS,
    WARP_IN_WHOLE,
};
use super::{Hold, Spot, HZ};
use crate::mirror::SimEvent;
use crate::well_area::{Aircraft, WellArea};
use crate::{SimError, UnitId};
use mc_core::{Angle, Fx, FxVec2, FxVec3, Rng, StateHasher};
use mc_data::survival::SiteKind;
use mc_data::{BlueprintId, MoveLayer};
use serde::{Deserialize, Serialize};

/// Ticks a held volley waits before it tries again.
const HELD_RETRY_TICKS: u32 = 5 * HZ;
/// Metres from the thing it is aimed at a mark falls: beside it, not on it.
const NEAR: (i32, i32) = (40, 120);
/// Metres the ground a squad can stand on may lie from the mark.
const LAND_REACH: i32 = 150;
/// Targets tried for one mark before the volley gives up on it. A bound on the search.
const TRIES: usize = 8;
/// Metres from the target a mark moved out of a field may go: one Undertow's field (1600
/// m) pushes a squad to its edge, two overlapping hold it off.
pub const WARP_REACH: i32 = 2000;
/// Metres past a field's edge a moved mark keeps, and between a squad's edge and a field.
const EDGE: i32 = 24;
/// Rings of ground tried round a target in a field, and metres between them (out to
/// `WARP_REACH`), sixteen points a ring. A bound on the search.
const RINGS: i32 = 8;
const RING_STEP: i32 = WARP_REACH / RINGS;
/// Ticks between looks for a field come up over a mark: twice a second.
const CANCEL_EVERY: u32 = 5;

/// A squad marked to warp in.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct WarpIn {
    pub(super) serial: u32,
    /// The Seedcaster that marked it (it may be gone since: the squad still comes).
    pub(super) source: UnitId,
    pub(super) site: u8,
    /// The mark, on the ground.
    pub(super) at: FxVec3,
    /// The way the squad faces as it comes out: on from the Seedcaster.
    pub(super) heading: Angle,
    /// Where each of `squad` comes out.
    pub(super) places: Vec<FxVec2>,
    /// Metres round the mark the squad stands in: no field may reach inside it.
    pub(super) reach: Fx,
    /// Tick it was marked, and the tick it comes in.
    pub(super) marked: u32,
    pub(super) arrive: u32,
    pub(super) squad: Vec<BlueprintId>,
    /// Its Seedcaster's level when it marked it: the shock's.
    pub(super) level: u16,
}

/// Every squad marked and not yet in (`Survival::warp_ins`), and the auxiliary Shapers'
/// own dice.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct WarpIns {
    pub(super) pending: Vec<WarpIn>,
    /// The last serial handed out (squads and fleets share them).
    serial: u32,
    /// The auxiliary Shapers' own dice (warp-in, warhead and fleet marks), so their aim
    /// never changes what the rounds plan.
    pub(super) rng: Rng,
}

impl WarpIns {
    pub(super) fn new(seed: u64) -> WarpIns {
        WarpIns {
            pending: Vec::new(),
            serial: 0,
            rng: Rng::new(seed),
        }
    }

    /// A fresh serial for a warp-in or a fleet's.
    pub(super) fn next_serial(&mut self) -> u32 {
        self.serial = self.serial.wrapping_add(1);
        self.serial
    }

    pub(super) fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.serial as u64 | (self.pending.len() as u64) << 32);
        h.write_u64(self.rng.state());
        for w in &self.pending {
            h.write_u64(w.serial as u64 | (w.source.0 as u64) << 32);
            h.write_u64(w.marked as u64 | (w.arrive as u64) << 32);
            h.write_u64(
                w.site as u64
                    | (w.level as u64) << 8
                    | (w.squad.len() as u64) << 24
                    | (w.heading.0 as u64) << 40,
            );
            h.write_i64(w.at.x.0);
            h.write_i64(w.at.y.0);
            h.write_i64(w.at.z.0);
            h.write_i64(w.reach.0);
            for p in &w.places {
                h.write_i64(p.x.0 ^ p.y.0.rotate_left(32));
            }
            for b in &w.squad {
                h.write_u64(b.0 as u64);
            }
        }
    }

    /// Every blueprint the marked squads carry, for `State::validate_ids`.
    pub(super) fn blueprints(&self) -> impl Iterator<Item = BlueprintId> + '_ {
        self.pending.iter().flat_map(|w| w.squad.iter().copied())
    }
}

/// A warp dampening field of the defenders': its dampener, middle and radius.
#[derive(Clone, Copy, Debug)]
pub(super) struct Field {
    pub(super) unit: UnitId,
    pub(super) at: FxVec2,
    pub(super) radius: Fx,
}

/// The first field reaching within `reach` of `p`, if any.
pub(super) fn field_over(fields: &[Field], p: FxVec2, reach: Fx) -> Option<&Field> {
    fields.iter().find(|f| {
        let r = f.radius + reach + Fx::from_int(EDGE);
        f.at.distance_sq(p) < r * r
    })
}

/// What looking for a mark came to.
enum Mark {
    At(FxVec2),
    /// Every target tried lay in the defenders' fields, too deep to move out of.
    Dampened,
    /// Nothing to aim at, or no ground near what was tried.
    Nothing,
}

/// Metres from the middle of a squad of `units` laid out as a block (`print::block`) to
/// the far edge of its furthest unit.
fn squad_reach(bps: &mc_data::Blueprints, units: &[BlueprintId]) -> Fx {
    let radius = units
        .iter()
        .map(|b| bps.unit(*b).radius)
        .max()
        .unwrap_or(Fx::ZERO);
    let spots = block(units.len(), radius, FxVec2::ZERO, Angle::ZERO);
    let n = spots.len().max(1) as i32;
    let sum = spots.iter().fold(FxVec2::ZERO, |a, b| a + *b);
    let mid = FxVec2::new(sum.x / n, sum.y / n);
    let far = spots
        .iter()
        .map(|p| p.distance(mid))
        .max()
        .unwrap_or(Fx::ZERO);
    far + radius
}

impl crate::World {
    /// A tick of the Seedcasters: marks a field has come up over are cancelled, squads
    /// due come in, and Seedcasters due mark their next volley.
    pub(super) fn survival_warp_ins(&mut self, now: u32) -> Result<(), SimError> {
        if now.is_multiple_of(CANCEL_EVERY) {
            self.survival_warp_ins_dampened();
        }
        self.survival_warp_ins_arrive(now)?;
        let s = self.state.survival.as_ref().unwrap();
        let due: Vec<usize> = (0..s.nodes.len())
            .filter(|&i| {
                let n = &s.nodes[i];
                n.raised && now >= n.next_fire && n.kind == SiteKind::Artillery
            })
            .collect();
        for i in due {
            self.survival_volley(i, now)?;
        }
        Ok(())
    }

    /// The defenders' warp dampening fields up now: every finished, powered dampener of
    /// an enemy of the engine's side.
    pub(super) fn survival_fields(&self) -> Vec<Field> {
        let Some(s) = self.state.survival.as_ref() else {
            return Vec::new();
        };
        let side = s.config.engine_player;
        let u = &self.state.units;
        u.slots
            .iter()
            .filter_map(|r| {
                let d = self.bp(r).warp_damper?;
                (self.are_enemies(side, u.owner[r]) && self.damper_live(r)).then(|| Field {
                    unit: u.id(r),
                    at: u.pos[r],
                    radius: d.radius,
                })
            })
            .collect()
    }

    /// What one squad of Shaper `i` is: its heavies first, then its light units.
    pub(super) fn squad_of(&self, i: usize) -> Vec<BlueprintId> {
        let n = &self.state.survival.as_ref().unwrap().nodes[i];
        let bps = self.blueprints.as_ref();
        let (light, heavy): (Vec<BlueprintId>, Vec<BlueprintId>) =
            n.products.iter().partition(|p| bps.unit(**p).tech < 4);
        let light_tech = light.iter().map(|p| bps.unit(*p).tech).max().unwrap_or(1);
        let (nl, nh) = squad_size(n.level, light_tech, !heavy.is_empty());
        let mut squad: Vec<BlueprintId> = (0..nh).map(|k| heavy[k % heavy.len()]).collect();
        if !light.is_empty() {
            squad.extend((0..nl).map(|k| light[k % light.len()]));
        }
        squad
    }

    /// Shaper `i` marks a volley: a squad a mark, each its own. One the field cap cannot
    /// take, or with nowhere outside the defenders' fields to go, holds the volley there,
    /// and it tries again shortly.
    fn survival_volley(&mut self, i: usize, now: u32) -> Result<(), SimError> {
        let s = self.state.survival.as_ref().unwrap();
        let n = s.nodes[i].clone();
        let Some(row) = self.state.units.row(n.unit) else {
            return Ok(());
        };
        let from = self.state.units.pos[row];
        let squad = self.squad_of(i);
        let bps = self.blueprints.as_ref();
        let size_class = squad
            .iter()
            .filter_map(|b| bps.unit(*b).motion)
            .map(|m| m.size_class)
            .max()
            .unwrap_or(0);
        let reach = squad_reach(bps, &squad) + Fx::from_int(EDGE);
        let fields = self.survival_fields();
        let mut rng = s.warp_ins.rng;
        let mut marks: Vec<(FxVec2, Vec<FxVec2>, Angle)> = Vec::new();
        let mut hold = Hold::None;
        let want = squads_per_volley(n.level);
        for _ in 0..want {
            if squad.is_empty() {
                break;
            }
            let taken: Vec<FxVec2> = marks.iter().map(|m| m.0).collect();
            let mark = match self.warp_mark(&mut rng, size_class, reach, &taken, &fields) {
                Mark::At(at) => at,
                Mark::Dampened => {
                    hold = Hold::Dampened;
                    break;
                }
                Mark::Nothing => break,
            };
            let heading = (mark - from).angle();
            let places = self.formation_places(&squad, mark, heading);
            // A unit the ground pushed into a field: this mark will not do.
            let bps = self.blueprints.as_ref();
            if places
                .iter()
                .zip(&squad)
                .any(|(p, b)| field_over(&fields, *p, bps.unit(*b).radius).is_some())
            {
                hold = Hold::Dampened;
                break;
            }
            marks.push((mark, places, heading));
        }
        let ticks = warp_in_ticks(n.level);
        let mut announced = Vec::new();
        for (k, (mark, places, heading)) in marks.iter().enumerate() {
            let ground = self
                .terrain
                .height_at(*mark)
                .max(self.terrain.water_level());
            let at = mark.extend(ground);
            let w = &mut self.state.survival.as_mut().unwrap().warp_ins;
            let serial = w.next_serial();
            w.pending.push(WarpIn {
                serial,
                source: n.unit,
                site: n.site,
                at,
                heading: *heading,
                places: places.clone(),
                reach,
                marked: now,
                arrive: now + ticks,
                squad: squad.clone(),
                level: n.level,
            });
            announced.push(SimEvent::WarpInMarked {
                serial,
                source: n.unit,
                site: n.site,
                at,
                ticks,
                units: squad.len().min(u16::MAX as usize) as u16,
                volley: k.min(u8::MAX as usize) as u8,
            });
        }
        self.events.extend(announced);
        let s = self.state.survival.as_mut().unwrap();
        s.warp_ins.rng = rng;
        let node = &mut s.nodes[i];
        node.hold = hold;
        node.next_fire = if hold != Hold::None && marks.is_empty() {
            now + HELD_RETRY_TICKS
        } else {
            now + volley_ticks(n.level)
        };
        Ok(())
    }

    /// A mark beside something of the defenders': six in ten at a structure, the rest at
    /// an army on the ground, each a little way off it on ground a squad of `size_class`
    /// can stand on, its `reach` clear of every field, and not within two reaches of a
    /// mark already chosen. A target in a field has its mark moved out (`clear_ground`).
    fn warp_mark(
        &self,
        rng: &mut Rng,
        size_class: u8,
        reach: Fx,
        taken: &[FxVec2],
        fields: &[Field],
    ) -> Mark {
        let (structures, armies) = self.survival_targets();
        let apart = reach * 2;
        let mut dampened = false;
        for _ in 0..TRIES {
            let structure = armies.is_empty() || (!structures.is_empty() && rng.below(10) < 6);
            let pool = if structure { &structures } else { &armies };
            if pool.is_empty() {
                return Mark::Nothing;
            }
            let target = pool[rng.below(pool.len() as u32) as usize];
            let turn = Angle(rng.below(1 << 16) as u16);
            let off = Fx::from_int(NEAR.0 + rng.below((NEAR.1 - NEAR.0) as u32) as i32);
            let first = self.clamp_to_map(target + FxVec2::from_angle(turn) * off);
            let Some(mark) = self.clear_ground(target, first, reach, fields, turn) else {
                dampened = true;
                continue;
            };
            let Some(ground) = self
                .nav
                .nearest_passable(MoveLayer::Land, size_class, mark)
                .filter(|p| p.distance(mark) <= Fx::from_int(LAND_REACH))
            else {
                continue;
            };
            if field_over(fields, ground, reach).is_some() {
                dampened = true;
                continue;
            }
            if taken.iter().all(|t| t.distance(ground) >= apart) {
                return Mark::At(ground);
            }
        }
        if dampened {
            Mark::Dampened
        } else {
            Mark::Nothing
        }
    }

    /// The defenders' structures and their units on the ground: what the auxiliary
    /// Shapers aim at.
    pub(super) fn survival_targets(&self) -> (Vec<FxVec2>, Vec<FxVec2>) {
        let Some(s) = self.state.survival.as_ref() else {
            return (Vec::new(), Vec::new());
        };
        let side = s.config.engine_player;
        let u = &self.state.units;
        let (mut structures, mut armies) = (Vec::new(), Vec::new());
        for r in u.slots.iter() {
            if !u.is_active(r) || !self.are_enemies(side, u.owner[r]) {
                continue;
            }
            match self.bp(r).motion {
                None => structures.push(u.pos[r]),
                Some(m) if m.layer != MoveLayer::Air => armies.push(u.pos[r]),
                Some(_) => {}
            }
        }
        (structures, armies)
    }

    /// `p` if a squad `reach` across can stand there clear of every field; else the
    /// nearest place to it that can, no further than `WARP_REACH` from `target`: each
    /// field's edge straight out from `p`, and rings of points round `p`. None when there
    /// is none. `turn` picks the way out of a field whose middle is `p` itself.
    pub(super) fn clear_ground(
        &self,
        target: FxVec2,
        p: FxVec2,
        reach: Fx,
        fields: &[Field],
        turn: Angle,
    ) -> Option<FxVec2> {
        if field_over(fields, p, reach).is_none() {
            return Some(p);
        }
        let most = Fx::from_int(WARP_REACH);
        let mut best: Option<(Fx, FxVec2)> = None;
        let mut consider = |q: FxVec2| {
            let q = self.clamp_to_map(q);
            if q.distance(target) > most || field_over(fields, q, reach).is_some() {
                return;
            }
            let d = q.distance_sq(p);
            if best.is_none_or(|(b, _)| d < b) {
                best = Some((d, q));
            }
        };
        for f in fields {
            let out = if f.at == p {
                FxVec2::from_angle(turn)
            } else {
                (p - f.at).normalize()
            };
            consider(f.at + out * (f.radius + reach + Fx::from_int(EDGE * 2)));
        }
        for ring in 1..=RINGS {
            for k in 0..16u16 {
                let angle = Angle(turn.0.wrapping_add(k * 4096 + ring as u16 * 2048));
                consider(p + FxVec2::from_angle(angle) * Fx::from_int(ring * RING_STEP));
            }
        }
        best.map(|b| b.1)
    }

    /// Marks a defender's field has come up over since they were announced are
    /// cancelled: those squads are lost.
    fn survival_warp_ins_dampened(&mut self) {
        let Some(s) = self.state.survival.as_ref() else {
            return;
        };
        if s.warp_ins.pending.is_empty() {
            return;
        }
        let fields = self.survival_fields();
        if fields.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.state.survival.as_mut().unwrap().warp_ins.pending);
        let mut keep = Vec::with_capacity(pending.len());
        for w in pending {
            match field_over(&fields, w.at.xy(), w.reach) {
                Some(f) => self.events.push(SimEvent::WarpInCancelled {
                    serial: w.serial,
                    site: w.site,
                    at: w.at,
                    by: f.unit,
                    units: w.squad.len().min(u16::MAX as usize) as u16,
                }),
                None => keep.push(w),
            }
        }
        self.state.survival.as_mut().unwrap().warp_ins.pending = keep;
    }

    /// Squads whose countdown is over come in: the shock where they come out, and the
    /// squad out of the warp all but whole.
    fn survival_warp_ins_arrive(&mut self, now: u32) -> Result<(), SimError> {
        let s = self.state.survival.as_mut().unwrap();
        let side = s.config.engine_player;
        let (due, pending): (Vec<WarpIn>, Vec<WarpIn>) = std::mem::take(&mut s.warp_ins.pending)
            .into_iter()
            .partition(|w| now >= w.arrive);
        s.warp_ins.pending = pending;
        for w in due {
            let (radius, damage) = arrival_shock(w.level);
            self.crush_area(&WellArea {
                mark: w.at.xy(),
                z: w.at.z,
                radius,
                owner: side,
                source: w.source,
                per_tick: damage,
                aircraft: Aircraft::Spared,
            });
            self.events.push(SimEvent::WarpInArrived {
                serial: w.serial,
                at: w.at,
                radius,
                units: w.squad.len().min(u16::MAX as usize) as u16,
            });
            let job = Job {
                spot: Spot::WarpIn {
                    at: w.at.xy(),
                    heading: w.heading,
                },
                source: crate::Handle::NONE,
                route: Route::Free,
                round: false,
                ticks: WARP_IN_PRINT_TICKS,
                done: WARP_IN_PRINT_TICKS * WARP_IN_WHOLE.0 / WARP_IN_WHOLE.1,
            };
            self.survival_print_at(&w.squad, &w.places, w.heading, job, now)?;
        }
        Ok(())
    }
}
