//! The print bays: each takes a whole formation and prints it at once, its
//! units laid out in their block on the bay's apron and out past its doors,
//! then the formation musters at the head of its front, and when the round's
//! last formation is out the round goes together.

use super::{domain_of, Formation, Muster, Phase, Print, HZ, NODE_PRINT};
use crate::mirror::SimEvent;
use crate::{Command, SimError, UnitId};
use mc_core::{Angle, Fx, FxVec2};
use mc_data::survival::Domain;
use mc_data::BlueprintId;

/// A ship's goal is open water no further than this from a defender (m).
const SEA_GOAL_REACH: i32 = 500;

/// Ticks to print a unit of `tech` and collision `radius`: a second more for
/// every 4 m of radius, so a capital ship takes most of a minute.
pub(super) fn print_ticks(tech: u8, radius: Fx) -> u32 {
    (3 + 2 * tech as u32 + radius.round_int().max(0) as u32 / 4) * HZ
}

/// Ticks to print a whole formation at once: its slowest unit's time, and a
/// quarter of that again for every unit after the first.
pub(super) fn formation_ticks(bps: &mc_data::Blueprints, units: &[BlueprintId]) -> u32 {
    let one = units
        .iter()
        .map(|b| {
            let u = bps.unit(*b);
            print_ticks(u.tech, u.radius)
        })
        .max()
        .unwrap_or(HZ);
    one + one * units.len().saturating_sub(1) as u32 / 4
}

/// Where `n` units of up to `radius` stand in a block whose front row is
/// centred on `at`, facing `heading`, the further rows ahead of it: wider than
/// deep, the way the block will march.
pub(super) fn block(n: usize, radius: Fx, at: FxVec2, heading: Angle) -> Vec<FxVec2> {
    let mut cols = 1;
    while cols * cols < 2 * n {
        cols += 1;
    }
    let cols = cols.min(n.max(1));
    let gap = (radius * 2 + Fx::from_int(8)).max(Fx::from_int(16));
    let ahead = FxVec2::from_angle(heading);
    let across = FxVec2::new(-ahead.y, ahead.x);
    (0..n)
        .map(|i| {
            let (row, col) = (i / cols, i % cols);
            let in_row = cols.min(n - row * cols);
            let off = Fx::ratio(2 * col as i64 - (in_row as i64 - 1), 2);
            at + across * (gap * off) + ahead * (gap * Fx::from_int(row as i32))
        })
        .collect()
}

impl crate::World {
    /// Hands queued formations to free print bays: land units and aircraft to
    /// the halls' bays, ships to the sea gate's slips.
    pub(super) fn survival_bays(&mut self, now: u32) -> Result<(), SimError> {
        let s = self.state.survival.as_ref().unwrap();
        if s.queue.is_empty() {
            return Ok(());
        }
        let side = s.config.engine_player;
        if self.hostile_count(side) >= super::FIELD_CAP {
            return Ok(());
        }
        let bays = s.config.bays.clone();
        let mut queue = std::mem::take(&mut self.state.survival.as_mut().unwrap().queue);
        let mut i = 0;
        while i < queue.len() {
            let Some(lead) = queue[i].units.first().copied() else {
                queue.remove(i);
                continue;
            };
            let Some(motion) = self.blueprints.unit(lead).motion else {
                queue.remove(i);
                continue;
            };
            // Ships go to the slips; aircraft to the aeries, else the halls' bays;
            // land units to the halls' bays. Of those, the smallest free one that
            // fits its biggest unit, so the great bays wait for the great units.
            // A unit too big for every bay takes the biggest.
            let domain = domain_of(motion.layer);
            let takes = |b: &super::Bay| match domain {
                Domain::Naval => b.domain == Domain::Naval,
                Domain::Air => b.domain != Domain::Naval,
                Domain::Land => b.domain == Domain::Land,
            };
            let radius = queue[i]
                .units
                .iter()
                .map(|b| self.blueprints.unit(*b).radius)
                .max()
                .unwrap_or(Fx::ZERO);
            if !bays.iter().any(takes) {
                queue.remove(i);
                continue;
            }
            let fits_any = bays.iter().any(|b| takes(b) && b.room(radius).is_some());
            let room = |b: &super::Bay| {
                if fits_any {
                    b.room(radius)
                } else {
                    // Too big for all of them: the biggest will do.
                    Some(Fx::from_int(1 << 20) - b.room(Fx::ZERO).unwrap_or(Fx::ZERO))
                }
            };
            let Some(bay) = (0..bays.len())
                .filter(|b| takes(&bays[*b]) && !self.survival_bay_taken(*b as u8))
                .filter_map(|b| {
                    room(&bays[b]).map(|r| {
                        (
                            (domain == Domain::Air && bays[b].domain != Domain::Air, r),
                            b,
                        )
                    })
                })
                .min()
                .map(|(_, b)| b)
            else {
                i += 1;
                continue;
            };
            let f = queue.remove(i);
            self.survival_print_formation(&f.units, bay as u8, f.front, crate::Handle::NONE, now)?;
        }
        self.state.survival.as_mut().unwrap().queue = queue;
        Ok(())
    }

