//! Space elevators (`mc_data::elevator`, `docs/ELEVATOR.md`).
//!
//! A finished elevator first has its cable let down out of orbit and clamped
//! (`Elevator::tether_ticks`). From then on its clock runs in cycles of
//! `period_ticks`: three cargo crates fall down the cable, land in the well, open, and
//! sink into the ground when they are empty. The tick they touch down they let off the
//! front of the elevator's queue (its `OrderKind::Produce` orders, queued like a
//! factory's) as finished units, free, seated in the crates (`seat_units`) as long as
//! the next one fits in what is left of the `payload`; the rest of the payload goes into
//! its side's store as materials. A crate's units drive out of its doors row by row
//! (`state.rollouts`, like a factory's) and, once they are all out, leave as one group on
//! the elevator's standing orders or rally (`Squad`), unless the elevator is batching.
//! Never up: the crates only ever come down.

use std::collections::BTreeMap;

use mc_core::elevator_bay as bay;
use mc_core::{Angle, Fx, FxVec2, StateHasher};
use serde::{Deserialize, Serialize};

use crate::mirror::SimEvent;
use crate::orders::Rollout;
use crate::tables::{flag, OrderKind, UnitId};
use crate::world::World;
use crate::{Command, SimError};

/// The bays' bearings off the elevator's heading, in degrees: the ways out between the
/// clamp towers, which stand at 0, 120 and 240 (`gpu_consts::elevator`).
const EXITS: [i32; bay::COUNT] = [60, 180, 300];

/// Ticks the crates' doors and roofs take to open once they have landed.
pub const OPEN_TICKS: u32 = 40;
/// Ticks before the first row drives out (the doors are all but open), and between rows.
const FIRST_ROW_TICKS: u32 = 35;
const ROW_TICKS: u32 = 10;

/// A seat in a crate: which bay, which row (0 nearest the doors), and where its unit's
/// centre stands in the bay's frame, `along` the bearing from the cable's axis and
/// `across` it (left positive), in metres.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seat {
    pub bay: u8,
    pub row: u16,
    pub along: Fx,
    pub across: Fx,
}

/// Each elevator's clock, by unit: ticks since it was finished; and the groups of units
/// still driving out of its crates.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Elevators {
    pub clocks: BTreeMap<UnitId, u32>,
    pub squads: Vec<Squad>,
}

/// The units of one crate, who leave together once every one of them is out of it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Squad {
    pub elevator: UnitId,
    pub bay: u8,
    pub units: Vec<UnitId>,
}

impl Elevators {
    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.clocks.len() as u64);
        for (id, age) in &self.clocks {
            h.write_u64(id.0 as u64 | (*age as u64) << 32);
        }
        h.write_u64(self.squads.len() as u64);
        for q in &self.squads {
            h.write_u64(q.elevator.0 as u64 | (q.bay as u64) << 32);
            h.write_u64(q.units.len() as u64);
            for u in &q.units {
                h.write_u64(u.0 as u64);
            }
        }
    }
}

/// Where an elevator is in its life, from its clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The cable coming down, `ticks` in.
    Tethering { ticks: u32 },
    /// Running: `ticks` into the current cycle (descent, dock, ascent, away).
    Cycle { ticks: u32 },
}

pub fn phase(e: &mc_data::elevator::Elevator, age: u32) -> Phase {
    if age < e.tether_ticks {
        Phase::Tethering { ticks: age }
    } else {
        Phase::Cycle {
            ticks: (age - e.tether_ticks) % e.period_ticks,
        }
    }
}

/// Share of the tether spent letting the cable down; the clamps close in the rest.
const CABLE_SHARE: (u32, u32) = (3, 4);

