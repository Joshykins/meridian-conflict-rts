//! Airbases: underground hangars aircraft land in, mend in, and are fired out of.
//!
//! An aircraft told to dock (`OrderKind::Dock`) flies to the base. Those coming
//! in are laid out each tick (`lay_out_arrivals`): as many as fit side by side
//! in the shaft get a place in it and, once the hatch is open, go straight down
//! it to the lift at the bottom; the rest hold in rings over the base, in a
//! cluster, until there is room. A place is the dock order's `offset` from the
//! base, and its `radius` is one for a place in the shaft. On reaching the lift
//! the aircraft is taken below: it keeps its row, but is `IN_FACTORY` with
//! `hangar` naming the base, so nothing can see, hit, select or order it, and
//! the renderer is not given it. Below, it mends at the base's `heal` rate for nothing.
//!
//! Aircraft leave through the launch tunnels, one per tunnel every
//! `launch_ticks`: the player calls them out (`Command::Launch`) or gives them
//! orders while they are below (they go out to carry them out), or the base's
//! guard does when an enemy comes into the area it watches (`Units::guard`,
//! the whole reach unless the player sets it). Each is put at the back of its
//! tunnel and runs `run` metres down it, speeding up, still `IN_FACTORY`, then
//! leaves the mouth at `launch_speed`. Those the guard sent go after what came in
//! and come home once the area is clear.
//!
//! Aircraft with nowhere to be that need somewhere to land look for a base of
//! their side with room, whose reach they are in and that takes them
//! (`Units::auto_land`), before any open ground (`idle_air_land`).
//! When a base is destroyed, what it holds dies with it. Upgrades keep the base.

use crate::mirror::SimEvent;
use crate::spatial::kind;
use crate::tables::*;
use crate::{Handle, SimError, World};
use mc_core::{Angle, Fx, FxVec2, FxVec3, TICKS_PER_SECOND};
use mc_data::MoveLayer;

/// `Units::sortie` of a stored aircraft the player called out.
pub const SORTIE_CALLED: u16 = 1;
/// `Units::sortie` of a stored aircraft the base's guard called out.
pub const SORTIE_GUARD: u16 = 2;
/// `Units::sortie` of a stored aircraft the player gave orders to: it goes out to
/// carry them out.
pub const SORTIE_ORDERED: u16 = 3;
/// `Units::sortie` of an aircraft running down a launch tunnel, with the ticks
/// left before it leaves the mouth in the low bits.
pub const SORTIE_RUN: u16 = 0x8000;
/// Ticks after leaving a tunnel before an aircraft with nothing to do may go
/// home by itself (so one the player called out stays out).
pub const LAUNCH_GRACE: u16 = 30 * TICKS_PER_SECOND as u16;
/// Ticks after leaving a tunnel that it flies straight on at speed, instead of
/// stopping to lift clear of the ground the way an aircraft taking off does.
pub const LAUNCH_BOOST: u16 = 3 * TICKS_PER_SECOND as u16;
/// The square shaft under the hatch: half its width, and the depth of the lift at
/// its bottom (as `models/aster/airbase.rs` draws them; the renderer shows aircraft in it).
pub const SHAFT_HALF: Fx = Fx::ratio(37, 2);
pub const SHAFT_FLOOR: Fx = Fx::from_int(-21);
/// A docking aircraft this near (metres) has the hatch opened for it.
const HATCH_CALL: i32 = 420;
/// Over its place in the shaft this near (metres), a docking aircraft goes down.
const PLACE_CATCH: Fx = Fx::from_int(2);
/// This far above the lift (metres), a descending aircraft is taken below.
const TAKEN_BELOW: Fx = Fx::ONE;
/// Holding rings round the hatch: the first this far outside the shaft, the
/// next ones a hull's width and this gap apart.
const HOLD_FIRST: i32 = 14;
const HOLD_GAP: i32 = 8;
/// Stored aircraft below this share of full health stay below to mend when the
/// guard calls; aircraft out on guard below `GO_MEND` come home.
const SORTIE_HEALTH: (i64, i64) = (1, 3);
const GO_MEND: (i64, i64) = (1, 4);
/// How often (ticks) a base's guard looks over its area.
const GUARD_SCAN: usize = 5;
/// Most intruders a base's guard weighs at once.
const MAX_INTRUDERS: usize = 32;

