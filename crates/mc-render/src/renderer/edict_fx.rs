//! The Edict, the Regent's main gun, as it is drawn on the ship and in the sky (mc-sim
//! `edict.rs`; `RenderFrame::edicts` and the `Edict*` events). Pinch fusion: white-hot with
//! the prism's pastel rim, the Regency's red behind it; hard-edged, no mist, never violet
//! (construction's colour) and never fire's orange. It comes from no gun: the whole hull
//! gathers it.
//!
//! - **The gather** (regency.wgsl `reg_edict`, fed each ship's Edict through
//!   `UnitInstance::mount` as `gpu_consts::edict` lays out): light runs in along the plate
//!   seams from the bow and the stern, two fronts closing on the firing point, white-hot
//!   at each front with the prism's pink at its rim, red left burning in the seams behind
//!   them and pulses running on toward the point; stalled it gutters. A ship with an Edict
//!   carries its guns on houses of their own, so its `mount` is free for this.
//! - **The star** (`star.rs`): at the firing point under the keel, from a spark to a
//!   blinding white-hot point over the gather, its rim the prism, a red corona round it,
//!   lightning crawling in to it off the hull; its light thrown up on the hull and down on
//!   the ground, and a lens flare. In its last third of a second it draws in, then lets go.
//! - **The shot** (`shot.rs`): a column of pinch-fusion plasma from the firing point down to
//!   the mark, there at once, held a second and a quarter, then collapsing into a red
//!   filament that cools and breaks up; at the hull a blinding flash, a hard pressure ring
//!   and the star's streamers flung out round the shot.
//! - **The light** (`exposure.rs`): the scene dims a little round the star as it blazes;
//!   at the shot a short white-out, then the day darkens to half and comes back over six
//!   seconds, weighed by how near the camera is.
//! - **The clouds** (sky/cut.rs): the deck cut along the shot's line and round the mark,
//!   the cloud thrown back round the mark and the hull; it heals over a minute.
//!
//! The strike at the mark, the molten wave and the scar are the ground's (elsewhere).
//! Presentation only: the renderer's own clock, nothing written back.

mod exposure;
mod shot;
mod star;

use std::borrow::Cow;

use glam::Vec3;
use mc_sim::mirror::{EdictStage, EdictView, RenderFrame, SimEvent, UnitInstance};

use super::Renderer;
use crate::camera::Camera;
use crate::gpu_consts::edict;

/// One ship's Edict as the renderer follows it.
#[derive(Clone, Copy)]
struct Ship {
    id: u32,
    view: EdictView,
    /// Its hull's heading this tick, radians, and its keel's height over its origin at the
    /// firing point (metres).
    heading: f32,
    keel: f32,
    /// The renderer's clock at its shot, once fired.
    fired: Option<f32>,
    /// Let go before the shot: when, and how full it was.
    let_go: Option<(f32, f32)>,
    seen: bool,
}

impl Ship {
    fn new(view: EdictView, time: f32) -> Self {
        Self {
            id: view.unit_id,
            view,
            heading: 0.0,
            keel: 0.0,
            fired: (view.stage == EdictStage::Fired).then_some(time - view.seconds),
            let_go: None,
            seen: true,
        }
    }

    /// Its record for the shader (`gpu_consts::edict`), or none once its light is out.
    fn record(&self, time: f32) -> Option<[f32; 4]> {
        let v = &self.view;
        if let Some((when, share)) = self.let_go {
            return (time < when + edict::FADE).then_some([share, when, v.along, edict::CANCELLED]);
        }
        match v.stage {
            EdictStage::Gathering if v.powered => {
                // The share it gains a second at full power, kept under one (the tag's room).
                let rate = (1.0 / (v.seconds + v.left).max(1.0)).min(0.99);
                Some([v.progress, time, v.along, edict::GATHERING - rate])
            }
            EdictStage::Gathering => Some([v.progress, 0.0, v.along, edict::STALLED]),
            EdictStage::Fired => {
                let fired = self.fired.unwrap_or(time - v.seconds);
                (time - fired < edict::AFTERGLOW + 1.0).then_some([
                    fired,
                    0.0,
                    v.along,
                    edict::FIRED,
                ])
            }
        }
    }

    /// The firing point on the keel, in the world.
    fn keel_point(&self) -> Vec3 {
        Vec3::from(self.view.from) + Vec3::Z * self.keel
    }

