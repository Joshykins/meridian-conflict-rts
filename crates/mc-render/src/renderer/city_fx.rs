//! A city coming apart (`mc_sim::city`; the map's city kit, `mc_map::city`):
//! glass and masonry thrown off where a shot strikes a building, fires in its
//! windows and smoke over it, its collapse into a cloud of dust, and the heap
//! of rubble it leaves.
//!
//! Presentation only. What a building looks like as it is hurt (its panes
//! breaking, soot, the glow of a fire inside) is drawn by the entity shader
//! from each map prop's word in `city_look` (scene set binding 33, laid out
//! as `gpu_consts::city_look`). A collapsing building is drawn as a dynamic
//! copy sinking into its dust; its rubble is a static heap the renderer adds
//! under every building at load, hidden while the building stands (the
//! hidden bits ride with the dead props', `hide_heaps`).

use super::{
    Renderer, PUFF_CLOD, PUFF_DUST, PUFF_FIRE, PUFF_SHOCK_DUST, PUFF_SMOKE, PUFF_SPARK,
    PUFF_TREE_FIRE, PUFF_TREE_SMOKE,
};
use crate::camera::Camera;
use crate::gpu::{Buffer, Gpu, GpuError};
use crate::gpu_consts::city_look as look;
use crate::renderer::wreck_fx::hash;
use ash::vk;
use glam::{Vec2, Vec3};
use mc_map::{city, MapFile, Prop};
use mc_sim::mirror::city::StructureView;
use mc_sim::mirror::{RenderFrame, SimEvent, UnitInstance, KIND_PROP};

/// A pane's worth of glass thrown off a building (puffs.wgsl).
pub(super) const PUFF_GLASS: f32 = crate::gpu_consts::puff::GLASS as f32;
/// Most buildings drawn coming down at once (dynamic slots).
pub(super) const MOST_FALLING: usize = 96;
/// Seconds a building takes to come down, and how much longer its dust rolls.
const FALL: f32 = 5.5;
/// Puffs all fires may lay in one tick, at most: a burning district shares them.
const FIRE_PUFFS: usize = 220;
/// Seconds a gutted shell keeps smoking after its fire is out.
const SMOULDER: f32 = 90.0;
/// The rubble heap model's footprint at scale 1 (`mc-models` `city_rubble`),
/// as `mc_map::city` gives no solid plan for it: about 18 by 14 m.
const HEAP: Vec2 = Vec2::new(18.0, 14.0);

/// A city structure's solid shape in the world, for throwing things off it.
#[derive(Clone, Copy, Debug)]
pub(super) struct Shape {
    /// The middle of its footprint, its half extents along and across its
    /// heading, and that heading's unit vector.
    centre: Vec2,
    half: Vec2,
    along: Vec2,
    /// Its tallest part's top over the ground, metres.
    top: f32,
    /// 0-1: how much of it is glass.
    glazing: f32,
    burns: bool,
}

impl Shape {
    fn of(p: &Prop) -> Option<Shape> {
        let s = city::structure(p.kind).filter(|s| s.health > 0)?;
        let scale = p.scale_milli as f32 / 1000.0;
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for &(cx, cy, hx, hy) in s.plan {
            let (c, h) = (
                Vec2::new(cx as f32, cy as f32),
                Vec2::new(hx as f32, hy as f32),
            );
            lo = lo.min(c - h);
            hi = hi.max(c + h);
        }
        let heading = p.heading.to_radians_f32();
        let along = Vec2::from_angle(heading);
        let mid = (lo + hi) * 0.5 * scale;
        let origin = Vec2::from(p.pos.to_f32());
        Some(Shape {
            centre: origin + along.rotate(mid),
            half: (hi - lo) * 0.5 * scale,
            along,
            top: s.tops.iter().copied().max().unwrap_or(0) as f32 * scale,
            glazing: s.glazing as f32 / 255.0,
            burns: s.burns,
        })
    }

