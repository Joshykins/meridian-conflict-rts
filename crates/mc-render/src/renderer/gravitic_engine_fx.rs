//! A gravitic engine at work, as it is drawn (mc-sim `gravitic.rs`, the Regency's tier 5
//! "The Fallen"; `RenderFrame::wells` and the rift events). Gravity, not fire: violet light
//! with white-hot hearts, no orange, heavy and grounded.
//!
//! - **Gathering** (`engine.rs`): the machinery inside the crown whirs up from a crawl
//!   (its orbit pieces turn by a phase run up here, `UnitInstance::deploy`); a column of
//!   violet light gathers between the shards, strands twisting round a white thread
//!   (gravity_puffs.wgsl), bending the view round it; violet lightning jumps from it to
//!   the shards and, as it fills, from the shards down to the ground; cracks run out from
//!   the foot with violet light in them, slabs of ground round it heave and shake, dust is
//!   shaken up and shards of rock lift off and circle it.
//! - **Launch and the rift** (`rift.rs`): a blast down at the foot, then the crack races
//!   out to the mark, a fissure lit violet with a white head, slabs thrown up either side,
//!   lightning crawling along it. It stays as a scar: lit while the well holds, light
//!   pulsing along it toward the well, dark once let go.
//! - **The strike and the well** (`well.rs`): a hard violet-white flash, a pressure ring,
//!   the ground broken into plates across its reach and dust driven out; then the plates
//!   lift and hang, grinding, shards of rock circle slowly over it, lightning crawls over
//!   the ground and onto what it crushes, its edge a ring of cracks, the view bent over
//!   it. An aircraft torn down is dragged to the ground by a stroke of violet and lands in
//!   a burst. Let go, everything held up falls and the light dies.
//! - **Faltering** (`WellView::powered` false): the light dims and gutters, the machinery
//!   winds down.
//!
//! The broken ground is drawn by the terrain (`ground.rs`), the rock torn loose as props
//! (`slabs.rs`). Presentation only: the renderer's own clock, nothing written back.

mod engine;
pub(crate) mod ground;
mod rift;
mod slabs;
mod well;

use glam::{Vec2, Vec3};
use mc_sim::mirror::{
    RenderFrame, SimEvent, UnitInstance, WellStage, WellView, KIND_GHOST, KIND_WRECK, STATE_RADAR,
};
use std::borrow::Cow;

use super::heat_haze::GpuHeatPlume;
use super::Renderer;
use crate::camera::Camera;
use crate::gpu::{Gpu, GpuError};

pub(crate) use slabs::MOST as MOST_SLABS;

/// The light of it: deep violet, violet, lilac, and the white of its hearts.
const VIOLET: Vec3 = Vec3::new(0.55, 0.12, 1.0);
const LILAC: Vec3 = Vec3::new(0.82, 0.6, 1.0);
const WHITE: Vec3 = Vec3::new(1.0, 0.95, 1.0);
/// The machinery's turn, as a share of its full rate: idle, and flat out as it launches.
const SPIN_IDLE: f32 = 0.06;
const SPIN_LAUNCH: f32 = 5.5;
const SPIN_HOLD: f32 = 4.0;
/// Seconds for the machinery to take up a change of pace (it is heavy).
const SPIN_INERTIA: f32 = 1.2;
/// Lenses (gravity bending the view) handed to the tone map at most: a deliberate cap,
/// two an engine for the nearest few.
const MOST_LENSES: usize = 8;

/// An engine as this tick sees it.
#[derive(Clone, Copy)]
struct Engine {
    id: u32,
    owner: u8,
    /// Its middle on the ground, its held gravity (world), and its size.
    base: Vec3,
    core: Vec3,
    radius: f32,
    height: f32,
    /// Its work this tick, and last tick's stage.
    view: Option<WellView>,
    was: Option<WellStage>,
    /// The machinery's pace (share of full) and the phase it has turned through.
    rate: f32,
    phase: f32,
    /// How much of its light shows, eased: 0 dark, 1 full.
    charge: f32,
    /// How far its rift had run last tick, metres.
    run: f32,
    seen: bool,
}

impl Engine {
    fn stage(&self) -> Option<WellStage> {
        self.view.map(|v| v.stage)
    }

    fn powered(&self) -> bool {
        self.view.is_none_or(|v| v.powered)
    }

