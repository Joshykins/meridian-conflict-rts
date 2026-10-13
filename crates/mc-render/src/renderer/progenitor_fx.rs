//! Survival's Progenitor in hard light: everything its Shapers do that is not their
//! model, drawn in its own language, the same as nothing else in the game. Pale alloy
//! and a dark core for what is solid, cold blue-white light going white-hot for what is
//! not; crisp lines, panes, rings and columns rather than glows and balls; no fire and
//! no smoke. It grows with the Shaper's tier (`ShaperInstance::tier`, one every four
//! levels): more lines, more beams, more rings, and new parts.
//!
//! - **The Lance** (`lance.rs`): a crystal of light forming at the Lens as it charges,
//!   lines drawn in to it, rings closing round it and a thin line to its mark; then the
//!   strike from the Lens to the mark (its look chosen by `MERIDIAN_PROGENITOR_LANCE`),
//!   a ring out to the sim's splash and a mark burnt into the ground. Higher tiers add
//!   beams down from the collars, a lattice over the strike and a pillar rising from it.
//! - **The anti-nuke and the warheads** (`strategic.rs`): an interceptor is a shard of
//!   light lifting out of a column of it, leaving a line of light behind; it closes a
//!   cage round the warhead it meets and unmakes it into lines of light. The Unmaker's
//!   warhead is a solid body seamed with light, pushed by a spear of it, and goes off as
//!   a cold sphere of hard light that opens and falls in again over a burnt mark.
//! - **Replication shells** (`shells.rs`): a solid body seamed with light on a ribbon of
//!   it; its mark turning on the ground where it will land; landing, a burst ring, a
//!   column the squad is printed in and a mark left burnt into the ground.
//!
//! The dome is the shield's own look (progenitor_glass.wgsl). Presentation only; the
//! renderer's own clock.

pub(crate) mod pass;

mod lance;
mod shells;
#[cfg(test)]
mod shots;
mod strategic;

use super::Renderer;
use crate::camera::Camera;
use crate::gpu::{Gpu, GpuError};
use crate::gpu_consts::progenitor_fx::{
    BEAM, BODY, HOT_B, HOT_G, HOT_R, LIGHT_B, LIGHT_G, LIGHT_R, MAX_PIECES, PRISM, RING, SHELL,
};
use crate::pipelines::{Layouts, Passes};
use bytemuck::Zeroable;
use glam::Vec3;
use mc_sim::mirror::{RenderFrame, SimEvent, StrategicInstance};
use mc_sim::survival::{ShaperInstance, ShellInstance};
use pass::{GpuPiece, ProgenitorPass};

/// The Progenitor's light, and its white-hot (gpu_consts `progenitor_fx`).
pub(super) const LIGHT: Vec3 = Vec3::new(LIGHT_R, LIGHT_G, LIGHT_B);
pub(super) const HOT: Vec3 = Vec3::new(HOT_R, HOT_G, HOT_B);
/// Pieces laid by events kept at once: a deliberate cosmetic cap, the oldest going
/// first, leaving room in the frame for what is drawn fresh each frame.
const MOST_LAID: usize = MAX_PIECES as usize * 3 / 4;
/// Seconds a piece drawn fresh each frame is given: it never fades in that time.
const HELD: f32 = 1.0e5;

/// How the Lance strikes (`MERIDIAN_PROGENITOR_LANCE`, docs/SWITCHES.md): three looks
/// for the user to choose between.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum LanceLook {
    /// A broad beam of hard light with chevrons running down it to the mark (`a`).
    Spear,
    /// Three strands of light wound round a thin core, turning as they run (`b`).
    Braid,
    /// A thin core run through rings of light that fly down it one after another (`c`).
    Gates,
}

impl LanceLook {
    /// `MERIDIAN_PROGENITOR_LANCE`: `a`, `b` or `c`; the spear when unset.
    pub(crate) fn from_env() -> Self {
        match std::env::var("MERIDIAN_PROGENITOR_LANCE").as_deref() {
            Ok("b") => Self::Braid,
            Ok("c") => Self::Gates,
            _ => Self::Spear,
        }
    }
}