    /// A point on its walls: `u` and `v` (-1 to 1) along and across, put out
    /// on the nearer face, at `h` (0 to 1) of its height.
    fn wall_point(&self, u: f32, v: f32, h: f32) -> (Vec3, Vec2) {
        let across = self.along.perp();
        let (u, v) = if u.abs() > v.abs() {
            (u.signum(), v)
        } else {
            (u, v.signum())
        };
        let local = Vec2::new(u * self.half.x, v * self.half.y);
        let at = self.centre + self.along * local.x + across * local.y;
        let out = if local.x.abs() >= self.half.x - 1e-3 {
            self.along * u.signum()
        } else {
            across * v.signum()
        };
        (at.extend(self.top * h), out)
    }

    fn radius(&self) -> f32 {
        self.half.length()
    }
}

/// One solid part of a structure in the world: an oriented box from the
/// ground to `top` (`mc_map::city`'s plan as laid).
#[derive(Clone, Copy, Debug)]
struct Part {
    prop: u32,
    centre: Vec2,
    half: Vec2,
    along: Vec2,
    top: f32,
}

impl Part {
    fn covers(&self, p: Vec2) -> bool {
        let d = p - self.centre;
        d.dot(self.along).abs() <= self.half.x && d.dot(self.along.perp()).abs() <= self.half.y
    }
}

/// Edge of the buckets the solid parts are filed in, metres.
const BUCKET: f32 = 64.0;

/// A building on its way down.
#[derive(Clone, Copy)]
struct Falling {
    instance: UnitInstance,
    prop: u32,
    start: f32,
    top: f32,
    lean: f32,
}

pub(super) struct CityFx {
    /// Per map prop: its shape, for the city's structures.
    shapes: Vec<Option<Shape>>,
    /// Per map prop, the word the entity shader reads (`gpu_consts::city_look`).
    looks: Vec<u32>,
    buffer: Buffer,
    /// The props whose word is set (all others are whole and zero).
    marked: Vec<u32>,
    /// Rubble heaps: the static entity, and the prop it lies under.
    heaps: Vec<(u32, u32)>,
    falling: Vec<Falling>,
    /// Burning or smouldering structures, and when their fire started or went out.
    fires: Vec<(u32, f32, bool)>,
    /// Every structure's solid parts, filed by bucket (`buckets` wide), and the
    /// sim's dead props as last seen: what a line of fire must clear.
    parts: Vec<Part>,
    buckets: Vec<Vec<u32>>,
    buckets_wide: usize,
    dead: Vec<u32>,
}