/// How an elevator looks at `age` ticks, as the mirror publishes it: how far the cable
/// has come down (0 in orbit, 1 seated), how hard the clamps grip it (0 open, 1 shut),
/// and the crates' height (1 high in orbit, falling to 0 landed; below 0 their doors and
/// roofs opening, to `-DOORS_OPEN`, and then sinking into the ground, to -1; 2 away and
/// not drawn). `None`: not finished yet, so nothing is lowered and no crate falls.
#[expect(
    clippy::float_arithmetic,
    clippy::disallowed_types,
    reason = "presentation: the mirror publishes it in UnitInstance::mount and deploy for the renderer and audio"
)]
pub fn pose(e: &mc_data::elevator::Elevator, age: Option<u32>) -> [f32; 3] {
    let Some(age) = age else {
        return [0.0, 0.0, 2.0];
    };
    let ease = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    match phase(e, age) {
        Phase::Tethering { ticks } => {
            let lower = e.tether_ticks * CABLE_SHARE.0 / CABLE_SHARE.1;
            let t = ticks as f32;
            // The cable falls fast out of the sky and is eased in over its last stretch.
            let fall = t / lower as f32;
            let drop = 1.0 - (1.0 - fall.clamp(0.0, 1.0)).powi(2);
            let grip = ease((t - lower as f32) / (e.tether_ticks - lower) as f32);
            [drop, grip, 2.0]
        }
        Phase::Cycle { ticks } => {
            let t = ticks as f32;
            let (down, dock) = (e.descent_ticks as f32, e.dock_ticks as f32);
            let open = bay::DOORS_OPEN;
            let car = if t < down {
                // Comes down fast and brakes hard over the last third.
                let x = t / down;
                (1.0 - x).powi(3)
            } else if t < down + dock {
                // Landed: the doors and roofs open.
                -open * ease((t - down) / OPEN_TICKS as f32)
            } else if t < down + dock + e.sink_ticks as f32 {
                // Empty, they sink into the ground.
                let x = (t - down - dock) / e.sink_ticks as f32;
                -open - (1.0 - open) * ease(x)
            } else {
                2.0
            };
            [1.0, 1.0, car]
        }
    }
}

impl World {
    /// The elevator spec of a finished unit, if it is one.
    pub(crate) fn elevator_spec(&self, row: usize) -> Option<mc_data::elevator::Elevator> {
        if self.state.units.has_flag(row, flag::UNDER_CONSTRUCTION) {
            return None;
        }
        self.bp(row).elevator
    }

    /// An elevator's clock, `None` until it is finished.
    pub(crate) fn elevator_age(&self, row: usize) -> Option<u32> {
        self.elevator_spec(row)?;
        self.state
            .elevators
            .clocks
            .get(&self.state.units.id(row))
            .copied()
    }

    /// Runs every finished elevator's clock: new ones start their tether, and crates
    /// touching down let their cargo off.
    pub(crate) fn run_elevators(&mut self) -> Result<(), SimError> {
        let units = &self.state.units;
        self.state
            .elevators
            .clocks
            .retain(|id, _| units.row(*id).is_some());
        self.state
            .elevators
            .squads
            .retain(|q| units.row(q.elevator).is_some());
        let rows: Vec<usize> = self
            .state
            .units
            .slots
            .iter()
            .filter(|&r| self.bp(r).elevator.is_some() && self.state.units.is_active(r))
            .collect();
        for row in rows {
            let Some(e) = self.elevator_spec(row) else {
                continue;
            };
            let id = self.state.units.id(row);
            let owner = self.state.units.owner[row];
            let pos = self.state.units.pos[row].extend(self.state.units.z[row]);
            let age = match self.state.elevators.clocks.get_mut(&id) {
                Some(age) => {
                    *age = age.saturating_add(1);
                    *age
                }
                None => {
                    self.state.elevators.clocks.insert(id, 0);
                    self.events.push(SimEvent::ElevatorTethering {
                        unit: id,
                        pos,
                        owner,
                    });
                    continue;
                }
            };
            match phase(&e, age) {
                Phase::Cycle { ticks: 0 } => {
                    if age == e.tether_ticks {
                        self.events.push(SimEvent::ElevatorTethered {
                            unit: id,
                            pos,
                            owner,
                        });
                    }
                    self.events.push(SimEvent::ElevatorInbound {
                        unit: id,
                        pos,
                        owner,
                    });
                }
                Phase::Cycle { ticks } if ticks == e.touchdown() => {
                    let (units, materials) = self.unload_crates(row, &e)?;
                    self.events.push(SimEvent::ElevatorDocked {
                        unit: id,
                        pos,
                        owner,
                        units,
                        materials,
                    });
                }
                Phase::Cycle { ticks } if ticks == e.touchdown() + e.dock_ticks => {
                    // The crates start to sink: whatever is still in them is out.
                    self.empty_crates(id);
                    self.events.push(SimEvent::ElevatorSinking {
                        unit: id,
                        pos,
                        owner,
                    });
                }
                _ => {}
            }
        }
        self.release_squads()
    }

