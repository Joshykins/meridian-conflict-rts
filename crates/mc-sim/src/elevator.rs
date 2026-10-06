//! Space elevators (`mc_data::elevator`, `docs/ELEVATOR.md`).
//!
//! A finished elevator first has its cable let down out of orbit and clamped
//! (`Elevator::tether_ticks`). From then on its clock runs in cycles of
//! `period_ticks`: the platform comes down the cable, docks, goes back up, and is away.
//! The tick it touches down it lets off the front of the elevator's queue (its
//! `OrderKind::Produce` orders, queued like a factory's) as finished units, free, as
//! long as the next one fits in what is left of the `payload`; the rest of the payload
//! goes into its side's store as mass. Its units drive off the deck between the clamp
//! towers (`state.rollouts`, like a factory's) and take its standing orders or rally.

use std::collections::BTreeMap;

use mc_core::{Angle, Fx, FxVec2, StateHasher};
use serde::{Deserialize, Serialize};

use crate::mirror::SimEvent;
use crate::orders::Rollout;
use crate::tables::{flag, OrderKind, UnitId};
use crate::world::World;
use crate::SimError;

/// Where the units come off: the clamp towers stand at 0, 120 and 240 degrees off the
/// elevator's heading (`gpu_consts::elevator`), so the ways out are between them.
const EXITS: [i32; 3] = [60, 180, 300];

/// Each elevator's clock, by unit: ticks since it was finished.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Elevators {
    pub clocks: BTreeMap<UnitId, u32>,
}