/// The rubble heaps for every city structure a map has, as static entities
/// from `first` on, drawn as `blueprint` (the rubble model's slot): enough
/// heaps, side by side and scaled to the footprint's width, to cover it. Each
/// carries its building's prop as its `unit_id`. Also the heaps' list.
pub(super) fn heaps(
    map: &MapFile,
    first: u32,
    blueprint: u32,
    height: impl Fn(Vec2) -> f32,
) -> (Vec<UnitInstance>, Vec<(u32, u32)>) {
    let mut instances = Vec::new();
    let mut list = Vec::new();
    for (i, p) in map.props().iter().enumerate() {
        let Some(shape) = Shape::of(p) else {
            continue;
        };
        // Heaps across the narrow way, end to end along the long one.
        let (long, narrow, along) = if shape.half.x >= shape.half.y {
            (shape.half.x, shape.half.y, shape.along)
        } else {
            (shape.half.y, shape.half.x, shape.along.perp())
        };
        let scale = (2.0 * narrow * 1.15 / HEAP.y).clamp(0.6, 4.5);
        let n = ((2.0 * long) / (HEAP.x * scale * 0.85)).ceil().max(1.0) as usize;
        for k in 0..n {
            let t = (k as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let jitter = Vec2::new(
                hash(i as u32, k as u32 * 3),
                hash(i as u32, k as u32 * 3 + 1),
            ) - 0.5;
            let at = shape.centre + along * (t * long) + jitter * narrow * 0.25;
            let heading = along.y.atan2(along.x) + (hash(i as u32, k as u32 * 3 + 2) - 0.5) * 0.6;
            let pos = [at.x, at.y, height(at)];
            let mut u: UnitInstance = bytemuck::Zeroable::zeroed();
            u.pos = pos;
            u.prev_pos = pos;
            u.heading = heading;
            u.prev_heading = heading;
            u.blueprint = blueprint;
            u.owner_flags = KIND_PROP;
            u.health = 1.0;
            u.build = 1.0;
            u.radius = 4.0;
            u.unit_id = i as u32;
            u.packed = (scale * (0.9 + 0.2 * hash(i as u32, 77 + k as u32)) * 1000.0) as u32;
            list.push((first + instances.len() as u32, i as u32));
            instances.push(u);
        }
    }
    (instances, list)
}

impl CityFx {
    pub(super) fn new(
        gpu: &Gpu,
        map: &MapFile,
        staged: usize,
        heaps: Vec<(u32, u32)>,
    ) -> Result<CityFx, GpuError> {
        // A word for each of the map's props, and for each prop a shot stages after them.
        let props = (map.props().len() + staged).max(1);
        let buffer = gpu.host_buffer((props * 4) as u64, vk::BufferUsageFlags::STORAGE_BUFFER)?;
        buffer.write(0, &vec![0u8; props * 4]);
        let size = Vec2::from(map.info().size_metres().to_f32());
        let buckets_wide = (size.x / BUCKET).ceil().max(1.0) as usize;
        let buckets_high = (size.y / BUCKET).ceil().max(1.0) as usize;
        let mut parts = Vec::new();
        let mut buckets = vec![Vec::new(); buckets_wide * buckets_high];
        for (i, p) in map.props().iter().enumerate() {
            let Some(s) = city::structure(p.kind).filter(|s| s.health > 0) else {
                continue;
            };
            let scale = p.scale_milli as f32 / 1000.0;
            let along = Vec2::from_angle(p.heading.to_radians_f32());
            let origin = Vec2::from(p.pos.to_f32());
            for (&(cx, cy, hx, hy), &top) in s.plan.iter().zip(s.tops) {
                let part = Part {
                    prop: i as u32,
                    centre: origin + along.rotate(Vec2::new(cx as f32, cy as f32) * scale),
                    half: Vec2::new(hx as f32, hy as f32) * scale,
                    along,
                    top: top as f32 * scale,
                };
                let r = part.half.length();
                let lo = ((part.centre - r) / BUCKET).floor().max(Vec2::ZERO);
                let hi = ((part.centre + r) / BUCKET).floor();
                for by in lo.y as usize..=(hi.y as usize).min(buckets_high - 1) {
                    for bx in lo.x as usize..=(hi.x as usize).min(buckets_wide - 1) {
                        buckets[by * buckets_wide + bx].push(parts.len() as u32);
                    }
                }
                parts.push(part);
            }
        }
        Ok(CityFx {
            parts,
            buckets,
            buckets_wide,
            dead: Vec::new(),
            shapes: map.props().iter().map(Shape::of).collect(),
            looks: vec![0; props],
            buffer,
            marked: Vec::new(),
            heaps,
            falling: Vec::new(),
            fires: Vec::new(),
        })
    }

    pub(super) fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        gpu.destroy_buffer(std::mem::replace(&mut self.buffer, Buffer::null()));
    }

    /// Hides, in `bits` (one per static entity), every rubble heap whose
    /// building still stands (`dead`: the sim's dead props).
    pub(super) fn hide_heaps(&mut self, dead: &[u32], bits: &mut [u32]) {
        self.dead.clear();
        self.dead.extend_from_slice(dead);
        for &(entity, prop) in &self.heaps {
            let down = dead
                .get(prop as usize / 32)
                .is_some_and(|w| w & (1 << (prop % 32)) != 0);
            if !down {
                if let Some(w) = bits.get_mut(entity as usize / 32) {
                    *w |= 1 << (entity % 32);
                }
            }
        }
    }

    /// Sets the look of staged prop `index` (`Renderer::new_staged`): a prop shot's
    /// damage, fire or gutting, held until the next call.
    pub(super) fn stage_look(&mut self, index: usize, word: u32) {
        let prop = self.shapes.len() + index;
        if let Some(w) = self.looks.get_mut(prop) {
            *w = word;
            self.buffer.write(0, bytemuck::cast_slice(&self.looks));
        }
    }

    /// Writes each hurt structure's look for the entity shader.
    pub(super) fn set_looks(&mut self, city: &[StructureView], tick: u32) {
        for &prop in &self.marked {
            self.looks[prop as usize] = 0;
        }
        self.marked.clear();
        for s in city {
            let Some(word) = self.looks.get_mut(s.prop as usize) else {
                continue;
            };
            let age = tick.saturating_sub(s.since) / mc_core::TICKS_PER_SECOND;
            *word = (255 - s.health) as u32
                | if s.flags & mc_sim::city::BURNING != 0 {
                    look::BURNING
                } else {
                    0
                }
                | if s.flags & mc_sim::city::GUTTED != 0 {
                    look::GUTTED
                } else {
                    0
                }
                | if s.flags & mc_sim::city::DOWN != 0 {
                    look::DOWN
                } else {
                    0
                }
                | age.min(look::AGE_MAX) << look::AGE_SHIFT;
            self.marked.push(s.prop);
        }
        self.buffer.write(0, bytemuck::cast_slice(&self.looks));
    }

    /// The top of the standing structure over `p`, metres over the ground; 0
    /// where none stands.
    pub(super) fn standing_top(&self, p: Vec2) -> f32 {
        let (bx, by) = ((p.x / BUCKET).floor(), (p.y / BUCKET).floor());
        if bx < 0.0 || by < 0.0 || bx as usize >= self.buckets_wide {
            return 0.0;
        }
        let Some(bucket) = self
            .buckets
            .get(by as usize * self.buckets_wide + bx as usize)
        else {
            return 0.0;
        };
        bucket
            .iter()
            .map(|&i| &self.parts[i as usize])
            .filter(|part| {
                let dead = self
                    .dead
                    .get(part.prop as usize / 32)
                    .is_some_and(|w| w & (1 << (part.prop % 32)) != 0);
                !dead && part.covers(p)
            })
            .map(|part| part.top)
            .fold(0.0, f32::max)
    }

    /// The burning buildings nearest `focus` (a few dozen; past that a fire is
    /// its glow and smoke), each as its middle a third of the way up, its size
    /// and how long it has burned.
    pub(super) fn fire_lights(&self, time: f32, focus: Vec3) -> Vec<(Vec3, f32, f32)> {
        let mut lit: Vec<(Vec3, f32, f32)> = self
            .fires
            .iter()
            .filter(|f| f.2)
            .filter_map(|&(prop, since, _)| {
                let s = self.shapes.get(prop as usize).copied().flatten()?;
                Some((s.centre.extend(s.top * 0.3), s.radius() * 2.0, time - since))
            })
            .collect();
        if lit.len() > 48 {
            lit.sort_by(|a, b| {
                a.0.distance_squared(focus)
                    .total_cmp(&b.0.distance_squared(focus))
            });
            lit.truncate(48);
        }
        lit
    }

    /// The buildings coming down at `time`, posed for the vertex shader: each
    /// sinks into its own dust, slow to start, leaning a little as it goes.
    pub(super) fn falling_instances(&self, time: f32) -> Vec<UnitInstance> {
        self.falling
            .iter()
            .filter(|f| time - f.start < FALL)
            .map(|f| {
                let t = ((time - f.start) / FALL).clamp(0.0, 1.0);
                let sink = f.top * 0.95 * t * t;
                let lean = f.lean * t;
                let mut u = f.instance;
                u.arm_pitch = [lean, lean, sink, u.arm_pitch[3]];
                u
            })
            .collect()
    }
}

