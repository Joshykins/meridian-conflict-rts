//! Heat haze over engine exhausts: the hot air rising off a running engine's ports bends
//! the scene behind it. A model marks its ports (`models::Exhaust`, `MeshBuilder::add_exhaust`);
//! each frame the ports of the running units nearest the eye become plumes, columns of
//! air that rise and widen from the mouth, and the tone map (`screen.wgsl`, `haze_bend`)
//! shifts what is seen through a plume by a few pixels of upward-scrolling noise.
//!
//! Presentation only. A unit's engine idles faintly and works harder as the unit moves;
//! wrecks, radar contacts, units in a factory or still being built have no running engine.

use std::collections::HashMap;
use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use mc_sim::mirror::{UnitInstance, KIND_WRECK, STATE_RADAR};

use crate::camera::Camera;
use crate::gpu::{Buffer, Gpu, GpuError};
use crate::gpu_consts::haze::{MAX_PLUMES, REACH_M};
use crate::models::Exhaust;

/// One rising column of hot air (`HeatPlume` in screen.wgsl).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct GpuHeatPlume {
    /// The middle of the port's mouth, world metres.
    pub(crate) port: [f32; 3],
    /// The column's radius at the mouth.
    pub(crate) radius: f32,
    /// Which way the column rises (unit length).
    pub(crate) axis: [f32; 3],
    /// How far up it can be seen.
    pub(crate) height: f32,
    /// How hard the engine works: 0 cold, 1 flat out.
    pub(crate) strength: f32,
    /// Sets each plume's noise apart.
    pub(crate) seed: f32,
    pub(crate) _pad: [f32; 2],
}

/// Engine load: what a unit standing still burns, and the speed (m/s) at which it runs flat out.
const IDLE: f32 = 0.3;
const FULL_SPEED: f32 = 4.0;
/// The column is this many mouth radii tall, and widens by this much of its height.
const HEIGHT_RADII: f32 = 6.0;

/// A unit with exhausts as the last tick left it.
#[derive(Clone, Copy)]
struct Running {
    id: u32,
    model: u32,
    prev_pos: Vec3,
    pos: Vec3,
    prev_heading: f32,
    heading: f32,
}

pub(super) struct HeatHaze {
    /// Per model slot (`UnitInstance::blueprint`): its exhaust ports.
    ports: Vec<Vec<Exhaust>>,
    buffer: Buffer,
    units: Vec<Running>,
    /// Each running unit's engine load, eased so a stop or start does not snap.
    load: HashMap<u32, f32>,
    /// Seconds a tick lasts, and the renderer time of the last frame.
    tick_seconds: f32,
    last_time: f32,
    scratch: Vec<(f32, GpuHeatPlume)>,
}

impl HeatHaze {
    pub(super) fn new(gpu: &Gpu, ports: Vec<Vec<Exhaust>>) -> Result<Self, GpuError> {
        let buffer = gpu.host_buffer(
            (16 + MAX_PLUMES as usize * size_of::<GpuHeatPlume>()) as u64,
            ash::vk::BufferUsageFlags::STORAGE_BUFFER,
        )?;
        buffer.write(0, &vec![0u8; buffer.size as usize]);
        Ok(Self {
            ports,
            buffer,
            units: Vec::new(),
            load: HashMap::new(),
            tick_seconds: 0.1,
            last_time: 0.0,
            scratch: Vec::new(),
        })
    }

    /// The exhaust ports of model slot `model` (`UnitInstance::blueprint`), in the unit's
    /// frame at its drawn size.
    pub(super) fn ports(&self, model: u32) -> &[Exhaust] {
        self.ports.get(model as usize).map_or(&[], Vec::as_slice)
    }

    pub(super) fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        gpu.destroy_buffer(std::mem::replace(&mut self.buffer, Buffer::null()));
    }

    /// Takes the running units with exhausts from a new tick.
    pub(super) fn note(&mut self, units: &[UnitInstance], tick_seconds: f32) {
        self.tick_seconds = tick_seconds.max(0.01);
        self.units.clear();
        let off = KIND_WRECK
            | STATE_RADAR
            | ((mc_sim::tables::flag::IN_FACTORY | mc_sim::tables::flag::UNDER_CONSTRUCTION)
                as u32)
                << 8;
        for u in units {
            let has_ports = self
                .ports
                .get(u.blueprint as usize)
                .is_some_and(|p| !p.is_empty());
            if !has_ports || u.owner_flags & off != 0 || u.build < 1.0 {
                continue;
            }
            self.units.push(Running {
                id: u.unit_id,
                model: u.blueprint,
                prev_pos: Vec3::from(u.prev_pos),
                pos: Vec3::from(u.pos),
                prev_heading: u.prev_heading,
                heading: u.heading,
            });
        }
        let live: std::collections::HashSet<u32> = self.units.iter().map(|u| u.id).collect();
        self.load.retain(|id, _| live.contains(id));
    }

    /// Hands the GPU this frame's plumes: those of the running units nearest the eye.
    pub(super) fn upload(&mut self, time: f32, alpha: f32, camera: &Camera) {
        let dt = (time - self.last_time).clamp(0.0, 0.25);
        self.last_time = time;
        let eye = camera.eye();
        self.scratch.clear();
        for u in &self.units {
            let travel = u.pos - u.prev_pos;
            let speed = travel.truncate().length() / self.tick_seconds;
            let goal = IDLE + (1.0 - IDLE) * (speed / FULL_SPEED).min(1.0);
            let load = self.load.entry(u.id).or_insert(goal);
            // Revs climb quickly and fall away slowly.
            let rate = if goal > *load { 3.0 } else { 0.8 };
            *load += (goal - *load) * (1.0 - (-rate * dt).exp());
            let strength = *load;
            let pos = u.prev_pos + travel * alpha;
            let turn = (u.heading - u.prev_heading + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            let (s, c) = (u.prev_heading + turn * alpha).sin_cos();
            let rotate = |v: [f32; 3]| Vec3::new(v[0] * c - v[1] * s, v[0] * s + v[1] * c, v[2]);
            // The air the hull leaves behind drags the column back.
            let drift = (travel.truncate() / self.tick_seconds).extend(0.0) * 0.12;
            for (i, port) in self.ports[u.model as usize].iter().enumerate() {
                let at = pos + rotate(port.at);
                let distance = at.distance(eye);
                if distance > REACH_M {
                    continue;
                }
                // Hot gas leaves along the stack and then rises.
                let axis = (rotate(port.toward) * 0.35 + Vec3::Z - drift).normalize_or(Vec3::Z);
                self.scratch.push((
                    distance,
                    GpuHeatPlume {
                        port: at.to_array(),
                        radius: port.radius,
                        axis: axis.to_array(),
                        height: port.radius * HEIGHT_RADII,
                        strength,
                        seed: (u.id.wrapping_mul(7) + i as u32) as f32 * 0.618,
                        _pad: [0.0; 2],
                    },
                ));
            }
        }
        // Nearest first; distances can tie, and the stable sort keeps the unit order then.
        self.scratch.sort_by(|a, b| a.0.total_cmp(&b.0));
        // A deliberate cap: past the nearest plumes the rest are too small to see bend.
        self.scratch.truncate(MAX_PLUMES as usize);
        let plumes: Vec<GpuHeatPlume> = self.scratch.iter().map(|p| p.1).collect();
        self.buffer
            .write(0, bytemuck::cast_slice(&[plumes.len() as u32, 0, 0, 0]));
        self.buffer.write(16, bytemuck::cast_slice(&plumes));
    }
}
