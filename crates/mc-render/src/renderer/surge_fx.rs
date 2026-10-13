//! A surge shell's wake as it is drawn (`Weapon::surge`, the Regency Saker's mortar;
//! `mc_sim::surge`). The shell itself lands as any pinch-fusion shell does (its own
//! impact); the wake is what follows, and is nothing like a Wake's wall of plasma: low and
//! wide, a ground swell rather than a wave.
//!
//! - **The front** (surge_fx.wgsl): a low ripple of light skimming out over the ground all
//!   round the crater at the sim's pace (`Surge::speed`), a pale rose crest no taller than
//!   a man over a swell of bent violet air, thinning away as it reaches its radius. Its
//!   foot throws a low ring of dust and a few sparks outward as it passes, and the trees
//!   sway away from it.
//! - **The burn:** the ground it has rolled over is left veined with cracks of light, white
//!   and rose just behind the front, settling to crimson embers that pulse slowly, with
//!   faint rings of violet running out across them like the swell's aftershocks. Embers
//!   drift up off it. It burns as long as the sim's does (`Surge::linger`), then dies
//!   down.
//!
//! Its ground effects are laid a tick at a time as the front gets there (`surge_tick`);
//! the front and the burn are drawn where the front stands each frame (`SurgeFx::upload`).
//! Presentation only; the renderer's own clock.
//!
//! The Edict's strike (`nuke_fx/edict.rs`) is drawn with the same machinery at its own
//! scale and in its own look (`Surge::look`, `gpu_consts::surge_fx`): its molten wave, its
//! shock and the lens of plasma over its mark; what each lays on the ground as it goes is
//! in `edict`.

mod edict;
pub(crate) mod pass;

use super::regency_guns_fx::{HOT, MOTE, RED, WHITE};
use super::{Renderer, PUFF_DUST};
use crate::gpu::{Gpu, GpuError};
use crate::gpu_consts::surge_fx::{EDICT_SHOCK, EDICT_WAVE, FADE, MAX_EDICT, MAX_SURGES, SAKER};
use crate::pipelines::{Layouts, Passes};
use glam::{Vec2, Vec3};
use mc_sim::mirror::SimEvent;
use pass::{Counts, GpuSurge, SurgePass};
use std::f32::consts::TAU;

/// Metres between the rings of the front's foot laid at once.
const RING: f32 = 10.0;
/// Metres round a ring between the dust kicked off it.
const KICK_STEP: f32 = 28.0;
/// The Regency's violet: the swell's bent air and the burn's aftershocks.
const VIOLET: Vec3 = Vec3::new(0.55, 0.16, 1.0);
/// Embers let rise off a burning disc each tick, before the burn dies down.
const EMBERS: usize = 6;

/// A surge's wake, from the shell landing to its embers dying down; or one of the
/// Edict's records (`look`).
#[derive(Clone, Copy)]
pub(super) struct Surge {
    pub(super) at: Vec3,
    pub(super) start: f32,
    pub(super) radius: f32,
    pub(super) speed: f32,
    pub(super) linger: f32,
    pub(super) power: f32,
    pub(super) seed: f32,
    /// `gpu_consts::surge_fx::SAKER`, `EDICT_WAVE`, ...
    pub(super) look: u32,
    /// Rings of its foot laid so far.
    laid: usize,
}

impl Surge {
    /// Seconds until its front reaches its radius.
    pub(super) fn roll(&self) -> f32 {
        self.radius / self.speed
    }

    pub(super) fn end(&self) -> f32 {
        self.start + self.roll() + self.linger
    }

    /// Metres out its front stands at `time` (surge_fx.wgsl `front_of`, before it thins):
    /// the Edict's shock runs out on the blast's square-root law, fast at first, the rest
    /// at their pace.
    pub(super) fn front(&self, time: f32) -> f32 {
        let age = (time - self.start).max(0.0);
        if self.look == EDICT_SHOCK {
            let t = (age / self.roll()).min(1.0);
            return self.radius * (1.0 / 12.0 + 11.0 / 12.0 * t.sqrt());
        }
        (age * self.speed).min(self.radius)
    }

    /// Metres between the rings of its foot laid at once.
    fn ring(&self) -> f32 {
        match self.look {
            SAKER => RING,
            _ => edict::RING,
        }
    }

    /// Records drawn on the Edict's meshes.
    fn edict(&self) -> bool {
        self.look != SAKER
    }
}

/// The surges burning and the pass that draws them.
pub(super) struct SurgeFx {
    surges: Vec<Surge>,
    pass: SurgePass,
}

impl SurgeFx {
    pub(super) fn new(gpu: &Gpu, layouts: &Layouts, passes: &Passes) -> Result<Self, GpuError> {
        Ok(Self {
            surges: Vec::new(),
            pass: SurgePass::new(gpu, layouts, passes)?,
        })
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        self.pass.destroy(gpu);
    }

