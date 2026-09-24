//! Local lights: what weapons, blasts, fires, shots in flight and beams throw
//! on the ground and the hulls around them, and the lamps units carry
//! (headlights, floodlights) once it gets dark.
//!
//! Everything is gathered on the CPU each frame into one list, culled to the
//! view, ranked, and binned into clusters (a 32x18 screen grid times 24
//! slices of distance from the eye). Shaders find a point's cluster and walk
//! only the lights listed there (shaders/lights.wgsl), so the cost is paid
//! where the light falls and nowhere else.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use mc_data::{cat, Blueprints, LampKind, LightMount};
use mc_sim::mirror::{
    FireInstance, ProjectileInstance, RenderFrame, KIND_GHOST, KIND_PROP, KIND_WRECK,
    PROJECTILE_APOGEE, PROJECTILE_BEAM, PROJECTILE_BOMB, PROJECTILE_MISSILE, PROJECTILE_SKIM, PROJECTILE_TORPEDO,
    PROJECTILE_TRAIL,
    STATE_RADAR, STATE_UNPOWERED, UNIT_BURNING,
};

use crate::camera::Camera;

pub const TILES_X: usize = 32;
pub const TILES_Y: usize = 18;
pub const SLICES: usize = 24;
pub const CLUSTERS: usize = TILES_X * TILES_Y * SLICES;
const NEAR: f32 = 4.0;
const FAR: f32 = 40_000.0;
/// Lights drawn at most in one frame, after ranking.
pub const MAX_LIGHTS: usize = 2048;
/// Cluster entries at most (the grid buffer holds CLUSTERS words before them).
pub const MAX_INDICES: usize = 1 << 18;
/// Lights one cluster lists at most: past this the weakest are left out.
const PER_CLUSTER: u32 = 64;
/// A light whose reach covers fewer pixels than this is not drawn.
const MIN_PIXELS: f32 = 2.5;

const _: () = assert!(CLUSTERS == 13824, "lights.wgsl LIGHT_CLUSTERS");

const KIND_POINT: u32 = 0;
const KIND_SPOT: u32 = 1;
const KIND_LINE: u32 = 2;
const KIND_PAIR: u32 = 3;

/// Mirrors `Light` in shaders/lights.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct GpuLight {
    pos: [f32; 3],
    range: f32,
    color: [f32; 3],
    kind: u32,
    axis: [f32; 3],
    cos_outer: f32,
    cos_inner: f32,
    pair: f32,
    size: f32,
    spill: f32,
}

const _: () = assert!(std::mem::size_of::<GpuLight>() == 64);

impl GpuLight {
    fn point(pos: Vec3, color: Vec3, range: f32, size: f32) -> GpuLight {
        GpuLight {
            pos: pos.to_array(),
            range,
            color: color.to_array(),
            kind: KIND_POINT,
            axis: [0.0, 0.0, 1.0],
            cos_outer: -1.0,
            cos_inner: -1.0,
            pair: 0.0,
            size,
            spill: 1.0,
        }
    }

    fn line(from: Vec3, to: Vec3, color: Vec3, range: f32, size: f32) -> GpuLight {
        GpuLight {
            kind: KIND_LINE,
            axis: (to - from).to_array(),
            ..GpuLight::point(from, color, range, size)
        }
    }

    /// Centre and radius of everything it can reach.
    fn bounds(&self) -> (Vec3, f32) {
        let pos = Vec3::from(self.pos);
        let axis = Vec3::from(self.axis);
        if self.kind == KIND_LINE {
            (pos + axis * 0.5, axis.length() * 0.5 + self.range)
        } else if self.spill <= 0.0 && self.cos_outer > std::f32::consts::FRAC_1_SQRT_2 {
            // A narrow cone that spills nothing: the smallest sphere round the cone,
            // widened by the pair's spacing.
            let half = self.range * 0.5 / self.cos_outer;
            (pos + axis * half, half + self.pair)
        } else {
            (pos, self.range)
        }
    }

    fn strength(&self) -> f32 {
        let c = self.color;
        c[0].max(c[1]).max(c[2])
    }
}

/// How a timed light rises and dies.
#[derive(Clone, Copy, PartialEq)]
enum Envelope {
    /// A muzzle or a hit: all at once, gone at once.
    Flash,
    /// An explosion: a white-hot instant, then a fireball that sags and flickers.
    Blast,
}

/// A light from an effect: it knows when it starts and how long it lasts.
#[derive(Clone, Copy)]
struct Flash {
    pos: Vec3,
    color: Vec3,
    range: f32,
    start: f32,
    life: f32,
    envelope: Envelope,
    seed: f32,
}

