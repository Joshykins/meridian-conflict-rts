//! Rock on the canyon's cliffs. The 8 m heightfield draws a wall as a smooth sheet a
//! few vertices across; here, once per map, every steep wall is dressed in jointed
//! sandstone pieces (mc-models `cliffs.rs`) stepped back to its slope, so it shows
//! blocks, ledges and a broken outline, coloured by the wall's own beds (rock.wgsl).
//! Pieces outside the desert are hidden (`CliffRocks::set_look`). Drawn as static
//! props after the map's own, which the sim knows nothing of: scenery only, on
//! ground nothing can walk.

use glam::Vec2;
use mc_map::{MapFile, CELL_SIZE_M};
use mc_models::cliffs;
use mc_sim::mirror::{UnitInstance, KIND_PROP};

/// Candidate spots are this far apart, metres, each jittered within its cell.
const SPACING: f32 = 12.0;
/// The least slope (rise over run) that is a cliff: about 53 degrees. Mountainsides
/// less steep stay the terrain's own rock.
const STEEP: f32 = 1.35;
/// A wall goes on up (or down) while the slope stays over this.
const WALL: f32 = 0.55;
/// Walls lower than this, metres top to foot, are left as they are.
const LEAST_DROP: f32 = 10.0;
/// Glacier ice over this (0..255 in the snow layer) is ice, not rock.
const ICE: u8 = 64;

/// The map's heights in metres, with the slope worked out over a few cells so
/// one cell's facet does not turn a piece.
struct Ground {
    heights: Vec<f32>,
    stride: usize,
    size: Vec2,
}

impl Ground {
    fn load(map: &MapFile) -> Option<Self> {
        let field = mc_map::Heightfield::load(map).ok()?;
        let info = map.info();
        let (w, _) = field.size_cells();
        let (min_z, step) = (info.min_z.to_f32(), info.z_step.to_f32());
        let size = field.size_metres().to_f32();
        Some(Self {
            heights: field
                .samples()
                .iter()
                .map(|&s| min_z + s as f32 * step)
                .collect(),
            stride: w as usize + 1,
            size: Vec2::from(size),
        })
    }

    fn height(&self, p: Vec2) -> f32 {
        let cell = CELL_SIZE_M as f32;
        let rows = self.heights.len() / self.stride;
        let q = (p / cell).clamp(
            Vec2::ZERO,
            Vec2::new(self.stride as f32 - 1.001, rows as f32 - 1.001),
        );
        let (x, y) = (q.x as usize, q.y as usize);
        let f = q - q.floor();
        let at = |x: usize, y: usize| self.heights[y * self.stride + x];
        at(x, y) * (1.0 - f.x) * (1.0 - f.y)
            + at(x + 1, y) * f.x * (1.0 - f.y)
            + at(x, y + 1) * (1.0 - f.x) * f.y
            + at(x + 1, y + 1) * f.x * f.y
    }

    /// Rise over run, uphill.
    fn gradient(&self, p: Vec2) -> Vec2 {
        let r = 1.5 * CELL_SIZE_M as f32;
        Vec2::new(
            self.height(p + Vec2::X * r) - self.height(p - Vec2::X * r),
            self.height(p + Vec2::Y * r) - self.height(p - Vec2::Y * r),
        ) / (2.0 * r)
    }

    /// How far the wall through `p` rises over it and falls under it, metres:
    /// walked up and down its fall line while it stays steep.
    fn wall_extent(&self, p: Vec2, down: Vec2) -> (f32, f32) {
        let z = self.height(p);
        let walk = |dir: Vec2| {
            let mut last = z;
            for k in 1..120 {
                let q = p + dir * (3.0 * k as f32);
                if q.cmplt(Vec2::ZERO).any() || q.cmpge(self.size).any() {
                    break;
                }
                last = self.height(q);
                if self.gradient(q).length() < WALL {
                    break;
                }
            }
            last
        };
        (walk(-down) - z, z - walk(down))
    }
}

/// A hash of a spot and a salt, in 0..1.
fn roll(x: u32, y: u32, salt: u32) -> f32 {
    let mut h =
        x.wrapping_mul(0x8DA6_B343) ^ y.wrapping_mul(0xD816_3841) ^ salt.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Rock pieces for every cliff on `map`. `first_model` is the first cliff piece's
/// model slot (`cliffs::KEYS` in order), `first_id` the first id past the map's props.
pub(super) fn cliff_rocks(map: &MapFile, first_model: u32, first_id: u32) -> Vec<UnitInstance> {
    let Some(ground) = Ground::load(map) else {
        return Vec::new();
    };
    let info = map.info();
    let water = info.water_level.to_f32();
    let snow = map.snow();
    let (snow_w, _) = info.snow_dims();
    // The snow layer's samples span the map, edge to edge.
    let snow_step = ground.size.x / (snow_w.max(2) - 1) as f32;
    let iced = |p: Vec2| {
        snow.is_some_and(|s| {
            let stride = snow_step;
            let (x, y) = (
                (p.x / stride).round() as usize,
                (p.y / stride).round() as usize,
            );
            s.get(2 * (y * snow_w as usize + x))
                .is_some_and(|&ice| ice > ICE)
        })
    };
    let (nx, ny) = (
        (ground.size.x / SPACING) as u32,
        (ground.size.y / SPACING) as u32,
    );
    // Rows in bands across the cores; each band's pieces in row order, so the
    // list is the same however many threads there are.
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()) as u32;
    let band = ny.div_ceil(threads.max(1)).max(1);
    let bands: Vec<Vec<UnitInstance>> = std::thread::scope(|s| {
        let jobs: Vec<_> = (0..ny)
            .step_by(band as usize)
            .map(|y0| {
                let ground = &ground;
                let iced = &iced;
                s.spawn(move || {
                    let mut out = Vec::new();
                    for gy in y0..(y0 + band).min(ny) {
                        for gx in 0..nx {
                            let jitter = Vec2::new(roll(gx, gy, 1), roll(gx, gy, 2));
                            let p =
                                (Vec2::new(gx as f32, gy as f32) + 0.1 + jitter * 0.8) * SPACING;
                            if let Some(piece) = place(ground, p, water, (gx, gy)) {
                                if !iced(p) {
                                    out.push(piece);
                                }
                            }
                        }
                    }
                    out
                })
            })
            .collect();
        jobs.into_iter()
            .map(|j| j.join().unwrap_or_default())
            .collect()
    });
    let mut out: Vec<UnitInstance> = bands.into_iter().flatten().collect();
    for (i, piece) in out.iter_mut().enumerate() {
        piece.blueprint += first_model;
        piece.unit_id = first_id + i as u32;
    }
    out
}