    /// Starts printing `units` as one block: on bay `bay` (its apron, then out
    /// past its doors), or in front of the Shaper `source` when `bay` is
    /// `NODE_PRINT`. All of it takes the same time and finishes together.
    pub(super) fn survival_print_formation(
        &mut self,
        units: &[BlueprintId],
        bay: u8,
        front: u8,
        source: UnitId,
        now: u32,
    ) -> Result<(), SimError> {
        let s = self.state.survival.as_ref().unwrap();
        let side = s.config.engine_player;
        let (at, heading) = if bay == NODE_PRINT {
            let Some(row) = self.state.units.row(source) else {
                return Ok(());
            };
            let heading = self.state.units.heading[row];
            let ahead = FxVec2::from_angle(heading);
            (
                self.state.units.pos[row] + ahead * Fx::from_int(super::NODE_PRINT_REACH),
                heading,
            )
        } else {
            let b = s.config.bays[bay as usize];
            (b.at, b.heading)
        };
        let radius = units
            .iter()
            .map(|b| self.blueprints.unit(*b).radius)
            .max()
            .unwrap_or(Fx::ZERO);
        let ticks = formation_ticks(self.blueprints.as_ref(), units);
        let formation = {
            let s = self.state.survival.as_mut().unwrap();
            s.next_formation = s.next_formation.wrapping_add(1);
            s.next_formation
        };
        for (bp, spot) in units.iter().zip(block(units.len(), radius, at, heading)) {
            let Some(motion) = self.blueprints.unit(*bp).motion else {
                continue;
            };
            let spot = self
                .nav
                .nearest_passable(motion.layer, motion.size_class, spot)
                .unwrap_or(spot);
            let row = self.spawn_unit(*bp, side, spot, heading, false)?;
            let unit = self.state.units.id(row);
            self.state.survival.as_mut().unwrap().printing.push(Print {
                unit,
                bay,
                source,
                front,
                formation,
                started: now,
                ticks,
            });
        }
        Ok(())
    }

    /// A formation still standing on its bay keeps the bay.
    fn survival_bay_taken(&self, bay: u8) -> bool {
        let s = self.state.survival.as_ref().unwrap();
        s.printing.iter().any(|p| p.bay == bay)
    }

    pub(super) fn hostile_count(&self, side: u8) -> usize {
        let u = &self.state.units;
        u.slots
            .iter()
            .filter(|&r| u.owner[r] == side && self.blueprints.unit(u.blueprint[r]).is_mobile())
            .count()
    }

    /// Builds up everything being printed; releases each formation that is done.
    pub(super) fn survival_prints(&mut self, now: u32) -> Result<(), SimError> {
        let Some(s) = self.state.survival.as_mut() else {
            return Ok(());
        };
        let prints = std::mem::take(&mut s.printing);
        let mut keep = Vec::with_capacity(prints.len());
        let mut done: Vec<Print> = Vec::new();
        for p in prints {
            let Some(row) = self.state.units.row(p.unit) else {
                continue; // shot down on the bay
            };
            if p.bay == NODE_PRINT && self.state.units.row(p.source).is_none() {
                // Its node fell: what it was printing falls apart with it.
                self.state.units.health[row] = Fx::ZERO;
                continue;
            }
            let bp = self.blueprints.unit(self.state.units.blueprint[row]);
            let t = Fx::ratio((now - p.started) as i64, p.ticks.max(1) as i64).min(Fx::ONE);
            self.state.units.build_progress[row] = bp.build_time * t;
            let full = self.unit_max_health(row);
            let floor = full / 10;
            let want = floor + (full - floor) * t;
            if self.state.units.health[row] < want {
                self.state.units.health[row] = want.min(full);
            }
            if t >= Fx::ONE {
                self.complete_unit(row)?;
                done.push(p);
            } else {
                keep.push(p);
            }
        }
        self.state.survival.as_mut().unwrap().printing = keep;
        // A formation finishes together; it goes on as one.
        let mut formations: Vec<u16> = done.iter().map(|p| p.formation).collect();
        formations.dedup();
        for f in formations {
            let group: Vec<&Print> = done.iter().filter(|p| p.formation == f).collect();
            let units: Vec<UnitId> = group.iter().map(|p| p.unit).collect();
            if group[0].front == NODE_PRINT {
                self.survival_send(&units, None, true)?;
            } else {
                self.survival_muster(&units, group[0].front, f)?;
            }
        }
        Ok(())
    }