    /// Down the hull (its +X) and across it, in the world.
    fn axes(&self) -> (Vec3, Vec3) {
        let (s, c) = self.heading.sin_cos();
        (Vec3::new(c, s, 0.0), Vec3::new(-s, c, 0.0))
    }
}

/// Bins a keel is sampled in, stern to bow, and how far either side of the centreline
/// (metres) its hull is looked for.
const KEEL_BINS: usize = 96;
const KEEL_BAND: f32 = 60.0;

/// A hull's keel: the lowest point of the hull near its centreline along its length
/// (model x), so the star hangs under the plating, not under the ship's origin.
#[derive(Clone)]
pub(super) struct Keel {
    x0: f32,
    step: f32,
    z: Vec<f32>,
}

impl Keel {
    /// The keel of a model's finest mesh.
    pub(super) fn of(model: &crate::models::Model) -> Option<Self> {
        let vertices = &model.lods[0].vertices;
        let (lo, hi) = vertices.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| {
            (lo.min(v.pos[0]), hi.max(v.pos[0]))
        });
        if vertices.is_empty() || hi - lo < 1.0 {
            return None;
        }
        let step = (hi - lo) / KEEL_BINS as f32;
        let mut z = vec![f32::MAX; KEEL_BINS];
        for v in vertices.iter().filter(|v| v.pos[1].abs() < KEEL_BAND) {
            let bin = (((v.pos[0] - lo) / step) as usize).min(KEEL_BINS - 1);
            z[bin] = z[bin].min(v.pos[2]);
        }
        // A bin with no plating near the centreline takes its nearest neighbour's.
        let found: Vec<usize> = (0..KEEL_BINS).filter(|&i| z[i] < f32::MAX).collect();
        if found.is_empty() {
            return None;
        }
        for i in 0..KEEL_BINS {
            if z[i] == f32::MAX {
                let near = found.iter().min_by_key(|&&j| j.abs_diff(i)).copied();
                z[i] = near.map_or(0.0, |j| z[j]);
            }
        }
        Some(Self { x0: lo, step, z })
    }

    /// The keel's height at `x` along the model.
    fn at(&self, x: f32) -> f32 {
        let f = ((x - self.x0) / self.step - 0.5).clamp(0.0, (KEEL_BINS - 1) as f32);
        let i = (f as usize).min(KEEL_BINS - 2);
        let k = f - i as f32;
        self.z[i] + (self.z[i + 1] - self.z[i]) * k
    }
}

pub(crate) struct EdictFx {
    /// Per model slot: the keel of a hull that carries an Edict.
    keels: Vec<Option<Keel>>,
    ships: Vec<Ship>,
    shots: Vec<shot::Shot>,
    pub(super) exposure: exposure::Exposure,
    /// Light renewed each tick: where, colour times brightness, reach.
    lamps: Vec<(Vec3, Vec3, f32)>,
    /// This frame's shimmer round the columns, for the tone map (heat_haze.rs).
    pub(super) shimmer: Vec<super::heat_haze::GpuHeatPlume>,
    /// The stars burning this tick, for their lens flares each frame: where, how bright
    /// and how far their spikes reach (metres).
    flares: Vec<(Vec3, f32, f32)>,
    last: f32,
}

impl EdictFx {
    pub(super) fn new(keels: Vec<Option<Keel>>) -> Self {
        Self {
            keels,
            ships: Vec::new(),
            shots: Vec::new(),
            exposure: exposure::Exposure::default(),
            lamps: Vec::new(),
            shimmer: Vec::new(),
            flares: Vec::new(),
            last: 0.0,
        }
    }

    /// A new match: nothing gathers, nothing burns.
    pub(super) fn clear(&mut self) {
        self.ships.clear();
        self.shots.clear();
        self.exposure.clear();
        self.lamps.clear();
        self.flares.clear();
    }