    /// The pace its machinery is driven toward, and the light it holds.
    fn goal(&self) -> (f32, f32) {
        let (rate, light) = match self.view {
            None => (SPIN_IDLE, 0.0),
            Some(v) => match v.stage {
                WellStage::Gathering => {
                    let p = v.progress.clamp(0.0, 1.0);
                    (SPIN_IDLE + (SPIN_LAUNCH - SPIN_IDLE) * p.powf(1.8), p)
                }
                WellStage::Running => (SPIN_LAUNCH, 1.0),
                WellStage::Holding => (SPIN_HOLD, 0.85),
            },
        };
        if self.powered() {
            (rate, light)
        } else {
            (0.0, light * 0.3)
        }
    }
}

pub(crate) struct GraviticEngineFx {
    /// Per model slot: its held gravity (`Model::gravity_core`).
    cores: Vec<Option<[f32; 4]>>,
    /// The first rift slab's model slot (`rift_slabs::SLAB_KEYS`, then the shards).
    first_slab: u32,
    engines: Vec<Engine>,
    pub(crate) ground: ground::RiftGround,
    slabs: slabs::Slabs,
    /// This frame's gravity lenses, for the tone map (heat_haze.rs).
    pub(super) lenses: Vec<GpuHeatPlume>,
    /// Lights renewed each tick: where, colour, reach.
    lamps: Vec<(Vec3, Vec3, f32)>,
    last: f32,
}

impl GraviticEngineFx {
    pub(super) fn new(
        gpu: &Gpu,
        cores: Vec<Option<[f32; 4]>>,
        first_slab: u32,
    ) -> Result<Self, GpuError> {
        Ok(Self {
            cores,
            first_slab,
            engines: Vec::new(),
            ground: ground::RiftGround::new(gpu)?,
            slabs: slabs::Slabs::default(),
            lenses: Vec::new(),
            lamps: Vec::new(),
            last: 0.0,
        })
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        self.ground.destroy(gpu);
    }

    /// A new match: the ground is whole, nothing hangs in the air.
    pub(super) fn clear(&mut self) {
        self.engines.clear();
        self.ground.clear();
        self.slabs.clear();
    }

    /// A tree at `at` the sim struck off with `center` as the reason: if that is an
    /// engine's rift front or well, the way it is thrown (out from the rift's line or the
    /// well's middle).
    pub(super) fn tears_away(&self, center: Vec2, at: Vec2) -> Option<Vec2> {
        self.engines.iter().find_map(|e| {
            let v = e.view?;
            let from = Vec2::from_slice(&v.from[..2]);
            let mark = Vec2::from_slice(&v.mark[..2]);
            let front = Vec2::from_slice(&v.front[..2]);
            match v.stage {
                WellStage::Gathering => None,
                WellStage::Holding if center.distance(mark) < 1.0 => {
                    Some((at - mark).normalize_or(Vec2::X))
                }
                _ if center.distance(front) < 200.0 => {
                    let along = (mark - from).normalize_or(Vec2::X);
                    let side = Vec2::new(-along.y, along.x);
                    Some(side * (at - from).dot(side).signum())
                }
                _ => None,
            }
        })
    }

    fn holds_gravity(&self, u: &UnitInstance) -> Option<[f32; 4]> {
        let hidden = KIND_WRECK | KIND_GHOST | STATE_RADAR;
        if u.owner_flags & hidden != 0 || u.build < 1.0 {
            return None;
        }
        self.cores.get(u.blueprint as usize).copied().flatten()
    }

