//! City structures (`mc_map::city`): the blocks, towers and the wall a city map
//! is built of, which stop shots, take damage, burn and come down.
//!
//! - One row of [`Structures`] (state) per map prop whose kind has health
//!   (`mc_map::city::structure`); its health is the kind's, times the square of
//!   the prop's scale, less the wear the map gives it. Pre-match wear is old
//!   damage: nothing starts on fire.
//! - [`CityShapes`] (derived from the map, never state) holds each structure's
//!   solid parts as boxes in the world, from the ground (no floor: the terrain
//!   check meets anything under it first) up to each part's top, and a grid of
//!   64 m buckets listing them, with the tallest top in each bucket.
//! - Shots (`impacts.rs`) and lines of fire (`line_of_fire.rs`) both ask
//!   [`CityShapes::first_hit`], so a gun holds fire exactly when its shell would
//!   meet a wall. A segment that starts inside a part passes out of it: a
//!   muzzle standing in the edge of a block's cell does not shoot itself.
//! - Damage: [`World::damage_structure`]. Below [`BURN_AT_PERMILLE`] a structure
//!   that `burns` catches fire, and the fire takes it down to [`GUTTED_PERMILLE`]
//!   and goes out; it never burns down by itself. A fire weapon lights it at once.
//!   At zero it comes down: its prop's `props_dead` bit is set, its cells open
//!   for walking, and [`SimEvent::StructureCollapsed`] tells the renderer.

use crate::mirror::SimEvent;
use crate::World;
use mc_core::{Fx, FxVec2, FxVec3, StateHasher, TICKS_PER_SECOND};
use mc_map::{Heightfield, Prop};
use serde::{Deserialize, Serialize};

/// Flag: on fire.
pub const BURNING: u8 = 1;
/// Flag: a fire burned out in it; it does not catch again.
pub const GUTTED: u8 = 2;
/// Flag: it came down. Its prop is dead.
pub const DOWN: u8 = 4;
/// Flag: listed in [`Structures::touched`].
const LISTED: u8 = 8;
const KNOWN_FLAGS: u8 = BURNING | GUTTED | DOWN | LISTED;

/// Below this share of its health (thousandths) a structure that burns catches fire.
pub const BURN_AT_PERMILLE: i64 = 550;
/// A fire burns a structure down to this share (thousandths), then goes out:
/// gutted, still standing, a shell for the guns to finish.
pub const GUTTED_PERMILLE: i64 = 200;
/// Seconds a fire takes from [`BURN_AT_PERMILLE`] to [`GUTTED_PERMILLE`].
const FIRE_SECONDS: i64 = 90;

/// Share of a direct hit's damage a structure takes from a shot with no splash:
/// slugs, bolts and rail rounds punch holes in a wall rather than bring it down.
/// A Rhino-class tank (26 a shot, 1.3 s) on its own takes two minutes and more
/// over a 1 400-point house; a T2 rail tank a little over half a minute.
pub(crate) const KINETIC_SHARE: Fx = Fx::ratio(1, 2);
/// Share of a blast's damage each structure in its reach takes: shells, bombs
/// and rockets with splash are what knocks blocks down. A T1 howitzer shell
/// (120) takes a twelfth of a house; a T3 howitzer's (4 467 over 48 m) levels
/// every house round where it lands and a skyscraper in ten.
pub(crate) const BLAST_SHARE: Fx = Fx::ONE;
/// Share of a nuclear front's damage a structure takes. Twice a hull's: steel
/// and glass towers go down where armour only buckles, so a warhead flattens
/// everything out to about 300 m (a skyscraper's 45 000 at about the 40 000
/// the front still carries there) and guts what stands beyond.
pub(crate) const NUCLEAR_SHARE: Fx = Fx::from_int(2);

