//! The Edict (`UnitBlueprint::edict`, the Regent's main gun): a weapon with no barrel.
//!
//! - **Ordered** (`Command::Edict`, `OrderKind::Edict`): the ship closes until the point
//!   on its hull nearest the mark (`edict_point`, on its keel) is within
//!   `Edict::range`, then begins to gather (`EdictGathering`) and the order is done: the
//!   ship goes on to its next order, and may move while it gathers. A mark off the map is
//!   refused (`Refusal::EdictOffMap`), and so is an order given while the Edict is not
//!   ready (`Refusal::EdictNotReady`) unless it is queued, when it waits its turn.
//! - **The gather** runs `Edict::gather_ticks`, drawing `Edict::upkeep` a second, paused
//!   while its side cannot pay (`shields_unpowered`), it is paused or it is stunned. The
//!   firing point follows the hull: it is worked out again every tick, and the shot
//!   leaves from wherever it is when the gather ends. Ordered on a new mark while it
//!   gathers, it keeps what it has gathered and turns on the new mark. A stop, or the
//!   ship's death, lets the gather go (`EdictCancelled`).
//! - **The shot** (`EdictFired`) is a nuclear-scale blast at the mark
//!   (`Edict::blast`, run out by `nukes.rs` without a warhead's look): everything within
//!   its core dies, the rest takes less the farther out, every side's units alike (the
//!   ship that fired it is spared). Then the molten wave rolls out over the ground
//!   (`Edict::wave`): each tick everything it has rolled over takes its burn, full at the
//!   mark and `EdictWave::edge` of it at the rim; a dome takes it for what stands under
//!   it (`well_area.rs`). Never aircraft or spacecraft.
//! - **Silence:** from the gather's start until `Edict::quiet_ticks` after the shot,
//!   every other weapon on the ship is silent (`weapons_silent`, the same gate a stun
//!   shuts).
//! - It may gather again `Edict::reload_ticks` after the shot.

use mc_core::{Fx, FxVec2, FxVec3, StateHasher, TICKS_PER_SECOND};
use mc_data::edict::Edict;
use mc_data::BlueprintId;
use serde::{Deserialize, Serialize};

use crate::mirror::Refusal;
use crate::tables::{Handle, Order, OrderKind, UnitId};
use crate::well_area::{Aircraft, Sides, WellArea};
use crate::{SimError, SimEvent, World};

/// Where a ship's Edict is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdictPhase {
    /// Gathering: `left` ticks to go.
    Gathering { left: u16 },
    /// Fired `age` ticks ago: the wave rolling and burning, then reloading.
    Fired { age: u16 },
}

/// One ship's Edict at work, from its gather until it is ready again: at most one a ship.
#[derive(Clone, Serialize, Deserialize)]
pub struct EdictCharge {
    pub source: UnitId,
    pub owner: u8,
    pub blueprint: BlueprintId,
    /// The mark, and the ground there (the sea's surface over water).
    pub mark: FxVec2,
    pub z: Fx,
    /// The firing point: the hull's keel nearest the mark, this tick (fixed once fired).
    pub from: FxVec3,
    pub phase: EdictPhase,
    /// Its side could pay for it this tick: a stalled gather stands still.
    pub powered: bool,
}

impl EdictCharge {
    fn hash(&self, h: &mut StateHasher) {
        for v in [
            self.mark.x,
            self.mark.y,
            self.z,
            self.from.x,
            self.from.y,
            self.from.z,
        ] {
            h.write_i64(v.0);
        }
        let (kind, n) = match self.phase {
            EdictPhase::Gathering { left } => (0u64, left),
            EdictPhase::Fired { age } => (1, age),
        };
        h.write_u64(
            kind | (self.owner as u64) << 8
                | (self.powered as u64) << 16
                | (n as u64) << 24
                | (self.blueprint.0 as u64) << 40,
        );
        h.write_u64(self.source.0 as u64);
    }
}

/// The ships' Edicts, for the state hash. Nothing is written while there are none, so a
/// match without one hashes as it always has.
pub(crate) fn hash_edicts(edicts: &[EdictCharge], h: &mut StateHasher) {
    if edicts.is_empty() {
        return;
    }
    h.write_u64(edicts.len() as u64);
    for e in edicts {
        e.hash(h);
    }
}

impl World {
    /// The Edict of the unit in `row`, if it carries one.
    pub(crate) fn edict(&self, row: usize) -> Option<&Edict> {
        self.bp(row).edict.as_ref()
    }

    /// Where on the hull of the unit in `row` the Edict fires at `mark` from: the point
    /// of its spine (`body::spine`) nearest the mark, on its keel.
    pub(crate) fn edict_point(&self, row: usize, mark: FxVec2) -> FxVec3 {
        let units = &self.state.units;
        let (a, b, _) = crate::body::spine(self.bp(row), units.pos[row], units.heading[row]);
        let d = b - a;
        let along = if d.length_sq() > Fx::ONE {
            ((mark - a).dot(d) / d.length_sq()).clamp(Fx::ZERO, Fx::ONE)
        } else {
            Fx::ZERO
        };
        (a + d * along).extend(units.z[row])
    }