impl Elevators {
    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.clocks.len() as u64);
        for (id, age) in &self.clocks {
            h.write_u64(id.0 as u64 | (*age as u64) << 32);
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

/// Metres round the cable's collar in the deck's middle where nothing is set down.
const COLLAR: Fx = Fx::from_int(30);

/// Share of the tether spent letting the cable down; the clamps close in the rest.
const CABLE_SHARE: (u32, u32) = (3, 4);

/// How an elevator looks at `age` ticks, as the mirror publishes it: how far the cable
/// has come down (0 in orbit, 1 seated), how hard the clamps grip it (0 open, 1 shut),
/// and the platform's height up the cable (0 docked, 1 at the top, over 1 away and not
/// drawn). `None`: not finished yet, so nothing is lowered and no platform runs.
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
            let (down, dock, up) = (
                e.descent_ticks as f32,
                e.dock_ticks as f32,
                e.ascent_ticks as f32,
            );
            let car = if t < down {
                // Comes down fast and brakes hard over the last third.
                let x = t / down;
                (1.0 - x).powi(3)
            } else if t < down + dock {
                0.0
            } else if t < down + dock + up {
                let x = (t - down - dock) / up;
                x * x * x
            } else {
                1.0 + (t - down - dock - up) / up
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

    /// Runs every finished elevator's clock: new ones start their tether, and a platform
    /// touching down lets its cargo off.
    pub(crate) fn run_elevators(&mut self) -> Result<(), SimError> {
        let units = &self.state.units;
        self.state
            .elevators
            .clocks
            .retain(|id, _| units.row(*id).is_some());
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
                    let (units, mass) = self.unload_platform(row, &e)?;
                    self.events.push(SimEvent::ElevatorDocked {
                        unit: id,
                        pos,
                        owner,
                        units,
                        mass,
                    });
                }
                Phase::Cycle { ticks } if ticks == e.touchdown() + e.dock_ticks => {
                    self.events.push(SimEvent::ElevatorDeparting {
                        unit: id,
                        pos,
                        owner,
                    });
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Lets off the front of the elevator's queue, as much as fits in the payload, and
    /// pays the rest of the payload into its side's store. Returns how many units came
    /// off and the mass paid in.
    fn unload_platform(
        &mut self,
        row: usize,
        e: &mc_data::elevator::Elevator,
    ) -> Result<(u16, Fx), SimError> {
        let mut left = e.payload;
        let mut made: u16 = 0;
        // Paused, its queue is held: the platform brings the whole payload down as mass.
        let queued = if self.work_paused(row) {
            0
        } else {
            self.state.orders.iter(&self.state.units, row).count()
        };
        let owner = self.state.units.owner[row];
        let centre = self.state.units.pos[row];
        let heading = self.state.units.heading[row];
        let repeat = self.state.units.has_flag(row, flag::REPEAT);
        // At most one pass over the queue as it stood: a repeated order goes to the back
        // and must not come round again on the same platform.
        for _ in 0..queued {
            let Some(o) = self.state.orders.front(&self.state.units, row).copied() else {
                break;
            };
            if o.kind != OrderKind::Produce {
                break;
            }
            let cost = self.blueprints.unit(o.blueprint).cost_mass;
            if cost > left {
                break;
            }
            left -= cost;
            self.state.orders.pop_front(&mut self.state.units, row);
            if repeat {
                self.state.orders.push_back(&mut self.state.units, row, o)?;
            }
            let at = centre + deck_spot(made, self.blueprints.unit(o.blueprint).radius);
            let t = self.spawn_unit(o.blueprint, owner, at, heading, true)?;
            made = made.saturating_add(1);
            self.state.players[owner as usize].units_built += 1;
            self.events.push(SimEvent::UnitCompleted {
                unit: self.state.units.id(t),
                owner,
            });
            self.let_off(row, t)?;
        }
        if left > Fx::ZERO {
            self.state.players[owner as usize].mass += left;
            self.flow(row).made[0] += left;
        }
        Ok((made, left))
    }

    /// Sends a unit just set down on the deck on its way: an aircraft lifts off and takes
    /// the elevator's orders at once; anything on the ground drives out between the
    /// nearest pair of clamp towers first.
    fn let_off(&mut self, row: usize, t: usize) -> Result<(), SimError> {
        let centre = self.state.units.pos[row];
        let at = self.state.units.pos[t];
        let motion = self.bp(t).motion;
        if motion.is_some_and(|m| m.layer == mc_data::MoveLayer::Air) {
            return self.send_off(row, t, at);
        }
        let heading = self.state.units.heading[row];
        let off = (at - centre).angle() - heading;
        let gap = EXITS
            .iter()
            .map(|&d| Angle::from_degrees(d))
            .min_by_key(|&g| off.delta_to(g).unsigned_abs())
            .unwrap_or(Angle::from_degrees(EXITS[0]));
        let reach = Fx::from_int(self.bp(row).footprint.0 as i32 * mc_map::BUILD_CELL_M / 2)
            + self.bp(t).radius
            + Fx::from_int(12);
        let dir = FxVec2::from_angle(heading + gap);
        // Out along the gap from where it stands, so the column fans through it.
        let side = FxVec2::new(-dir.y, dir.x);
        let want = centre + dir * reach + side * (at - centre).dot(side);
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
            },
        );
        Ok(())
    }
}

/// Where the `n`th unit of a platform is set down, off the deck's middle: in rings out
/// from the cable's collar (`COLLAR`), each seating as many units of `radius` as its
/// round holds. A platform of small units can fill rings past the deck's edge; those
/// stand on the well's apron.
fn deck_spot(n: u16, radius: Fx) -> FxVec2 {
    let pitch = (radius * 2 + Fx::from_int(4)).max(Fx::from_int(8));
    let mut first = 0i32;
    let mut ring = 0i32;
    loop {
        let r = COLLAR + radius + pitch * ring;
        // Seats round a ring of radius r: 2 pi r / pitch, a little under.
        let seats = ((r * 6) / pitch).0 / Fx::ONE.0;
        let seats = (seats as i32).max(1);
        if (n as i32) < first + seats || ring >= 64 {
            let i = n as i32 - first;
            let angle = Angle::from_degrees(i * 360 / seats + ring * 17);
            return FxVec2::from_angle(angle) * r;
        }
        first += seats;
        ring += 1;
    }
}