impl World {
    /// The finished airbase in `id`, if it is one `owner` may use.
    pub(crate) fn airbase_for(&self, id: UnitId, owner: u8) -> Option<usize> {
        let units = &self.state.units;
        units.row(id).filter(|&b| {
            units.owner[b] == owner && units.is_active(b) && self.bp(b).airbase.is_some()
        })
    }

    /// Aircraft stored below the base in `base`.
    pub fn hangar_count(&self, base: usize) -> usize {
        let units = &self.state.units;
        let id = units.id(base);
        units
            .slots
            .iter()
            .filter(|&r| units.hangar[r] == id)
            .count()
    }

    /// Whether the aircraft in `r` is on its way down the base `id`'s hatch.
    fn docking_at(&self, r: usize, id: UnitId) -> bool {
        self.state
            .orders
            .front(&self.state.units, r)
            .is_some_and(|o| o.kind == OrderKind::Dock && o.target == id)
    }

    /// Room left below the base in `base`, counting aircraft already on their way down.
    pub(crate) fn hangar_room(&self, base: usize, except: usize) -> usize {
        let Some(a) = self.bp(base).airbase.as_ref() else {
            return 0;
        };
        let units = &self.state.units;
        let id = units.id(base);
        let used = units
            .slots
            .iter()
            .filter(|&r| r != except && (units.hangar[r] == id || self.docking_at(r, id)))
            .count();
        (a.capacity as usize).saturating_sub(used)
    }

    /// Whether this unit can go down an airbase's hatch: an aircraft, not a
    /// carrier's drone or the carrier itself.
    pub(crate) fn can_dock(&self, row: usize) -> bool {
        let bp = self.bp(row);
        bp.motion.is_some_and(|m| m.layer == MoveLayer::Air)
            && bp.drone.is_none()
            && bp.visual.mesh != "reclaim_drone"
            && self.state.units.drone_parent[row] == Handle::NONE
    }

    /// The nearest base of this aircraft's side that takes idle aircraft, has room,
    /// and whose reach it is in.
    pub(crate) fn airbase_to_land_at(&self, row: usize) -> Option<usize> {
        let units = &self.state.units;
        let (pos, owner) = (units.pos[row], units.owner[row]);
        units
            .slots
            .iter()
            .filter_map(|b| {
                let a = self.bp(b).airbase.as_ref()?;
                let d = units.pos[b].distance(pos);
                (units.owner[b] == owner
                    && units.is_active(b)
                    && units.auto_land[b]
                    && d <= a.reach)
                    .then_some((d, b))
            })
            .min_by_key(|&(d, b)| (d, b))
            .map(|(_, b)| b)
            .filter(|&b| self.hangar_room(b, row) > 0)
    }

    /// `Command::Dock`: aircraft among `ids` fly to `base` and go down its hatch.
    pub(crate) fn order_dock(
        &mut self,
        player: u8,
        ids: &[UnitId],
        base: UnitId,
        queue: bool,
    ) -> Result<(), SimError> {
        let Some(b) = self.airbase_for(base, player) else {
            return Ok(());
        };
        let pos = self.state.units.pos[b];
        for row in self.owned(player, ids, 0) {
            if self.can_dock(row) {
                let o = crate::orders::order(OrderKind::Dock, pos, base);
                self.give(row, o, queue)?;
            }
        }
        Ok(())
    }

    /// `Command::Launch`: calls out what these bases hold, `blueprint` only if
    /// given, at most `count` per base (zero: everything).
    pub(crate) fn order_launch(
        &mut self,
        player: u8,
        ids: &[UnitId],
        blueprint: Option<mc_data::BlueprintId>,
        count: u16,
    ) {
        for b in self.owned(player, ids, 0) {
            // An aircraft named on its own is called out by itself.
            if self.state.units.hangar[b] != Handle::NONE {
                if self.state.units.sortie[b] == 0 {
                    self.state.units.sortie[b] = SORTIE_CALLED;
                }
                continue;
            }
            if self.bp(b).airbase.is_none() {
                continue;
            }
            let id = self.state.units.id(b);
            let rows: Vec<usize> = self.state.units.slots.iter().collect();
            let units = &mut self.state.units;
            let mut left = if count == 0 {
                usize::MAX
            } else {
                count as usize
            };
            for r in rows {
                if left == 0 {
                    break;
                }
                if units.hangar[r] == id
                    && units.sortie[r] == 0
                    && blueprint.is_none_or(|bp| units.blueprint[r] == bp)
                {
                    units.sortie[r] = SORTIE_CALLED;
                    left -= 1;
                }
            }
        }
    }