/// How a blast falls on the structures it reaches (`World::blast_structures`).
#[derive(Clone, Copy)]
pub(crate) struct Blow {
    /// Share of the damage they take (`BLAST_SHARE`, `NUCLEAR_SHARE`).
    pub share: Fx,
    /// A fire weapon's: it lights what burns.
    pub ignite: bool,
    /// A dome between holds it off (a warhead's front goes through).
    pub domes: bool,
}

impl Blow {
    /// A shell's, a bomb's or a blast's: the whole of it, held by domes.
    pub(crate) const BLAST: Blow = Blow {
        share: BLAST_SHARE,
        ignite: false,
        domes: true,
    };
}

/// Metres a bucket of the grid is across.
const BUCKET_M: i32 = 64;
/// Slack round each part's bucket cover, so a segment along a bucket's edge
/// still finds what straddles it.
const BUCKET_SLACK: Fx = Fx::ONE;
/// Steps along an axis shorter than this (a millimetre) count as running
/// parallel to it: no division by a near-zero step.
const PARALLEL: Fx = Fx(64);

/// The city's structures, as game state: one row per structure, in prop order.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Structures {
    /// The map prop each row stands for, ascending.
    pub prop: Vec<u32>,
    /// Health left, whole points.
    pub health: Vec<i32>,
    /// Health whole: the kind's times the square of the prop's scale.
    pub max: Vec<i32>,
    /// `BURNING`, `GUTTED`, `DOWN`.
    pub flags: Vec<u8>,
    /// The tick its flags last changed: it caught fire, went out or came down.
    pub since: Vec<u32>,
    /// Rows that are not whole, in the order they were first hurt (worn ones
    /// first). Every row not listed is whole and not burning.
    pub touched: Vec<u32>,
}

impl Structures {
    pub fn len(&self) -> usize {
        self.prop.len()
    }

    pub fn is_empty(&self) -> bool {
        self.prop.is_empty()
    }

    /// Whether `row` has come down.
    pub fn is_down(&self, row: usize) -> bool {
        self.flags[row] & DOWN != 0
    }

    /// Health left, 0 (down) to 255 (whole).
    pub fn health_byte(&self, row: usize) -> u8 {
        let max = self.max[row].max(1) as i64;
        (self.health[row].max(0) as i64 * 255 / max) as u8
    }

    fn touch(&mut self, row: usize) {
        if self.flags[row] & LISTED == 0 {
            self.flags[row] |= LISTED;
            self.touched.push(row as u32);
        }
    }

    /// The table for `props`: one row per structure that can be hit, worn as the map says.
    fn new(props: &[Prop]) -> Structures {
        let mut s = Structures::default();
        for (i, p) in props.iter().enumerate() {
            let Some(kind) = mc_map::city::structure(p.kind).filter(|k| k.health > 0) else {
                continue;
            };
            let scale = p.scale_milli as i64;
            let max = (kind.health as i64 * scale * scale / 1_000_000).clamp(1, i32::MAX as i64);
            // A map's worst wear leaves it standing on its last point: what it
            // wants knocked down it lays as rubble.
            let worn = (max * p.wear_milli.min(1000) as i64 / 1000).min(max - 1);
            let row = s.prop.len();
            s.prop.push(i as u32);
            s.max.push(max as i32);
            s.health.push((max - worn) as i32);
            s.flags.push(0);
            s.since.push(0);
            if worn > 0 {
                s.touch(row);
            }
        }
        s
    }

