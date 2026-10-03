//! Core mines: materials out of the ground around them.
//!
//! Every patch of land within a mine's reach is worth materials, ore far more
//! than bare ground. Where mines' reaches overlap, anyone's, the ground is
//! divided between them along the straight line through the two points where
//! their circles cross (a power diagram: a patch goes to the mine for which
//! `distance^2 - reach^2` is least), so every mine has a territory, the part
//! of its circle nobody else has a better claim to. Mines crowded together
//! each make less: a mine's efficiency is what its territory gives over what
//! its whole circle would. The shaft's `base` is shared the same way, by the
//! part of its circle it holds, land or sea, so packing mines together never
//! pays more than spreading them out. The tiers are the investment: upgrading raises what
//! each hectare gives, not the reach.
//!
//! Ore lies deep (`OreRegion::depth`). A new mine sinks its main shaft
//! straight down at [`SHAFT_SPEED`], and when the shaft reaches a field's
//! depth it drives a drift out to it at [`DRIFT_SPEED`]; a field's ore pays
//! only once its drift has arrived. The land it works spreads out from the
//! mine at [`SPREAD_SPEED`] too, so a new mine starts with only its shaft's
//! `base` and grows into its territory. Mines stand on land only: the sea in
//! a mine's circle is worth nothing, though it counts in its shaft's share.

use crate::tables::UnitId;
use crate::World;
use mc_core::{player_bit, Fx, FxVec2, PlayerMask, StateHasher, TICKS_PER_SECOND};
use mc_map::OreRegion;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const DT: i32 = TICKS_PER_SECOND as i32;
/// Ore is counted on a grid of this pitch, metres.
pub const ORE_CELL_M: i32 = 8;
/// Land is counted on a coarser grid: the map overview's own pitch, so the
/// interface can count it the same way from the overview.
pub const GROUND_CELL_M: i32 = 32;
/// Metres a second a new mine's main shaft goes down.
pub const SHAFT_SPEED: i32 = 8;
/// Metres a second a drift goes out from the shaft to a field.
pub const DRIFT_SPEED: i32 = 24;
/// Metres a second the land a mine works spreads out from it.
pub const SPREAD_SPEED: i32 = 10;
/// The share of its output a mine still makes with none of its energy upkeep
/// paid, as a fraction: enough to climb out of a stall, never to live on.
pub const UNPOWERED: (i64, i64) = (1, 4);

/// One mine's own state: its territory, the fields in it, and how long it
/// has been digging.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MineState {
    /// Its share of the land and the ore in reach, hectares, and what it would
    /// have with its reach to itself.
    pub land: Share,
    /// The ore fields with ore in its territory.
    pub veins: Vec<Vein>,
    /// Hectares of its land by distance from it, one entry per
    /// [`GROUND_CELL_M`] ring, so the land it has spread over can be summed.
    pub rings: Vec<Fx>,
    /// Ticks since it was finished. Kept through an upgrade.
    pub age: u32,
}

/// An ore field a mine draws on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vein {
    /// The field, in map order.
    pub field: u16,
    /// Hectares of its ore in the mine's territory.
    pub ore: Fx,
    /// Tick of the mine's age at which its drift reaches the field.
    pub reached_at: u32,
}

/// One ore field in a mine's territory: `(field, hectares, its middle, its depth)`.
pub type FieldShare = (u16, Fx, FxVec2, Fx);

/// Hectares of land and of ore a mine draws on, and of its whole territory,
/// land or sea, which shares out its shaft's `base`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Share {
    pub ground: Fx,
    pub ore: Fx,
    pub ground_alone: Fx,
    pub ore_alone: Fx,
    pub rock: Fx,
    pub rock_alone: Fx,
}

impl MineState {
    /// Materials per second now: the land it has spread over, and the ore it
    /// has dug out to.
    pub fn rate(&self, m: &mc_data::Mine) -> Fx {
        let reached = self
            .veins
            .iter()
            .filter(|v| self.age >= v.reached_at)
            .fold(Fx::ZERO, |a, v| a + v.ore);
        mine_rate(m, self.land.rock_part(), self.worked_ground(), reached)
    }