    /// `Command::SetAutoLand`.
    pub(crate) fn set_auto_land(&mut self, player: u8, ids: &[UnitId], on: bool) {
        for b in self.owned(player, ids, 0) {
            if self.bp(b).airbase.is_some() {
                self.state.units.auto_land[b] = on;
            }
        }
    }

    /// `Command::Guard` given to an airbase: the area, kept inside its reach.
    pub(crate) fn set_airbase_guard(&mut self, b: usize, pos: FxVec2, radius: Fx) {
        let a = self.bp(b).airbase.as_ref().expect("airbase");
        let base_pos = self.state.units.pos[b];
        let offset = pos - base_pos;
        let pos = if offset.length() > a.reach {
            base_pos + offset.normalize() * a.reach
        } else {
            pos
        };
        let radius = radius.clamp(crate::command::MIN_GUARD_RADIUS, a.reach);
        self.state.units.guard[b] = (pos, radius);
    }

    /// A new airbase guards its whole reach round itself.
    pub(crate) fn arm_airbase(&mut self, b: usize) {
        if let Some(a) = self.bp(b).airbase.as_ref() {
            let reach = a.reach;
            self.state.units.guard[b] = (self.state.units.pos[b], reach);
        }
    }

    /// An airbase went up a tier in place: a guard that covered the old reach
    /// covers the new one.
    pub(crate) fn airbase_upgraded(&mut self, b: usize, old: mc_data::BlueprintId) {
        let (Some(was), Some(now)) = (
            self.blueprints.unit(old).airbase.as_ref().map(|a| a.reach),
            self.bp(b).airbase.as_ref().map(|a| a.reach),
        ) else {
            return;
        };
        let (pos, radius) = self.state.units.guard[b];
        if radius >= was {
            self.state.units.guard[b] = (pos, now);
        }
    }

    /// `OrderKind::Dock`: fly to the place `lay_out_arrivals` gave it; one in the
    /// shaft goes down once the hatch is open (`stand_z`), and `run_hatch` takes it
    /// below at the bottom.
    pub(crate) fn run_dock(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let owner = self.state.units.owner[row];
        let base = self.airbase_for(o.target, owner);
        let Some(b) = base.filter(|&b| self.can_dock(row) && self.hangar_room(b, row) > 0) else {
            // Gone, or full: the aircraft is left to find somewhere else.
            self.finish_order(row);
            return Ok(());
        };
        let place = self.state.units.pos[b] + o.offset;
        self.state.units.flags[row] &= !flag::AIR_RUN;
        self.ensure_moving(row, place, place)
    }

    /// Whether the aircraft in `row` is over its place in an open shaft it is
    /// docking at, and so goes down it rather than holding cruise height.
    pub(crate) fn descending_to_hatch(&self, row: usize) -> bool {
        let units = &self.state.units;
        let Some(o) = self.state.orders.front(units, row) else {
            return false;
        };
        if o.kind != OrderKind::Dock || o.radius <= Fx::ZERO {
            return false;
        }
        let Some(b) = units.row(o.target) else {
            return false;
        };
        let Some(a) = self.bp(b).airbase.as_ref() else {
            return false;
        };
        units.deploy[b] >= a.hatch_ticks
            && units.pos[row].distance(units.pos[b] + o.offset) <= PLACE_CATCH
    }

    /// Where an aircraft going down a shaft stands at the bottom: the lift.
    pub(crate) fn shaft_floor(&self, row: usize) -> Fx {
        let units = &self.state.units;
        let base = self
            .state
            .orders
            .front(units, row)
            .and_then(|o| units.row(o.target))
            .map_or(units.pos[row], |b| units.pos[b]);
        self.terrain.height_at(base) + SHAFT_FLOOR
    }