/// One piece of hard light being built (`GpuPiece`; progenitor_fx.wgsl says what each
/// field is to each kind).
#[derive(Clone, Copy)]
pub(super) struct Piece(GpuPiece);

impl Piece {
    fn of(kind: u32, a: Vec3, b: Vec3) -> Self {
        Self(GpuPiece {
            a: a.to_array(),
            b: b.to_array(),
            kind,
            power: 1.0,
            life: HELD,
            ..GpuPiece::zeroed()
        })
    }

    /// A line of light from `a` to `b`.
    pub(super) fn beam(a: Vec3, b: Vec3) -> Self {
        Self::of(BEAM, a, b)
    }

    /// A ring round `at` facing `normal` (`DRAPE`: on the ground).
    pub(super) fn ring(at: Vec3, normal: Vec3) -> Self {
        Self::of(RING, at, normal)
    }

    /// A column standing from `foot` along `up` (its length the column's height).
    pub(super) fn prism(foot: Vec3, up: Vec3) -> Self {
        Self::of(PRISM, foot, up)
    }

    /// A body, nose at `nose`, flying along `axis`, its back toward `up`.
    pub(super) fn body(nose: Vec3, axis: Vec3, up: Vec3) -> Self {
        let mut p = Self::of(BODY, nose, axis);
        p.0.c = up.to_array();
        p
    }

    /// A sphere round `at`, `squash` of its height (1 round).
    pub(super) fn shell(at: Vec3, squash: f32) -> Self {
        let mut p = Self::of(SHELL, at, Vec3::ZERO);
        p.0.c = [squash, 0.0, 0.0];
        p
    }

    /// Its size: widths at either end, radii from and to, a body's half-width and length.
    pub(super) fn size(mut self, from: f32, to: f32) -> Self {
        self.0.size_from = from;
        self.0.size_to = to;
        self
    }

    /// Lit from `start` for `life` seconds, drawn on over `grow` of it.
    pub(super) fn time(mut self, start: f32, life: f32, grow: f32) -> Self {
        self.0.start = start;
        self.0.life = life;
        self.0.grow = grow;
        self
    }

    /// Held lit from `time`, drawn fresh each frame.
    pub(super) fn now(self, time: f32) -> Self {
        self.time(time - 0.001, HELD, 0.0)
    }

    pub(super) fn power(mut self, power: f32) -> Self {
        self.0.power = power;
        self
    }

    pub(super) fn flags(mut self, flags: u32) -> Self {
        self.0.flags |= flags;
        self
    }

    /// A ring's band across, metres.
    pub(super) fn width(mut self, width: f32) -> Self {
        self.0.width = width;
        self
    }

    pub(super) fn tier(mut self, tier: f32) -> Self {
        self.0.tier = tier;
        self
    }

    pub(super) fn seed(mut self, seed: f32) -> Self {
        self.0.seed = seed;
        self
    }

    fn end(&self) -> f32 {
        self.0.start + self.0.life
    }
}

/// A light the hard light throws for a while: where, its colour times brightness, its
/// reach, and when and how long.
#[derive(Clone, Copy)]
struct Flash {
    at: Vec3,
    color: Vec3,
    range: f32,
    start: f32,
    life: f32,
}

/// The Progenitor's hard light: what events have laid, what the last tick says is
/// standing and flying, and the pass that draws it.
pub(super) struct ProgenitorFx {
    pass: ProgenitorPass,
    lance: LanceLook,
    /// Pieces laid by events, until they go out.
    laid: Vec<Piece>,
    flashes: Vec<Flash>,
    /// This tick's Shapers and the ground each stands on.
    shapers: Vec<(ShaperInstance, Vec3)>,
    /// The splash each Lance last struck with, by unit (its mark's size while it charges).
    splashes: Vec<(u32, f32)>,
    /// Replication shells in the air, and each one's Seedcaster's level by serial (kept
    /// to its landing).
    shells: Vec<ShellInstance>,
    shell_levels: Vec<(u32, u16)>,
    /// The Progenitor's missiles in flight, and where each one's trail was last laid to.
    missiles: Vec<StrategicInstance>,
    trails: Vec<(u32, Vec3)>,
    /// Detonations already drawn this second (the same tick's events can come twice).
    heard: Vec<(Vec3, f32)>,
    /// When the last tick came, for easing between ticks.
    tick_time: f32,
}