    /// Lets off the front of the elevator's queue, as much as fits in the payload and
    /// the crates' seats, and pays the rest of the payload into its side's store.
    /// Returns how many units came off and the materials paid in.
    fn unload_crates(
        &mut self,
        row: usize,
        e: &mc_data::elevator::Elevator,
    ) -> Result<(u16, Fx), SimError> {
        // Paused, its queue is held: the crates bring the whole payload down as materials.
        let queue: Vec<_> = if self.work_paused(row) {
            Vec::new()
        } else {
            let o = self.state.orders.iter(&self.state.units, row);
            o.take_while(|o| o.kind == OrderKind::Produce)
                .copied()
                .collect()
        };
        let seats = seat_units(
            queue.iter().map(|o| {
                let bp = self.blueprints.unit(o.blueprint);
                (bp.radius, bp.cost_materials)
            }),
            e.payload,
        );
        let owner = self.state.units.owner[row];
        let id = self.state.units.id(row);
        let centre = self.state.units.pos[row];
        let heading = self.state.units.heading[row];
        let repeat = self.state.units.has_flag(row, flag::REPEAT);
        // Batching overrides the crates' own groups: each unit joins the batch's muster.
        let batching = self.batch_of(id).is_some();
        let mut squads: Vec<Squad> = Vec::new();
        let mut left = e.payload;
        for (o, seat) in queue.iter().zip(&seats) {
            left -= self.blueprints.unit(o.blueprint).cost_materials;
            self.state.orders.pop_front(&mut self.state.units, row);
            if repeat {
                self.state
                    .orders
                    .push_back(&mut self.state.units, row, *o)?;
            }
            let bearing = heading + Angle::from_degrees(EXITS[seat.bay as usize]);
            let (dir, side) = bay_frame(bearing);
            let at = centre + dir * seat.along + side * seat.across;
            let t = self.spawn_unit(o.blueprint, owner, at, heading, true)?;
            self.state.players[owner as usize].units_built += 1;
            self.events.push(SimEvent::UnitCompleted {
                unit: self.state.units.id(t),
                owner,
            });
            let unit = self.state.units.id(t);
            self.seat_in_crate(row, t, seat, bearing, !batching)?;
            match squads.iter_mut().find(|q| q.bay == seat.bay) {
                Some(q) => q.units.push(unit),
                None => squads.push(Squad {
                    elevator: id,
                    bay: seat.bay,
                    units: vec![unit],
                }),
            }
        }
        if !batching {
            self.state.elevators.squads.extend(squads);
        }
        if left > Fx::ZERO {
            self.state.players[owner as usize].materials += left;
            self.flow(row).made[0] += left;
        }
        Ok((seats.len() as u16, left))
    }