    /// Every tick, after orders: tunnel runs, hatches, taking aircraft below,
    /// mending, launches, guards, and what dies with a lost base.
    pub(crate) fn run_airbases(&mut self) -> Result<(), SimError> {
        let rows: Vec<usize> = self.state.units.slots.iter().collect();
        let mut any_base = false;
        for &row in &rows {
            let is_base = self.bp(row).airbase.is_some();
            any_base |= is_base;
            let sortie = self.state.units.sortie[row];
            if sortie & SORTIE_RUN != 0 {
                self.run_tunnel_step(row)?;
            } else if sortie > 0 && self.state.units.hangar[row] == Handle::NONE && !is_base {
                self.state.units.sortie[row] -= 1;
            }
        }
        // Stored aircraft: mend, or die with a base that is gone.
        for &row in &rows {
            let base = self.state.units.hangar[row];
            if base == Handle::NONE {
                continue;
            }
            match self.state.units.row(base) {
                Some(b) if self.bp(b).airbase.is_some() => {
                    self.orders_below(row, base)?;
                    let heal = self.bp(b).airbase.as_ref().expect("airbase").heal;
                    let max = self.unit_max_health(row);
                    let units = &mut self.state.units;
                    units.health[row] =
                        (units.health[row] + max * heal / TICKS_PER_SECOND as i32).min(max);
                }
                // In a lift ship's hold (`transport.rs`).
                Some(b) if self.bp(b).transport.is_some() => {}
                _ => {
                    self.state.units.health[row] = Fx::ZERO;
                    self.despawn_unit(row, false)?;
                }
            }
        }
        if !any_base {
            return Ok(());
        }
        for &b in &rows {
            if !self.state.units.slots.is_alive(b)
                || self.bp(b).airbase.is_none()
                || !self.state.units.is_active(b)
            {
                continue;
            }
            self.run_hatch(b, &rows)?;
            self.run_tunnels(b)?;
            self.run_airbase_guard(b)?;
        }
        Ok(())
    }

    /// A stored aircraft given orders is called out to carry them out; one told to
    /// dock where it already is forgets it; one whose orders were taken back stays.
    fn orders_below(&mut self, row: usize, base: UnitId) -> Result<(), SimError> {
        let units = &self.state.units;
        let front = self.state.orders.front(units, row).copied();
        if front.is_some_and(|o| o.kind == OrderKind::Dock && o.target == base) {
            self.state.orders.pop_front(&mut self.state.units, row);
        }
        let has_orders = self.state.units.order_head[row] != NO_ORDER;
        let units = &mut self.state.units;
        match units.sortie[row] {
            0 if has_orders => units.sortie[row] = SORTIE_ORDERED,
            SORTIE_ORDERED if !has_orders => units.sortie[row] = 0,
            _ => {}
        }
        Ok(())
    }

    /// Opens the hatch while aircraft are coming in to dock, closes it otherwise,
    /// lays the arrivals out, and takes below whatever has reached the lift.
    fn run_hatch(&mut self, b: usize, rows: &[usize]) -> Result<(), SimError> {
        let a = self.bp(b).airbase.clone().expect("airbase");
        let units = &self.state.units;
        let (id, hatch) = (units.id(b), units.pos[b]);
        let floor = self.terrain.height_at(hatch) + SHAFT_FLOOR;
        let mut arriving = Vec::new();
        let mut down = Vec::new();
        for &r in rows {
            if !units.slots.is_alive(r) || !units.is_active(r) || !self.docking_at(r, id) {
                continue;
            }
            if units.pos[r].distance(hatch) <= Fx::from_int(HATCH_CALL) {
                arriving.push(r);
            }
            if units.z[r] <= floor + TAKEN_BELOW {
                down.push(r);
            }
        }
        let units = &mut self.state.units;
        units.deploy[b] = if arriving.is_empty() {
            units.deploy[b].saturating_sub(1)
        } else {
            (units.deploy[b] + 1).min(a.hatch_ticks)
        };
        for r in down {
            if self.hangar_count(b) < a.capacity as usize {
                self.store_aircraft(r, b)?;
                arriving.retain(|&x| x != r);
            }
        }
        self.lay_out_arrivals(b, &arriving);
        Ok(())
    }