    /// Metres out from the mine the land it works reaches by now.
    pub fn spread(&self) -> Fx {
        Fx::from_int(SPREAD_SPEED) * self.age as i32 / TICKS_PER_SECOND as i32
    }

    /// Hectares of its land within [`Self::spread`].
    pub fn worked_ground(&self) -> Fx {
        let spread = self.spread();
        self.rings
            .iter()
            .enumerate()
            .take_while(|&(i, _)| Fx::from_int(i as i32 * GROUND_CELL_M) <= spread)
            .fold(Fx::ZERO, |a, (_, v)| a + *v)
    }

    /// Materials per second once every drift is dug.
    pub fn full_rate(&self, m: &mc_data::Mine) -> Fx {
        self.land.rate(m)
    }
}

/// Ticks a mine at `mine` takes to dig out to a field at `centre`, `depth` down.
pub fn dig_ticks(mine: FxVec2, centre: FxVec2, depth: Fx) -> u32 {
    let down = depth / SHAFT_SPEED;
    let out = mine.distance(centre) / DRIFT_SPEED;
    ((down + out) * TICKS_PER_SECOND as i32).ceil_int().max(0) as u32
}

impl Share {
    /// The part of its circle it holds, 0..=1: what it gets of its shaft's `base`.
    pub fn rock_part(&self) -> Fx {
        if self.rock_alone <= Fx::ZERO {
            Fx::ONE
        } else {
            (self.rock / self.rock_alone).clamp(Fx::ZERO, Fx::ONE)
        }
    }

    /// Materials per second once dug out.
    pub fn rate(&self, m: &mc_data::Mine) -> Fx {
        mine_rate(m, self.rock_part(), self.ground, self.ore)
    }

    /// What it gets over what it would alone, 0..=1, its shaft's share included.
    pub fn efficiency(&self, m: &mc_data::Mine) -> Fx {
        let alone = m.base + land_rate(m, self.ground_alone, self.ore_alone);
        if alone <= Fx::ZERO {
            Fx::ONE
        } else {
            (self.rate(m) / alone).clamp(Fx::ZERO, Fx::ONE)
        }
    }
}

/// Every live, finished mine, by unit id.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Mines {
    pub by_unit: BTreeMap<UnitId, MineState>,
    /// The mine layout the shares were last counted for; not hashed, only a
    /// trigger to count again.
    #[serde(skip)]
    counted: u64,
}

impl Mines {
    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.by_unit.len() as u64);
        for (id, m) in &self.by_unit {
            h.write_u64(id.0 as u64 | (m.age as u64) << 32);
            let l = &m.land;
            for v in [
                l.ground,
                l.ore,
                l.ground_alone,
                l.ore_alone,
                l.rock,
                l.rock_alone,
            ] {
                h.write_i64(v.0);
            }
            h.write_u64(m.rings.len() as u64);
            for v in &m.rings {
                h.write_i64(v.0);
            }
            h.write_u64(m.veins.len() as u64);
            for v in &m.veins {
                h.write_u64(v.field as u64 | (v.reached_at as u64) << 16);
                h.write_i64(v.ore.0);
            }
        }
    }
}

/// The map rasterised for counting: a land bit per ground cell, and one bit
/// per ore cell inside each field's box. A mine works land and the ore under
/// land; the sea is worth nothing.
#[derive(Clone, Default)]
pub struct OreGrid {
    fields: Vec<OreField>,
    land: Vec<u64>,
    land_w: i32,
    land_h: i32,
}

#[derive(Clone)]
struct OreField {
    /// First cell (x, y) of the box, and its size in cells.
    x0: i32,
    y0: i32,
    w: i32,
    h: i32,
    bits: Vec<u64>,
    centre: FxVec2,
    depth: Fx,
}

impl OreField {
    fn has(&self, cx: i32, cy: i32) -> bool {
        let (lx, ly) = (cx - self.x0, cy - self.y0);
        if lx < 0 || ly < 0 || lx >= self.w || ly >= self.h {
            return false;
        }
        let i = (ly * self.w + lx) as usize;
        self.bits[i / 64] >> (i % 64) & 1 != 0
    }
}