    /// Each tick, before the instances go to the GPU: takes the engines and their work from
    /// the mirror, runs their machinery on over the coming tick (`tick_seconds`) and hands
    /// each its turn (`deploy`, last tick's in `prev_deploy`), which the entity shader turns
    /// its orbit pieces by (`ModelInfo::orbit_driven`).
    pub(super) fn patch<'a>(
        &mut self,
        units: Cow<'a, [UnitInstance]>,
        wells: &[WellView],
        radii: impl Fn(u32) -> (f32, f32),
        tick_seconds: f32,
    ) -> Cow<'a, [UnitInstance]> {
        for e in &mut self.engines {
            e.seen = false;
        }
        if !units.iter().any(|u| self.holds_gravity(u).is_some()) {
            self.engines.clear();
            return units;
        }
        let dt = tick_seconds.clamp(0.02, 0.5);
        let mut units = units;
        for u in units.to_mut().iter_mut() {
            let Some(core) = self.holds_gravity(u) else {
                continue;
            };
            let (s, c) = u.heading.sin_cos();
            let base = Vec3::from(u.pos);
            let at = base
                + Vec3::new(
                    core[0] * c - core[1] * s,
                    core[0] * s + core[1] * c,
                    core[2],
                );
            let view = wells.iter().find(|w| w.unit_id == u.unit_id).copied();
            let (radius, height) = radii(u.blueprint);
            let e = match self.engines.iter_mut().find(|e| e.id == u.unit_id) {
                Some(e) => e,
                None => {
                    self.engines.push(Engine {
                        id: u.unit_id,
                        owner: (u.owner_flags & 0xFF) as u8,
                        base,
                        core: at,
                        radius,
                        height,
                        view: None,
                        was: None,
                        rate: SPIN_IDLE,
                        phase: (u.unit_id % 97) as f32,
                        charge: 0.0,
                        run: 0.0,
                        seen: true,
                    });
                    let last = self.engines.len() - 1;
                    &mut self.engines[last]
                }
            };
            e.was = e.stage();
            e.view = view;
            e.base = base;
            e.core = at;
            e.seen = true;
            let (rate, light) = e.goal();
            e.rate += (rate - e.rate) * (1.0 - (-dt / SPIN_INERTIA).exp());
            // Light comes quickly and dies more slowly.
            let pace = if light > e.charge { 4.0 } else { 1.5 };
            e.charge += (light - e.charge) * (1.0 - (-dt * pace).exp());
            let start = e.phase;
            e.phase += e.rate * dt;
            // Kept small so the shader's turn stays exact: a whole number of turns of the
            // slowest piece is far longer than any match, so wrap at a large multiple.
            if e.phase > 40_000.0 {
                e.phase -= 40_000.0;
            }
            u.prev_deploy = start;
            u.deploy = if e.phase < start {
                start + e.rate * dt
            } else {
                e.phase
            };
        }
        self.engines.retain(|e| e.seen);
        units
    }
}

impl Renderer {
    /// Once a tick (from `upload_sim`): every engine's work drawn.
    pub(super) fn gravitic_engine_tick(&mut self, frame: &RenderFrame, time: f32, camera: &Camera) {
        if time + 1.0 < self.gravitic_engine_fx.last {
            // The clock went back (a restaged backdrop): nothing of the old world is left.
            self.gravitic_engine_fx.clear();
        }
        self.gravitic_engine_fx.last = time;
        self.gravitic_engine_fx.lamps.clear();
        let engines = self.gravitic_engine_fx.engines.clone();
        for e in &engines {
            let near = e.core.truncate().distance(camera.focus.truncate())
                < camera.distance * 2.5 + 4000.0
                || e.view.is_some_and(|v| {
                    Vec2::from_slice(&v.mark[..2]).distance(camera.focus.truncate())
                        < camera.distance * 2.5 + 2000.0
                });
            self.engine_charge(e, time, near);
            let Some(v) = e.view else {
                continue;
            };
            self.catch_up(e, &v, time);
            match v.stage {
                WellStage::Gathering => {}
                WellStage::Running => self.rift_running(e, &v, time, camera),
                WellStage::Holding => self.well_holding(e, &v, frame, time, camera),
            }
            if let Some(stored) = self
                .gravitic_engine_fx
                .engines
                .iter_mut()
                .find(|s| s.id == e.id)
            {
                stored.run = Vec3::from(v.front)
                    .truncate()
                    .distance(Vec3::from(v.from).truncate());
            }
        }
    }

