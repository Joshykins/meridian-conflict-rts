//! Ground that melts: a heat per square metre of terrain. Whatever burns the ground (a
//! bore's channel, a plasma strike, a titan's storm, a melting shell) adds heat to the
//! cells under it; heat stacks where hits overlap, so ground hit again and again melts
//! where one hit would only scorch it. Past `MELT` a cell is molten, and from then on
//! it is glass: the heat drains away over the source's cooling time, the glow dies, and
//! black glass is left where it ran, scorch round its edge. Glass stays for the match.
//!
//! Most hits only sear: one strike glows dull red, below the melt, and it takes a
//! stream of them, or several guns on one spot, to melt the ground. Stacking slows as
//! the ground nears white heat (`STACK`), so steady fire from one gun all but never
//! gets there; only the few sources that burn white-hot by themselves (`Burn::peak` at
//! 1, a star's beam) do.
//!
//! A wave rolling over the ground (a cone weapon's wake) sears it with `Sweep`s, bands
//! of an arc laid one behind another, each cell heated once, rather than discs laid
//! thick enough to cover the arc.
//!
//! Presentation only. The cells live in 32 m tiles made only where something heated
//! the ground, found through a hash table; both ride in scene set binding 32 (the
//! layout is `gpu_consts::melt_field`). scars.wgsl `melt_shade` draws it as part of
//! the ground, so overlapping burns are one surface, and grass_gen.wgsl grows nothing
//! on glass.

use crate::gpu::{Buffer, Gpu, GpuError};
use crate::gpu_consts::melt_field as mf;
use ash::vk;
use glam::Vec2;
use std::collections::HashMap;
use std::f32::consts::PI;

const TILE: usize = mf::TILE as usize;
const CELLS: usize = TILE * TILE;
const TILES: usize = mf::TILES as usize;
const SLOTS: usize = mf::SLOTS as usize;
/// Share of the heat already in a cell a new hit adds on top of its own, at cold
/// ground: ground under steady fire heats up past what one hit gives it. It falls off to
/// nothing as the cell nears white heat (1), so stacked hits approach it but never reach
/// it.
const STACK: f32 = 0.35;
/// How far past its radius a source reaches, in radii: its heat falls off to nothing
/// by its radius, its scorch by this (charred near, only seared far out).
const REACH: f32 = 1.9;
/// How far a source's edge wanders in and out, as a share of its radius.
const RAGGED: f32 = 0.25;
/// Share of its heating time a slow source shows nothing (the ground only warming).
const LATENT: f32 = 0.4;
/// How deep inside its own edge a sweep's band counts every cell, in radii (a disc's
/// middle is 0): its heat wanders a little under its peak across the band, as a field
/// of overlapping discs' does.
const SWEEP_CORE: f32 = 0.5;
/// Longest step the field cools in one go: a hitch must not cool it at once.
const MOST_STEP: f32 = 0.25;

/// What heats the ground: `peak` heat (1 white-hot) at `pos`, falling off to nothing by
/// `radius` metres. It reaches its heat over `rise` seconds from `start` (at once for
/// zero), and the heat it leaves drains over `cool` seconds.
#[derive(Clone, Copy, Debug)]
pub(super) struct Burn {
    pub(super) pos: Vec2,
    pub(super) radius: f32,
    pub(super) peak: f32,
    pub(super) start: f32,
    pub(super) rise: f32,
    pub(super) cool: f32,
}

/// A band of an arc swept over the ground at once (a wake's front as it passes): every
/// cell more than `inner` and at most `outer` metres from `apex`, within `half` radians
/// either side of `ahead`, heated to `peak`. It falls off over `edge` metres (a burn's
/// radius) past the arc's sides, and past `outer` too when it is `open` (the last band,
/// where the wave ran out; otherwise the next band takes over there). Its heat drains
/// over `cool` seconds from `start`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Sweep {
    pub(super) apex: Vec2,
    pub(super) ahead: Vec2,
    pub(super) half: f32,
    pub(super) inner: f32,
    pub(super) outer: f32,
    pub(super) open: bool,
    pub(super) edge: f32,
    pub(super) peak: f32,
    pub(super) start: f32,
    pub(super) cool: f32,
}