/// Centre of cell `c` of a grid of pitch `pitch`, metres.
fn cell_centre(c: i32, pitch: i32) -> Fx {
    Fx::from_int(c * pitch + pitch / 2)
}

/// The part of the patch at `at` that falls to the mine at `pos` with `reach`,
/// against `others`: all of it when its power distance is the least, none
/// when another's is less, split evenly on an exact tie.
fn portion(at: FxVec2, pos: FxVec2, reach: Fx, others: &[(FxVec2, Fx)]) -> Fx {
    let mine = at.distance_sq(pos) - reach * reach;
    let mut tied = 1;
    for &(p, r) in others {
        let theirs = at.distance_sq(p) - r * r;
        if theirs < mine {
            return Fx::ZERO;
        }
        tied += (theirs == mine) as i64;
    }
    Fx::ratio(1, tied)
}

impl OreGrid {
    /// `size` is the map's extent in metres; `is_land` says whether the ground
    /// at a point is above water.
    pub fn new(regions: &[OreRegion], size: FxVec2, is_land: impl Fn(FxVec2) -> bool) -> OreGrid {
        let fields = regions
            .iter()
            .map(|r| {
                let (lo, hi) = r.bounds();
                let x0 = lo.x.floor_int() / ORE_CELL_M;
                let y0 = lo.y.floor_int() / ORE_CELL_M;
                let w = hi.x.floor_int() / ORE_CELL_M - x0 + 1;
                let h = hi.y.floor_int() / ORE_CELL_M - y0 + 1;
                let mut bits = vec![0u64; ((w * h) as usize).div_ceil(64)];
                for ly in 0..h {
                    for lx in 0..w {
                        let at = FxVec2::new(
                            cell_centre(x0 + lx, ORE_CELL_M),
                            cell_centre(y0 + ly, ORE_CELL_M),
                        );
                        if r.contains(at) {
                            let i = (ly * w + lx) as usize;
                            bits[i / 64] |= 1 << (i % 64);
                        }
                    }
                }
                OreField {
                    x0,
                    y0,
                    w,
                    h,
                    bits,
                    centre: r.centre(),
                    depth: r.depth(),
                }
            })
            .collect();
        let land_w = size.x.floor_int() / GROUND_CELL_M;
        let land_h = size.y.floor_int() / GROUND_CELL_M;
        let mut land = vec![0u64; ((land_w * land_h).max(0) as usize).div_ceil(64)];
        for y in 0..land_h {
            for x in 0..land_w {
                let at = FxVec2::new(cell_centre(x, GROUND_CELL_M), cell_centre(y, GROUND_CELL_M));
                if is_land(at) {
                    let i = (y * land_w + x) as usize;
                    land[i / 64] |= 1 << (i % 64);
                }
            }
        }
        OreGrid {
            fields,
            land,
            land_w,
            land_h,
        }
    }

    fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.land_w && y < self.land_h
    }

    fn is_land(&self, x: i32, y: i32) -> bool {
        if !self.in_bounds(x, y) {
            return false;
        }
        let i = (y * self.land_w + x) as usize;
        self.land[i / 64] >> (i % 64) & 1 != 0
    }

    /// The ground and ore of the territory of a mine at `pos` with `reach`
    /// among `others`; and of its whole circle, as if it had nobody near. Hectares.
    pub fn share(&self, pos: FxVec2, reach: Fx, others: &[(FxVec2, Fx)]) -> Share {
        self.share_by_field(pos, reach, others).0
    }

    /// `share`, and the ore of each field in the territory: `(field, hectares,
    /// its middle, its depth)`, in map order.
    pub fn share_by_field(
        &self,
        pos: FxVec2,
        reach: Fx,
        others: &[(FxVec2, Fx)],
    ) -> (Share, Vec<FieldShare>) {
        let (share, fields, _) = self.share_with_rings(pos, reach, others);
        (share, fields)
    }

    /// `share_by_field`, and its land by distance: hectares per
    /// [`GROUND_CELL_M`] ring out from `pos`.
    pub fn share_with_rings(
        &self,
        pos: FxVec2,
        reach: Fx,
        others: &[(FxVec2, Fx)],
    ) -> (Share, Vec<FieldShare>, Vec<Fx>) {
        let reach_sq = reach * reach;
        let mut out = Share::default();

        // Land, and the whole territory, on the coarse grid.
        let r = reach.ceil_int() / GROUND_CELL_M + 1;
        let (cx, cy) = (
            pos.x.floor_int() / GROUND_CELL_M,
            pos.y.floor_int() / GROUND_CELL_M,
        );
        let (mut shared, mut alone) = (Fx::ZERO, 0i32);
        let (mut rock, mut rock_alone) = (Fx::ZERO, 0i32);
        let mut rings = vec![Fx::ZERO; (r + 1) as usize];
        for y in cy - r..=cy + r {
            for x in cx - r..=cx + r {
                let at = FxVec2::new(cell_centre(x, GROUND_CELL_M), cell_centre(y, GROUND_CELL_M));
                if at.distance_sq(pos) > reach_sq {
                    continue;
                }
                let ours = self.in_bounds(x, y) && self.is_land(x, y);
                let part = portion(at, pos, reach, others);
                rock_alone += 1;
                rock += part;
                if !ours {
                    continue;
                }
                alone += 1;
                shared += part;
                let ring = (at.distance(pos).floor_int() / GROUND_CELL_M) as usize;
                if let Some(v) = rings.get_mut(ring) {
                    *v += part;
                }
            }
        }
        // 32 m cells: 1024 m^2, 0.1024 ha each.
        let per = Fx::ratio((GROUND_CELL_M * GROUND_CELL_M) as i64, 10_000);
        out.ground = shared * per;
        out.ground_alone = per * alone;
        out.rock = rock * per;
        out.rock_alone = per * rock_alone;
        for v in &mut rings {
            *v *= per;
        }

        // Ore, on the fine grid.
        let r = reach.ceil_int() / ORE_CELL_M + 1;
        let (cx, cy) = (
            pos.x.floor_int() / ORE_CELL_M,
            pos.y.floor_int() / ORE_CELL_M,
        );
        let (mut shared, mut alone) = (Fx::ZERO, 0i32);
        let per = Fx::ratio((ORE_CELL_M * ORE_CELL_M) as i64, 10_000);
        let mut by_field = Vec::new();
        for (index, f) in self.fields.iter().enumerate() {
            if cx + r < f.x0 || cx - r >= f.x0 + f.w || cy + r < f.y0 || cy - r >= f.y0 + f.h {
                continue;
            }
            let before = shared;
            for y in (cy - r).max(f.y0)..=(cy + r).min(f.y0 + f.h - 1) {
                for x in (cx - r).max(f.x0)..=(cx + r).min(f.x0 + f.w - 1) {
                    if !f.has(x, y)
                        || !self.is_land(
                            x * ORE_CELL_M / GROUND_CELL_M,
                            y * ORE_CELL_M / GROUND_CELL_M,
                        )
                    {
                        continue;
                    }
                    let at = FxVec2::new(cell_centre(x, ORE_CELL_M), cell_centre(y, ORE_CELL_M));
                    if at.distance_sq(pos) > reach_sq {
                        continue;
                    }
                    alone += 1;
                    shared += portion(at, pos, reach, others);
                }
            }
            if shared > before {
                by_field.push((index as u16, (shared - before) * per, f.centre, f.depth));
            }
        }
        out.ore = shared * per;
        out.ore_alone = per * alone;
        (out, by_field, rings)
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty() && self.land_w == 0
    }
}

/// Materials per second of a mine holding `rock_part` of its circle and working
/// this land and ore: that part of its shaft's `base`, and the land, ore land
/// being worth `per_hectare` instead of `ground`.
/// The share of its full output a mine makes when `powered` of its side's
/// upkeep is paid: [`UNPOWERED`] with no energy at all, rising to all of it.
pub fn mine_power(powered: Fx) -> Fx {
    let unpowered = Fx::ratio(UNPOWERED.0, UNPOWERED.1);
    unpowered + (Fx::ONE - unpowered) * powered.clamp(Fx::ZERO, Fx::ONE)
}

pub fn mine_rate(m: &mc_data::Mine, rock_part: Fx, ground: Fx, ore: Fx) -> Fx {
    m.base * rock_part + land_rate(m, ground, ore)
}