    /// Each tick, before the instances go to the GPU: takes the ships' Edicts from the
    /// mirror and hands each ship's hull its record in `mount` (`gpu_consts::edict`).
    pub(super) fn patch<'a>(
        &mut self,
        units: Cow<'a, [UnitInstance]>,
        edicts: &[EdictView],
        time: f32,
    ) -> Cow<'a, [UnitInstance]> {
        if time + 1.0 < self.last {
            // The clock went back (a restaged backdrop): nothing of the old world is left.
            self.clear();
        }
        self.last = time;
        for s in &mut self.ships {
            s.seen = false;
        }
        for v in edicts {
            match self.ships.iter_mut().find(|s| s.id == v.unit_id) {
                Some(s) => {
                    let again = v.stage == EdictStage::Gathering
                        && (s.view.stage == EdictStage::Fired || s.let_go.is_some());
                    if again {
                        *s = Ship::new(*v, time);
                    }
                    if v.stage == EdictStage::Fired && s.fired.is_none() {
                        s.fired = Some(time - v.seconds);
                    }
                    s.view = *v;
                    s.seen = true;
                }
                None => self.ships.push(Ship::new(*v, time)),
            }
        }
        for s in &mut self.ships {
            if !s.seen && s.let_go.is_none() && s.view.stage == EdictStage::Gathering {
                s.let_go = Some((time, s.view.progress));
            }
        }
        self.ships
            .retain(|s| s.seen || s.let_go.is_some_and(|(when, _)| time < when + edict::FADE));
        if self.ships.is_empty() {
            return units;
        }
        let mut units = units;
        for u in units.to_mut().iter_mut() {
            let Some(s) = self.ships.iter_mut().find(|s| s.id == u.unit_id) else {
                continue;
            };
            s.heading = u.heading;
            if let Some(Some(keel)) = self.keels.get(u.blueprint as usize) {
                s.keel = keel.at(s.view.along);
            }
            if let Some(record) = s.record(time) {
                u.mount = record;
            }
        }
        units
    }
}

impl Renderer {
    /// Once a tick (from `upload_sim`, after the held beams are laid): the stars, the
    /// shots' columns and filaments, and their light.
    pub(super) fn edict_tick(&mut self, frame: &RenderFrame, time: f32, camera: &Camera) {
        let fx = &mut self.edict_fx;
        fx.lamps.clear();
        fx.flares.clear();
        fx.shots.retain(|s| s.alive(time));
        if fx.ships.is_empty() && fx.shots.is_empty() && frame.edicts.is_empty() {
            return;
        }
        let focus = camera.focus.truncate();
        let reach = camera.distance * 2.5 + 6000.0;
        let ships = self.edict_fx.ships.clone();
        for s in &ships {
            let at = s.keel_point().truncate();
            let near = at.distance(focus) < reach;
            self.edict_star(s, time, near, camera);
        }
        let shots = self.edict_fx.shots.clone();
        for s in &shots {
            self.edict_shot_tick(s, time);
        }
    }

    /// The Edict's part of a sim event.
    pub(super) fn edict_event(&mut self, event: &SimEvent, time: f32) {
        match event {
            SimEvent::EdictFired {
                unit,
                from,
                mark,
                radius,
                ..
            } => {
                let (mut from, mark) = (Vec3::from(from.to_f32()), Vec3::from(mark.to_f32()));
                if let Some(s) = self.edict_fx.ships.iter_mut().find(|s| s.id == unit.0) {
                    s.fired = Some(time);
                    // It leaves from the keel, not the ship's origin under it.
                    from.z += s.keel;
                }
                self.edict_fired(from, mark, radius.to_f32(), time);
            }
            SimEvent::EdictCancelled { unit, from } => {
                let mut from = Vec3::from(from.to_f32());
                let share = match self.edict_fx.ships.iter_mut().find(|s| s.id == unit.0) {
                    Some(s) => {
                        let share = s.view.progress;
                        s.let_go.get_or_insert((time, share));
                        from.z += s.keel;
                        share
                    }
                    None => 0.0,
                };
                self.edict_let_go(from, share, time);
            }
            _ => {}
        }
    }

    /// Every frame: the light the stars and shots throw.
    pub(super) fn edict_lights(&mut self) {
        for &(pos, color, range) in &self.edict_fx.lamps {
            self.lights.lamp(pos, Vec3::NEG_Z, color, range, 180.0, 1.0);
        }
    }

    /// Every frame, before the lens flares go up: the stars' flares, and the exposure the
    /// tone map takes this frame (`Exposure::value`).
    pub(super) fn edict_frame(&mut self, time: f32, camera: &Camera) {
        let fx = &mut self.edict_fx;
        fx.shimmer.clear();
        fx.shimmer
            .extend(fx.shots.iter().filter_map(|s| s.shimmer(time)));
        for &(at, glare, size) in &fx.flares {
            self.lens_flares.hold(
                at,
                super::lens_flare::Flare {
                    color: star::FLARE * glare,
                    size,
                },
            );
        }
        let stars = fx
            .ships
            .iter()
            .filter(|s| s.view.stage == EdictStage::Gathering && s.let_go.is_none())
            .map(|s| (s.keel_point(), s.view.progress));
        fx.exposure.frame(time, camera.focus, stars);
    }
}