    /// The Edict's work for the unit in `row`, if any.
    fn edict_charge(&self, row: usize) -> Option<&EdictCharge> {
        let id = self.state.units.id(row);
        self.state.edicts.iter().find(|e| e.source == id)
    }

    /// The unit in `row` holds its fire for its Edict: gathering, or within
    /// `Edict::quiet_ticks` of the shot.
    fn edict_holds(&self, row: usize) -> bool {
        if self.state.edicts.is_empty() {
            return false;
        }
        let quiet = self.edict(row).map_or(0, |e| e.quiet_ticks);
        self.edict_charge(row).is_some_and(|c| match c.phase {
            EdictPhase::Gathering { .. } => true,
            EdictPhase::Fired { age } => age < quiet,
        })
    }

    /// Every weapon of the unit in `row` is silent this tick: it is stunned (`warp.rs`) or
    /// holding its fire for its Edict. The one gate every weapon goes through.
    pub(crate) fn weapons_silent(&self, row: usize) -> bool {
        self.stunned(row) || self.edict_holds(row)
    }

    /// Energy a second the unit in `row` draws for its Edict this tick.
    pub(crate) fn edict_upkeep(&self, row: usize) -> Fx {
        if self.state.edicts.is_empty() {
            return Fx::ZERO;
        }
        match (self.edict(row), self.edict_charge(row)) {
            (Some(e), Some(c)) if matches!(c.phase, EdictPhase::Gathering { .. }) => e.upkeep,
            _ => Fx::ZERO,
        }
    }

    /// The Edict of the unit in `row` may begin a gather: it has none under way and has
    /// reloaded.
    fn edict_ready(&self, row: usize) -> bool {
        self.edict(row).is_some() && self.edict_charge(row).is_none()
    }

    /// `Command::Edict`: each ship among `ids` with an Edict closes on `pos` and fires.
    pub(crate) fn order_edict(
        &mut self,
        player: u8,
        ids: &[UnitId],
        pos: FxVec2,
        queue: bool,
    ) -> Result<(), SimError> {
        if self.clamp_to_map(pos) != pos {
            self.refuse(player, Refusal::EdictOffMap);
            return Ok(());
        }
        let rows: Vec<usize> = self
            .owned(player, ids, 0)
            .into_iter()
            .filter(|&r| self.edict(r).is_some())
            .collect();
        let mut refused = false;
        for row in rows {
            // Turning a gather on a new mark is always allowed; a new shot waits for the
            // reload, and only a queued one may wait for it.
            let gathering = self
                .edict_charge(row)
                .is_some_and(|c| matches!(c.phase, EdictPhase::Gathering { .. }));
            if !queue && !gathering && !self.edict_ready(row) {
                refused = true;
                continue;
            }
            self.give(
                row,
                crate::orders::order(OrderKind::Edict, pos, Handle::NONE),
                queue,
            )?;
        }
        if refused {
            self.refuse(player, Refusal::EdictNotReady);
        }
        Ok(())
    }

    /// `OrderKind::Edict` at the front of the queue: closes until the mark is in range,
    /// then gathers on it (or turns a gather under way on it) and is done. Not ready, it
    /// holds there until it is.
    pub(crate) fn run_edict_order(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let Some(range) = self.edict(row).map(|e| e.range) else {
            self.finish_order(row);
            return Ok(());
        };
        let from = self.edict_point(row, o.pos);
        if from.xy().distance(o.pos) > range {
            if self.state.units.stuck_ticks[row] == u16::MAX {
                self.finish_order(row);
                return Ok(());
            }
            // Close to most of the range, along the line from the firing point.
            let pos = self.state.units.pos[row];
            let away = (from.xy() - o.pos).normalize();
            let goal = self.clamp_to_map(pos + (o.pos + away * (range * 4 / 5) - from.xy()));
            return self.ensure_moving(row, goal, goal);
        }
        let id = self.state.units.id(row);
        match self.state.edicts.iter().position(|e| e.source == id) {
            Some(i) => {
                if !matches!(self.state.edicts[i].phase, EdictPhase::Gathering { .. }) {
                    // Reloading: hold here until it is ready.
                    self.stop_moving(row);
                    return Ok(());
                }
                let z = self.edict_ground(o.pos);
                let c = &mut self.state.edicts[i];
                c.mark = o.pos;
                c.z = z;
                c.from = from;
                let left = match c.phase {
                    EdictPhase::Gathering { left } => left,
                    EdictPhase::Fired { .. } => 0,
                };
                self.announce_gather(row, left);
            }
            None => self.begin_edict(row, o.pos),
        }
        self.stop_moving(row);
        self.finish_order(row);
        Ok(())
    }