impl Sweep {
    /// Metres past its sides (and past `outer`, open) it reaches: its scorch's reach, its
    /// edge wandering out.
    fn reach(&self) -> f32 {
        self.edge * REACH / (1.0 - RAGGED)
    }

    /// Metres out from the apex the furthest cell it heats stands.
    fn furthest(&self) -> f32 {
        if self.open {
            self.outer + self.reach()
        } else {
            self.outer
        }
    }

    /// The corners of the box round the band and what it reaches past its sides.
    fn bounds(&self) -> (Vec2, Vec2) {
        let outer = self.furthest();
        let wide = (self.half + self.reach() / self.inner.max(1.0)).min(PI);
        if wide >= PI {
            return (self.apex - outer, self.apex + outer);
        }
        let mut lo = Vec2::splat(f32::MAX);
        let mut hi = Vec2::splat(f32::MIN);
        let mut take = |p: Vec2| {
            lo = lo.min(p);
            hi = hi.max(p);
        };
        for side in [-wide, wide] {
            let out = Vec2::from_angle(side).rotate(self.ahead);
            take(self.apex + out * self.inner);
            take(self.apex + out * outer);
        }
        // The arc bulges furthest out where it crosses an axis.
        for axis in [Vec2::X, Vec2::Y, -Vec2::X, -Vec2::Y] {
            if axis.angle_to(self.ahead).abs() <= wide {
                take(self.apex + axis * outer);
            }
        }
        (lo, hi)
    }

    /// Metres outside the band a point `d` from the apex stands: past the arc's side
    /// (`side`, its direction in the frame along `ahead`) while that is nearest, else from
    /// the apex (behind it), and past `outer`, open.
    fn outside(&self, d: Vec2, dist: f32, side: Vec2) -> f32 {
        // In the frame along `ahead`, folded onto one side.
        let p = Vec2::new(d.dot(self.ahead), d.perp_dot(self.ahead).abs());
        let past = side.x * p.y - side.y * p.x;
        let across = if past <= 0.0 || p.dot(side) >= 0.0 {
            past
        } else {
            dist
        };
        let beyond = if self.open { dist - self.outer } else { 0.0 };
        Vec2::new(across.max(0.0), beyond.max(0.0)).length()
    }
}

#[derive(Clone, Copy, Default)]
struct Cell {
    heat: f32,
    /// Seconds a heat of 1 takes to drain here.
    cool: f32,
    glass: f32,
    scorch: f32,
}

impl Cell {
    fn packed(&self) -> u32 {
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
        byte(self.heat / mf::HEAT_MAX) | byte(self.glass) << 8 | byte(self.scorch) << 16
    }
}

struct Tile {
    at: (i32, i32),
    cells: Box<[Cell]>,
    /// Some cell still has heat to lose.
    hot: bool,
    /// Changed since its last upload.
    dirty: bool,
    /// When it was last heated.
    touched: f32,
}

/// The table key of the tile at `at`, and the entry its lookup starts at. bindings.wgsl
/// `melt_slot` is the same.
fn key(at: (i32, i32)) -> u32 {
    (at.0 as u32 & 0xFFFF) << 16 | (at.1 as u32 & 0xFFFF)
}

