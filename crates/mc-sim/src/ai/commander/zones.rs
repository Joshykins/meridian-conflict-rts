//! Zones (`docs/AI_COMMANDER.md`, "Zones"): the whole map, land and water, split
//! once into regions that know their neighbours. Every side and the observer share
//! one zone map, so zone 12 is the same place to all of them. Operations pick
//! targets and gauge danger by zone, and what a side knows of the map is kept by
//! zone (`knowledge.rs`).
//!
//! The map depends on the terrain classes alone (the pathing grid never takes the
//! terrain's later edits), so it is the same on every machine and is rebuilt after a
//! snapshot is restored rather than saved. What changes (fire, enemy materials, what we
//! hold) is read each think (`ZoneView`).
use super::profile::Target;
use super::reach::NODE;
use super::world_model::WorldModel;
use crate::World;
use mc_core::{Fx, FxVec2};
use std::collections::VecDeque;

/// No zone.
pub(in crate::ai) const NONE: u16 = u16::MAX;
/// Seeds nearer than this to a zone already made on the same ground join it:
/// neighbouring ore fields are one zone.
const MERGE: i32 = 400;
/// Metres between the seeds laid over ground no start or ore field claims, so open
/// sea and bare land are split into zones of a few kilometres too.
const LATTICE: i32 = 1536;
/// A lattice seed is laid only this far from every other seed on its ground.
const LATTICE_CLEAR: i32 = 1000;
/// Ground left over without a seed (an island with no ore) gets a zone of its own
/// when it holds at least this many nodes; a smaller pocket belongs to no zone.
const MIN_POCKET: usize = 4;
/// Zones a map gets at most (a deliberate cap: on a map that would make more, the
/// last seeds are dropped and their ground goes to the zones nearest it).
const MOST_ZONES: usize = 1024;
/// Materials a mine point is worth to whoever holds the zone.
pub(in crate::ai) const ORE_VALUE: i32 = 250;
/// Nodes looked at to judge how much of a zone is in view: every other one each way.
pub(in crate::ai) const SAMPLE: i32 = 2;

/// What a zone's ground is: land units walk it, ships sail it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub(in crate::ai) enum Ground {
    #[default]
    Land = 0,
    Water = 1,
}

#[derive(Clone, Debug, Default)]
pub(in crate::ai) struct Zone {
    pub centre: FxVec2,
    pub ground: Ground,
    /// Ore fields in it.
    pub ore: u16,
    /// Nodes in it, and of those the ones sampled for how much is in view.
    pub nodes: u32,
    pub samples: u32,
    /// Zones on the same ground a unit can cross into, by id.
    pub adj: Vec<u16>,
    /// Zones on the other ground it touches: the coast between land and water.
    pub coast: Vec<u16>,
}

#[derive(Clone, Debug, Default)]
pub(in crate::ai) struct Zones {
    w: i32,
    h: i32,
    /// The zone of each `reach::NODE` square, `NONE` on cliffs and pockets.
    id: Vec<u16>,
    pub zones: Vec<Zone>,
    /// The zone of each player's start, by player.
    pub starts: Vec<u16>,
}

/// The ground of every node, and whether a unit crosses from one node to the next.
pub(in crate::ai) struct Grid<'a> {
    pub w: i32,
    pub h: i32,
    pub ground: &'a [Option<Ground>],
    /// Whether the edge between nodes `a` and `b` (neighbours on the same ground)
    /// can be crossed.
    pub crosses: &'a dyn Fn(usize, usize) -> bool,
}