    /// The ground at `p`, or the sea's surface over water.
    fn edict_ground(&self, p: FxVec2) -> Fx {
        self.terrain.height_at(p).max(self.terrain.water_level())
    }

    /// The ship in `row` begins to gather on `mark`.
    fn begin_edict(&mut self, row: usize, mark: FxVec2) {
        let Some(e) = self.edict(row) else {
            return;
        };
        let left = e.gather_ticks;
        let units = &self.state.units;
        self.state.edicts.push(EdictCharge {
            source: units.id(row),
            owner: units.owner[row],
            blueprint: units.blueprint[row],
            mark,
            z: self.edict_ground(mark),
            from: self.edict_point(row, mark),
            phase: EdictPhase::Gathering { left },
            powered: true,
        });
        self.announce_gather(row, left);
    }

    fn announce_gather(&mut self, row: usize, left: u16) {
        let Some(c) = self.edict_charge(row) else {
            return;
        };
        self.events.push(SimEvent::EdictGathering {
            unit: c.source,
            owner: c.owner,
            blueprint: c.blueprint,
            from: c.from,
            mark: c.mark.extend(c.z),
            ticks: left,
        });
    }

    /// Lets a gather under way go (a stop): what was gathered is lost.
    pub(crate) fn cancel_edict(&mut self, row: usize) {
        let id = self.state.units.id(row);
        let Some(i) = self
            .state
            .edicts
            .iter()
            .position(|e| e.source == id && matches!(e.phase, EdictPhase::Gathering { .. }))
        else {
            return;
        };
        let c = self.state.edicts.remove(i);
        self.events.push(SimEvent::EdictCancelled {
            unit: id,
            from: c.from,
        });
    }

    /// This tick's Edicts: gathers counting down, shots fired, waves burning, reloads.
    pub(crate) fn run_edicts(&mut self) {
        if self.state.edicts.is_empty() {
            return;
        }
        let charges = std::mem::take(&mut self.state.edicts);
        let mut kept = Vec::with_capacity(charges.len());
        for mut c in charges {
            let row = self.state.units.row(c.source);
            let spec = self.blueprints.unit(c.blueprint).edict.clone();
            let Some(e) = spec else {
                continue;
            };
            match c.phase {
                EdictPhase::Gathering { left } => {
                    let Some(row) = row.filter(|&r| self.state.units.is_active(r)) else {
                        self.events.push(SimEvent::EdictCancelled {
                            unit: c.source,
                            from: c.from,
                        });
                        continue;
                    };
                    c.powered = !self.shields_unpowered(c.owner)
                        && !self.powered_down(row)
                        && !self.stunned(row);
                    c.from = self.edict_point(row, c.mark);
                    let left = if c.powered {
                        left.saturating_sub(1)
                    } else {
                        left
                    };
                    c.phase = EdictPhase::Gathering { left };
                    if left == 0 {
                        self.fire_edict(&mut c, &e);
                    }
                }
                EdictPhase::Fired { age } => {
                    let age = age.saturating_add(1);
                    c.phase = EdictPhase::Fired { age };
                    if age <= e.wave.life_ticks() {
                        self.burn_wave(&c, &e, age);
                    }
                    // Gone once reloaded, or once its wave is out with no ship to reload.
                    if age >= e.reload_ticks || (row.is_none() && age >= e.wave.life_ticks()) {
                        continue;
                    }
                }
            }
            kept.push(c);
        }
        // Gathers begun while these ran (none are, but keep any).
        kept.append(&mut self.state.edicts);
        self.state.edicts = kept;
    }

    /// The gather is done: the shot leaves from where the firing point is now.
    fn fire_edict(&mut self, c: &mut EdictCharge, e: &Edict) {
        let mark = c.mark.extend(c.z);
        self.events.push(SimEvent::EdictFired {
            unit: c.source,
            owner: c.owner,
            blueprint: c.blueprint,
            from: c.from,
            mark,
            radius: e.blast.radius,
            wave: e.wave.radius,
        });
        self.start_detonation(mark, e.blast, c.owner, c.source, vec![c.source]);
        c.phase = EdictPhase::Fired { age: 0 };
    }

    /// One tick of the molten wave, `age` ticks after the shot.
    fn burn_wave(&mut self, c: &EdictCharge, e: &Edict, age: u16) {
        self.crush_area(&WellArea {
            mark: c.mark,
            z: c.z,
            radius: e.wave.reach(age),
            owner: c.owner,
            source: c.source,
            per_tick: e.wave.damage / Fx::from_int(TICKS_PER_SECOND as i32),
            aircraft: Aircraft::Spared,
            edge: e.wave.edge,
            sides: Sides::Everyone,
        });
    }
}
