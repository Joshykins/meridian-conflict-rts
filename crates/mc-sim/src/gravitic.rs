//! The gravitic engine (`UnitBlueprint::gravitic`, the Regency's "The Fallen"): a
//! structure that tears a rift through the ground to a point anywhere on the map and
//! holds a gravity well where it strikes.
//!
//! - **Aimed** (`Command::Fracture`): the engine gathers for `Gravitic::gather_ticks`
//!   (`RiftGathering`), paused while its side cannot pay for it. Aimed again while it
//!   gathers, it only turns to the new point. Aimed while a rift runs or a well holds, the
//!   well is let go (`WellReleased`) and it gathers anew. A stop order lets it go too.
//! - **The rift** (`RiftLaunched`): it leaves the ground under the engine's core and runs
//!   straight to the point at `Gravitic::step` a tick, splitting the ground: the trees on
//!   its line are torn up and the ground is scarred. It harms nothing on its way.
//! - **The strike** (`RiftStruck`): the ground round the point shatters out to
//!   `Gravitic::radius`, and the well holds from then on. It strikes no blow of its own.
//! - **The well** holds for as long as the engine stands and its side pays for it: each
//!   tick the enemy's units in it take `Gravitic::damage` a second, the same anywhere in
//!   it, and keep only `Gravitic::slow` of their speed (`well_slow`); an aircraft takes
//!   `Gravitic::air_damage` of the crush and is slowed in full. Spacecraft are out of its
//!   reach.
//! - **Shields** counter it: a live dome reaching into the well takes the crush as a unit
//!   does, and what stands under it is spared while it holds. A hull field takes its unit's share.
//!
//! Friends are never touched. The engine dies, and its well collapses (`WellReleased`).

use mc_core::{Fx, FxVec2, StateHasher, TICKS_PER_SECOND};
use mc_data::gravitic::Gravitic;
use serde::{Deserialize, Serialize};

use crate::spatial::kind;
use crate::tables::UnitId;
use crate::well_area::{Aircraft, Sides, WellArea};
use crate::{SimError, SimEvent, World};

const DT: i32 = TICKS_PER_SECOND as i32;
/// Metres either side of the rift's front where it tears the trees up.
const RIFT_WIDTH: i32 = 16;

/// Where an engine's work is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WellPhase {
    /// Gathering where it stands: `left` ticks to go.
    Gathering { left: u16 },
    /// The rift running: `run` metres out of `length`.
    Running { run: Fx, length: Fx },
    /// The well holding, `age` ticks since it struck.
    Holding { age: u32 },
}

/// One engine's rift and well: at most one an engine.
#[derive(Clone, Serialize, Deserialize)]
pub struct GravityWell {
    pub source: UnitId,
    pub owner: u8,
    /// Where the rift leaves: the ground under the engine's core.
    pub from: FxVec2,
    /// The point, and the ground there (the sea's surface over water).
    pub mark: FxVec2,
    pub z: Fx,
    pub phase: WellPhase,
    /// Its side could pay for it this tick: a faltering well neither crushes nor slows.
    pub powered: bool,
}

impl GravityWell {
    pub(crate) fn hash(&self, h: &mut StateHasher) {
        for v in [self.from.x, self.from.y, self.mark.x, self.mark.y, self.z] {
            h.write_i64(v.0);
        }
        let (kind, a, b) = match self.phase {
            WellPhase::Gathering { left } => (0u64, left as i64, 0),
            WellPhase::Running { run, length } => (1, run.0, length.0),
            WellPhase::Holding { age } => (2, age as i64, 0),
        };
        h.write_u64(
            kind | (self.owner as u64) << 8
                | (self.powered as u64) << 16
                | (self.source.0 as u64) << 32,
        );
        h.write_i64(a);
        h.write_i64(b);
    }

    /// Where the rift's front is now: `from` while gathering, `mark` once it has struck.
    pub fn front(&self) -> FxVec2 {
        match self.phase {
            WellPhase::Gathering { .. } => self.from,
            WellPhase::Running { run, length } => {
                self.from + (self.mark - self.from).normalize() * run.min(length)
            }
            WellPhase::Holding { .. } => self.mark,
        }
    }
}

pub(crate) fn hash_wells(wells: &[GravityWell], h: &mut StateHasher) {
    h.write_u64(wells.len() as u64);
    for w in wells {
        w.hash(h);
    }
}

impl World {
    /// The engine in `row`'s gravitic spec, if it has one.
    fn gravitic(&self, row: usize) -> Option<&Gravitic> {
        self.bp(row).gravitic.as_ref()
    }