    pub(crate) fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.prop.len() as u64 | (self.touched.len() as u64) << 32);
        for &row in &self.touched {
            let r = row as usize;
            h.write_u64(row as u64 | (self.flags[r] as u64) << 32);
            h.write_u64(self.health[r] as u32 as u64 | (self.since[r] as u64) << 32);
        }
    }

    /// The table is whole and self-consistent: every column as long as the
    /// others, health within its whole, known flags, the touched list naming
    /// each hurt row once and only those. Whether it is this map's is checked
    /// against the map's own table (`World::restore`).
    pub(crate) fn validate(&self) -> Result<(), String> {
        let n = self.prop.len();
        if [
            self.health.len(),
            self.max.len(),
            self.flags.len(),
            self.since.len(),
        ]
        .iter()
        .any(|&l| l != n)
            || self.touched.len() > n
        {
            return Err("city: columns of different lengths".into());
        }
        let mut listed = vec![false; n];
        for &row in &self.touched {
            let r = row as usize;
            if r >= n || listed[r] || self.flags[r] & LISTED == 0 {
                return Err(format!("city: touched row {row} out of place"));
            }
            listed[r] = true;
        }
        for (r, &listed) in listed.iter().enumerate() {
            let (health, max, flags) = (self.health[r], self.max[r], self.flags[r]);
            let down = flags & DOWN != 0;
            if max <= 0
                || health > max
                || health < 0
                || flags & !KNOWN_FLAGS != 0
                || down != (health == 0)
                || (flags & LISTED != 0) != listed
                || (!listed && (health != max || flags != 0))
            {
                return Err(format!("city: row {r} is inconsistent"));
            }
        }
        Ok(())
    }
}

/// One solid part of a structure, placed in the world.
#[derive(Clone, Copy)]
struct Part {
    mid: FxVec2,
    cos: Fx,
    sin: Fx,
    half: FxVec2,
    /// Its top, metres over the datum.
    top: Fx,
}

impl Part {
    /// `p` in the part's own frame, from its middle.
    fn local(&self, p: FxVec2) -> FxVec2 {
        let d = p - self.mid;
        FxVec2::new(
            d.x * self.cos + d.y * self.sin,
            d.y * self.cos - d.x * self.sin,
        )
    }

    fn turn(&self, d: FxVec2) -> FxVec2 {
        FxVec2::new(
            d.x * self.cos + d.y * self.sin,
            d.y * self.cos - d.x * self.sin,
        )
    }

    /// Back from the part's frame into the world.
    fn world(&self, l: FxVec2) -> FxVec2 {
        self.mid
            + FxVec2::new(
                l.x * self.cos - l.y * self.sin,
                l.x * self.sin + l.y * self.cos,
            )
    }

    /// Share of the step `from + d * t`, `t` in 0..1, at which it enters the
    /// part; `None` if it misses it or starts inside it.
    fn entry(&self, from: FxVec3, d: FxVec3) -> Option<Fx> {
        let l = self.local(from.xy());
        let ld = self.turn(d.xy());
        if l.x.abs() <= self.half.x && l.y.abs() <= self.half.y && from.z <= self.top {
            return None;
        }
        let (mut enter, mut leave) = (Fx::ZERO, Fx::ONE);
        slab(l.x, ld.x, self.half.x, &mut enter, &mut leave)?;
        slab(l.y, ld.y, self.half.y, &mut enter, &mut leave)?;
        if d.z.abs() <= PARALLEL {
            if from.z > self.top {
                return None;
            }
        } else {
            let t = (self.top - from.z) / d.z;
            if d.z > Fx::ZERO {
                leave = leave.min(t);
            } else {
                enter = enter.max(t);
            }
        }
        (enter <= leave).then_some(enter)
    }

    /// The point of the part's footprint nearest `p`, and how far it is.
    fn nearest(&self, p: FxVec2) -> (FxVec2, Fx) {
        let l = self.local(p);
        let c = FxVec2::new(
            l.x.clamp(-self.half.x, self.half.x),
            l.y.clamp(-self.half.y, self.half.y),
        );
        (self.world(c), (l - c).length())
    }
}

/// Narrows `enter..leave` to where `p + d * t` lies within `-h..=h`.
fn slab(p: Fx, d: Fx, h: Fx, enter: &mut Fx, leave: &mut Fx) -> Option<()> {
    if d.abs() <= PARALLEL {
        return (p.abs() <= h).then_some(());
    }
    let (a, b) = ((-h - p) / d, (h - p) / d);
    *enter = (*enter).max(a.min(b));
    *leave = (*leave).min(a.max(b));
    (*enter <= *leave).then_some(())
}