    /// A rift or well under way that the renderer missed the start of (it was out of
    /// sight, or the renderer is new): its broken ground and rock laid as they stand now,
    /// without the flash of their start.
    fn catch_up(&mut self, e: &Engine, v: &WellView, time: f32) {
        let from = Vec2::from_slice(&v.from[..2]);
        let mark = Vec2::from_slice(&v.mark[..2]);
        let length = from.distance(mark);
        let ground = &mut self.gravitic_engine_fx.ground;
        match v.stage {
            WellStage::Gathering => {}
            WellStage::Running => {
                if !ground.has_live(e.id, crate::gpu_consts::rift::LINE) {
                    ground.line(e.id, from, mark, rift::SPEED, rift::WIDTH, time - v.seconds);
                }
            }
            WellStage::Holding => {
                if !ground.has_live(e.id, crate::gpu_consts::rift::LINE) {
                    let began = time - v.seconds - length / rift::SPEED;
                    ground.line(e.id, from, mark, rift::SPEED, rift::WIDTH, began);
                    ground.line_cap(e.id, length);
                }
                if !ground.has_live(e.id, crate::gpu_consts::rift::WELL) {
                    let began = time - v.seconds;
                    ground.well(e.id, mark, v.radius, began);
                    let z = self.ground_height(mark).max(v.mark[2]);
                    self.well_rock(e.id, mark, z, v.radius.max(20.0), time);
                }
            }
        }
    }

    /// The engines' part of a sim event.
    pub(super) fn gravitic_engine_event(&mut self, event: &SimEvent, time: f32) {
        match event {
            SimEvent::RiftLaunched {
                unit, from, mark, ..
            } => self.rift_launched(
                unit.0,
                Vec3::from(from.to_f32()),
                Vec3::from(mark.to_f32()),
                time,
            ),
            SimEvent::RiftStruck { unit, pos, radius } => {
                self.well_struck(unit.0, Vec3::from(pos.to_f32()), radius.to_f32(), time)
            }
            SimEvent::WellReleased { unit, pos, radius } => {
                self.well_released(unit.0, Vec3::from(pos.to_f32()), radius.to_f32(), time)
            }
            SimEvent::AircraftDowned { pos, .. } => {
                self.aircraft_downed(Vec3::from(pos.to_f32()), time)
            }
            _ => {}
        }
    }

    /// Every frame: the broken ground to the GPU, the lenses for the tone map.
    pub(super) fn gravitic_engine_frame(&mut self, time: f32, camera: &Camera) {
        let fx = &mut self.gravitic_engine_fx;
        fx.ground.upload(time);
        fx.lenses.clear();
        let eye = camera.eye();
        let mut lenses: Vec<(f32, GpuHeatPlume)> = Vec::new();
        for e in &fx.engines {
            if e.charge > 0.02 {
                let c = e.charge;
                let (foot, tall) = engine::column(e);
                lenses.push((
                    foot.distance(eye),
                    GpuHeatPlume {
                        port: foot.to_array(),
                        radius: e.radius * (0.35 + 0.25 * c),
                        axis: [0.0, 0.0, 1.0],
                        height: tall,
                        strength: c * (0.7 + 0.08 * (time * 2.3 + e.id as f32).sin()),
                        seed: e.id as f32 * 0.37,
                        kind: crate::gpu_consts::haze::KIND_LENS,
                        _pad: 0.0,
                    },
                ));
            }
            if let Some(v) = e.view.filter(|v| v.stage == WellStage::Holding) {
                let mark = Vec3::from(v.mark);
                let strength = if v.powered { 0.3 } else { 0.08 };
                lenses.push((
                    mark.distance(eye),
                    GpuHeatPlume {
                        port: (mark + Vec3::Z * v.radius * 0.12).to_array(),
                        radius: v.radius * 0.9,
                        axis: [0.0, 0.0, 1.0],
                        height: v.radius * 0.25,
                        strength,
                        seed: e.id as f32 * 0.71,
                        kind: crate::gpu_consts::haze::KIND_LENS,
                        _pad: 0.0,
                    },
                ));
            }
        }
        // Nearest first; ties keep the engines' order.
        lenses.sort_by(|a, b| a.0.total_cmp(&b.0));
        fx.lenses
            .extend(lenses.into_iter().take(MOST_LENSES).map(|l| l.1));
    }

    /// Every frame: the rock the engines have torn loose, as props for the dynamic list.
    pub(super) fn gravitic_engine_instances(&mut self, time: f32) -> Vec<UnitInstance> {
        let mut out = Vec::new();
        let first = self.gravitic_engine_fx.first_slab;
        self.gravitic_engine_fx
            .slabs
            .instances(time, first, &mut out);
        out
    }

    /// Every frame: the engines' light.
    pub(super) fn gravitic_engine_lights(&mut self) {
        for &(pos, color, range) in &self.gravitic_engine_fx.lamps {
            self.lights.lamp(pos, Vec3::NEG_Z, color, range, 180.0, 1.0);
        }
    }
}