    /// The ground under the engine's core: where its rift leaves.
    fn rift_foot(&self, row: usize, g: &Gravitic) -> FxVec2 {
        let units = &self.state.units;
        let fwd = FxVec2::from_angle(units.heading[row]);
        units.pos[row] + fwd * g.core.x + fwd.perp() * g.core.y
    }

    /// `Command::Fracture`: each engine among `ids` turns its rift on `pos`.
    pub(crate) fn order_fracture(&mut self, player: u8, ids: &[UnitId], pos: FxVec2) {
        let pos = self.clamp_to_map(pos);
        let rows: Vec<usize> = self
            .owned(player, ids, 0)
            .into_iter()
            .filter(|&r| self.gravitic(r).is_some() && self.state.units.is_active(r))
            .collect();
        for row in rows {
            self.aim_well(row, pos);
        }
    }

    fn aim_well(&mut self, row: usize, mark: FxVec2) {
        let Some(g) = self.gravitic(row).cloned() else {
            return;
        };
        let id = self.state.units.id(row);
        let z = self.terrain.height_at(mark).max(self.terrain.water_level());
        let from = self.rift_foot(row, &g);
        let core = from.extend(self.state.units.z[row] + g.core.z);
        let at = self.state.wells.iter().position(|w| w.source == id);
        let left = match at.map(|i| self.state.wells[i].phase) {
            // Still gathering: it keeps what it has gathered and only turns.
            Some(WellPhase::Gathering { left }) => left,
            Some(_) => {
                self.release_well(row);
                g.gather_ticks
            }
            None => g.gather_ticks,
        };
        let well = GravityWell {
            source: id,
            owner: self.state.units.owner[row],
            from,
            mark,
            z,
            phase: WellPhase::Gathering { left },
            powered: true,
        };
        match self.state.wells.iter().position(|w| w.source == id) {
            Some(i) => self.state.wells[i] = well,
            None => self.state.wells.push(well),
        }
        self.events.push(SimEvent::RiftGathering {
            unit: id,
            core,
            mark,
            ticks: left,
            blueprint: self.state.units.blueprint[row],
        });
    }

    /// Lets go of whatever the engine in `row` is doing: a well collapses, a rift dies
    /// where it is, a gathering stops.
    pub(crate) fn release_well(&mut self, row: usize) {
        let id = self.state.units.id(row);
        let Some(i) = self.state.wells.iter().position(|w| w.source == id) else {
            return;
        };
        let w = self.state.wells.remove(i);
        let radius = self.gravitic(row).map_or(Fx::ZERO, |g| g.radius);
        self.events.push(SimEvent::WellReleased {
            unit: id,
            pos: w.front().extend(w.z),
            radius: if matches!(w.phase, WellPhase::Holding { .. }) {
                radius
            } else {
                Fx::ZERO
            },
        });
    }

    /// What is left of `speed` for the unit in `row` at `pos`: the slowest share of every
    /// enemy well holding over it.
    pub(crate) fn well_slow(&self, row: usize, pos: FxVec2, speed: Fx) -> Fx {
        if self.state.wells.is_empty() {
            return speed;
        }
        let mut keep = Fx::ONE;
        for w in &self.state.wells {
            if !w.powered || !matches!(w.phase, WellPhase::Holding { .. }) {
                continue;
            }
            let Some(g) = self
                .state
                .units
                .row(w.source)
                .and_then(|r| self.gravitic(r))
            else {
                continue;
            };
            let area = WellArea {
                mark: w.mark,
                z: w.z,
                radius: g.radius,
                owner: w.owner,
                source: w.source,
                per_tick: Fx::ZERO,
                aircraft: Aircraft::Crushed(g.air_damage),
                edge: Fx::ONE,
                sides: Sides::Enemies,
            };
            if self.well_catches(&area, row, pos) {
                keep = keep.min(g.slow);
            }
        }
        speed * keep
    }

    /// Energy a second the engine in `row` draws for its work this tick.
    pub(crate) fn gravitic_upkeep(&self, row: usize) -> Fx {
        let Some(g) = self.gravitic(row) else {
            return Fx::ZERO;
        };
        let id = self.state.units.id(row);
        if self.state.wells.iter().any(|w| w.source == id) {
            g.upkeep
        } else {
            Fx::ZERO
        }
    }