impl Flash {
    fn at(&self, time: f32) -> Option<f32> {
        let t = (time - self.start) / self.life;
        if !(0.0..1.0).contains(&t) {
            return None;
        }
        Some(match self.envelope {
            Envelope::Flash => (1.0 - t) * (1.0 - t),
            Envelope::Blast => {
                // The first instant is far brighter than the fireball after it.
                let core = 2.2 * (-t * 22.0).exp();
                let burn = (1.0 - t).powf(1.6);
                let flicker = 0.82 + 0.18 * (time * 37.0 + self.seed * 40.0).sin() * (time * 23.0 + self.seed).cos();
                core + burn * flicker
            }
        })
    }
}

/// One lamp of a blueprint, resolved and in hull space.
#[derive(Clone, Copy)]
struct Lamp {
    kind: u32,
    at: Vec3,
    aim: Vec3,
    color: Vec3,
    range: f32,
    cos_outer: f32,
    cos_inner: f32,
    pair: f32,
    spill: f32,
    night_only: bool,
}

impl Lamp {
    fn from_mount(m: &LightMount) -> Lamp {
        let cone = m.cone.clamp(1.0, 89.0).to_radians();
        let (kind, spill) = match m.kind {
            LampKind::Point => (KIND_POINT, 1.0),
            LampKind::Spot => (KIND_SPOT, 0.06),
            LampKind::Headlights => (KIND_PAIR, 0.0),
        };
        Lamp {
            kind,
            at: Vec3::from(m.at),
            aim: Vec3::from(m.aim).try_normalize().unwrap_or(Vec3::X),
            color: Vec3::from(m.color).max(Vec3::ZERO) * m.intensity.max(0.0),
            range: m.range.clamp(1.0, 2000.0),
            cos_outer: cone.cos(),
            // A soft edge: the beam fades over most of its width, no hard rim.
            cos_inner: (cone * if m.kind == LampKind::Headlights { 0.25 } else { 0.55 }).cos(),
            pair: (m.spread * 0.5).max(0.0),
            spill,
            night_only: m.night_only,
        }
    }
}

/// The lamps a blueprint carries when its data names none.
fn usual_lamps(bp: &mc_data::UnitBlueprint) -> Vec<LightMount> {
    let radius = bp.radius.to_f32();
    let height = bp.height.to_f32().max(0.5);
    if let Some(motion) = &bp.motion {
        if motion.layer == mc_data::MoveLayer::Air {
            return Vec::new();
        }
        // Headlights low on the bow, dipped at the ground ahead.
        let size = (radius / 3.0).clamp(0.5, 4.0);
        return vec![LightMount {
            kind: LampKind::Headlights,
            at: [radius * 0.85, 0.0, (height * 0.45).clamp(0.6, 4.0)],
            aim: [1.0, 0.0, -0.2],
            color: [0.78, 0.88, 1.0],
            intensity: 650.0 * size,
            range: 44.0 + 22.0 * size,
            cone: 28.0,
            spread: radius * 0.8,
            night_only: true,
        }];
    }
    if bp.categories & cat::STRUCTURE == 0 || bp.categories & cat::WALL != 0 || radius < 2.5 {
        return Vec::new();
    }
    // Sodium floodlights high on the corners, thrown out and down across the yard,
    // spilling back onto the walls.
    let up = (height * 0.85).max(3.0);
    let warm = [1.0, 0.7, 0.4];
    if radius < 7.0 {
        return vec![LightMount {
            kind: LampKind::Point,
            at: [0.0, 0.0, up + 2.0],
            color: warm,
            intensity: 90.0 * radius,
            range: radius * 3.5 + 14.0,
            ..LightMount::default()
        }];
    }
    let reach = radius * 0.75;
    [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)]
        .iter()
        .map(|&(x, y)| LightMount {
            kind: LampKind::Spot,
            at: [x * reach, y * reach, up],
            aim: [x, y, -1.1],
            color: warm,
            // Throw grows with the yard, so light at its edge by the square.
            intensity: 26.0 * radius * radius,
            range: radius * 2.4 + 40.0,
            cone: 55.0,
            ..LightMount::default()
        })
        .collect()
}

/// A unit that carries lamps, as of the last tick.
#[derive(Clone, Copy)]
struct Carrier {
    from: Vec3,
    to: Vec3,
    from_heading: f32,
    heading: f32,
    blueprint: u32,
    /// 0 off, 1 fully on; below one flickers (a hull on its last legs).
    power: f32,
    seed: f32,
}

/// Something in flight that glows, as of the last tick.
#[derive(Clone, Copy)]
struct Glow {
    from: Vec3,
    to: Vec3,
    color: Vec3,
    range: f32,
    /// Line: the glow runs from `from` to `to` (a beam), not along them.
    line: bool,
}

/// A fire burning on the ground or on a hull.
#[derive(Clone, Copy)]
struct Fire {
    pos: Vec3,
    strength: f32,
    range: f32,
    seed: f32,
}