/// Where the city's structures stand, built once from the map. Not state.
#[derive(Default)]
pub struct CityShapes {
    parts: Vec<Part>,
    /// Row `r`'s parts are `parts[first[r]..first[r + 1]]`.
    first: Vec<u32>,
    /// Each row's footprint middle at the ground, the reach of its footprint
    /// from there, and its tallest top over that ground.
    middle: Vec<FxVec3>,
    reach: Vec<Fx>,
    height: Vec<Fx>,
    /// Buckets across and down.
    dims: (i32, i32),
    /// Bucket `b` lists `rows[start[b]..start[b + 1]]`.
    start: Vec<u32>,
    rows: Vec<u32>,
    /// The tallest top in each bucket; `Fx::MIN` for an empty one.
    bucket_top: Vec<Fx>,
    /// The tallest top of all.
    highest: Fx,
    /// Map props that block cells and are not city structures (`lots::prop_cells`):
    /// what a collapse must leave blocked where it overlaps them.
    pub(crate) other_solids: Vec<u32>,
}

impl CityShapes {
    pub(crate) fn new(table: &Structures, props: &[Prop], terrain: &Heightfield) -> CityShapes {
        let size = terrain.size_metres();
        let dims = (
            (size.x.ceil_int() / BUCKET_M).max(1),
            (size.y.ceil_int() / BUCKET_M).max(1),
        );
        let mut c = CityShapes {
            dims,
            highest: Fx::MIN,
            ..Default::default()
        };
        let mut cover: Vec<Vec<u32>> = vec![Vec::new(); (dims.0 * dims.1) as usize];
        c.bucket_top = vec![Fx::MIN; cover.len()];
        for (row, &prop) in table.prop.iter().enumerate() {
            let p = &props[prop as usize];
            let kind = mc_map::city::structure(p.kind).expect("a structure row is a city kind");
            let scale = Fx::from_int(p.scale_milli as i32) / 1000;
            let ground = terrain.height_at(p.pos);
            let (sin, cos) = (p.heading.sin(), p.heading.cos());
            c.first.push(c.parts.len() as u32);
            let (mut lo, mut hi) = (p.pos, p.pos);
            let mut tallest = Fx::ZERO;
            for (&(cx, cy, hx, hy), &top) in kind.plan.iter().zip(kind.tops) {
                let (cx, cy) = (Fx::from_int(cx) * scale, Fx::from_int(cy) * scale);
                let half = FxVec2::new(Fx::from_int(hx) * scale, Fx::from_int(hy) * scale);
                let mid = FxVec2::new(p.pos.x + cx * cos - cy * sin, p.pos.y + cx * sin + cy * cos);
                let top = Fx::from_int(top) * scale;
                tallest = tallest.max(top);
                let part = Part {
                    mid,
                    cos,
                    sin,
                    half,
                    top: ground + top,
                };
                // The square round the part, whichever way it is turned.
                let r = half.x.abs() + half.y.abs() + BUCKET_SLACK;
                lo = FxVec2::new(lo.x.min(mid.x - r), lo.y.min(mid.y - r));
                hi = FxVec2::new(hi.x.max(mid.x + r), hi.y.max(mid.y + r));
                let (bx0, by0) = c.bucket_of(mid - FxVec2::new(r, r));
                let (bx1, by1) = c.bucket_of(mid + FxVec2::new(r, r));
                for by in by0..=by1 {
                    for bx in bx0..=bx1 {
                        let b = (by * dims.0 + bx) as usize;
                        if cover[b].last() != Some(&(row as u32)) {
                            cover[b].push(row as u32);
                        }
                        c.bucket_top[b] = c.bucket_top[b].max(part.top);
                    }
                }
                c.highest = c.highest.max(part.top);
                c.parts.push(part);
            }
            let middle = (lo + hi) * Fx::HALF;
            c.middle.push(middle.extend(ground));
            c.reach.push((hi - middle).length());
            c.height.push(tallest);
        }
        c.first.push(c.parts.len() as u32);
        c.start.push(0);
        for list in &cover {
            // A part listed twice in one bucket by another part of its row.
            let mut list = list.clone();
            list.dedup();
            c.rows.extend(list);
            c.start.push(c.rows.len() as u32);
        }
        c.other_solids = props
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                !p.kind.is_city() && (p.kind.is_building() || !p.kind.solid_plan().is_empty())
            })
            .map(|(i, _)| i as u32)
            .collect();
        c
    }

    fn bucket_of(&self, p: FxVec2) -> (i32, i32) {
        (
            (p.x.floor_int() / BUCKET_M).clamp(0, self.dims.0 - 1),
            (p.y.floor_int() / BUCKET_M).clamp(0, self.dims.1 - 1),
        )
    }

    /// The tallest city structure's top an aircraft at `p` heading along
    /// `dir` (a unit vector) must clear within `reach` metres ahead: the
    /// buckets round its course. `None` where the city has nothing. Tops of
    /// fallen structures still count: an aircraft keeps its height over a
    /// ruined district.
    pub(crate) fn roof_ahead(&self, p: FxVec2, dir: FxVec2, reach: Fx) -> Option<Fx> {
        if self.bucket_top.is_empty() || self.highest == Fx::MIN {
            return None;
        }
        let step = Fx::from_int(BUCKET_M);
        let steps = (reach / step).ceil_int().clamp(0, 16);
        let mut roof = Fx::MIN;
        for k in 0..=steps {
            let (bx, by) = self.bucket_of(p + dir * (step * k));
            for y in (by - 1).max(0)..=(by + 1).min(self.dims.1 - 1) {
                for x in (bx - 1).max(0)..=(bx + 1).min(self.dims.0 - 1) {
                    roof = roof.max(self.bucket_top[(y * self.dims.0 + x) as usize]);
                }
            }
        }
        (roof > Fx::MIN).then_some(roof)
    }

    fn parts_of(&self, row: usize) -> &[Part] {
        &self.parts[self.first[row] as usize..self.first[row + 1] as usize]
    }

    /// Row `row`'s footprint middle at the ground, the reach of its footprint
    /// from there, and its tallest top over that ground.
    pub(crate) fn outline(&self, row: usize) -> (FxVec3, Fx, Fx) {
        (self.middle[row], self.reach[row], self.height[row])
    }

    /// The first standing structure the step `from` to `to` runs into: the share
    /// of the step at which it enters it, and its row. Shots and lines of fire
    /// both ask this. The buckets are walked a column at a time from `from`'s
    /// end, so a hit near the start ends the walk.
    pub(crate) fn first_hit(
        &self,
        live: &Structures,
        from: FxVec3,
        to: FxVec3,
    ) -> Option<(Fx, usize)> {
        if self.rows.is_empty() || from.z.min(to.z) > self.highest {
            return None;
        }
        mc_core::perf_count!("city.sweeps");
        let d = to - from;
        let cell = Fx::from_int(BUCKET_M);
        let (x0, x1) = (from.x.min(to.x), from.x.max(to.x));
        let (y0, y1) = (from.y.min(to.y), from.y.max(to.y));
        let w = Fx::from_int(self.dims.0 * BUCKET_M);
        let h = Fx::from_int(self.dims.1 * BUCKET_M);
        if x1 < Fx::ZERO || y1 < Fx::ZERO || x0 >= w || y0 >= h {
            return None;
        }
        let (cx0, _) = self.bucket_of(FxVec2::new(x0, Fx::ZERO));
        let (cx1, _) = self.bucket_of(FxVec2::new(x1, Fx::ZERO));
        let mut best: Option<(Fx, usize)> = None;
        let columns = cx1 - cx0 + 1;
        for k in 0..columns {
            let cx = if d.x >= Fx::ZERO { cx0 + k } else { cx1 - k };
            // The stretch of the step over this column.
            let (ta, tb) = if d.x.abs() > PARALLEL {
                let xa = x0.max(cell * cx);
                let xb = x1.min(cell * (cx + 1));
                let (a, b) = ((xa - from.x) / d.x, (xb - from.x) / d.x);
                (
                    a.min(b).clamp(Fx::ZERO, Fx::ONE),
                    a.max(b).clamp(Fx::ZERO, Fx::ONE),
                )
            } else {
                (Fx::ZERO, Fx::ONE)
            };
            if best.is_some_and(|(t, _)| t < ta) {
                break;
            }
            let (ya, yb) = (from.y + d.y * ta, from.y + d.y * tb);
            let low = (from.z + d.z * ta).min(from.z + d.z * tb);
            let (_, cy0) = self.bucket_of(FxVec2::new(Fx::ZERO, ya.min(yb)));
            let (_, cy1) = self.bucket_of(FxVec2::new(Fx::ZERO, ya.max(yb)));
            for cy in cy0..=cy1 {
                let b = (cy * self.dims.0 + cx) as usize;
                if self.bucket_top[b] < low {
                    continue;
                }
                for &row in &self.rows[self.start[b] as usize..self.start[b + 1] as usize] {
                    let row = row as usize;
                    if live.is_down(row) {
                        continue;
                    }
                    mc_core::perf_count!("city.rows");
                    for part in self.parts_of(row) {
                        if part.top < low {
                            continue;
                        }
                        if let Some(t) = part.entry(from, d) {
                            if best.is_none_or(|(bt, br)| (t, row) < (bt, br)) {
                                best = Some((t, row));
                            }
                        }
                    }
                }
            }
        }
        best
    }

    /// The standing structures with a part within `reach` of `at` (its top no
    /// more than `reach` under it): each row, the point of it nearest `at`, and
    /// how far that is. In row order.
    pub(crate) fn near(
        &self,
        live: &Structures,
        at: FxVec3,
        reach: Fx,
    ) -> Vec<(usize, FxVec3, Fx)> {
        let mut found = Vec::new();
        if self.rows.is_empty() || at.z - reach > self.highest || reach < Fx::ZERO {
            return found;
        }
        let (bx0, by0) = self.bucket_of(at.xy() - FxVec2::new(reach, reach));
        let (bx1, by1) = self.bucket_of(at.xy() + FxVec2::new(reach, reach));
        let mut rows = Vec::new();
        for by in by0..=by1 {
            for bx in bx0..=bx1 {
                let b = (by * self.dims.0 + bx) as usize;
                if self.bucket_top[b] < at.z - reach {
                    continue;
                }
                rows.extend_from_slice(
                    &self.rows[self.start[b] as usize..self.start[b + 1] as usize],
                );
            }
        }
        rows.sort_unstable();
        rows.dedup();
        for row in rows {
            let row = row as usize;
            if live.is_down(row) {
                continue;
            }
            let mut closest: Option<(FxVec3, Fx)> = None;
            for part in self.parts_of(row) {
                let (xy, flat) = part.nearest(at.xy());
                let z = at.z.min(part.top);
                let rise = (at.z - part.top).max(Fx::ZERO);
                let dist = if rise > Fx::ZERO {
                    FxVec2::new(flat, rise).length()
                } else {
                    flat
                };
                if dist <= reach && closest.is_none_or(|(_, d)| dist < d) {
                    closest = Some((xy.extend(z), dist));
                }
            }
            if let Some((point, dist)) = closest {
                found.push((row, point, dist));
            }
        }
        found
    }
}