    /// This tick's gravitic engines: gathering, rifts running, wells holding.
    pub(crate) fn run_gravitic(&mut self) -> Result<(), SimError> {
        if self.state.wells.is_empty() {
            return Ok(());
        }
        let wells = std::mem::take(&mut self.state.wells);
        let mut kept = Vec::with_capacity(wells.len());
        for mut w in wells {
            let Some(row) = self.state.units.row(w.source) else {
                self.events.push(SimEvent::WellReleased {
                    unit: w.source,
                    pos: w.front().extend(w.z),
                    radius: Fx::ZERO,
                });
                continue;
            };
            let Some(g) = self.gravitic(row).cloned() else {
                continue;
            };
            w.powered =
                !self.shields_unpowered(w.owner) && !self.powered_down(row) && !self.stunned(row);
            match w.phase {
                WellPhase::Gathering { left } => {
                    let left = if w.powered {
                        left.saturating_sub(1)
                    } else {
                        left
                    };
                    w.phase = if left == 0 {
                        let length = w.from.distance(w.mark);
                        self.events.push(SimEvent::RiftLaunched {
                            unit: w.source,
                            from: w.from.extend(self.terrain.height_at(w.from)),
                            mark: w.mark.extend(w.z),
                            ticks: (length / g.step.max(Fx::EPSILON))
                                .ceil_int()
                                .clamp(0, u16::MAX as i32)
                                as u16,
                        });
                        WellPhase::Running {
                            run: Fx::ZERO,
                            length,
                        }
                    } else {
                        WellPhase::Gathering { left }
                    };
                }
                WellPhase::Running { run, length } => {
                    let run = run + g.step;
                    w.phase = WellPhase::Running { run, length };
                    self.tear_rift(&w);
                    if run >= length {
                        w.phase = WellPhase::Holding { age: 0 };
                        self.events.push(SimEvent::RiftStruck {
                            unit: w.source,
                            pos: w.mark.extend(w.z),
                            radius: g.radius,
                        });
                        if w.powered {
                            self.crush(&w, &g);
                        }
                        w.phase = WellPhase::Holding { age: 1 };
                    }
                }
                WellPhase::Holding { age } => {
                    if w.powered {
                        self.crush(&w, &g);
                    }
                    w.phase = WellPhase::Holding {
                        age: age.saturating_add(1),
                    };
                }
            }
            kept.push(w);
        }
        kept.append(&mut self.state.wells);
        self.state.wells = kept;
        Ok(())
    }

    /// The rift's front this tick: the trees on its line torn up, the ground split.
    fn tear_rift(&mut self, w: &GravityWell) {
        let front = w.front();
        let width = Fx::from_int(RIFT_WIDTH);
        let mut trees = Vec::new();
        self.prop_index.query(front, width * 4, kind::PROP, |e| {
            let prop = e.row as usize;
            if e.pos.distance(front) <= width * 4
                && self.is_scenery_tree(prop)
                && self.is_prop_alive(prop)
            {
                trees.push(prop);
            }
            true
        });
        for prop in trees {
            self.state.props_dead[prop / 64] |= 1 << (prop % 64);
            self.events.push(SimEvent::TreeVaporized {
                prop: prop as u32,
                center: front,
            });
        }
        self.add_stain(front, width * 2, 90);
    }

    /// One tick's crush over the well, the same everywhere in it (`well_area.rs`), then
    /// the ground going on breaking under it.
    fn crush(&mut self, w: &GravityWell, g: &Gravitic) {
        let (mark, radius) = (w.mark, g.radius);
        self.crush_area(&WellArea {
            mark,
            z: w.z,
            radius,
            owner: w.owner,
            source: w.source,
            per_tick: g.damage / DT,
            aircraft: Aircraft::Crushed(g.air_damage),
            edge: Fx::ONE,
            sides: Sides::Enemies,
        });
        let age = match w.phase {
            WellPhase::Holding { age } => age,
            _ => 0,
        };
        // The ground goes on breaking under it: the trees, and the scar.
        if age % 5 == 0 {
            let mut trees = Vec::new();
            self.prop_index.query(mark, radius, kind::PROP, |e| {
                let prop = e.row as usize;
                if e.pos.distance(mark) <= radius
                    && self.is_scenery_tree(prop)
                    && self.is_prop_alive(prop)
                {
                    trees.push(prop);
                }
                true
            });
            for prop in trees {
                self.state.props_dead[prop / 64] |= 1 << (prop % 64);
                self.events.push(SimEvent::TreeVaporized {
                    prop: prop as u32,
                    center: mark,
                });
            }
        }
        if age % 20 == 0 {
            self.add_stain(mark, radius, 140);
        }
    }
}