    /// Hands the GPU this frame's surges: the Saker's, the Edict's waves, then its other
    /// records (`pass::Counts`).
    pub(super) fn upload(&mut self) {
        let rank = |s: &Surge| match s.look {
            SAKER => 0,
            EDICT_WAVE => 1,
            _ => 2,
        };
        let mut counts = Counts::default();
        let mut surges: Vec<(u8, GpuSurge)> = Vec::with_capacity(self.surges.len());
        for s in &self.surges {
            let k = rank(s);
            match k {
                0 => counts.saker += 1,
                1 => counts.waves += 1,
                _ => counts.fronts += 1,
            }
            surges.push((
                k,
                GpuSurge {
                    centre: s.at.to_array(),
                    start: s.start,
                    radius: s.radius,
                    speed: s.speed,
                    linger: s.linger,
                    seed: s.seed,
                    power: s.power,
                    look: s.look,
                    _pad1: 0.0,
                    _pad2: 0.0,
                },
            ));
        }
        // Stable: within a kind, oldest first.
        surges.sort_by_key(|(k, _)| *k);
        let surges: Vec<GpuSurge> = surges.into_iter().map(|(_, g)| g).collect();
        self.pass.upload(&surges, counts);
    }

    /// Inside `scene_over` (see `SurgePass::draw`).
    pub(super) fn draw(
        &self,
        device: &ash::Device,
        cmd: ash::vk::CommandBuffer,
        layout: ash::vk::PipelineLayout,
    ) {
        self.pass.draw(device, cmd, layout);
    }
}

impl Renderer {
    /// A surge shell landed (`Impact` of a `surge` weapon): its wake starts rolling out from
    /// the crater. The shell's own blast is drawn as any shell's.
    pub(super) fn surge_event(&mut self, event: &SimEvent, time: f32) {
        let SimEvent::Impact {
            pos,
            after,
            blueprint,
            weapon,
            ..
        } = *event
        else {
            return;
        };
        let w = &self.blueprints.unit(blueprint).weapons[weapon as usize];
        let Some(surge) = w.surge else {
            return;
        };
        let pos = Vec3::from(pos.to_f32());
        let water = self.map_info.water_level.to_f32();
        let at = pos
            .truncate()
            .extend(self.ground_height(pos.truncate()).max(water));
        let linger = surge.linger_ticks as f32 * self.tick_seconds.max(0.02);
        self.push_surge(
            at,
            time + after.to_f32() * self.tick_seconds,
            surge.radius.to_f32(),
            surge.speed.to_f32(),
            linger,
            w.impact.max(0.3),
        );
    }

    /// A wake rolling out over the ground from `at` from `start`: out to `radius` metres at
    /// `speed` metres a second, burning `linger` seconds after, `power` as bright as a
    /// shell's `impact`. A surge shell's, and the small one a surge gun drives into the
    /// ground as it fires (`regency_guns_fx::surge`).
    pub(super) fn push_surge(
        &mut self,
        at: Vec3,
        start: f32,
        radius: f32,
        speed: f32,
        linger: f32,
        power: f32,
    ) {
        self.add_surge(at, start, radius, speed, linger, power, SAKER);
    }