    /// Gives each aircraft coming in a place: side by side in the shaft while they
    /// fit, keeping any that already have one there, the rest in rings over the
    /// base. The place goes in the dock order (`offset`; `radius` one in the shaft).
    fn lay_out_arrivals(&mut self, b: usize, arriving: &[usize]) {
        let hatch = self.state.units.pos[b];
        let front =
            |w: &World, r: usize| *w.state.orders.front(&w.state.units, r).expect("docking");
        // Those already given a place in the shaft keep it.
        let mut placed: Vec<(usize, FxVec2)> = Vec::new();
        for &r in arriving {
            let o = front(self, r);
            if o.radius > Fx::ZERO {
                placed.push((r, o.offset));
            }
        }
        // The rest, nearest first, into the shaft where they fit, or to hold.
        let mut rest: Vec<usize> = arriving
            .iter()
            .copied()
            .filter(|&r| !placed.iter().any(|&(p, _)| p == r))
            .collect();
        rest.sort_by_key(|&r| (self.state.units.pos[r].distance(hatch), r));
        let mut holding = Vec::new();
        for r in rest {
            let size = self.bp(r).radius;
            let spot = shaft_places().into_iter().find(|&at| {
                at.x.abs().max(at.y.abs()) + size <= SHAFT_HALF
                    && placed
                        .iter()
                        .all(|&(p, q)| q.distance(at) >= size + self.bp(p).radius + Fx::ONE)
            });
            match spot {
                Some(at) => placed.push((r, at)),
                None => holding.push(r),
            }
        }
        // Holding: rings over the base, a hull's width apart.
        let widest = holding
            .iter()
            .map(|&r| self.bp(r).radius)
            .max()
            .unwrap_or(Fx::ONE);
        let spacing = widest * 2 + Fx::from_int(HOLD_GAP);
        let mut ring_radius = SHAFT_HALF + Fx::from_int(HOLD_FIRST) + widest;
        let mut held = holding.into_iter().peekable();
        let mut places: Vec<(usize, FxVec2, bool)> =
            placed.into_iter().map(|(r, at)| (r, at, true)).collect();
        while held.peek().is_some() {
            let around = (ring_radius * 6 / spacing).floor_int().max(3);
            for k in 0..around {
                let Some(r) = held.next() else { break };
                let a = Angle((k * 0x10000 / around) as u16);
                places.push((r, FxVec2::from_angle(a) * ring_radius, false));
            }
            ring_radius += spacing;
        }
        for (r, at, in_shaft) in places {
            let head = self.state.units.order_head[r];
            let o = &mut self.state.orders.order[head as usize];
            o.offset = at;
            o.radius = if in_shaft { Fx::ONE } else { Fx::ZERO };
        }
    }

    /// Takes the aircraft in `row` below the base in `b`.
    fn store_aircraft(&mut self, row: usize, b: usize) -> Result<(), SimError> {
        self.clear_orders(row)?;
        self.stop_moving(row);
        let (base_id, pos) = (self.state.units.id(b), self.state.units.pos[b]);
        let floor = self.terrain.height_at(pos) + SHAFT_FLOOR;
        let units = &mut self.state.units;
        units.flags[row] |= flag::IN_FACTORY;
        units.flags[row] &= !(flag::AIR_RUN | flag::MOVING);
        units.hangar[row] = base_id;
        units.sortie[row] = 0;
        units.pos[row] = pos;
        units.prev_pos[row] = pos;
        units.z[row] = floor;
        units.prev_z[row] = floor;
        units.speed[row] = Fx::ZERO;
        units.air_velocity[row] = FxVec3::ZERO;
        units.bank[row] = 0;
        units.prev_bank[row] = 0;
        units.weapon_target[row] = [Handle::NONE; mc_data::MAX_WEAPONS];
        units.burn_ticks[row] = 0;
        let blueprint = units.blueprint[row];
        self.events.push(SimEvent::AircraftStored {
            pos: pos.extend(floor),
            blueprint,
        });
        Ok(())
    }