fn first_entry(key: u32) -> usize {
    (key.wrapping_mul(0x9E37_79B1) >> (32 - mf::SLOT_BITS)) as usize
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Smooth value noise, 0 to 1, over cells `scale` metres across.
fn noise(p: Vec2, scale: f32, seed: u32) -> f32 {
    let hash = |x: i32, y: i32| {
        let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
            ^ (y as u32).wrapping_mul(0xD816_3841)
            ^ seed.wrapping_mul(0xCB1A_B31F);
        h ^= h >> 13;
        h = h.wrapping_mul(0x5BD1_E995);
        h ^= h >> 15;
        (h & 0xFFFF) as f32 / 65535.0
    };
    let q = p / scale;
    let (i, f) = (q.floor(), q - q.floor());
    let u = f * f * (Vec2::splat(3.0) - 2.0 * f);
    let (x, y) = (i.x as i32, i.y as i32);
    let a = hash(x, y) + (hash(x + 1, y) - hash(x, y)) * u.x;
    let b = hash(x, y + 1) + (hash(x + 1, y + 1) - hash(x, y + 1)) * u.x;
    a + (b - a) * u.y
}

pub(super) struct GroundMelt {
    buffer: Buffer,
    tiles: Vec<Tile>,
    index: HashMap<(i32, i32), usize>,
    /// Burns still to come or still heating, each with its seed.
    burns: Vec<(Burn, u32)>,
    /// Sweeps still to come, each with its seed.
    sweeps: Vec<(Sweep, u32)>,
    table_dirty: bool,
    clock: Option<f32>,
    seed: u32,
}

impl GroundMelt {
    pub(super) fn new(gpu: &Gpu) -> Result<GroundMelt, GpuError> {
        let buffer = gpu.host_buffer(
            ((mf::ATLAS as usize + TILES * CELLS) * 4) as u64,
            vk::BufferUsageFlags::STORAGE_BUFFER,
        )?;
        buffer.write(0, &vec![0u8; mf::ATLAS as usize * 4]);
        let mut melt = GroundMelt::empty();
        melt.buffer = buffer;
        Ok(melt)
    }

    fn empty() -> GroundMelt {
        GroundMelt {
            buffer: Buffer::null(),
            tiles: Vec::new(),
            index: HashMap::new(),
            burns: Vec::new(),
            sweeps: Vec::new(),
            table_dirty: false,
            clock: None,
            seed: 0x1234_5677,
        }
    }

    /// What the scene set's binding 32 reads.
    pub(super) fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Frees the field. The renderer calls this from its `Drop`, after the device has gone
    /// idle.
    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        gpu.destroy_buffer(std::mem::replace(&mut self.buffer, Buffer::null()));
    }

    /// A new world: the ground is whole again.
    pub(super) fn clear(&mut self) {
        self.tiles.clear();
        self.index.clear();
        self.burns.clear();
        self.sweeps.clear();
        self.table_dirty = true;
        self.clock = None;
    }

    /// Heats the ground with `burn`.
    pub(super) fn burn(&mut self, burn: Burn) {
        self.seed = self.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        self.burns.push((burn, self.seed >> 8));
    }

    /// Sears the ground with `sweep` when its time comes.
    pub(super) fn sweep(&mut self, sweep: Sweep) {
        self.seed = self.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        self.sweeps.push((sweep, self.seed >> 8));
    }

    /// A pool of ground heated at once to `peak` (1 white-hot): `radius` metres at `pos`,
    /// its heat draining over `cool` seconds from `start`.
    pub(super) fn melt(&mut self, pos: Vec2, radius: f32, peak: f32, start: f32, cool: f32) {
        self.burn(Burn {
            pos,
            radius,
            peak,
            start,
            rise: 0.0,
            cool,
        });
    }

    /// Moves the field on to `time`: the hot cells cool, the burns due heat the ground.
    pub(super) fn step(&mut self, time: f32) {
        let dt = match self.clock {
            Some(last) if time >= last => (time - last).min(MOST_STEP),
            // The clock went back: a new world, or a replay sought back.
            Some(_) => {
                self.clear();
                0.0
            }
            None => 0.0,
        };
        self.clock = Some(time);
        if dt > 0.0 {
            let _t = mc_core::perf_span!("melt.cool");
            for tile in self.tiles.iter_mut().filter(|t| t.hot) {
                mc_core::perf_count!("melt.hot_tiles");
                let mut hot = false;
                for c in tile.cells.iter_mut().filter(|c| c.heat > 0.0) {
                    c.heat = (c.heat - dt / c.cool.max(0.5)).max(0.0);
                    hot |= c.heat > 0.0;
                }
                tile.hot = hot;
                tile.dirty = true;
            }
        }
        let burns = mc_core::perf_span!("melt.burns");
        let mut i = 0;
        while i < self.burns.len() {
            let (burn, seed) = self.burns[i];
            let since = time - burn.start;
            if since < 0.0 {
                i += 1;
                continue;
            }
            let done = since >= burn.rise;
            if burn.rise > 0.0 {
                // A slow burn shows nothing at first, then heats up to its peak, the
                // ground scorching, then glowing, then melting as it goes.
                let k = ((since / burn.rise - LATENT) / (1.0 - LATENT)).min(1.0);
                if k > 0.0 {
                    self.heat(&burn, seed, k * k * (3.0 - 2.0 * k), 0.0, time);
                }
            } else {
                self.heat(&burn, seed, 1.0, STACK, time);
            }
            if done {
                self.burns.swap_remove(i);
            } else {
                i += 1;
            }
        }
        drop(burns);
        let _t = mc_core::perf_span!("melt.sweeps");
        let mut i = 0;
        while i < self.sweeps.len() {
            let (sweep, seed) = self.sweeps[i];
            if sweep.start <= time {
                self.heat_sweep(&sweep, seed, time);
                self.sweeps.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// Sears the cells of `sweep`'s band, each once: row by row, only the cells of the
    /// ring it lies in.
    fn heat_sweep(&mut self, sweep: &Sweep, seed: u32, time: f32) {
        let r = sweep.edge.max(mf::CELL);
        let reach = sweep.reach();
        let sear = (sweep.peak * 1.6).min(1.0);
        let (inner, outer) = (sweep.inner.max(0.0), sweep.furthest());
        let (lo, hi) = sweep.bounds();
        let side = Vec2::from_angle(sweep.half.min(PI));
        // The cells whose middles lie from `a` to `b` metres.
        let first = |a: f32| (a / mf::CELL - 0.5).ceil() as i32;
        let after = |b: f32| (b / mf::CELL - 0.5).floor() as i32;
        let mut last: Option<((i32, i32), usize)> = None;
        for cy in first(lo.y)..=after(hi.y) {
            let y = (cy as f32 + 0.5) * mf::CELL - sweep.apex.y;
            if y.abs() > outer {
                continue;
            }
            // The row crosses the ring in one run, or in two either side of its hole.
            let xo = (outer * outer - y * y).sqrt();
            let xi = (inner * inner - y * y).max(0.0).sqrt();
            let runs = if xi > 0.0 {
                [(-xo, -xi), (xi, xo)]
            } else {
                [(-xo, xo), (1.0, 0.0)]
            };
            for (a, b) in runs {
                let from = first((sweep.apex.x + a).max(lo.x));
                let to = after((sweep.apex.x + b).min(hi.x));
                for cx in from..=to {
                    let at = (Vec2::new(cx as f32, cy as f32) + 0.5) * mf::CELL;
                    let d = at - sweep.apex;
                    let dist = d.length();
                    if dist <= inner || dist > outer {
                        continue;
                    }
                    let outside = sweep.outside(d, dist, side);
                    if outside >= reach {
                        continue;
                    }
                    let wander = noise(at, r * 0.6, seed) * 0.7 + noise(at, 2.5, seed ^ 0x55) * 0.3;
                    let d = (SWEEP_CORE + outside / r) * (1.0 - RAGGED + 2.0 * RAGGED * wander);
                    if d >= REACH {
                        continue;
                    }
                    let target = sweep.peak * (1.0 - smoothstep(0.4, 1.0, d));
                    let scorch = sear * (1.0 - smoothstep(0.6, REACH, d));
                    if target < 0.002 && scorch < 0.004 {
                        continue;
                    }
                    let t = (cx.div_euclid(TILE as i32), cy.div_euclid(TILE as i32));
                    let slot = match last {
                        Some((at, slot)) if at == t && self.tiles[slot].at == t => slot,
                        _ => self.tile(t, time),
                    };
                    last = Some((t, slot));
                    let i = cy.rem_euclid(TILE as i32) as usize * TILE
                        + cx.rem_euclid(TILE as i32) as usize;
                    self.warm(slot, i, target, scorch, STACK, sweep.cool, time);
                }
            }
        }
    }

    /// Raises cell `i` of the tile in `slot` towards `target` heat, adding `stack` of the
    /// heat it already had (less the nearer it is to white heat), and its scorch to
    /// `scorch`.
    #[expect(
        clippy::too_many_arguments,
        reason = "one cell's share of a burn or sweep"
    )]
    fn warm(
        &mut self,
        slot: usize,
        i: usize,
        target: f32,
        scorch: f32,
        stack: f32,
        cool: f32,
        time: f32,
    ) {
        let tile = &mut self.tiles[slot];
        let c = &mut tile.cells[i];
        let headroom = (1.0 - c.heat).max(0.0);
        let heat = (c.heat.max(target) + stack * c.heat.min(target) * headroom).min(mf::HEAT_MAX);
        if heat > c.heat {
            c.cool = if c.heat > 0.0 { c.cool.max(cool) } else { cool };
            c.heat = heat;
            tile.hot = true;
        }
        // How far it melted: barely fused crust just past the melt point, smooth glass
        // where it ran white-hot.
        c.glass = c.glass.max(smoothstep(mf::MELT, 1.0, c.heat));
        c.scorch = c.scorch.max(scorch);
        tile.dirty = true;
        tile.touched = time;
    }

    /// Raises the cells under `burn` to `share` of its heat, adding `stack` of the heat a
    /// cell already had (less the nearer it is to white heat).
    fn heat(&mut self, burn: &Burn, seed: u32, share: f32, stack: f32, time: f32) {
        let r = burn.radius.max(mf::CELL);
        // The edge wanders by `RAGGED`, so the reach does too.
        let reach = r * REACH / (1.0 - RAGGED);
        let lo = ((burn.pos - reach) / mf::CELL).floor();
        let hi = ((burn.pos + reach) / mf::CELL).floor();
        let (lo, hi) = ((lo.x as i32, lo.y as i32), (hi.x as i32, hi.y as i32));
        let tile_of = |c: i32| c.div_euclid(TILE as i32);
        let peak = burn.peak * share;
        // Scorch spreads round a burn hot enough to sear, more round a hotter one.
        let sear = (peak * 1.6).min(1.0);
        for ty in tile_of(lo.1)..=tile_of(hi.1) {
            for tx in tile_of(lo.0)..=tile_of(hi.0) {
                let mut slot = None;
                let base = (tx * TILE as i32, ty * TILE as i32);
                for cy in lo.1.max(base.1)..=hi.1.min(base.1 + TILE as i32 - 1) {
                    for cx in lo.0.max(base.0)..=hi.0.min(base.0 + TILE as i32 - 1) {
                        let at = (Vec2::new(cx as f32, cy as f32) + 0.5) * mf::CELL;
                        let wander =
                            noise(at, r * 0.6, seed) * 0.7 + noise(at, 2.5, seed ^ 0x55) * 0.3;
                        let d = at.distance(burn.pos) / r * (1.0 - RAGGED + 2.0 * RAGGED * wander);
                        if d >= REACH {
                            continue;
                        }
                        let target = peak * (1.0 - smoothstep(0.4, 1.0, d));
                        let scorch = sear * (1.0 - smoothstep(0.6, REACH, d));
                        if target < 0.002 && scorch < 0.004 {
                            continue;
                        }
                        let s = *slot.get_or_insert_with(|| self.tile((tx, ty), time));
                        let i = (cy - base.1) as usize * TILE + (cx - base.0) as usize;
                        self.warm(s, i, target, scorch, stack, burn.cool, time);
                    }
                }
            }
        }
    }

    /// The tile at `at`, made if there is none. When all are in use the coldest, longest
    /// untouched tile is given up (a deliberate cap: old glass far back in a long match
    /// goes first).
    fn tile(&mut self, at: (i32, i32), time: f32) -> usize {
        if let Some(&slot) = self.index.get(&at) {
            return slot;
        }
        let fresh = || Tile {
            at,
            cells: vec![Cell::default(); CELLS].into_boxed_slice(),
            hot: false,
            dirty: true,
            touched: time,
        };
        let slot = if self.tiles.len() < TILES {
            self.tiles.push(fresh());
            self.tiles.len() - 1
        } else {
            let slot = (0..self.tiles.len())
                .min_by(|&a, &b| {
                    let (a, b) = (&self.tiles[a], &self.tiles[b]);
                    a.hot.cmp(&b.hot).then(a.touched.total_cmp(&b.touched))
                })
                .unwrap_or(0);
            self.index.remove(&self.tiles[slot].at);
            self.tiles[slot] = fresh();
            slot
        };
        self.index.insert(at, slot);
        self.table_dirty = true;
        slot
    }

    /// The hash table as the GPU reads it: two words an entry.
    fn table(&self) -> Vec<u32> {
        let mut table = vec![0u32; SLOTS * 2];
        for (slot, tile) in self.tiles.iter().enumerate() {
            let key = key(tile.at);
            let mut e = first_entry(key);
            // A tile whose entries are all taken is left out of sight (a deliberate cap:
            // at a quarter full a run of `PROBES` taken entries all but never happens).
            for _ in 0..mf::PROBES {
                if table[e * 2 + 1] == 0 {
                    table[e * 2] = key;
                    table[e * 2 + 1] = slot as u32 + 1;
                    break;
                }
                e = (e + 1) % SLOTS;
            }
        }
        table
    }

    /// Writes what changed this frame.
    pub(super) fn upload(&mut self) {
        let _t = mc_core::perf_span!("melt.upload");
        if self.table_dirty {
            let header = [self.tiles.len() as u32, 0, 0, 0];
            self.buffer.write(0, bytemuck::cast_slice(&header));
            self.buffer
                .write(mf::TABLE as u64 * 4, bytemuck::cast_slice(&self.table()));
            self.table_dirty = false;
        }
        let mut words = vec![0u32; CELLS];
        for (slot, tile) in self.tiles.iter_mut().enumerate() {
            if !tile.dirty {
                continue;
            }
            mc_core::perf_count!("melt.uploaded_tiles");
            for (w, c) in words.iter_mut().zip(tile.cells.iter()) {
                *w = c.packed();
            }
            let at = (mf::ATLAS as usize + slot * CELLS) * 4;
            self.buffer.write(at as u64, bytemuck::cast_slice(&words));
            tile.dirty = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(m: &GroundMelt, at: Vec2) -> Cell {
        let c = (at / mf::CELL).floor();
        let (cx, cy) = (c.x as i32, c.y as i32);
        let t = (cx.div_euclid(TILE as i32), cy.div_euclid(TILE as i32));
        m.index.get(&t).map_or(Cell::default(), |&s| {
            m.tiles[s].cells
                [cy.rem_euclid(TILE as i32) as usize * TILE + cx.rem_euclid(TILE as i32) as usize]
        })
    }

    fn hit(m: &mut GroundMelt, at: Vec2, peak: f32, time: f32) {
        m.burn(Burn {
            pos: at,
            radius: 4.0,
            peak,
            start: time,
            rise: 0.0,
            cool: 10.0,
        });
    }

    /// One hit too weak to melt the ground only scorches it; the same hit again and again
    /// heats it up until it melts, but never white-hot, and it is glass once it has cooled.
    #[test]
    fn heat_stacks_until_the_ground_melts_and_glass_stays() {
        let mut m = GroundMelt::empty();
        let at = Vec2::new(100.5, -40.5);
        hit(&mut m, at, 0.3, 0.0);
        m.step(0.0);
        let once = cell(&m, at);
        assert!(once.heat > 0.25 && once.heat < mf::MELT, "{}", once.heat);
        assert_eq!(once.glass, 0.0);
        assert!(once.scorch > 0.3);
        let mut t = 0.0;
        while cell(&m, at).heat < mf::MELT + 0.1 && t < 5.0 {
            t += 0.1;
            hit(&mut m, at, 0.3, t);
            m.step(t);
        }
        assert!(t < 5.0, "steady fire never melted the ground");
        // However long it goes on, one gun's fire does not run the ground white-hot.
        while t < 30.0 {
            t += 0.1;
            hit(&mut m, at, 0.3, t);
            m.step(t);
        }
        let held = cell(&m, at);
        assert!(held.heat > 0.6 && held.heat < 0.97, "{}", held.heat);
        let glass = held.glass;
        assert!(glass > 0.3, "{glass}");
        while t < 60.0 {
            t += 0.2;
            m.step(t);
        }
        let cold = cell(&m, at);
        assert_eq!(cold.heat, 0.0);
        assert_eq!(cold.glass, glass);
        assert!(m.tiles.iter().all(|t| !t.hot));
        // Well outside the burn nothing happened.
        assert_eq!(cell(&m, at + Vec2::new(12.0, 0.0)).scorch, 0.0);
    }

    /// A slow burn shows nothing at first, then heats the ground up to its peak.
    #[test]
    fn a_slow_burn_heats_up_over_its_rise() {
        let mut m = GroundMelt::empty();
        let at = Vec2::new(5.5, 5.5);
        m.burn(Burn {
            pos: at,
            radius: 6.0,
            peak: 1.0,
            start: 1.0,
            rise: 4.0,
            cool: 30.0,
        });
        m.step(0.0);
        m.step(2.0);
        assert_eq!(cell(&m, at).heat, 0.0, "still latent");
        m.step(3.5);
        let mid = cell(&m, at).heat;
        assert!(mid > 0.05 && mid < 0.95, "{mid}");
        m.step(5.0);
        assert!(cell(&m, at).heat > 0.95);
        assert!(m.burns.is_empty());
    }

    /// Past its last tile the field gives up the coldest, longest untouched one, and the
    /// table finds every tile it keeps where the shader looks for it.
    #[test]
    fn tiles_are_found_and_the_oldest_cold_one_goes_first() {
        let mut m = GroundMelt::empty();
        for i in 0..TILES as i32 + 1 {
            m.tile((i % 40 - 20, i / 40 - 10), i as f32);
        }
        assert_eq!(m.tiles.len(), TILES);
        assert!(!m.index.contains_key(&(-20, -10)), "the oldest went");
        let table = m.table();
        for (slot, tile) in m.tiles.iter().enumerate() {
            let key = key(tile.at);
            let mut e = first_entry(key);
            let found = (0..mf::PROBES).any(|_| {
                let hit = table[e * 2] == key && table[e * 2 + 1] == slot as u32 + 1;
                e = (e + 1) % SLOTS;
                hit
            });
            assert!(found, "tile {:?} not found", tile.at);
        }
    }
    fn band(apex: Vec2, half: f32, r: f32, open: bool) -> Sweep {
        Sweep {
            apex,
            ahead: Vec2::new(0.6, 0.8),
            half,
            inner: r - 3.0,
            outer: r + 3.0,
            open,
            edge: 7.0,
            peak: 0.16,
            start: 0.0,
            cool: 1.5,
        }
    }

    /// Bands laid one behind another sear the whole arc between them, seamless, under
    /// the peak and never melting it; past the arc's sides and past the last band the
    /// scorch fades out, and behind the apex nothing happens.
    #[test]
    fn sweeps_sear_their_arc_once_and_fade_past_its_edges() {
        let mut m = GroundMelt::empty();
        let apex = Vec2::new(-200.3, 1000.7);
        let half = 0.25;
        let mut r = 4.0;
        while r < 300.0 {
            m.sweep(band(apex, half, r, r + 6.0 >= 300.0));
            r += 6.0;
        }
        m.step(0.0);
        let ahead = Vec2::new(0.6, 0.8);
        let at = |a: f32, out: f32| apex + Vec2::from_angle(a).rotate(ahead) * out;
        for out in (10..=295).step_by(5) {
            for a in [-0.8, -0.4, 0.0, 0.4, 0.8] {
                let c = cell(&m, at(half * a, out as f32));
                assert!(c.heat > 0.09 && c.heat <= 0.16, "{out} {a}: {}", c.heat);
                assert!(c.scorch > 0.2, "{out} {a}: {}", c.scorch);
                assert_eq!(c.glass, 0.0);
            }
        }
        // Past a side the heat goes first, then the scorch.
        let side = |past: f32| cell(&m, at(half + past / 200.0, 200.0));
        assert!(side(5.0).heat < 0.02 && side(5.0).scorch > 0.0);
        assert_eq!(side(20.0).scorch, 0.0);
        // Past the last band, as past a side.
        assert!(cell(&m, at(0.0, 308.0)).heat < 0.02);
        assert_eq!(cell(&m, at(0.0, 325.0)).scorch, 0.0);
        assert_eq!(cell(&m, at(PI, 20.0)).scorch, 0.0);
    }

    /// A band right round the apex (a ring) sears all round, nothing inside it.
    #[test]
    fn a_full_turn_sweep_sears_a_ring() {
        let mut m = GroundMelt::empty();
        m.sweep(band(Vec2::new(40.0, 40.0), PI, 100.0, false));
        m.step(0.0);
        for k in 0..16 {
            let p = Vec2::new(40.0, 40.0) + Vec2::from_angle(k as f32 * PI / 8.0) * 100.0;
            assert!(cell(&m, p).heat > 0.1, "{k}");
        }
        assert_eq!(cell(&m, Vec2::new(40.0, 40.0)).scorch, 0.0);
        assert_eq!(cell(&m, Vec2::new(40.0, 120.0)).scorch, 0.0);
    }
}