impl ProgenitorFx {
    pub(super) fn new(gpu: &Gpu, layouts: &Layouts, passes: &Passes) -> Result<Self, GpuError> {
        Ok(Self {
            pass: ProgenitorPass::new(gpu, layouts, passes)?,
            lance: LanceLook::from_env(),
            laid: Vec::new(),
            flashes: Vec::new(),
            shapers: Vec::new(),
            splashes: Vec::new(),
            shells: Vec::new(),
            shell_levels: Vec::new(),
            missiles: Vec::new(),
            trails: Vec::new(),
            heard: Vec::new(),
            tick_time: 0.0,
        })
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        self.pass.destroy(gpu);
    }

    /// Inside `scene_over` (see `ProgenitorPass::draw`).
    pub(super) fn draw(
        &self,
        device: &ash::Device,
        cmd: ash::vk::CommandBuffer,
        layout: ash::vk::PipelineLayout,
    ) {
        self.pass.draw(device, cmd, layout);
    }

    /// Keeps `p` until it goes out.
    fn lay(&mut self, p: Piece) {
        if self.laid.len() >= MOST_LAID {
            // The deliberate cosmetic cap: the oldest goes first.
            self.laid.remove(0);
        }
        self.laid.push(p);
    }

    fn flash(&mut self, at: Vec3, color: Vec3, range: f32, start: f32, life: f32) {
        // A cosmetic cap on lights kept: the oldest goes first.
        if self.flashes.len() >= 64 {
            self.flashes.remove(0);
        }
        self.flashes.push(Flash {
            at,
            color,
            range,
            start,
            life,
        });
    }

    /// The Shaper `unit` this tick, and the ground it stands on.
    fn shaper(&self, unit: u32) -> Option<&(ShaperInstance, Vec3)> {
        self.shapers.iter().find(|(s, _)| s.unit == unit)
    }
}

/// Where a Shaper's Lens is, `foot` the ground it stands on (its model's,
/// `models::shaper::lens_height`).
pub(super) fn lens_of(s: &ShaperInstance, foot: Vec3) -> Vec3 {
    foot + Vec3::Z * crate::models::shaper::lens_height(s.kind, s.height)
}

/// Its tier as drawn: past the sixth the pieces stop multiplying and only grow brighter.
pub(super) fn tier_of(s: &ShaperInstance) -> f32 {
    f32::from(s.tier.max(1))
}

impl Renderer {
    /// A sim event the Progenitor draws in its own light; true when it was one (and
    /// nothing else is to draw it).
    pub(super) fn progenitor_event(&mut self, event: &SimEvent, time: f32) -> bool {
        use mc_data::strategic::StrategicLook::Progenitor;
        match event {
            SimEvent::LanceStrike {
                unit, to, splash, ..
            } => {
                self.lance_strike(unit.0, Vec3::from(to.to_f32()), splash.to_f32(), time);
                true
            }
            SimEvent::ReplicationShellFired {
                serial, from, to, ..
            } => {
                self.shell_fired(
                    *serial,
                    Vec3::from(from.to_f32()),
                    Vec3::from(to.to_f32()),
                    time,
                );
                true
            }
            SimEvent::ReplicationShellLanded {
                serial,
                at,
                radius,
                units,
            } => {
                self.shell_landed(
                    *serial,
                    Vec3::from(at.to_f32()),
                    radius.to_f32(),
                    *units,
                    time,
                );
                true
            }
            SimEvent::InterceptorLaunch {
                from,
                look: Progenitor,
                ..
            } => {
                self.shard_launch(Vec3::from(from.to_f32()), time);
                true
            }
            SimEvent::NuclearLaunch {
                from,
                look: Progenitor,
                ..
            } => {
                self.warhead_launch(Vec3::from(from.to_f32()), time);
                true
            }
            SimEvent::NuclearDetonation {
                pos,
                radius,
                look: Progenitor,
                ..
            } => {
                self.unmaking(Vec3::from(pos.to_f32()), radius.to_f32(), time);
                true
            }
            SimEvent::WarheadIntercepted {
                pos,
                killed,
                look,
                by,
                ..
            } if *by == Progenitor || (*killed && *look == Progenitor) => {
                let at = Vec3::from(pos.to_f32());
                if !killed {
                    self.shard_spent(at, time);
                } else if *by == Progenitor {
                    self.warhead_unmade(at, time);
                } else {
                    self.warhead_broken(at, time);
                }
                true
            }
            _ => false,
        }
    }

