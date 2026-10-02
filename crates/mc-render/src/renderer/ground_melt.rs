//! Ground that melts: a heat per square metre of terrain. Whatever burns the ground (a
//! bore's channel, a plasma strike, a titan's storm, a melting shell) adds heat to the
//! cells under it; heat stacks where hits overlap, so ground hit again and again melts
//! where one hit would only scorch it. Past `MELT` a cell is molten, and from then on
//! it is glass: the heat drains away over the source's cooling time, the glow dies, and
//! black glass is left where it ran, scorch round its edge. Glass stays for the match.
//!
//! Presentation only. The cells live in 32 m tiles made only where something heated
//! the ground, found through a hash table; both ride in scene set binding 32 (the
//! layout is `gpu_consts::melt_field`). terrain.wgsl `melt_shade` draws it as part of
//! the ground, so overlapping burns are one surface, and grass_gen.wgsl grows nothing
//! on glass.

use crate::gpu::{Buffer, Gpu, GpuError};
use crate::gpu_consts::melt_field as mf;
use ash::vk;
use glam::Vec2;
use std::collections::HashMap;

const TILE: usize = mf::TILE as usize;
const CELLS: usize = TILE * TILE;
const TILES: usize = mf::TILES as usize;
const SLOTS: usize = mf::SLOTS as usize;
/// Share of the heat already in a cell a new hit adds on top of its own: ground under
/// steady fire heats up past what one hit gives it.
const STACK: f32 = 0.2;
/// How far past its radius a source reaches, in radii: its heat falls off to nothing
/// by its radius, its scorch by this.
const REACH: f32 = 1.3;
/// How far a source's edge wanders in and out, as a share of its radius.
const RAGGED: f32 = 0.25;
/// Share of its heating time a slow source shows nothing (the ground only warming).
const LATENT: f32 = 0.4;
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
        self.table_dirty = true;
        self.clock = None;
    }

    /// Heats the ground with `burn`.
    pub(super) fn burn(&mut self, burn: Burn) {
        self.seed = self.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        self.burns.push((burn, self.seed >> 8));
    }

    /// A pool of ground melted white-hot at once: `radius` metres at `pos`, its heat
    /// draining over `cool` seconds from `start`.
    pub(super) fn melt(&mut self, pos: Vec2, radius: f32, start: f32, cool: f32) {
        self.burn(Burn {
            pos,
            radius,
            peak: 1.0,
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
            for tile in self.tiles.iter_mut().filter(|t| t.hot) {
                let mut hot = false;
                for c in tile.cells.iter_mut().filter(|c| c.heat > 0.0) {
                    c.heat = (c.heat - dt / c.cool.max(0.5)).max(0.0);
                    hot |= c.heat > 0.0;
                }
                tile.hot = hot;
                tile.dirty = true;
            }
        }
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
    }

    /// Raises the cells under `burn` to `share` of its heat, adding `stack` of the heat a
    /// cell already had.
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
                        let scorch = sear * (1.0 - smoothstep(0.8, REACH, d));
                        if target < 0.002 && scorch < 0.004 {
                            continue;
                        }
                        let s = *slot.get_or_insert_with(|| self.tile((tx, ty), time));
                        let tile = &mut self.tiles[s];
                        let c =
                            &mut tile.cells[(cy - base.1) as usize * TILE + (cx - base.0) as usize];
                        let heat =
                            (c.heat.max(target) + stack * c.heat.min(target)).min(mf::HEAT_MAX);
                        if heat > c.heat {
                            c.cool = if c.heat > 0.0 {
                                c.cool.max(burn.cool)
                            } else {
                                burn.cool
                            };
                            c.heat = heat;
                            tile.hot = true;
                        }
                        c.glass = c.glass.max(smoothstep(mf::MELT, mf::MELT + 0.15, c.heat));
                        c.scorch = c.scorch.max(scorch);
                        tile.dirty = true;
                        tile.touched = time;
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
    /// heats it up until it melts, and it is glass once it has cooled.
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
        while cell(&m, at).glass < 1.0 && t < 5.0 {
            t += 0.1;
            hit(&mut m, at, 0.3, t);
            m.step(t);
        }
        assert!(t < 5.0, "steady fire never melted the ground");
        while t < 60.0 {
            t += 0.2;
            m.step(t);
        }
        let cold = cell(&m, at);
        assert_eq!(cold.heat, 0.0);
        assert_eq!(cold.glass, 1.0);
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
}