    /// A record of `look` (`gpu_consts::surge_fx`) from `start`, as `push_surge`'s.
    #[expect(
        clippy::too_many_arguments,
        reason = "a record's own fields, as push_surge's"
    )]
    pub(super) fn add_surge(
        &mut self,
        at: Vec3,
        start: f32,
        radius: f32,
        speed: f32,
        linger: f32,
        power: f32,
        look: u32,
    ) {
        let seed = self.scatter.unit() * 61.0;
        let surges = &mut self.surge_fx.surges;
        // A deliberate cosmetic cap, as many of each mesh as are drawn: the oldest of its
        // kind goes first, so a battle's Saker shells never put out an Edict's.
        let edict = look != SAKER;
        let most = if edict { MAX_EDICT } else { MAX_SURGES } as usize;
        if surges.iter().filter(|s| s.edict() == edict).count() >= most {
            if let Some(i) = surges.iter().position(|s| s.edict() == edict) {
                surges.remove(i);
            }
        }
        surges.push(Surge {
            at,
            start,
            radius: radius.max(1.0),
            speed: speed.max(1.0),
            linger,
            power,
            seed,
            look,
            laid: 0,
        });
    }

    /// Every surge's foot laid out as far as its front gets by the next tick, embers let
    /// rise off its burn; one that has died down is let go.
    pub(super) fn surge_tick(&mut self, time: f32) {
        let until = time + self.tick_seconds.max(0.02);
        let mut i = 0;
        while i < self.surge_fx.surges.len() {
            let mut surge = self.surge_fx.surges[i];
            if time > surge.end() {
                self.surge_fx.surges.remove(i);
                continue;
            }
            let front = surge.front(until);
            while ring_at(surge.laid, surge.ring()) <= front {
                if surge.edict() {
                    self.lay_edict_ring(&surge, surge.laid);
                } else {
                    self.lay_surge_ring(&surge, surge.laid);
                }
                surge.laid += 1;
            }
            if front > surge.ring() && time < surge.end() - FADE {
                match surge.look {
                    SAKER => self.surge_embers(&surge, front, time),
                    EDICT_WAVE => self.edict_embers(&surge, front, time),
                    _ => {}
                }
            }
            self.surge_fx.surges[i] = surge;
            i += 1;
        }
    }

    /// A point on the ground `r` out from the crater, `turn` of the way round it, and
    /// which way out it faces.
    pub(super) fn surge_foot(&self, surge: &Surge, r: f32, turn: f32) -> (Vec3, Vec2) {
        let water = self.map_info.water_level.to_f32();
        let out = Vec2::from_angle(turn * TAU);
        let xy = surge.at.truncate() + out * r;
        (xy.extend(self.ground_height(xy).max(water)), out)
    }

    /// Ring `k` of a surge's foot, as the front reaches it: a low ring of dust and a few
    /// sparks thrown outward, its light, the trees swaying away.
    fn lay_surge_ring(&mut self, surge: &Surge, k: usize) {
        let r = ring_at(k, RING);
        let when = surge.start + r / surge.speed;
        let water = self.map_info.water_level.to_f32();
        // Thrown off all round, fewer close in. A deliberate cosmetic cap per ring.
        let kicks = ((TAU * r / KICK_STEP).ceil() as usize).clamp(4, 64);
        let spin = self.scatter.unit();
        for n in 0..kicks {
            let turn = (n as f32 + 0.6 * self.scatter.unit() + spin) / kicks as f32;
            let (p, out) = self.surge_foot(surge, r, turn);
            // Low dust off every other ring, thrown on out with the front and soon settled.
            if k % 2 == 1 && p.z > water + 0.05 {
                let speed = 0.4 * surge.speed * (0.6 + 0.4 * self.scatter.unit());
                let vel = (out * speed).extend(1.0 + 1.5 * self.scatter.unit());
                let size = 2.0 + 1.4 * self.scatter.unit();
                let life = 0.6 + 0.4 * self.scatter.unit();
                self.push_puff(
                    PUFF_DUST,
                    p + Vec3::Z * 0.6,
                    vel,
                    when,
                    life,
                    (size, size * 2.4),
                );
            }
            if self.scatter.unit() < 0.3 {
                let spark = (out * (10.0 + 8.0 * self.scatter.unit()))
                    .extend(3.0 + 4.0 * self.scatter.unit());
                let tone = HOT.lerp(WHITE, self.scatter.unit() * 0.6);
                let life = 0.3 + 0.3 * self.scatter.unit();
                self.push_lit(
                    MOTE,
                    p + Vec3::Z * 0.4,
                    spark,
                    when,
                    life,
                    (0.26, 0.08),
                    tone * 4.0,
                    0.0,
                );
            }
        }
        // Its light runs out with it, round the ring a few points at a time.
        if k.is_multiple_of(2) {
            let lights = ((TAU * r / 90.0).ceil() as usize).clamp(2, 8);
            for n in 0..lights {
                let (p, _) = self.surge_foot(surge, r, (n as f32 + spin) / lights as f32);
                self.plasma_fx.guns.flare(
                    p + Vec3::Z * 2.0,
                    HOT.lerp(VIOLET, 0.35) * 60.0 * surge.power.sqrt(),
                    (TAU * r / lights as f32 * 0.6).clamp(14.0, 50.0),
                    when,
                    0.3,
                );
            }
        }
        // The trees sway away from it, a stretch of the ring at a time.
        if k % 3 == 1 {
            let bends = ((TAU * r / 40.0).ceil() as usize).clamp(1, 16);
            for n in 0..bends {
                let (p, _) = self.surge_foot(surge, r, (n as f32 + 0.5) / bends as f32);
                self.tree_blasts.record(p, when, 22.0, 0.5, false);
            }
        }
    }

    /// A few embers rising off the disc the front has rolled over, drifting up slowly and
    /// dying out, crimson, now and then a hotter one.
    fn surge_embers(&mut self, surge: &Surge, front: f32, time: f32) {
        for _ in 0..EMBERS {
            // Even over the disc's area, not bunched at the crater.
            let r = front * self.scatter.unit().sqrt();
            let turn = self.scatter.unit();
            let (p, _) = self.surge_foot(surge, r, turn);
            let drift = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 0.6;
            let vel = drift + Vec3::Z * (1.2 + 1.8 * self.scatter.unit());
            let tone = if self.scatter.unit() < 0.15 { HOT } else { RED };
            let start = time + self.tick_seconds * self.scatter.unit();
            let life = 0.9 + 0.9 * self.scatter.unit();
            self.push_lit(
                MOTE,
                p + Vec3::Z * 0.3,
                vel,
                start,
                life,
                (0.18, 0.06),
                tone * 2.4,
                0.0,
            );
        }
    }
}

/// How far out ring `k` of a surge's foot is laid, metres, `step` apart.
fn ring_at(k: usize, step: f32) -> f32 {
    6.0 + k as f32 * step
}
