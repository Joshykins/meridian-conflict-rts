//! The ground a gravitic engine breaks, as the terrain draws it (scene set 39, scars.wgsl
//! `rifts_at`): the cracks round its foot as it gathers, the rift's fissure from the foot
//! to the mark, and the well's ground broken into plates. Each is one record; the engine
//! feeds the ones it is working each tick (how bright, how hard light pulses along them),
//! and between ticks their light and their plates' heave ease toward that, so nothing
//! snaps. Let go, a record's light dies over a few seconds and its plates settle, and it
//! stays as a dark scar until newer breaks push the oldest out.

use ash::vk;
use bytemuck::{Pod, Zeroable};
use glam::Vec2;
use std::mem::size_of;

use crate::gpu::{Buffer, Gpu, GpuError};
use crate::gpu_consts::rift;

/// One break (`Rift` in bindings.wgsl).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default)]
pub(crate) struct GpuRift {
    /// A line's two ends (xy, zw); a disc's middle in xy.
    pub(crate) line: [f32; 4],
    /// `rift::*` kind, reach (a disc's radius, a line's half width), a foot's inner
    /// radius, seed.
    pub(crate) shape: [f32; 4],
    /// When it began (the renderer's clock), how fast its crack front runs (m/s), how far
    /// the front may have got (metres), the share of its plates' heave showing.
    pub(crate) time: [f32; 4],
    /// How bright its cracks glow, how hard light pulses along it toward the mark, how much
    /// of the light's white heart shows, how hot the head of a line burns.
    pub(crate) glow: [f32; 4],
}

/// Seconds a break's light takes to die once it is let go, and its plates to settle.
const LIGHT_DIES: f32 = 2.5;
const PLATES_SETTLE: f32 = 6.0;
/// How quickly a fed break's light follows what it is fed (1/s).
const FOLLOW: f32 = 6.0;

struct Record {
    /// The engine (unit id) that broke it.
    engine: u32,
    kind: u32,
    gpu: GpuRift,
    /// Still the engine's work; once let go, when.
    released: Option<f32>,
    /// What the engine feeds it this tick: glow, pulse, heart, head, heave.
    fed: [f32; 5],
    /// The light and heave as they stand, eased toward `fed`.
    now: [f32; 5],
}

pub(crate) struct RiftGround {
    records: Vec<Record>,
    buffer: Buffer,
    seed: u32,
    last: f32,
}

impl RiftGround {
    pub(super) fn new(gpu: &Gpu) -> Result<Self, GpuError> {
        let buffer = gpu.host_buffer(
            (16 + rift::MAX as usize * size_of::<GpuRift>()) as u64,
            vk::BufferUsageFlags::STORAGE_BUFFER,
        )?;
        buffer.write(0, &vec![0u8; buffer.size as usize]);
        Ok(Self {
            records: Vec::new(),
            buffer,
            seed: 0x51ed_270b,
            last: 0.0,
        })
    }

    /// What the scene set's binding 39 reads.
    pub(crate) fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub(crate) fn destroy(&mut self, gpu: &Gpu) {
        gpu.destroy_buffer(std::mem::replace(&mut self.buffer, Buffer::null()));
    }

    /// A new match: the ground is whole again.
    pub(super) fn clear(&mut self) {
        self.records.clear();
    }

    fn next_seed(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (self.seed >> 9) as f32 / (1u32 << 23) as f32
    }

    /// The engine's break of `kind` it is still working, if any.
    fn live(&mut self, engine: u32, kind: u32) -> Option<&mut Record> {
        self.records
            .iter_mut()
            .find(|r| r.engine == engine && r.kind == kind && r.released.is_none())
    }

    /// Whether the engine has a break of `kind` it is still working.
    pub(super) fn has_live(&self, engine: u32, kind: u32) -> bool {
        self.records
            .iter()
            .any(|r| r.engine == engine && r.kind == kind && r.released.is_none())
    }

    fn push(&mut self, engine: u32, kind: u32, gpu: GpuRift) {
        if self.records.len() >= rift::MAX as usize {
            // A deliberate cap (cosmetic): the oldest scar the engines have let go goes
            // first; with none let go, the oldest of all.
            let at = self
                .records
                .iter()
                .position(|r| r.released.is_some())
                .unwrap_or(0);
            self.records.remove(at);
        }
        self.records.push(Record {
            engine,
            kind,
            gpu,
            released: None,
            fed: [0.0; 5],
            now: [0.0; 5],
        });
    }