impl Grid<'_> {
    fn centre(&self, i: usize) -> FxVec2 {
        let (x, y) = (i as i32 % self.w, i as i32 / self.w);
        FxVec2::from_ints(x * NODE + NODE / 2, y * NODE + NODE / 2)
    }

    /// The node of ground `g` nearest `p`, within `within` metres along either axis.
    fn near(&self, p: FxVec2, g: Ground, within: i32) -> Option<usize> {
        let (px, py) = (p.x.floor_int(), p.y.floor_int());
        let span = within / NODE + 1;
        let (cx, cy) = (px / NODE, py / NODE);
        (cy - span..=cy + span)
            .flat_map(|y| (cx - span..=cx + span).map(move |x| (x, y)))
            .filter(|&(x, y)| x >= 0 && y >= 0 && x < self.w && y < self.h)
            .map(|(x, y)| (y * self.w + x) as usize)
            .filter(|&i| self.ground[i] == Some(g))
            .map(|i| {
                let c = self.centre(i);
                let (dx, dy) = (
                    i64::from(c.x.floor_int() - px),
                    i64::from(c.y.floor_int() - py),
                );
                (dx * dx + dy * dy, i)
            })
            .filter(|&(d, _)| d <= i64::from(within) * i64::from(within) * 2)
            .min()
            .map(|(_, i)| i)
    }
}

impl Zones {
    /// Zones seeded at `starts`, then `ores`, then a lattice over the ground those
    /// leave (in that order, so ids are the same on every machine), grown over the
    /// nodes of their own ground a ring at a time.
    pub(in crate::ai) fn build(grid: &Grid, starts: &[FxVec2], ores: &[FxVec2]) -> Zones {
        let (w, h) = (grid.w, grid.h);
        let n = (w * h).max(0) as usize;
        let mut id = vec![NONE; n];
        let mut zones: Vec<Zone> = Vec::new();
        let mut queue = VecDeque::new();
        let mut seed = |at: usize, id: &mut Vec<u16>, zones: &mut Vec<Zone>| -> bool {
            let Some(g) = grid.ground[at] else {
                return false;
            };
            let c = grid.centre(at);
            if id[at] != NONE
                || zones.len() >= MOST_ZONES
                || zones
                    .iter()
                    .any(|z| z.ground == g && z.centre.distance(c) < Fx::from_int(MERGE))
            {
                return false;
            }
            id[at] = zones.len() as u16;
            zones.push(Zone {
                centre: c,
                ground: g,
                ..Zone::default()
            });
            queue.push_back(at);
            true
        };
        for p in starts.iter().chain(ores) {
            if let Some(at) = grid.near(*p, Ground::Land, 2 * NODE) {
                seed(at, &mut id, &mut zones);
            }
        }
        let mut y = LATTICE / 2;
        while y < h * NODE {
            let mut x = LATTICE / 2;
            while x < w * NODE {
                let p = FxVec2::from_ints(x, y);
                for g in [Ground::Land, Ground::Water] {
                    let Some(at) = grid.near(p, g, LATTICE / 4) else {
                        continue;
                    };
                    let c = grid.centre(at);
                    if !zones.iter().any(|z| {
                        z.ground == g && z.centre.distance(c) < Fx::from_int(LATTICE_CLEAR)
                    }) {
                        seed(at, &mut id, &mut zones);
                    }
                }
                x += LATTICE;
            }
            y += LATTICE;
        }
        let grow = |queue: &mut VecDeque<usize>, id: &mut Vec<u16>| {
            while let Some(i) = queue.pop_front() {
                let (x, y) = (i as i32 % w, i as i32 / w);
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w || ny >= h {
                        continue;
                    }
                    let j = (ny * w + nx) as usize;
                    if id[j] == NONE && grid.ground[j] == grid.ground[i] && (grid.crosses)(i, j) {
                        id[j] = id[i];
                        queue.push_back(j);
                    }
                }
            }
        };
        grow(&mut queue, &mut id);
        // Ground no seed reached: an island with no ore, a lake. Each piece big
        // enough is a zone of its own.
        for i in 0..n {
            if id[i] != NONE || grid.ground[i].is_none() || zones.len() >= MOST_ZONES {
                continue;
            }
            let mark = zones.len() as u16;
            id[i] = mark;
            let mut piece = vec![i];
            let mut k = 0;
            while k < piece.len() {
                let j = piece[k];
                k += 1;
                let (x, y) = (j as i32 % w, j as i32 / w);
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w || ny >= h {
                        continue;
                    }
                    let m = (ny * w + nx) as usize;
                    if id[m] == NONE && grid.ground[m] == grid.ground[j] && (grid.crosses)(j, m) {
                        id[m] = mark;
                        piece.push(m);
                    }
                }
            }
            if piece.len() >= MIN_POCKET {
                zones.push(Zone {
                    centre: grid.centre(piece[piece.len() / 2]),
                    ground: grid.ground[i].unwrap_or_default(),
                    ..Zone::default()
                });
            } else {
                // Too small to matter; `NONE + 1` marks it looked at, cleared below.
                for &j in &piece {
                    id[j] = NONE - 1;
                }
            }
        }
        for v in &mut id {
            if *v == NONE - 1 {
                *v = NONE;
            }
        }
        for i in 0..n {
            let a = id[i];
            if a == NONE {
                continue;
            }
            let (x, y) = (i as i32 % w, i as i32 / w);
            let z = &mut zones[a as usize];
            z.nodes += 1;
            if x % SAMPLE == 0 && y % SAMPLE == 0 {
                z.samples += 1;
            }
            for (dx, dy) in [(1, 0), (0, 1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= w || ny >= h {
                    continue;
                }
                let j = (ny * w + nx) as usize;
                let b = id[j];
                if b == NONE || b == a {
                    continue;
                }
                let same = grid.ground[i] == grid.ground[j];
                if same && !(grid.crosses)(i, j) {
                    continue;
                }
                for (p, q) in [(a, b), (b, a)] {
                    let list = if same {
                        &mut zones[p as usize].adj
                    } else {
                        &mut zones[p as usize].coast
                    };
                    if !list.contains(&q) {
                        list.push(q);
                    }
                }
            }
        }
        for z in &mut zones {
            z.adj.sort_unstable();
            z.coast.sort_unstable();
        }
        let mut zs = Zones {
            w,
            h,
            id,
            zones,
            starts: Vec::new(),
        };
        for o in ores {
            if let Some(z) = zs.of(*o) {
                zs.zones[z as usize].ore += 1;
            }
        }
        zs.starts = starts.iter().map(|s| zs.of(*s).unwrap_or(NONE)).collect();
        zs
    }

    /// The zone `p` stands in, or the one beside it when it is on a cliff or a
    /// pocket too small for a zone.
    pub(in crate::ai) fn of(&self, p: FxVec2) -> Option<u16> {
        let (x, y) = (p.x.floor_int() / NODE, p.y.floor_int() / NODE);
        (-1..=1)
            .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
            .filter_map(|(dx, dy)| {
                let (nx, ny) = (x + dx, y + dy);
                (nx >= 0 && ny >= 0 && nx < self.w && ny < self.h)
                    .then(|| self.id[(ny * self.w + nx) as usize])
                    .filter(|&z| z != NONE)
                    .map(|z| (dx.abs() + dy.abs(), dy, dx, z))
            })
            .min()
            .map(|(.., z)| z)
    }

    /// Nodes across and down, and the zone of each (`NONE` for none).
    pub(in crate::ai) fn grid(&self) -> (i32, i32, &[u16]) {
        (self.w, self.h, &self.id)
    }
}