/// What a city row is built from: the state table and the map's shapes.
pub(crate) fn build(props: &[Prop], terrain: &Heightfield) -> (Structures, CityShapes) {
    let table = Structures::new(props);
    let shapes = CityShapes::new(&table, props, terrain);
    (table, shapes)
}

impl World {
    /// `damage` (weapon points, before any share) to structure `row`; `ignite`
    /// sets it alight at once if it burns. A standing structure only.
    pub(crate) fn damage_structure(&mut self, row: usize, damage: Fx, ignite: bool) {
        let tick = self.state.tick;
        let s = &mut self.state.city;
        if s.is_down(row) || damage <= Fx::ZERO {
            return;
        }
        let points = damage.min(Fx::from_int(i32::MAX / 2)).round_int().max(1);
        s.touch(row);
        s.health[row] = (s.health[row] - points).max(0);
        if s.health[row] == 0 {
            self.collapse_structure(row);
            return;
        }
        let max = s.max[row] as i64;
        let burns = mc_map::city::structure(self.map.props[s.prop[row] as usize].kind)
            .is_some_and(|k| k.burns);
        let low = (s.health[row] as i64) * 1000 < max * BURN_AT_PERMILLE;
        if burns && s.flags[row] & (BURNING | GUTTED) == 0 && (low || ignite) {
            s.flags[row] |= BURNING;
            s.since[row] = tick;
            let (middle, _, height) = self.city_shapes.outline(row);
            self.events.push(SimEvent::StructureAlight {
                prop: s.prop[row],
                pos: middle + FxVec3::new(Fx::ZERO, Fx::ZERO, height / 2),
            });
        }
    }