/// A site being printed or refitted: its work lights the ground and whatever stands
/// round it, and fades with the welds when the builders stop.
struct Site {
    pos: Vec3,
    strength: f32,
    range: f32,
    size: f32,
    seed: f32,
}

/// Where build beams meet a site, merged so a ring of engineers is a few arcs, not dozens.
struct Arc {
    pos: Vec3,
    beams: f32,
    seed: f32,
}

fn hash(x: f32) -> f32 {
    ((x * 12.9898).sin() * 43758.547).fract().abs()
}

pub struct Lights {
    lamps: Vec<Vec<Lamp>>,
    flashes: Vec<Flash>,
    carriers: Vec<Carrier>,
    glows: Vec<Glow>,
    fires: Vec<Fire>,
    sites: Vec<Site>,
    arcs: Vec<Arc>,
    /// Filled per frame by the renderer: beams and burning trees.
    extra: Vec<GpuLight>,
    list: Vec<GpuLight>,
    grid: Vec<u32>,
    /// Grid words the GPU copy still holds from the last frame that had any.
    grid_dirty: bool,
    /// `MERIDIAN_LIGHTS=0` turns every local light off (to compare looks and cost).
    enabled: bool,
}

impl Lights {
    pub fn new(blueprints: &Blueprints) -> Lights {
        let lamps = blueprints
            .units
            .iter()
            .map(|bp| {
                let mounts = bp.visual.lights.clone().unwrap_or_else(|| usual_lamps(bp));
                mounts.iter().map(Lamp::from_mount).collect()
            })
            .collect();
        Lights {
            lamps,
            flashes: Vec::new(),
            carriers: Vec::new(),
            glows: Vec::new(),
            fires: Vec::new(),
            sites: Vec::new(),
            arcs: Vec::new(),
            extra: Vec::new(),
            list: Vec::new(),
            grid: vec![0; CLUSTERS],
            grid_dirty: true,
            enabled: std::env::var("MERIDIAN_LIGHTS").map_or(true, |v| v != "0"),
        }
    }

    fn push_flash(&mut self, flash: Flash) {
        // Effects come in bunches at one spot (a core, a ring, a flare): keep one light.
        for f in self.flashes.iter_mut().rev().take(6) {
            if (f.start - flash.start).abs() < 0.03 && f.pos.distance(flash.pos) < 0.5 * f.range.max(flash.range) {
                let (a, b) = (f.color.max_element(), flash.color.max_element());
                f.color = if a >= b { f.color + flash.color * 0.5 } else { flash.color + f.color * 0.5 };
                f.range = f.range.max(flash.range);
                f.life = f.life.max(flash.life);
                if flash.envelope == Envelope::Blast {
                    f.envelope = Envelope::Blast;
                }
                return;
            }
        }
        if self.flashes.len() >= 4096 {
            self.flashes.remove(0);
        }
        self.flashes.push(flash);
    }

    /// The light of one sprite effect (renderer `push_effect`): its visual radius,
    /// lifetime and kind (sprites.wgsl `Effect`) say how big, how long and what colour.
    pub fn effect(&mut self, pos: [f32; 3], start: f32, radius: f32, life: f32, kind: f32) {
        let r = radius.max(0.2);
        let (color, gain, reach, envelope) = match kind as u32 {
            0 => (Vec3::new(0.35, 0.62, 1.0), 70.0, 7.0, Envelope::Flash),
            1 => (Vec3::new(1.0, 0.6, 0.28), 70.0, 7.0, Envelope::Flash),
            8 => (Vec3::new(1.0, 0.14, 0.06), 70.0, 7.0, Envelope::Flash),
            2 => (Vec3::new(1.0, 0.52, 0.2), 60.0, 5.5, Envelope::Blast),
            3 => (Vec3::new(1.0, 0.68, 0.26), 18.0, 5.0, Envelope::Flash),
            4 => (Vec3::new(1.0, 0.86, 0.7), 60.0, 3.2, Envelope::Blast),
            5 => (Vec3::new(1.0, 0.5, 0.2), 20.0, 3.0, Envelope::Blast),
            6 => (Vec3::new(1.0, 0.72, 0.42), 50.0, 5.0, Envelope::Flash),
            _ => return,
        };
        // Brightness grows with the flash's area, but a big blast is not a sun.
        let power = gain * r.min(60.0).powf(1.7);
        let seed = hash(pos[0] * 0.13 + pos[1] * 0.71 + start);
        self.push_flash(Flash {
            pos: Vec3::from(pos) + Vec3::Z * (r * 0.25).min(4.0),
            color: color * power,
            range: (r * reach).clamp(5.0, 900.0),
            start,
            life: if envelope == Envelope::Blast { (life * 1.5).max(0.3) } else { life.max(0.06) },
            envelope,
            seed,
        });
    }