/// What the side believes of each zone this think.
pub(in crate::ai) struct ZoneView {
    /// Land fire on the zone's centre.
    pub threat: Vec<Fx>,
    /// Enemy materials remembered in the zone (the commander counted heavily).
    pub enemy: Vec<Fx>,
    /// Materials of our structures standing in the zone: what a nuke would cost us there.
    pub held: Vec<Fx>,
}

impl ZoneView {
    /// Materials the zone is worth: its ore and what the enemy has standing in it.
    pub(in crate::ai) fn value(&self, zones: &Zones, z: u16) -> Fx {
        Fx::from_int(ORE_VALUE * i32::from(zones.zones[z as usize].ore)) + self.enemy[z as usize]
    }

    /// For each zone, the most land fire on any zone along the fewest-hops way to it
    /// from `from`, both ends included (the lower id first on ties); zero where no
    /// way leads.
    pub(in crate::ai) fn fire_along(&self, zones: &Zones, from: u16) -> Vec<Fx> {
        let mut along = vec![Fx::ZERO; zones.zones.len()];
        let mut reached = vec![false; zones.zones.len()];
        let Some(first) = along.get_mut(from as usize) else {
            return along;
        };
        *first = self.threat[from as usize];
        reached[from as usize] = true;
        let mut queue = VecDeque::from([from]);
        while let Some(z) = queue.pop_front() {
            for &a in &zones.zones[z as usize].adj {
                if !reached[a as usize] {
                    reached[a as usize] = true;
                    along[a as usize] = along[z as usize].max(self.threat[a as usize]);
                    queue.push_back(a);
                }
            }
        }
        along
    }
}