    /// Structure `row` comes down: its prop is dead, its cells open (save where
    /// a neighbour still stands on them), and the renderer is told.
    fn collapse_structure(&mut self, row: usize) {
        let tick = self.state.tick;
        let s = &mut self.state.city;
        s.health[row] = 0;
        s.flags[row] = (s.flags[row] & !BURNING) | DOWN;
        s.since[row] = tick;
        let prop = s.prop[row] as usize;
        self.state.props_dead[prop / 64] |= 1 << (prop % 64);
        let (middle, reach, height) = self.city_shapes.outline(row);
        self.open_prop_cells(prop, middle, reach);
        self.events.push(SimEvent::StructureCollapsed {
            prop: prop as u32,
            pos: middle,
            radius: reach,
            height,
        });
        self.add_stain(middle.xy(), reach, 120);
    }

    /// A blast of `reach` at `at` over the structures round it: each takes
    /// `damage(distance)` times `blow.share`, unless a dome stands between (and
    /// `blow.domes` says domes hold it). `blow.ignite`: a fire weapon's, which
    /// lights what burns.
    pub(crate) fn blast_structures(
        &mut self,
        at: FxVec3,
        reach: Fx,
        blow: Blow,
        damage: impl Fn(Fx) -> Fx,
    ) {
        let near = self.city_shapes.near(&self.state.city, at, reach);
        let domes = blow.domes && !self.scratch.shielded.is_empty();
        let (share, ignite) = (blow.share, blow.ignite);
        for (row, point, dist) in near {
            if domes && self.blast_blocker(at, point, None).is_some() {
                continue;
            }
            self.damage_structure(row, damage(dist) * share, ignite);
        }
    }

    /// Fires drain the structures they burn, once a second, down to gutted.
    pub(crate) fn run_city_fires(&mut self) {
        if !self.state.tick.is_multiple_of(TICKS_PER_SECOND) {
            return;
        }
        let tick = self.state.tick;
        let s = &mut self.state.city;
        for i in 0..s.touched.len() {
            let row = s.touched[i] as usize;
            if s.flags[row] & BURNING == 0 {
                continue;
            }
            let max = s.max[row] as i64;
            let floor = (max * GUTTED_PERMILLE / 1000) as i32;
            let drain =
                ((max * (BURN_AT_PERMILLE - GUTTED_PERMILLE)) / (1000 * FIRE_SECONDS)).max(1);
            s.health[row] = (s.health[row] - drain as i32).max(floor.min(s.health[row]));
            if s.health[row] <= floor {
                s.flags[row] = (s.flags[row] & !BURNING) | GUTTED;
                s.since[row] = tick;
            }
        }
    }
}