/// The piece for spot `p`, if it is on a cliff. Its `blueprint` is the piece's
/// index among `cliffs::KEYS`.
fn place(ground: &Ground, p: Vec2, water: f32, (gx, gy): (u32, u32)) -> Option<UnitInstance> {
    let g = ground.gradient(p);
    let grade = g.length();
    if grade < STEEP {
        return None;
    }
    let z = ground.height(p);
    if z < water - 2.0 {
        return None;
    }
    let down = -g / grade;
    let (above, below) = ground.wall_extent(p, down);
    if above + below < LEAST_DROP {
        return None;
    }
    // As tall as the wall has room for either side, so pieces near the rim stay
    // small and break it up, and the great faces get great blocks.
    let room = above.min(below) * 1.15 + 3.0;
    let stretch = 0.8 + 0.5 * roll(gx, gy, 3);
    let scale = (room / (cliffs::HALF_HEIGHT * stretch)).min(1.7) * (0.8 + 0.35 * roll(gx, gy, 4));
    if scale < 0.22 {
        return None;
    }
    // Upright, its courses stepping back as steeply as the wall leans: run over
    // rise, in the piece's own frame, where its height is stretched.
    let want = stretch / grade;
    let lean = (0..cliffs::LEANS.len())
        .min_by(|&a, &b| {
            (cliffs::LEANS[a] - want)
                .abs()
                .total_cmp(&(cliffs::LEANS[b] - want).abs())
        })
        .unwrap_or(0);
    let heading = down.y.atan2(down.x) + (roll(gx, gy, 5) - 0.5) * 0.25;
    let seed = (roll(gx, gy, 6) * cliffs::SEEDS as f32) as usize % cliffs::SEEDS;
    let model = (lean * cliffs::SEEDS + seed) as u32;
    let pos = [p.x, p.y, z];
    Some(UnitInstance {
        prev_pos: pos,
        prev_heading: heading,
        pos,
        heading,
        blueprint: model,
        owner_flags: KIND_PROP,
        health: 1.0,
        build: 1.0,
        radius: 4.0,
        packed: (scale * 1000.0).round() as u32,
        arm_pitch: [0.0, 0.0, 0.0, stretch],
        ..bytemuck::Zeroable::zeroed()
    })
}

/// Where the cliff pieces stand among the static props, and which are hidden. The
/// pieces are the canyon's jointed sandstone: a map's other climates keep their
/// walls as the terrain draws them, so pieces outside the desert are left out
/// (`set_look`) by the same bits that leave out the sim's dead props.
pub(super) struct CliffRocks {
    first: usize,
    spots: Vec<Vec2>,
    /// One bit per static prop: set for a hidden piece.
    hidden: Vec<u32>,
    /// The sim's dead props with the hidden pieces, as written to the GPU.
    merged: Vec<u32>,
}

impl CliffRocks {
    /// `statics` holds the map's props and then, from `first`, the pieces.
    pub(super) fn new(statics: &[UnitInstance], first: usize) -> Self {
        let spots = statics[first..]
            .iter()
            .map(|p| Vec2::new(p.pos[0], p.pos[1]))
            .collect();
        let mut rocks = Self {
            first,
            spots,
            hidden: vec![0; statics.len().div_ceil(32)],
            merged: Vec::new(),
        };
        rocks.set_look(&mc_data::weather::MapLook::default());
        rocks
    }

    /// Hides every piece standing outside the desert in `look`.
    pub(super) fn set_look(&mut self, look: &mc_data::weather::MapLook) {
        self.hidden.fill(0);
        for (i, at) in self.spots.iter().enumerate() {
            if look.climate_at(at.x, at.y) != mc_data::weather::Climate::Desert {
                let k = self.first + i;
                self.hidden[k / 32] |= 1 << (k % 32);
            }
        }
    }

    /// The bits for the GPU: the sim's dead props (`dead`, one bit per map prop)
    /// and the hidden pieces.
    pub(super) fn dead_props(&mut self, dead: &[u32]) -> &[u32] {
        self.merged.clear();
        self.merged.extend_from_slice(&self.hidden);
        for (word, d) in self.merged.iter_mut().zip(dead) {
            *word |= d;
        }
        &self.merged
    }
}