    /// Takes what glows and what carries lamps from a new tick of the mirror.
    pub fn tick(&mut self, frame: &RenderFrame, blueprints: &Blueprints) {
        self.carriers.clear();
        self.fires.clear();
        self.sites.clear();
        self.arcs.clear();
        let hidden = KIND_WRECK | KIND_PROP | KIND_GHOST | STATE_RADAR;
        let unbuilt = ((mc_sim::tables::flag::IN_FACTORY | mc_sim::tables::flag::UNDER_CONSTRUCTION) as u32) << 8;
        for u in &frame.units {
            if u.owner_flags & hidden == 0 {
                self.push_site(u, frame, blueprints);
            }
            if u.owner_flags & (hidden | unbuilt) != 0 || u.build < 1.0 {
                continue;
            }
            let from = Vec3::from(u.prev_pos);
            let to = Vec3::from(u.pos);
            let seed = hash(u.unit_id as f32 * 0.618);
            if self.lamps.get(u.blueprint as usize).is_some_and(|l| !l.is_empty()) {
                let unpowered = u.owner_flags & STATE_UNPOWERED != 0;
                self.carriers.push(Carrier {
                    from,
                    to,
                    from_heading: u.prev_heading,
                    heading: u.heading,
                    blueprint: u.blueprint,
                    power: if unpowered { 0.0 } else { (u.health * 4.0).clamp(0.3, 1.0) },
                    seed,
                });
            }
            // A hull on fire from napalm lights the ground too. Damage alone only smokes.
            let alight = if u._pad & UNIT_BURNING != 0 { 1.0 } else { 0.0 };
            if alight > 0.0 {
                let bp = blueprints.units.get(u.blueprint as usize);
                let r = bp.map_or(u.radius, |b| b.radius.to_f32()).max(1.0);
                let h = bp.map_or(2.0, |b| b.height.to_f32());
                self.fires.push(Fire {
                    pos: to + Vec3::Z * h * 0.8,
                    strength: alight * 22.0 * r.min(20.0),
                    range: r * 3.0 + 10.0,
                    seed,
                });
            }
        }
        for f in &frame.fires {
            self.push_ground_fire(f);
        }
        self.glows.clear();
        for p in &frame.projectiles {
            if p.color & PROJECTILE_BEAM != 0 {
                self.push_arc(Vec3::from(p.pos));
            }
            if let Some(g) = glow_of(p) {
                self.glows.push(g);
            }
        }
        for b in &frame.beams {
            if b.kind >= 4 {
                self.replication_light(b);
                continue;
            }
            let grip = Vec3::from(b.to) + Vec3::Z * b.height * 0.55;
            self.glows.push(Glow {
                from: Vec3::from(b.from),
                to: grip,
                color: Vec3::new(0.18, 0.82, 0.52) * 30.0,
                range: 9.0,
                line: true,
            });
        }
    }

    /// The work light of a site: as strong as its welds are live (they fade out
    /// over a second or so when the builders leave), sized by the hull.
    fn push_site(&mut self, u: &mc_sim::mirror::UnitInstance, frame: &RenderFrame, blueprints: &Blueprints) {
        let printing = (mc_sim::tables::flag::UNDER_CONSTRUCTION as u32) << 8;
        let work = if u.owner_flags & printing != 0 {
            let first = u.weld_first as usize;
            let end = (first + u.weld_count as usize).min(frame.welds.len());
            frame.welds.get(first..end).map_or(0.0, |w| w.iter().fold(0.0f32, |m, w| m.max(w.fade)))
        } else if u.upgrade > 0.0 && u.upgrade < 1.0 {
            1.0
        } else {
            0.0
        };
        if work <= 0.01 {
            return;
        }
        let bp = blueprints.units.get(u.blueprint as usize);
        let r = bp.map_or(u.radius, |b| b.radius.to_f32()).max(1.0);
        let h = bp.map_or(2.0, |b| b.height.to_f32()).max(1.0);
        // Irradiance near 12 on the ground at the hull's edge (a tree fire's is about 6
        // at its foot); the reach a few hulls out.
        let edge = r + 4.0;
        self.sites.push(Site {
            pos: Vec3::from(u.pos) + Vec3::Z * (h * 0.6 + 1.5),
            strength: 12.0 * edge * edge * work,
            range: r * 3.5 + 30.0,
            size: r * 0.5 + 1.0,
            seed: hash(u.unit_id as f32 * 0.377),
        });
    }