impl Renderer {
    /// What the sim's city did this tick: glass and masonry off every hit,
    /// fires catching and collapses starting; then the fires and the dust of
    /// the ones coming down.
    pub(super) fn city_events(&mut self, frame: &RenderFrame, time: f32, camera: &Camera) {
        self.city_fx.falling.retain(|f| time - f.start < FALL + 2.0);
        let reach = camera.distance * 2.5 + 400.0;
        for event in &frame.events {
            match *event {
                SimEvent::Impact {
                    pos,
                    splash,
                    on_structure: Some(prop),
                    ..
                } => {
                    let at = Vec3::from(pos.to_f32());
                    if at.distance(camera.focus) < reach {
                        self.city_hit(prop, at, splash.to_f32(), time);
                    }
                }
                SimEvent::StructureAlight { prop, pos } => {
                    let at = Vec3::from(pos.to_f32());
                    self.city_fx.fires.push((prop, time, true));
                    // The fire takes with a burst of flame out of the windows.
                    for _ in 0..6 {
                        let off =
                            Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.3) * 6.0;
                        self.push_puff(PUFF_FIRE, at + off, Vec3::Z * 4.0, time, 1.4, (3.0, 8.0));
                    }
                }
                SimEvent::StructureCollapsed {
                    prop,
                    pos,
                    radius,
                    height,
                } => self.city_collapse(
                    prop,
                    Vec3::from(pos.to_f32()),
                    radius.to_f32(),
                    height.to_f32(),
                    time,
                ),
                _ => {}
            }
        }
        self.city_fires(frame, time, camera);
        self.city_dust(time, camera);
    }

    /// A shot struck a building at `at`: panes burst outward and tumble down
    /// glinting, chips of its face fly, a puff of its dust.
    fn city_hit(&mut self, prop: u32, at: Vec3, splash: f32, time: f32) {
        let Some(Some(shape)) = self.city_fx.shapes.get(prop as usize).copied() else {
            return;
        };
        // Outward from the face it struck (or up off the roof).
        let local = at.truncate() - shape.centre;
        let (u, v) = (
            local.dot(shape.along) / shape.half.x.max(1.0),
            local.dot(shape.along.perp()) / shape.half.y.max(1.0),
        );
        let out = if at.z >= shape.top - 1.0 {
            Vec2::ZERO
        } else if u.abs() > v.abs() {
            shape.along * u.signum()
        } else {
            shape.along.perp() * v.signum()
        };
        let big = (splash / 6.0).clamp(0.4, 3.0);
        let panes = (shape.glazing * (6.0 + 10.0 * big)).round() as usize;
        for _ in 0..panes {
            let spread = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            );
            let vel = (out * (3.0 + 6.0 * self.scatter.unit())).extend(1.5) + spread * 3.0 * big;
            let (life, size) = (
                6.0 + 4.0 * self.scatter.unit(),
                0.35 + 0.4 * self.scatter.unit(),
            );
            self.push_puff(PUFF_GLASS, at + spread, vel, time, life, (size, 0.0));
        }
        let concrete = Vec3::new(0.32, 0.3, 0.27);
        for _ in 0..(3.0 * big) as usize + 2 {
            let spread = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.unit(),
            );
            let vel = (out * 5.0).extend(3.0) + spread * 5.0;
            self.push_puff(PUFF_CLOD, at, vel, time, 1.6, (0.4 * big, 0.3));
        }
        self.push_puff_with_motion(
            PUFF_DUST,
            at + (out * 2.0).extend(0.0),
            (out * 2.0).extend(0.8),
            time,
            3.5,
            (2.0 * big, 7.0 * big),
            concrete,
        );
    }

    /// A building is down: a dust cloud rolls out from its foot and boils up
    /// its height, debris and glass are thrown, and its copy starts sinking.
    fn city_collapse(&mut self, prop: u32, at: Vec3, radius: f32, height: f32, time: f32) {
        let Some(Some(shape)) = self.city_fx.shapes.get(prop as usize).copied() else {
            return;
        };
        let concrete = Vec3::new(0.34, 0.32, 0.29);
        let ring = (8.0 + radius / 4.0).min(28.0) as usize;
        for k in 0..ring {
            let a = k as f32 / ring as f32 * std::f32::consts::TAU + self.scatter.signed() * 0.2;
            let dir = Vec2::from_angle(a);
            let from = at + (dir * radius * 0.8).extend(2.0);
            self.push_puff_with_motion(
                PUFF_SHOCK_DUST,
                from,
                (dir * (6.0 + radius * 0.15)).extend(1.5),
                time,
                9.0,
                (radius * 0.35, radius * 0.9),
                concrete,
            );
        }
        let column = (height / 12.0).clamp(3.0, 16.0) as usize;
        for k in 0..column {
            let up = (k as f32 + 0.5) / column as f32;
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * radius * 0.4;
            self.push_puff_with_motion(
                PUFF_DUST,
                at + off + Vec3::Z * height * up * 0.7,
                Vec3::new(0.0, 0.0, 2.0 + 3.0 * up),
                time + up * 1.5,
                12.0,
                (radius * 0.4, radius * 1.1),
                concrete,
            );
        }
        for _ in 0..(radius as usize).min(40) {
            let (p, out) = shape.wall_point(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.unit() * 0.6,
            );
            let vel = (out * 6.0).extend(4.0)
                + Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.unit(),
                ) * 4.0;
            self.push_puff(PUFF_CLOD, p, vel, time, 2.2, (0.9, 0.5));
        }
        let panes = (shape.glazing * 40.0) as usize;
        for _ in 0..panes {
            let (p, out) = shape.wall_point(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.unit(),
            );
            let vel = (out * (2.0 + 4.0 * self.scatter.unit())).extend(0.5);
            let start = time + self.scatter.unit() * 2.0;
            self.push_puff(PUFF_GLASS, p, vel, start, 8.0, (0.5, 0.0));
        }
        let Some(&instance) = self.prop_instances.get(prop as usize) else {
            return;
        };
        if self.city_fx.falling.iter().any(|f| f.prop == prop) {
            return;
        }
        if self.city_fx.falling.len() >= MOST_FALLING {
            self.city_fx.falling.remove(0);
        }
        let lean = self.scatter.signed() * 0.05;
        self.city_fx.falling.push(Falling {
            instance,
            prop,
            start: time,
            top: shape.top,
            lean,
        });
    }

    /// Flames in the windows of every burning building near the camera and
    /// smoke over it; smoke still curling off the gutted shells.
    fn city_fires(&mut self, frame: &RenderFrame, time: f32, camera: &Camera) {
        let burning: Vec<(u32, bool)> = frame
            .city
            .iter()
            .filter(|s| s.flags & mc_sim::city::DOWN == 0)
            .filter_map(|s| {
                if s.flags & mc_sim::city::BURNING != 0 {
                    Some((s.prop, true))
                } else if s.flags & mc_sim::city::GUTTED != 0 {
                    Some((s.prop, false))
                } else {
                    None
                }
            })
            .collect();
        // When each fire started (or went out), as the renderer saw it.
        let fires = &mut self.city_fx.fires;
        fires.retain(|f| burning.contains(&(f.0, f.2)));
        for &(prop, alight) in &burning {
            if !fires.iter().any(|f| f.0 == prop && f.2 == alight) {
                // Seen first already burning (a restore, a late join): as if lit a while ago.
                fires.push((
                    prop,
                    time - if alight { 0.0 } else { SMOULDER * 0.5 },
                    alight,
                ));
            }
        }
        let reach = camera.distance * 2.5 + 400.0;
        let near: Vec<(Shape, f32, bool)> = self
            .city_fx
            .fires
            .iter()
            .filter_map(|&(prop, since, alight)| {
                let shape = self.city_fx.shapes.get(prop as usize).copied().flatten()?;
                let age = time - since;
                (shape.burns
                    && shape.centre.distance(camera.focus.truncate()) < reach
                    && (alight || age < SMOULDER))
                    .then_some((shape, age, alight))
            })
            .collect();
        if near.is_empty() {
            return;
        }
        let share = (FIRE_PUFFS as f32 / (near.len() as f32 * 5.0)).min(1.0);
        let wind = self.sky.wind_heading();
        for (shape, age, alight) in near {
            if self.scatter.unit() > share {
                continue;
            }
            let size = shape.radius().clamp(8.0, 40.0);
            if alight {
                // Fire takes the building floor by floor: low first, all of it within a minute.
                let height = (0.25 + age / 60.0).min(1.0);
                for _ in 0..3 {
                    let (p, out) = shape.wall_point(
                        self.scatter.signed(),
                        self.scatter.signed(),
                        self.scatter.unit() * height,
                    );
                    let flame = p + (out * 0.8).extend(0.0);
                    self.push_puff(
                        PUFF_TREE_FIRE,
                        flame,
                        (out * 1.5).extend(3.5),
                        time,
                        1.1,
                        (size * 0.08, size * 0.2),
                    );
                }
                if self.scatter.unit() < 0.3 {
                    let (p, _) = shape.wall_point(
                        self.scatter.signed(),
                        self.scatter.signed(),
                        self.scatter.unit() * height,
                    );
                    self.push_puff(
                        PUFF_SPARK,
                        p,
                        Vec3::new(0.5, 0.5, 9.0),
                        time,
                        1.2,
                        (0.2, 0.03),
                    );
                }
            }
            // Black smoke boils off the roof, thinning as a gutted shell cools.
            let thick = if alight {
                1.0
            } else {
                (1.0 - age / SMOULDER).max(0.0) * 0.5
            };
            if self.scatter.unit() < 0.6 * thick + 0.1 {
                let roof = shape
                    .centre
                    .extend(shape.top * (0.8 + 0.2 * self.scatter.unit()))
                    + Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * size * 0.4;
                let kind = if alight { PUFF_SMOKE } else { PUFF_TREE_SMOKE };
                self.push_puff_with_motion(
                    kind,
                    roof,
                    (wind * 2.0).extend(6.0),
                    time,
                    9.0,
                    (size * 0.3, size * (0.9 + thick)),
                    Vec3::new(0.05, 0.045, 0.04),
                );
            }
        }
    }

    /// Dust pouring from the foot of every building coming down.
    fn city_dust(&mut self, time: f32, camera: &Camera) {
        let reach = camera.distance * 2.5 + 400.0;
        let falling: Vec<(Shape, f32)> = self
            .city_fx
            .falling
            .iter()
            .filter_map(|f| {
                let shape = self
                    .city_fx
                    .shapes
                    .get(f.prop as usize)
                    .copied()
                    .flatten()?;
                let age = time - f.start;
                (age < FALL && shape.centre.distance(camera.focus.truncate()) < reach)
                    .then_some((shape, age))
            })
            .collect();
        let concrete = Vec3::new(0.34, 0.32, 0.29);
        for (shape, age) in falling {
            let pace = 1.0 - age / FALL;
            let size = shape.radius();
            for _ in 0..3 {
                let (p, out) = shape.wall_point(self.scatter.signed(), self.scatter.signed(), 0.05);
                self.push_puff_with_motion(
                    PUFF_DUST,
                    p,
                    (out * (4.0 + 6.0 * pace)).extend(2.0),
                    time,
                    7.0,
                    (size * 0.2, size * 0.6),
                    concrete,
                );
            }
            // The top coming down through its own dust.
            let sinking = shape.top * (1.0 - (age / FALL).powi(2) * 0.95);
            let top = shape.centre.extend(sinking);
            self.push_puff_with_motion(
                PUFF_DUST,
                top,
                Vec3::Z * 1.5,
                time,
                5.0,
                (size * 0.3, size * 0.7),
                concrete,
            );
        }
    }
}