    /// Sends called-out aircraft into the tunnels, one per tunnel, once they are ready.
    fn run_tunnels(&mut self, b: usize) -> Result<(), SimError> {
        if self.state.units.sortie[b] > 0 {
            self.state.units.sortie[b] -= 1;
            return Ok(());
        }
        let a = self.bp(b).airbase.clone().expect("airbase");
        let id = self.state.units.id(b);
        let units = &self.state.units;
        let called: Vec<usize> = units
            .slots
            .iter()
            .filter(|&r| units.hangar[r] == id && units.sortie[r] != 0)
            .take(a.tunnels.len())
            .collect();
        if called.is_empty() {
            return Ok(());
        }
        // Take the tunnels in turn, so one volley after another comes out all round.
        let first = (self.state.tick / a.launch_ticks.max(1) as u32) as usize;
        let (guard_at, guard_radius) = self.state.units.guard[b];
        for (i, r) in called.into_iter().enumerate() {
            let tunnel = a.tunnels[(first + i) % a.tunnels.len()];
            let why = self.state.units.sortie[r];
            self.launch_aircraft(r, b, tunnel, &a)?;
            if why == SORTIE_ORDERED && self.state.units.order_head[r] != NO_ORDER {
                // It was given orders below: it carries them out.
            } else if why == SORTIE_GUARD && guard_radius > Fx::ZERO {
                let mut o = crate::orders::order(OrderKind::Guard, guard_at, id);
                o.radius = guard_radius;
                self.give(r, o, false)?;
            } else {
                // Out along the tunnel, clear of the base, and waits there for orders.
                let units = &self.state.units;
                let heading = units.heading[r];
                let out = self.clamp_to_map(
                    units.pos[r] + FxVec2::from_angle(heading) * (a.run + Fx::from_int(320)),
                );
                let mut o = crate::orders::order(OrderKind::Move, out, Handle::NONE);
                o.heading = heading;
                self.give(r, o, false)?;
            }
        }
        self.state.units.sortie[b] = a.launch_ticks;
        Ok(())
    }

    /// Ticks an aircraft takes down a tunnel of the base's: from standing to
    /// `launch_speed` over `run`, speeding up evenly.
    fn run_ticks(a: &mc_data::Airbase) -> u16 {
        let per_tick = a.launch_speed / TICKS_PER_SECOND as i32;
        (a.run * 2 / per_tick.max(Fx::ONE)).round_int().clamp(2, 30) as u16
    }

    /// Puts the stored aircraft in `row` at the back of `tunnel` of the base in `b`,
    /// facing out. `run_tunnel_step` carries it to the mouth.
    fn launch_aircraft(
        &mut self,
        row: usize,
        b: usize,
        tunnel: mc_data::Tunnel,
        a: &mc_data::Airbase,
    ) -> Result<(), SimError> {
        let units = &self.state.units;
        let (base_pos, base_heading) = (units.pos[b], units.heading[b]);
        let mouth = self.clamp_to_map(base_pos + tunnel.mouth.xy().rotate(base_heading));
        let heading = base_heading + tunnel.yaw;
        let back = mouth - FxVec2::from_angle(heading) * a.run;
        let z = self
            .terrain
            .height_at(mouth)
            .max(self.terrain.water_level())
            + tunnel.mouth.z;
        let units = &mut self.state.units;
        units.hangar[row] = Handle::NONE;
        units.sortie[row] = SORTIE_RUN | Self::run_ticks(a);
        units.pos[row] = back;
        units.prev_pos[row] = back;
        units.z[row] = z;
        units.prev_z[row] = z;
        units.heading[row] = heading;
        units.prev_heading[row] = heading;
        units.speed[row] = Fx::ZERO;
        units.air_velocity[row] = FxVec3::ZERO;
        units.move_goal[row] = back;
        let blueprint = units.blueprint[row];
        self.events.push(SimEvent::AircraftLaunched {
            pos: mouth.extend(z),
            heading,
            blueprint,
        });
        Ok(())
    }

    /// One tick down a launch tunnel: the aircraft is carried on, faster each tick,
    /// and out of the mouth at `launch_speed` it is free.
    fn run_tunnel_step(&mut self, row: usize) -> Result<(), SimError> {
        let units = &self.state.units;
        // The base is the nearest of its side's: the aircraft only knows its own run.
        let base = units
            .slots
            .iter()
            .filter(|&b| self.bp(b).airbase.is_some() && units.owner[b] == units.owner[row])
            .min_by_key(|&b| (units.pos[b].distance(units.pos[row]), b));
        let Some(a) = base.and_then(|b| self.bp(b).airbase.clone()) else {
            // Its base is gone under it: out it comes as it is.
            self.free_from_tunnel(row, Fx::from_int(30));
            return Ok(());
        };
        let total = Self::run_ticks(&a) as i32;
        let left = (units.sortie[row] & !SORTIE_RUN) as i32;
        let done = total - left;
        // Covered by the end of tick k of n: run * (k/n)^2.
        let step = a.run * (2 * done + 1) / (total * total);
        let dir = FxVec2::from_angle(units.heading[row]);
        let units = &mut self.state.units;
        units.pos[row] += dir * step;
        units.flags[row] |= flag::MOVING;
        if left <= 1 {
            self.free_from_tunnel(row, a.launch_speed);
        } else {
            units.sortie[row] -= 1;
        }
        Ok(())
    }