    /// A build beam's end: joins an arc already within a few metres, or starts one.
    fn push_arc(&mut self, at: Vec3) {
        if let Some(a) = self.arcs.iter_mut().find(|a| a.pos.distance_squared(at) < 9.0) {
            a.beams += 1.0;
            return;
        }
        if self.arcs.len() < 512 {
            self.arcs.push(Arc { pos: at, beams: 1.0, seed: hash(at.x * 0.31 + at.y * 0.57) });
        }
    }

    fn push_ground_fire(&mut self, f: &FireInstance) {
        let fade = 1.0 - ((f.elapsed / f.duration.max(0.05)) - 0.8).max(0.0) * 5.0;
        let r = f.radius.max(1.0);
        self.fires.push(Fire {
            pos: Vec3::from(f.pos) + Vec3::Z * (1.0 + r * 0.15),
            strength: 55.0 * r.min(30.0).powf(1.4) * fade.max(0.0),
            range: r * 3.0 + 12.0,
            seed: hash(f.pos[0] + f.pos[1] * 0.37),
        });
    }

    /// Replication beams (Survival): the ray (kind 4) lights the country under it all the
    /// way to the site, in pieces short enough to cull and bin, with a strong light at each
    /// end; a print beam (kind 5) lights the unit it is printing.
    fn replication_light(&mut self, b: &mc_sim::reclaim::BeamInstance) {
        let violet = Vec3::new(0.52, 0.2, 1.0);
        let from = Vec3::from(b.from);
        if b.kind == 4 {
            let to = Vec3::from(b.to) + Vec3::Z * 46.0;
            let len = from.distance(to);
            let pieces = (len / 450.0).ceil().clamp(1.0, 40.0) as usize;
            let raise = b.height.clamp(0.0, 1.0);
            for i in 0..pieces {
                let (a, c) = (i as f32 / pieces as f32, (i + 1) as f32 / pieces as f32);
                self.glows.push(Glow {
                    from: from.lerp(to, a),
                    to: from.lerp(to, c),
                    color: violet * (160.0 + 80.0 * raise),
                    range: 70.0,
                    line: true,
                });
            }
            for (at, strength, range) in [(from, 5000.0, 160.0), (to, 3500.0 + 2500.0 * raise, 140.0)] {
                self.glows.push(Glow { from: at, to: at, color: (violet * 0.8 + Vec3::splat(0.2)) * strength, range, line: false });
            }
        } else {
            let middle = Vec3::from(b.to) + Vec3::Z * b.height * 0.5;
            self.glows.push(Glow {
                from,
                to: middle,
                color: violet * 45.0,
                range: 14.0 + b.radius,
                line: true,
            });
            self.glows.push(Glow { from: middle, to: middle, color: violet * 220.0, range: b.radius * 2.5 + 10.0, line: false });
        }
    }

    /// A tree burning at `pos` (`age` seconds alight): renderer tree fires.
    pub fn tree_fire(&mut self, pos: Vec3, age: f32) {
        let heat = (age / 1.5).clamp(0.0, 1.0) * (1.0 - ((age - 9.0) / 12.0).clamp(0.0, 1.0));
        if heat <= 0.0 {
            return;
        }
        let flicker = 0.75 + 0.25 * (age * 11.0 + pos.x).sin() * (age * 7.3 + pos.y).cos();
        self.extra.push(GpuLight::point(
            pos + Vec3::Z * 6.0,
            Vec3::new(1.0, 0.48, 0.16) * 260.0 * heat * flicker,
            34.0,
            3.0,
        ));
    }

    /// A lamp shining this frame: a point (`cone` of 180 degrees or more) or a spot of
    /// `cone` degrees half-angle aimed along `aim`, `color` already scaled by brightness.
    /// `spill` is how much of it falls outside the cone (0 none, 1 all round).
    pub fn lamp(&mut self, pos: Vec3, aim: Vec3, color: Vec3, range: f32, cone: f32, spill: f32) {
        if cone >= 180.0 {
            self.extra.push(GpuLight::point(pos, color, range, 1.5));
            return;
        }
        let cone = cone.clamp(1.0, 89.0).to_radians();
        self.extra.push(GpuLight {
            kind: KIND_SPOT,
            axis: aim.try_normalize().unwrap_or(Vec3::NEG_Z).to_array(),
            cos_outer: cone.cos(),
            cos_inner: (cone * 0.3).cos(),
            spill,
            ..GpuLight::point(pos, color, range, 1.5 + range * 0.02)
        });
    }

    /// A beam shining this frame from `from` to `to`, `color` already scaled by brightness.
    pub fn beam(&mut self, from: Vec3, to: Vec3, color: Vec3, range: f32) {
        self.extra.push(GpuLight::line(from, to, color, range, 1.0));
    }