/// Ticks between blows of a core mine's hammer at `tech`. The heavier rigs strike
/// slower. Only the look and sound keep this beat: output is paid every tick.
pub fn hammer_ticks(tech: u8) -> u32 {
    match tech {
        0 | 1 => 28,
        2 => 32,
        3 => 36,
        _ => 48,
    }
}

/// A mine's hammer as the mirror publishes it in `UnitInstance::gait`: blows struck since
/// it was finished (wrapping at 256), and what one tick adds to that, twice. So a blow
/// lands where the count passes a whole number. The renderer drops the hammer and the
/// interface hears it by this, the way a walker's feet go down.
#[expect(
    clippy::float_arithmetic,
    clippy::disallowed_types,
    reason = "presentation: the mirror publishes it as UnitInstance::gait for the renderer and audio"
)]
pub fn hammer_gait(age: u32, tech: u8) -> [f32; 3] {
    let period = hammer_ticks(tech);
    let step = 1.0 / period as f32;
    [(age % (period * 256)) as f32 * step, step, step]
}

/// Materials per second from the land and ore alone, without the shaft's `base`.
pub fn land_rate(m: &mc_data::Mine, ground: Fx, ore: Fx) -> Fx {
    m.ground * ground + (m.per_hectare - m.ground).max(Fx::ZERO) * ore
}

impl World {
    /// Adds finished mines and drops dead ones, and when the mines standing
    /// (or their tiers) change, counts again what each one shares.
    pub(crate) fn run_mines(&mut self) {
        let units = &self.state.units;
        let mines = &mut self.state.mines.by_unit;
        mines.retain(|id, _| units.row(*id).is_some_and(|row| units.is_active(row)));
        for m in mines.values_mut() {
            m.age = m.age.saturating_add(1);
        }
        for row in units.slots.iter() {
            if units.is_active(row) && self.blueprints.unit(units.blueprint[row]).mine.is_some() {
                mines.entry(units.id(row)).or_default();
            }
        }
        let placed = self.mine_sites(None);
        let mut h = StateHasher::new();
        for &(id, pos, reach) in &placed {
            h.write_u64(id.0 as u64);
            h.write_i64(pos.x.0);
            h.write_i64(pos.y.0);
            h.write_i64(reach.0);
        }
        let layout = h.finish();
        if layout == self.state.mines.counted && !placed.is_empty() {
            return;
        }
        self.state.mines.counted = layout;
        let mut counts = Vec::with_capacity(placed.len());
        for (i, &(id, pos, reach)) in placed.iter().enumerate() {
            let (land, fields, rings) =
                self.ore
                    .share_with_rings(pos, reach, &neighbours(&placed, i, pos, reach));
            let veins = fields
                .into_iter()
                .map(|(field, ore, centre, depth)| Vein {
                    field,
                    ore,
                    reached_at: dig_ticks(pos, centre, depth),
                })
                .collect::<Vec<_>>();
            counts.push((id, land, veins, rings));
        }
        for (id, land, veins, rings) in counts {
            if let Some(m) = self.state.mines.by_unit.get_mut(&id) {
                m.land = land;
                m.veins = veins;
                m.rings = rings;
            }
        }
    }

    /// Every finished mine's id, position and reach, in id order; `except` left out.
    pub fn mine_sites(&self, except: Option<UnitId>) -> Vec<(UnitId, FxVec2, Fx)> {
        let units = &self.state.units;
        self.state
            .mines
            .by_unit
            .keys()
            .filter(|&&id| Some(id) != except)
            .filter_map(|&id| {
                let row = units.row(id)?;
                let pos = units.pos[row];
                let reach = self.bp(row).mine?.reach;
                Some((id, pos, reach))
            })
            .collect()
    }

    /// What a mine of `bp` placed at `pos` would get, shared with the mines
    /// standing now. For the AI.
    pub fn mine_share_at(&self, bp: &mc_data::UnitBlueprint, pos: FxVec2) -> Share {
        let Some(m) = bp.mine else {
            return Share::default();
        };
        let reach = m.reach;
        let others: Vec<(FxVec2, Fx)> = self
            .mine_sites(None)
            .into_iter()
            .filter(|&(_, p, r)| p.distance(pos) < reach + r)
            .map(|(_, p, r)| (p, r))
            .collect();
        self.ore.share(pos, reach, &others)
    }