    /// Holds a unit just set down in its crate until its row's turn, then has it drive
    /// out of the doors (an aircraft lifts out through the open roof, where it stands).
    fn seat_in_crate(
        &mut self,
        row: usize,
        t: usize,
        seat: &Seat,
        bearing: Angle,
        held: bool,
    ) -> Result<(), SimError> {
        let centre = self.state.units.pos[row];
        let at = self.state.units.pos[t];
        let motion = self.bp(t).motion;
        let delay = FIRST_ROW_TICKS + ROW_TICKS * seat.row as u32;
        let want = if motion.is_some_and(|m| m.layer == mc_data::MoveLayer::Air) {
            at
        } else {
            // Out of the doors along its own lane, so a row fans out through them.
            let (dir, side) = bay_frame(bearing);
            let lot = Fx::from_int(self.bp(row).footprint.0 as i32 * mc_map::BUILD_CELL_M / 2);
            let reach = lot + self.bp(t).radius + Fx::from_int(12);
            centre + dir * reach + side * seat.across
        };
        let exit = match motion {
            Some(m) => self
                .nav
                .nearest_passable(m.layer, m.size_class, want)
                .filter(|&p| self.terrain.in_bounds(p))
                .unwrap_or(want),
            None => want,
        };
        self.state.units.flags[t] |= flag::IN_FACTORY;
        self.state.rollouts.insert(
            self.state.units.id(t),
            Rollout {
                factory: self.state.units.id(row),
                exit,
                delay: u16::try_from(delay).unwrap_or(u16::MAX),
                held,
            },
        );
        Ok(())
    }

    /// The crates of elevator `id` start to sink: units still rolling out of them are out.
    fn empty_crates(&mut self, id: UnitId) {
        let stuck: Vec<UnitId> = self
            .state
            .elevators
            .squads
            .iter()
            .filter(|q| q.elevator == id)
            .flat_map(|q| q.units.iter().copied())
            .collect();
        for unit in stuck {
            let Some(r) = self.state.rollouts.remove(&unit) else {
                continue;
            };
            let Some(t) = self.state.units.row(unit) else {
                continue;
            };
            let z = self.ground_z(t, r.exit);
            let units = &mut self.state.units;
            units.pos[t] = r.exit;
            units.z[t] = z;
            units.speed[t] = Fx::ZERO;
            units.flags[t] &= !flag::IN_FACTORY;
        }
    }

    /// Sends off every crate's group whose units are all out of it: one group, so one
    /// formation, on the elevator's standing orders, else to its rally point, else a way
    /// out along the crate's bearing.
    fn release_squads(&mut self) -> Result<(), SimError> {
        let rollouts = &self.state.rollouts;
        let (ready, waiting): (Vec<Squad>, Vec<Squad>) =
            std::mem::take(&mut self.state.elevators.squads)
                .into_iter()
                .partition(|q| q.units.iter().all(|u| !rollouts.contains_key(u)));
        self.state.elevators.squads = waiting;
        for q in ready {
            let Some(f) = self.state.units.row(q.elevator) else {
                continue;
            };
            let owner = self.state.units.owner[f];
            let ids: Vec<UnitId> = q
                .units
                .iter()
                .copied()
                .filter(|&u| self.state.units.row(u).is_some())
                .collect();
            if ids.is_empty() {
                continue;
            }
            for &u in &ids {
                if let Some(t) = self.state.units.row(u) {
                    let units = &mut self.state.units;
                    units.fire_state[t] = units.fire_state[f];
                }
            }
            if self.give_standing(f, &ids, true)? {
                continue;
            }
            let (rally, pos) = (self.state.units.rally[f], self.state.units.pos[f]);
            let target = if rally != pos {
                rally
            } else {
                let bearing =
                    self.state.units.heading[f] + Angle::from_degrees(EXITS[q.bay as usize]);
                let (dir, _) = bay_frame(bearing);
                self.clamp_to_map(pos + dir * Fx::from_int(bay::FROM + bay::LENGTH + 90))
            };
            self.apply_as(
                owner,
                &Command::Move {
                    units: ids,
                    target,
                    queue: false,
                },
            )?;
        }
        Ok(())
    }
}