    /// The cracks round an engine's foot at `centre`: out from `inner` (its lot) to `reach`
    /// now. Started at `time` if it has none.
    pub(super) fn foot(&mut self, engine: u32, centre: Vec2, inner: f32, reach: f32, time: f32) {
        if let Some(r) = self.live(engine, rift::FOOT) {
            r.gpu.shape[1] = reach;
            return;
        }
        let seed = self.next_seed();
        self.push(
            engine,
            rift::FOOT,
            GpuRift {
                line: [centre.x, centre.y, centre.x, centre.y],
                shape: [rift::FOOT as f32, reach, inner, seed],
                time: [time, 0.0, 0.0, 0.0],
                glow: [0.0; 4],
            },
        );
    }

    /// The rift tearing from `from` to `to` at `speed` m/s from `time`, `width` its broken
    /// band's half width.
    pub(super) fn line(
        &mut self,
        engine: u32,
        from: Vec2,
        to: Vec2,
        speed: f32,
        width: f32,
        time: f32,
    ) {
        self.release_kind(engine, rift::LINE, time);
        let seed = self.next_seed();
        self.push(
            engine,
            rift::LINE,
            GpuRift {
                line: [from.x, from.y, to.x, to.y],
                shape: [rift::LINE as f32, width, 0.0, seed],
                // The front may run a tick ahead of where the sim last had it.
                time: [time, speed, speed * 0.15, 0.0],
                glow: [0.0; 4],
            },
        );
    }

    /// How far the engine's rift may have run now: where the sim has its front, plus
    /// `ahead` metres (a tick of running), so a paused game never shows it running on.
    pub(super) fn line_cap(&mut self, engine: u32, run: f32) {
        if let Some(r) = self.live(engine, rift::LINE) {
            r.gpu.time[2] = run;
        }
    }

    /// The well shattering `radius` round `centre` from `time`.
    pub(super) fn well(&mut self, engine: u32, centre: Vec2, radius: f32, time: f32) {
        self.release_kind(engine, rift::WELL, time);
        let seed = self.next_seed();
        self.push(
            engine,
            rift::WELL,
            GpuRift {
                line: [centre.x, centre.y, centre.x, centre.y],
                // The shatter crosses the well in about a third of a second.
                shape: [rift::WELL as f32, radius, 0.0, seed],
                time: [time, radius * 3.0, 0.0, 0.0],
                glow: [0.0; 4],
            },
        );
    }

    /// This tick's feed to the engine's live break of `kind`: glow, pulse, heart, head,
    /// heave (`GpuRift::glow` and `time.w`).
    pub(super) fn feed(&mut self, engine: u32, kind: u32, fed: [f32; 5]) {
        if let Some(r) = self.live(engine, kind) {
            r.fed = fed;
        }
    }

    fn release_kind(&mut self, engine: u32, kind: u32, time: f32) {
        for r in &mut self.records {
            if r.engine == engine && r.kind == kind && r.released.is_none() {
                r.released = Some(time);
            }
        }
    }

    /// Lets go of everything the engine was working: its light dies, its plates settle.
    pub(super) fn release(&mut self, engine: u32, time: f32) {
        for kind in [rift::FOOT, rift::LINE, rift::WELL] {
            self.release_kind(engine, kind, time);
        }
    }

    /// Every frame: eases each break's light and heave and sends the list to the GPU.
    pub(super) fn upload(&mut self, time: f32) {
        let dt = (time - self.last).clamp(0.0, 0.25);
        self.last = time;
        let follow = 1.0 - (-FOLLOW * dt).exp();
        let mut gpu = [GpuRift::zeroed(); rift::MAX as usize];
        for (slot, r) in gpu.iter_mut().zip(&mut self.records) {
            match r.released {
                None => {
                    for (now, fed) in r.now.iter_mut().zip(r.fed) {
                        *now += (fed - *now) * follow;
                    }
                }
                Some(at) => {
                    let gone = time - at;
                    let light = (1.0 - gone / LIGHT_DIES).clamp(0.0, 1.0);
                    let settle = (1.0 - gone / PLATES_SETTLE).clamp(0.0, 1.0);
                    for k in 0..4 {
                        r.now[k] = r.now[k].min(r.fed[k] * light * light);
                    }
                    // Settled plates still show a little: the ground stays broken.
                    r.now[4] = r.now[4].min(r.fed[4] * (0.25 + 0.75 * settle * settle));
                }
            }
            r.gpu.glow = [r.now[0], r.now[1], r.now[2], r.now[3]];
            r.gpu.time[3] = r.now[4];
            *slot = r.gpu;
        }
        let n = self.records.len().min(rift::MAX as usize);
        self.buffer
            .write(0, bytemuck::cast_slice(&[n as u32, 0, 0, 0]));
        if n > 0 {
            self.buffer.write(16, bytemuck::cast_slice(&gpu[..n]));
        }
    }
}