    /// Everything that shines this frame, culled, ranked and binned. `dark`
    /// is how far the day has gone (0 daylight, 1 night) and switches lamps on.
    /// Returns the lights and the grid (CLUSTERS words, then the indices); the
    /// grid is empty when the GPU copy is already right (no lights now or last frame).
    pub fn build(&mut self, time: f32, alpha: f32, camera: &Camera, dark: f32) -> (&[GpuLight], &[u32]) {
        let mut list = std::mem::take(&mut self.list);
        list.clear();
        if !self.enabled {
            self.extra.clear();
            return self.bin(list, camera);
        }
        self.flashes.retain(|f| time < f.start + f.life);
        for f in &self.flashes {
            if let Some(k) = f.at(time) {
                let s = k.min(3.0);
                list.push(GpuLight::point(f.pos, f.color * k, f.range * (0.6 + 0.4 * s.min(1.0)), f.range * 0.05));
            }
        }
        for g in &self.glows {
            if g.line {
                list.push(GpuLight::line(g.from, g.to, g.color, g.range, 1.0));
            } else {
                list.push(GpuLight::point(g.from.lerp(g.to, alpha), g.color, g.range, 1.0));
            }
        }
        for f in &self.fires {
            let flicker = 0.7
                + 0.18 * (time * 9.0 + f.seed * 50.0).sin()
                + 0.12 * (time * 23.0 + f.seed * 13.0).sin() * (time * 5.0).cos();
            let rise = Vec3::Z * 0.4 * (time * 6.0 + f.seed * 9.0).sin();
            list.push(GpuLight::point(
                f.pos + rise,
                Vec3::new(1.0, 0.46, 0.15) * f.strength * flicker,
                f.range,
                2.0,
            ));
        }
        for s in &self.sites {
            // Work light breathes with the print; it does not strobe like the arcs.
            let breathe = 0.86 + 0.09 * (time * 3.1 + s.seed * 40.0).sin() + 0.05 * (time * 8.7 + s.seed * 17.0).sin();
            list.push(GpuLight::point(s.pos, Vec3::new(1.0, 0.6, 0.24) * s.strength * breathe, s.range, s.size));
        }
        for a in &self.arcs {
            // An arc stutters: a fresh level twenty-odd times a second.
            let step = (time * 23.0).floor();
            let k = 0.45 + 0.8 * hash(step * 0.137 + a.seed * 71.0);
            list.push(GpuLight::point(
                a.pos,
                Vec3::new(1.0, 0.8, 0.52) * 240.0 * a.beams.sqrt() * k,
                22.0 + 5.0 * a.beams.sqrt(),
                0.6,
            ));
        }
        list.append(&mut self.extra);
        if dark > 0.0 {
            for c in &self.carriers {
                // Lamps come on one by one through dusk, not all on the same frame.
                let on = ((dark - 0.15 - c.seed * 0.35) / 0.12).clamp(0.0, 1.0);
                let Some(lamps) = self.lamps.get(c.blueprint as usize) else { continue };
                let mut power = c.power;
                if power < 1.0 && power > 0.0 {
                    // Failing electrics: the lamps stutter.
                    let t = time * 13.0 + c.seed * 91.0;
                    power *= if (t.sin() * (t * 0.37).cos()) > 0.55 { 0.15 } else { 1.0 };
                }
                let pos = c.from.lerp(c.to, alpha);
                let turn = (c.heading - c.from_heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                    - std::f32::consts::PI;
                let (s, co) = (c.from_heading + turn * alpha).sin_cos();
                let rot = |v: Vec3| Vec3::new(v.x * co - v.y * s, v.x * s + v.y * co, v.z);
                for lamp in lamps {
                    let k = if lamp.night_only { on * power } else { power };
                    if k <= 0.0 {
                        continue;
                    }
                    list.push(GpuLight {
                        pos: (pos + rot(lamp.at)).to_array(),
                        range: lamp.range,
                        color: (lamp.color * k).to_array(),
                        kind: lamp.kind,
                        axis: rot(lamp.aim).to_array(),
                        cos_outer: lamp.cos_outer,
                        cos_inner: lamp.cos_inner,
                        pair: lamp.pair,
                        // A lamp is a lens, not a point: nothing a metre off it is blinding.
                        size: 1.5 + lamp.range * 0.04,
                        spill: lamp.spill,
                    });
                }
            }
        }
        self.bin(list, camera)
    }

    fn bin(&mut self, mut list: Vec<GpuLight>, camera: &Camera) -> (&[GpuLight], &[u32]) {
        let eye = camera.eye();
        let planes = camera.frustum();
        let px = camera.projection_scale();
        // Cull and rank: what can be seen, by how much of the screen it lights.
        let mut ranked: Vec<(f32, GpuLight)> = list
            .drain(..)
            .filter_map(|l| {
                let (c, r) = l.bounds();
                if planes[..4].iter().any(|p| p.truncate().dot(c) + p.w < -r) {
                    return None;
                }
                let d = c.distance(eye).max(1.0);
                let pixels = r * px / d;
                if pixels < MIN_PIXELS || l.strength() <= 0.0 {
                    return None;
                }
                Some((l.strength() / (l.range * l.range).max(1.0) * pixels, l))
            })
            .collect();
        if ranked.len() > MAX_LIGHTS {
            ranked.select_nth_unstable_by(MAX_LIGHTS, |a, b| b.0.total_cmp(&a.0));
            ranked.truncate(MAX_LIGHTS);
        }
        ranked.sort_unstable_by(|a, b| b.0.total_cmp(&a.0));
        list.extend(ranked.into_iter().map(|(_, l)| l));

        if list.is_empty() {
            self.list = list;
            let dirty = std::mem::replace(&mut self.grid_dirty, false);
            if dirty {
                self.grid.clear();
                self.grid.resize(CLUSTERS, 0);
                return (&self.list, &self.grid);
            }
            return (&self.list, &[]);
        }
        self.grid_dirty = true;

        // Each light's block of clusters: tiles its sphere can land on, slices it spans.
        let view_proj = camera.view_proj();
        let slice_scale = SLICES as f32 / (FAR / NEAR).ln();
        let slice_of = |d: f32| ((d.max(NEAR) / NEAR).ln() * slice_scale).min(SLICES as f32 - 1.0) as usize;
        let blocks: Vec<[usize; 6]> = list
            .iter()
            .map(|l| {
                let (c, r) = l.bounds();
                let d = c.distance(eye);
                let (x0, x1, y0, y1) = screen_rect(&view_proj, c, r);
                [x0, x1, y0, y1, slice_of(d - r), slice_of(d + r)]
            })
            .collect();

        let mut counts = vec![0u32; CLUSTERS];
        for b in &blocks {
            for_clusters(b, |i| counts[i] = (counts[i] + 1).min(PER_CLUSTER));
        }
        self.grid.clear();
        self.grid.resize(CLUSTERS, 0);
        let mut next = 0u32;
        for (i, &n) in counts.iter().enumerate() {
            let n = n.min((MAX_INDICES as u32).saturating_sub(next));
            self.grid[i] = next << 8 | n;
            next += n;
        }
        self.grid.resize(CLUSTERS + next as usize, 0);
        let mut fill = vec![0u32; CLUSTERS];
        for (li, b) in blocks.iter().enumerate() {
            let grid = &mut self.grid;
            for_clusters(b, |i| {
                let word = grid[i];
                if fill[i] < word & 0xFF {
                    grid[CLUSTERS + (word >> 8) as usize + fill[i] as usize] = li as u32;
                    fill[i] += 1;
                }
            });
        }
        self.list = list;
        (&self.list, &self.grid)
    }
}

fn for_clusters(b: &[usize; 6], mut f: impl FnMut(usize)) {
    for s in b[4]..=b[5] {
        for y in b[2]..=b[3] {
            let row = (s * TILES_Y + y) * TILES_X;
            for x in b[0]..=b[1] {
                f(row + x);
            }
        }
    }
}

/// Tiles (inclusive) the sphere can cover on screen; all of them when it reaches behind the eye.
fn screen_rect(view_proj: &Mat4, c: Vec3, r: f32) -> (usize, usize, usize, usize) {
    let all = (0, TILES_X - 1, 0, TILES_Y - 1);
    let (mut lo, mut hi) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
    for i in 0..8 {
        let corner = c + Vec3::new(
            if i & 1 == 0 { -r } else { r },
            if i & 2 == 0 { -r } else { r },
            if i & 4 == 0 { -r } else { r },
        );
        let clip: Vec4 = *view_proj * corner.extend(1.0);
        if clip.w <= 1e-3 {
            return all;
        }
        let ndc = clip.truncate().truncate() / clip.w;
        lo = lo.min(ndc);
        hi = hi.max(ndc);
    }
    let tile = |v: f32, n: usize| (((v * 0.5 + 0.5) * n as f32).floor().max(0.0) as usize).min(n - 1);
    if hi.x < -1.0 || hi.y < -1.0 || lo.x > 1.0 || lo.y > 1.0 {
        // Off screen, though the frustum test kept it: a sliver at the edge.
        return (tile(lo.x.max(-1.0), TILES_X), tile(hi.x.min(1.0), TILES_X), tile(lo.y.max(-1.0), TILES_Y), tile(hi.y.min(1.0), TILES_Y));
    }
    (tile(lo.x, TILES_X), tile(hi.x, TILES_X), tile(lo.y, TILES_Y), tile(hi.y, TILES_Y))
}

/// The light a shot in flight throws, if any.
fn glow_of(p: &ProjectileInstance) -> Option<Glow> {
    let flags = p.color;
    if flags & (PROJECTILE_BOMB | PROJECTILE_TORPEDO) != 0 {
        return None;
    }
    let from = Vec3::from(p.prev_pos);
    let to = Vec3::from(p.pos);
    if flags & PROJECTILE_BEAM != 0 {
        // A construction beam: amber along its length.
        return Some(Glow { from, to, color: Vec3::new(1.0, 0.66, 0.24) * 34.0, range: 11.0, line: true });
    }
    if flags & PROJECTILE_MISSILE != 0 {
        if flags & PROJECTILE_APOGEE != 0 && to.z < from.z {
            // Falling cold from its apogee: the nose glowing white with re-entry, a
            // bigger light than any motor, brighter the lower it comes.
            let heat = (1.0 - to.z / 1200.0).clamp(0.3, 1.0);
            let ahead = (to - from).normalize_or_zero() * 2.0;
            return Some(Glow { from: from + ahead, to: to + ahead, color: Vec3::new(1.0, 0.82, 0.6) * 320.0 * heat, range: 30.0 + 20.0 * heat, line: false });
        }
        // The motor's flame, behind the nose; a booster's is bigger, a skimmer's a little brighter.
        let back = (from - to).normalize_or_zero() * 1.5;
        let (gain, range) = if flags & PROJECTILE_APOGEE != 0 {
            (260.0, 40.0)
        } else if flags & PROJECTILE_SKIM != 0 {
            (180.0, 28.0)
        } else {
            (140.0, 26.0)
        };
        return Some(Glow { from: from + back, to: to + back, color: Vec3::new(1.0, 0.62, 0.3) * gain, range, line: false });
    }
    if p.plasma > 0.0 || flags & PROJECTILE_TRAIL != 0 {
        let k = 1.0 + p.plasma.min(6.0);
        return Some(Glow { from, to, color: Vec3::new(0.35, 0.66, 1.0) * 60.0 * k, range: 12.0 + 4.0 * k, line: false });
    }
    // Tracers: small, but a stream of them lights a night battle.
    let color = match flags & 0xF {
        0 => Vec3::new(0.35, 0.66, 1.0),
        _ if p._pad[0] > 1.5 => Vec3::new(1.0, 0.2, 0.08),
        _ => Vec3::new(1.0, 0.55, 0.22),
    };
    let size = p.size.clamp(0.2, 3.0);
    Some(Glow { from, to, color: color * 14.0 * size, range: 6.0 + 3.0 * size, line: false })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clusters_list_a_light_where_it_shines_only() {
        let mut camera = Camera::new(glam::Vec2::splat(8192.0), glam::Vec2::new(1600.0, 900.0));
        camera.distance = 400.0;
        let mut lights = Lights {
            lamps: Vec::new(),
            flashes: Vec::new(),
            carriers: Vec::new(),
            glows: Vec::new(),
            fires: Vec::new(),
            sites: Vec::new(),
            arcs: Vec::new(),
            extra: Vec::new(),
            list: Vec::new(),
            grid: vec![0; CLUSTERS],
            grid_dirty: true,
            enabled: true,
        };
        lights.effect(camera.focus.to_array(), 0.0, 4.0, 0.5, 2.0);
        let (list, grid) = lights.build(0.05, 0.0, &camera, 0.0);
        assert_eq!(list.len(), 1);
        let listed = grid[..CLUSTERS].iter().filter(|w| *w & 0xFF > 0).count();
        assert!(listed > 0 && listed < CLUSTERS / 4, "{listed} clusters");
        // Gone when its time is up, and the grid is cleared once.
        let (list, grid) = lights.build(5.0, 0.0, &camera, 0.0);
        assert!(list.is_empty());
        assert_eq!(grid.len(), CLUSTERS);
        assert!(grid.iter().all(|&w| w == 0));
        let (_, grid) = lights.build(5.1, 0.0, &camera, 0.0);
        assert!(grid.is_empty());
    }
}

#[cfg(test)]
mod data_tests {
    use super::*;

    #[test]
    fn a_blueprint_names_its_lamps_in_ron() {
        let mounts: Vec<LightMount> = ron::from_str(
            "[(kind: Spot, at: (4.0, 0.0, 9.0), aim: (1.0, 0.0, -1.0), color: (0.6, 0.8, 1.0), intensity: 3000.0, range: 90.0, cone: 20.0),
              (kind: Point, at: (0.0, 0.0, 3.0), night_only: false)]",
        )
        .expect("lamps parse");
        assert_eq!(mounts.len(), 2);
        assert_eq!(mounts[1].kind, LampKind::Point);
        assert!(!mounts[1].night_only);
        let spot = Lamp::from_mount(&mounts[0]);
        assert_eq!(spot.kind, KIND_SPOT);
        assert!((spot.aim.length() - 1.0).abs() < 1e-5);
    }
}