    /// Sends a printed formation to wait, in its block, near the head of its front.
    fn survival_muster(
        &mut self,
        units: &[UnitId],
        front: u8,
        formation: u16,
    ) -> Result<(), SimError> {
        let s = self.state.survival.as_ref().unwrap();
        let Some(head) = s
            .config
            .fronts
            .get(front as usize)
            .and_then(|f| f.path.first().copied())
        else {
            return self.survival_send(units, None, true);
        };
        let side = s.config.engine_player;
        let mut waiting: Vec<u16> = s
            .mustered
            .iter()
            .filter(|m| m.front == front)
            .map(|m| m.formation)
            .collect();
        waiting.dedup();
        let n = waiting.len() as i32;
        // Spread the formations round the head so the round does not stand on one point.
        let ring = Angle(((n as u32 * 40503) & 0xFFFF) as u16);
        let at = head
            + FxVec2::from_angle(ring) * Fx::from_int(if n == 0 { 0 } else { 60 + 45 * (n % 10) });
        let s = self.state.survival.as_mut().unwrap();
        s.mustered.extend(units.iter().map(|&unit| Muster {
            unit,
            front,
            formation,
        }));
        for chunk in units.chunks(crate::command::MAX_COMMAND_UNITS) {
            self.apply_as(side, &formation_move(chunk, at, false, false))?;
        }
        Ok(())
    }

    /// The round goes: every mustered formation down its front, then at the defenders.
    pub(super) fn survival_launch(&mut self, now: u32) -> Result<(), SimError> {
        let s = self.state.survival.as_mut().unwrap();
        let mustered = std::mem::take(&mut s.mustered);
        let round = s.round;
        let rules = s.config.rules;
        let last = rules.rounds != 0 && round >= rules.rounds;
        s.phase = if last { Phase::Final } else { Phase::Launched };
        s.next_at = now + rules.interval_secs as u32 * HZ;
        let mut formations: Vec<(u8, u16)> =
            mustered.iter().map(|m| (m.front, m.formation)).collect();
        formations.dedup();
        let mut sent = 0u16;
        for (front, f) in formations {
            let units: Vec<UnitId> = mustered
                .iter()
                .filter(|m| m.formation == f && self.state.units.row(m.unit).is_some())
                .map(|m| m.unit)
                .collect();
            if units.is_empty() {
                continue;
            }
            sent = sent.saturating_add(units.len() as u16);
            self.survival_send(&units, Some(front as usize), true)?;
        }
        let s = self.state.survival.as_mut().unwrap();
        s.waves.extend(mustered.iter().map(|m| m.unit));
        if !last {
            self.survival_forecast(round + 1);
        } else {
            let s = self.state.survival.as_mut().unwrap();
            s.incoming = [[0; 5]; 3];
            s.incoming_heavy.clear();
        }
        self.events
            .push(SimEvent::RoundLaunched { round, units: sent });
        Ok(())
    }

    /// Attack-moves `units` down front `front` (if any), then at the
    /// defenders: as one block when `together`, else each its own way. Ships
    /// can reach neither a commander ashore nor a front of another domain, so
    /// they go down the nearest sea front when they have none, and on to the
    /// water by the nearest defender they can reach.
    pub(super) fn survival_send(
        &mut self,
        units: &[UnitId],
        front: Option<usize>,
        together: bool,
    ) -> Result<(), SimError> {
        let naval = |w: &Self, u: &UnitId| {
            w.state.units.row(*u).is_some_and(|r| {
                w.blueprints
                    .unit(w.state.units.blueprint[r])
                    .motion
                    .is_some_and(|m| domain_of(m.layer) == Domain::Naval)
            })
        };
        let (ships, rest): (Vec<UnitId>, Vec<UnitId>) =
            units.iter().copied().partition(|u| naval(self, u));
        let s = self.state.survival.as_ref().unwrap();
        let side = s.config.engine_player;
        let path_of = |f: usize| -> Vec<FxVec2> {
            s.config
                .fronts
                .get(f)
                .map_or(Vec::new(), |f| f.path.iter().skip(1).copied().collect())
        };
        let mut legs: Vec<(Vec<UnitId>, Vec<FxVec2>)> = Vec::new();
        if !rest.is_empty() {
            let mut path = front.map_or(Vec::new(), path_of);
            path.push(self.survival_target());
            legs.push((rest, path));
        }
        if !ships.is_empty() {
            let from = self.survival_centre(&ships);
            let mut path = match front {
                Some(f) => path_of(f),
                None => self.nearest_sea_front(from),
            };
            let size = ships
                .iter()
                .filter_map(|u| self.state.units.row(*u))
                .filter_map(|r| self.blueprints.unit(self.state.units.blueprint[r]).motion)
                .map(|m| m.size_class)
                .max()
                .unwrap_or(0);
            let end = path.last().copied().unwrap_or(from);
            if let Some(goal) = self.survival_sea_goal(end, size) {
                path.push(goal);
            }
            legs.push((ships, path));
        }
        for (group, path) in legs {
            for chunk in group.chunks(crate::command::MAX_COMMAND_UNITS) {
                let mut queue = false;
                for &p in &path {
                    let command = if together {
                        formation_move(chunk, p, queue, true)
                    } else {
                        Command::AttackMove {
                            units: chunk.to_vec(),
                            target: p,
                            queue,
                        }
                    };
                    self.apply_as(side, &command)?;
                    queue = true;
                }
            }
        }
        Ok(())
    }