/// A bay's frame off its bearing: out along it, and to its left.
fn bay_frame(bearing: Angle) -> (FxVec2, FxVec2) {
    let dir = FxVec2::from_angle(bearing);
    (dir, FxVec2::new(-dir.y, dir.x))
}

/// Seats units in the crates, in queue order, and returns a seat for each that rides:
/// `queue` is each unit's radius and materials. Units fill the first crate in rows across it
/// from the doors inward, then the next; a unit rides while it fits in what is left of
/// `payload` and some crate has room for it, and the first that does not stops the load,
/// so the queue keeps its order. Shared by the sim and the interface's preview.
pub fn seat_units(queue: impl IntoIterator<Item = (Fx, Fx)>, payload: Fx) -> Vec<Seat> {
    let margin = Fx::from_int(bay::MARGIN);
    let across_room = Fx::from_int(bay::WIDTH) - margin * 2;
    let along_room = Fx::from_int(bay::LENGTH) - margin * 2;
    /// A row of one bay: its units' widths, and how deep it is.
    #[derive(Default)]
    struct Row {
        widths: Vec<Fx>,
        depth: Fx,
    }
    impl Row {
        fn span(&self) -> Fx {
            let gaps = self.widths.len().saturating_sub(1) as i32;
            self.widths.iter().fold(Fx::ZERO, |a, &w| a + w) + Fx::from_int(bay::MARGIN * gaps)
        }
    }
    let mut bays: Vec<Vec<Row>> = vec![Vec::new()];
    // Where each unit stands: its bay, row and place in the row.
    let mut placed: Vec<(usize, usize, usize)> = Vec::new();
    let mut left = payload;
    for (radius, cost) in queue {
        if cost > left {
            break;
        }
        let d = radius * 2;
        let mut seat = None;
        // The current bay, then the next ones; a unit too wide for an empty bay stops it.
        while seat.is_none() {
            let last = bays.len() - 1;
            let rows = &mut bays[last];
            let fits_row = rows
                .last()
                .is_some_and(|r| r.span() + margin + d <= across_room);
            if fits_row {
                let (b, r) = (bays.len() - 1, bays[bays.len() - 1].len() - 1);
                let row = &mut bays[b][r];
                row.widths.push(d);
                row.depth = row.depth.max(d);
                seat = Some((b, r, row.widths.len() - 1));
                break;
            }
            let used = rows.iter().fold(Fx::ZERO, |a, r| a + r.depth)
                + Fx::from_int(bay::MARGIN * rows.len() as i32);
            if d <= across_room && used + d <= along_room {
                rows.push(Row {
                    widths: vec![d],
                    depth: d,
                });
                seat = Some((last, rows.len() - 1, 0));
            } else if rows.is_empty() || bays.len() >= bay::COUNT {
                break;
            } else {
                bays.push(Vec::new());
            }
        }
        let Some(at) = seat else { break };
        left -= cost;
        placed.push(at);
    }
    // Rows' depths are settled (a later, wider unit deepens its row): lay the seats out.
    let door = Fx::from_int(bay::FROM + bay::LENGTH) - margin;
    placed
        .iter()
        .map(|&(b, r, i)| {
            let rows = &bays[b];
            let before = rows[..r].iter().fold(Fx::ZERO, |a, row| a + row.depth)
                + Fx::from_int(bay::MARGIN * r as i32);
            let row = &rows[r];
            let along = door - before - row.depth / 2;
            let ahead = row.widths[..i].iter().fold(Fx::ZERO, |a, &w| a + w)
                + Fx::from_int(bay::MARGIN * i as i32);
            let across = ahead + row.widths[i] / 2 - row.span() / 2;
            Seat {
                bay: b as u8,
                row: r as u16,
                along,
                across,
            }
        })
        .collect()
}