    /// Out of the mouth: flying on its own at `speed`.
    fn free_from_tunnel(&mut self, row: usize, speed: Fx) {
        let units = &mut self.state.units;
        let dir = FxVec2::from_angle(units.heading[row]);
        let per_tick = speed / TICKS_PER_SECOND as i32;
        units.flags[row] &= !flag::IN_FACTORY;
        units.sortie[row] = LAUNCH_GRACE;
        units.speed[row] = speed;
        units.air_velocity[row] = (dir * per_tick).extend(per_tick / 5);
    }

    /// Whether the aircraft in `row` left a tunnel moments ago and is still
    /// flying out on the launch.
    pub(crate) fn just_launched(&self, row: usize) -> bool {
        let units = &self.state.units;
        let s = units.sortie[row];
        units.hangar[row] == Handle::NONE
            && s & SORTIE_RUN == 0
            && s > LAUNCH_GRACE - LAUNCH_BOOST
            && self.bp(row).airbase.is_none()
    }

    /// The guard on an airbase: when an enemy it could send something at comes
    /// into the area, it calls out every stored aircraft that can strike one of
    /// them and is not too badly hurt.
    fn run_airbase_guard(&mut self, b: usize) -> Result<(), SimError> {
        let (at, radius) = self.state.units.guard[b];
        if radius <= Fx::ZERO || (self.state.tick as usize + b) % GUARD_SCAN != 0 {
            return Ok(());
        }
        let units = &self.state.units;
        let (id, owner) = (units.id(b), units.owner[b]);
        let stored: Vec<usize> = units
            .slots
            .iter()
            .filter(|&r| {
                units.hangar[r] == id
                    && units.sortie[r] == 0
                    && units.health[r] * (SORTIE_HEALTH.1 as i32)
                        >= self.unit_max_health(r) * (SORTIE_HEALTH.0 as i32)
            })
            .collect();
        if stored.is_empty() {
            return Ok(());
        }
        let mut intruders = Vec::new();
        self.index.query(
            at,
            radius + crate::reclaim::WIDEST_TARGET,
            kind::UNIT,
            |e| {
                let t = e.row as usize;
                if self.unit_entry_is_current(e)
                    && e.pos.distance(at) <= radius + e.radius
                    && self.are_enemies(owner, units.owner[t])
                    && !units.has_flag(t, flag::IN_FACTORY)
                    && self.detects(owner, t)
                {
                    intruders.push(t);
                }
                intruders.len() < MAX_INTRUDERS
            },
        );
        if intruders.is_empty() {
            return Ok(());
        }
        for r in stored {
            // The very test the aircraft uses out there (`guard_may_strike`).
            if intruders.iter().any(|&t| self.guard_may_strike(r, t, owner)) {
                self.state.units.sortie[r] = SORTIE_GUARD;
            }
        }
        Ok(())
    }

    /// Aircraft out on a base's guard that are badly hurt go home to mend.
    pub(crate) fn guard_should_mend(&self, row: usize) -> bool {
        let units = &self.state.units;
        units.health[row] * (GO_MEND.1 as i32) < self.unit_max_health(row) * (GO_MEND.0 as i32)
    }
}

/// Places side by side in the square shaft, nearest the middle first: a 3 m
/// hexagonal grid over it (offsets from the hatch). Each arrival takes the first it fits at.
fn shaft_places() -> Vec<FxVec2> {
    let mut out: Vec<(i64, i32, i32)> = Vec::new();
    for j in -8..=8i32 {
        for i in -8..=8i32 {
            // Millimetres: rows 2.598 m apart, every other row shifted half a step.
            let (x, y) = ((i * 3000 + j * 1500) as i64, (j * 2598) as i64);
            let d = x * x + y * y;
            if x.abs() <= 18_500 && y.abs() <= 18_500 {
                out.push((d, j, i));
            }
        }
    }
    out.sort();
    out.into_iter()
        .map(|(_, j, i)| {
            FxVec2::new(
                Fx::ratio((i * 3000 + j * 1500) as i64, 1000),
                Fx::ratio((j * 2598) as i64, 1000),
            )
        })
        .collect()
}