    /// Where `units` stand, on average.
    fn survival_centre(&self, units: &[UnitId]) -> FxVec2 {
        let at: Vec<FxVec2> = units
            .iter()
            .filter_map(|u| self.state.units.row(*u).map(|r| self.state.units.pos[r]))
            .collect();
        let n = at.len().max(1) as i32;
        let sum = at.iter().fold(FxVec2::ZERO, |a, b| a + *b);
        FxVec2::new(sum.x / n, sum.y / n)
    }

    /// The rest of the sea front nearest `from`: its points from the one nearest on.
    fn nearest_sea_front(&self, from: FxVec2) -> Vec<FxVec2> {
        let s = self.state.survival.as_ref().unwrap();
        let best = s
            .config
            .fronts
            .iter()
            .filter(|f| f.domain == Domain::Naval)
            .flat_map(|f| {
                f.path
                    .iter()
                    .enumerate()
                    .map(move |(i, p)| (p.distance_sq(from), i, f))
            })
            .min_by_key(|(d, i, _)| (*d, *i));
        best.map_or(Vec::new(), |(_, i, f)| f.path[i..].to_vec())
    }

    /// Water by the defender nearest `from` that ships can reach: the open
    /// water within a gun's reach of the nearest of their units and buildings
    /// that has any. None when none has water near it.
    fn survival_sea_goal(&self, from: FxVec2, size_class: u8) -> Option<FxVec2> {
        let s = self.state.survival.as_ref().unwrap();
        let side = s.config.engine_player;
        let u = &self.state.units;
        let mut near: Vec<(Fx, FxVec2)> = u
            .slots
            .iter()
            .filter(|&r| u.owner[r] != side && u.is_active(r))
            .map(|r| (u.pos[r].distance_sq(from), u.pos[r]))
            .collect();
        near.sort_by_key(|(d, _)| *d);
        // The nearest few: a ship that cannot reach the closest goes for the next.
        // A bound on the search, not on what ships do.
        near.iter().take(48).find_map(|(_, at)| {
            self.nav
                .nearest_passable(mc_data::MoveLayer::Naval, size_class, *at)
                .filter(|w| w.distance(*at) < Fx::from_int(SEA_GOAL_REACH))
        })
    }
}

/// A block move at standard spacing, kept together at the pace of its slowest unit.
fn formation_move(units: &[UnitId], target: FxVec2, queue: bool, attack_move: bool) -> Command {
    Command::FormationMove {
        units: units.to_vec(),
        target,
        queue,
        attack_move,
        facing: None,
        together: true,
        spacing: 1,
        shape: 0,
    }
}

/// Units per domain and tech of a plan.
pub(super) fn tally(bps: &mc_data::Blueprints, plan: &[Formation]) -> [[u16; 5]; 3] {
    let mut out = [[0u16; 5]; 3];
    for b in plan.iter().flat_map(|f| f.units.iter()) {
        let u = bps.unit(*b);
        let d = u.motion.map_or(Domain::Land, |m| domain_of(m.layer));
        let t = (u.tech.clamp(1, 5) - 1) as usize;
        let c = &mut out[super::domain_index(d)][t];
        *c = c.saturating_add(1);
    }
    out
}

/// The heavy wave of a plan: each squad's kind and size, in the plan's order.
pub(super) fn heavy_squads(plan: &[Formation]) -> Vec<(BlueprintId, u16)> {
    plan.iter()
        .filter(|f| f.heavy)
        .filter_map(|f| {
            Some((
                *f.units.first()?,
                f.units.len().min(u16::MAX as usize) as u16,
            ))
        })
        .collect()
}