    /// Once a sim tick: what stands and flies now, and the trails behind the missiles.
    pub(super) fn progenitor_tick(&mut self, frame: &RenderFrame, time: f32) {
        let fx = &mut self.progenitor_fx;
        fx.tick_time = time;
        fx.shapers.clear();
        for s in &frame.shapers {
            if let Some(u) = frame.units.iter().find(|u| u.unit_id == s.unit) {
                fx.shapers.push((s.clone(), Vec3::from(u.pos)));
            }
        }
        fx.shells.clone_from(&frame.shells);
        for s in &frame.shells {
            if !fx.shell_levels.iter().any(|l| l.0 == s.serial) {
                fx.shell_levels.push((s.serial, s.level));
            }
        }
        // Forgotten a while after it lands (its landing reads it the tick it lands).
        if fx.shell_levels.len() > 256 {
            fx.shell_levels.drain(..128);
        }
        self.progenitor_missiles(frame, time);
    }

    /// Every frame: this frame's pieces (what events laid, and what stands and flies now,
    /// between the last two ticks at `alpha`) handed to the GPU.
    pub(super) fn progenitor_frame(&mut self, time: f32, alpha: f32, camera: &Camera) {
        let fx = &mut self.progenitor_fx;
        fx.laid.retain(|p| time < p.end());
        fx.flashes.retain(|f| time < f.start + f.life);
        fx.heard.retain(|h| time - h.1 < 1.0);
        let mut frame: Vec<Piece> = Vec::new();
        self.lance_charges(&mut frame, time);
        self.shells_in_flight(&mut frame, time, alpha);
        self.missiles_in_flight(&mut frame, time, alpha);
        let mut all: Vec<GpuPiece> = self
            .progenitor_fx
            .laid
            .iter()
            .filter(|p| p.0.start <= time + 2.0)
            .chain(frame.iter())
            .map(|p| p.0)
            .collect();
        if all.len() > MAX_PIECES as usize {
            // Too many at once: the furthest from the eye are left out.
            let eye = camera.eye();
            all.sort_by(|a, b| {
                Vec3::from(a.a)
                    .distance_squared(eye)
                    .total_cmp(&Vec3::from(b.a).distance_squared(eye))
            });
            all.truncate(MAX_PIECES as usize);
        }
        all.sort_by_key(|p| p.kind);
        self.progenitor_fx.pass.upload(&all);
    }

    /// Every frame: the light the hard light throws.
    pub(super) fn progenitor_lights(&mut self, time: f32) {
        for f in &self.progenitor_fx.flashes {
            let k = 1.0 - ((time - f.start) / f.life).clamp(0.0, 1.0);
            if time < f.start || k <= 0.0 {
                continue;
            }
            self.lights
                .lamp(f.at, Vec3::NEG_Z, f.color * k * k, f.range, 180.0, 1.0);
        }
        for (s, foot) in &self.progenitor_fx.shapers {
            let f = s.lance_charge;
            if f <= 0.0 {
                continue;
            }
            let tier = tier_of(s).min(6.0);
            self.lights.lamp(
                lens_of(s, *foot),
                Vec3::NEG_Z,
                LIGHT.lerp(HOT, f) * (3.0e4 * f * f * (1.0 + tier * 0.3)),
                120.0 + 30.0 * tier,
                180.0,
                1.0,
            );
        }
    }
}