    /// Mines' output for this tick: into `income` (per player, per tick) and each mine's flow.
    /// Adds every mine's output to its owner's mass income, scaled by `powered`,
    /// the share of its side's first-paid energy that is covered. Returns the
    /// mass per tick each side's mines fell short of their full output by.
    pub(crate) fn mine_income(&mut self, income: &mut [(Fx, Fx)], powered: &[Fx]) -> Vec<Fx> {
        let mut lost = vec![Fx::ZERO; income.len()];
        for (&id, m) in &self.state.mines.by_unit {
            let Some(row) = self.state.units.row(id) else {
                continue;
            };
            let Some(bp) = self.blueprints.unit(self.state.units.blueprint[row]).mine else {
                continue;
            };
            let p = self.state.units.owner[row] as usize;
            let full = m.rate(&bp) / DT;
            let made = full * mine_power(powered[p]);
            income[p].0 += made;
            lost[p] += full - made;
            if row >= self.flows.len() {
                self.flows.resize(row + 1, Default::default());
            }
            self.flows[row].made[0] += made;
        }
        lost
    }

    /// Material fabricators' output for this tick, into `income` and each one's flow:
    /// all of it with `powered` (its side's energy share) whole, none with no energy.
    pub(crate) fn fabricator_income(&mut self, income: &mut [(Fx, Fx)], powered: &[Fx]) {
        for row in self.state.units.slots.iter() {
            if !self.state.units.is_active(row) {
                continue;
            }
            let Some(f) = self.bp(row).fabricator else {
                continue;
            };
            let p = self.state.units.owner[row] as usize;
            let made = f.mass / DT * powered[p].clamp(Fx::ZERO, Fx::ONE);
            income[p].0 += made;
            if row >= self.flows.len() {
                self.flows.resize(row + 1, Default::default());
            }
            self.flows[row].made[0] += made;
        }
    }

    /// The nearest mine, finished or begun, that a new mine of `reach` at `pos`
    /// would crowd: one whose reach takes in `pos`, or that stands in the new
    /// mine's own. Mines keep out of each other's circles ([`mines_clear`]).
    /// `sides`: whose mines count, `None` everyone's.
    pub fn mine_in_the_way(
        &self,
        pos: FxVec2,
        reach: Fx,
        sides: Option<PlayerMask>,
    ) -> Option<usize> {
        let units = &self.state.units;
        units
            .slots
            .iter()
            .filter(|&row| sides.is_none_or(|m| m & player_bit(units.owner[row]) != 0))
            .filter_map(|row| {
                let other = self.bp(row).mine?.reach;
                let d = units.pos[row].distance_sq(pos);
                (!mines_clear(d, reach, other)).then_some((d, row))
            })
            .min()
            .map(|(_, row)| row)
    }
}

/// Whether two mines `distance_sq` apart, of reach `a` and `b`, keep out of
/// each other's circles: neither stands inside the other's reach. Their
/// territories may still overlap; the ground there is divided as ever.
pub fn mines_clear(distance_sq: Fx, a: Fx, b: Fx) -> bool {
    let r = a.max(b);
    distance_sq >= r * r
}

/// The mines in `placed` other than `i` whose reach overlaps its own.
fn neighbours(
    placed: &[(UnitId, FxVec2, Fx)],
    i: usize,
    pos: FxVec2,
    reach: Fx,
) -> Vec<(FxVec2, Fx)> {
    placed
        .iter()
        .enumerate()
        .filter(|&(j, &(_, p, r))| j != i && p.distance(pos) < reach + r)
        .map(|(_, &(_, p, r))| (p, r))
        .collect()
}

impl World {
    /// The middle of every ore field (the mean of its corners), in map order.
    pub fn ore_centres(&self) -> Vec<FxVec2> {
        self.map
            .ore
            .iter()
            .map(|r| {
                let n = r.points.len().max(1) as i32;
                let sum = r.points.iter().fold(FxVec2::ZERO, |a, p| a + *p);
                FxVec2::new(sum.x / n, sum.y / n)
            })
            .collect()
    }
}