impl World {
    /// The match's zones, shared by every side: built from the terrain classes on
    /// first use.
    pub(in crate::ai) fn zones(&self) -> std::sync::Arc<Zones> {
        self.ai_shared
            .zones
            .get_or_init(|| std::sync::Arc::new(self.build_zones()))
            .clone()
    }

    fn build_zones(&self) -> Zones {
        let size = self.terrain.size_metres();
        let (w, h) = (size.x.floor_int() / NODE, size.y.floor_int() / NODE);
        let n = (w * h).max(0) as usize;
        let cell = |p: FxVec2| {
            let c = mc_path::Cell::from_pos(p);
            (c.x >= 0 && c.y >= 0).then_some((c.x as u32, c.y as u32))
        };
        let land = |p: FxVec2| cell(p).is_some_and(|c| self.nav.passable_terrain(c, c));
        let water = |p: FxVec2| cell(p).is_some_and(|c| self.nav.passable_naval_terrain(c, c));
        let centre = |i: usize| {
            FxVec2::from_ints(
                (i as i32 % w) * NODE + NODE / 2,
                (i as i32 / w) * NODE + NODE / 2,
            )
        };
        let ground: Vec<Option<Ground>> = (0..n)
            .map(|i| {
                let c = centre(i);
                if land(c) {
                    Some(Ground::Land)
                } else if water(c) {
                    Some(Ground::Water)
                } else {
                    None
                }
            })
            .collect();
        // A unit crosses to the next node when the ground halfway is its own too: a
        // cliff between two plateaus splits them.
        let crosses = |a: usize, b: usize| {
            let mid = centre(a).lerp(centre(b), Fx::HALF);
            match ground[a] {
                Some(Ground::Land) => land(mid),
                Some(Ground::Water) => water(mid),
                None => false,
            }
        };
        let grid = Grid {
            w,
            h,
            ground: &ground,
            crosses: &crosses,
        };
        let starts: Vec<FxVec2> = self.state.players.iter().map(|p| p.start).collect();
        Zones::build(&grid, &starts, &self.ore_centres())
    }

    /// The zone view for `player` from its world model and what it remembers.
    pub(in crate::ai) fn zone_view(&self, player: u8, zones: &Zones, wm: &WorldModel) -> ZoneView {
        let n = zones.zones.len();
        let ai = &self.state.ai[player as usize];
        let mut enemy = vec![Fx::ZERO; n];
        for c in &ai.contacts {
            if let Some(z) = zones.of(c.pos) {
                let bp = self.blueprints.unit(c.blueprint);
                enemy[z as usize] += if bp.has(mc_data::cat::COMMANDER) {
                    Fx::from_int(5000)
                } else {
                    bp.cost_materials
                };
            }
        }
        let mut held = vec![Fx::ZERO; n];
        let units = &self.state.units;
        for r in units.slots.iter() {
            if units.owner[r] == player && self.bp(r).is_structure() {
                if let Some(z) = zones.of(units.pos[r]) {
                    held[z as usize] += self.bp(r).cost_materials;
                }
            }
        }
        ZoneView {
            threat: zones
                .zones
                .iter()
                .map(|z| wm.threat_at(z.centre, Target::Land))
                .collect(),
            held,
            enemy,
        }
    }
}

#[cfg(test)]
mod tests;
