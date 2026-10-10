//! A gravitic lens at work, as it is drawn (`Weapon::lens`, the Lodestone; `mc_sim::lens`):
//! of a piece with the Regency's gravity engines, a small, quick cousin of the Fallen's.
//! Its heart is dark: it bends the scene behind it (the copy the water sees through,
//! lens_fx.wgsl) and darkens it. Round that heart runs the Fallen's violet energy (its
//! colours, and its lightning made small, `fade_beam::LENS_ARC`), wound round the dark,
//! never a ball of light; the gains stay moderate, as a bright violet blooms to white.
//!
//! - **Charge** (`WeaponCharging`): a black pinpoint swells over the emitter with violet
//!   filaments winding round it, rings of bent air close in on it one after another, and
//!   grit and dust are drawn in to it.
//! - **Launch** (`LensLaunched`): a deep pressure snap at the emitter; the pinpoint is let
//!   go as the front.
//! - **Front**: a dark lens running out to its target, black at its heart and bending what
//!   lies behind it round that heart, wrapped in swirling violet filaments with a frayed
//!   violet wake; small arcs crackle off it, back along its way and down to the ground,
//!   and its light falls on what it passes. Far off it is drawn larger, so it reads at a
//!   game's zoom. It leaves a short corridor of rippled air and kicks dust off the ground.
//! - **Trail**: under its path the front splits the ground, the Fallen's rift in small
//!   (`rift::TRAIL`, drawn by the terrain from the gravitic engines' records,
//!   `gravitic_engine_fx::ground`): a jagged crack, the ground either side broken into
//!   tipped plates, cracks branching off it at a slant. Violet light down in the crack
//!   burns behind the head and cools in about a second to embers, which pulse in toward
//!   the well while it holds. Let go, the light dies and the cracks fade out over some
//!   seconds. Trails keep to their own share of the
//!   records (`rift::TRAILS`), the oldest going first.
//! - **Strike** (`LensStruck`): a violet ring of bent air snaps inward onto the target,
//!   arcs run in from round the well's edge, a dark flash throws off a ragged violet ring
//!   and a violet light, then dust is pushed out across the well and the trees bowed.
//! - **Well**, locked on the target and carried with it: the ground under it darkened and
//!   rippling in toward the middle with thin violet arms spiralling in, its edge pressed
//!   into a ring with violet energy crawling along it and arcs flickering over it, a low
//!   dome of bent air over it, grit lifted off the ground and circling slowly low down,
//!   and a thread of rippled air back to the lens that holds it, violet pulses running
//!   out along it.
//! - **Release** (`LensReleased`): the well relaxes outward in a soft pulse, the grit
//!   drops, the trail goes dark, and the pressed ring is left on the ground, fading.
//!
//! Presentation only; the renderer's own clock.

pub(crate) mod pass;

use super::regency_guns_fx::RED;
use super::stun_fx::Flash;
use super::{Renderer, PUFF_DUST};
use crate::gpu::{Gpu, GpuError};
use crate::gpu_consts::fade_beam::LENS_ARC;
use crate::gpu_consts::lens_fx::{FLASH, IMPLODE, MAX_LENSES, PRINT, RELEASE};
use crate::gpu_consts::puff;
use crate::pipelines::{Layouts, Passes};
use glam::{Vec2, Vec3};
use mc_data::{BlueprintId, Weapon};
use mc_sim::mirror::{RenderFrame, SimEvent};
use pass::{GpuLens, LensPass};
use std::f32::consts::TAU;

/// A hard black speck with a red glint at its rim (warp_puffs.wgsl), slowed by the air.
const SPECK: f32 = puff::WARP_SWARM as f32;
/// How hard the air brakes a speck (warp_puffs.wgsl's drag on all but its glow).
const SPECK_DRAG: f32 = 1.8;
/// The Regency's violet, the Fallen's: for the glint of what a lens draws in and the
/// light its arcs and front throw.
const VIOLET: Vec3 = Vec3::new(0.55, 0.16, 1.0);
/// The reach of the cracks a front splits the ground with either side of its path, metres.
const TRAIL_WIDTH: f32 = 5.0;
/// What a trail is fed (`RiftGround::trail_run`): embers, pulses toward the well, -,
/// its head, its scar; while the front runs, and while the well holds.
const TRAIL_RUNNING: [f32; 5] = [0.1, 0.0, 0.0, 1.0, 1.0];
const TRAIL_HOLDING: [f32; 5] = [0.16, 1.0, 0.0, 1.0, 1.0];
/// Arcs a front throws off a tick, and a held well round its edge, at most: a deliberate
/// cosmetic cap, so a crowd of lenses stays within the strokes' budget.
const FRONT_ARCS: usize = 3;
const WELL_ARCS: usize = 2;
/// Seconds a lens lost from sight (fog) takes to fade out.
const LOST_FADE: f32 = 0.4;
/// Metres apart the dust under a running front is kicked up, and the height over the
/// ground past which the front raises none.
const DUST_STEP: f32 = 3.0;
const DUST_HEIGHT: f32 = 30.0;
/// How much of a blast's dust a lens's puffs carry.
const DUST_THIN: f32 = 0.4;

/// One lens's work, from its charge to the print its well leaves.
#[derive(Clone, Copy)]
struct Lens {
    unit: u32,
    weapon: u8,
    /// Last tick's and this tick's: the emitter, the front, the ground under the target.
    emitter: (Vec3, Vec3),
    front: (Vec3, Vec3),
    ground: (Vec3, Vec3),
    radius: f32,
    /// How fast its front runs, m/s.
    speed: f32,
    /// When its charge began, and how long it takes.
    charge: Option<(f32, f32)>,
    launch: Option<f32>,
    strike: Option<f32>,
    release: Option<f32>,
    /// When it dropped out of the sim's list without letting go (lost from sight).
    lost: Option<f32>,
    /// In this tick's list.
    seen: bool,
    seed: f32,
}

impl Lens {
    fn new(unit: u32, weapon: u8, at: Vec3, seed: f32) -> Self {
        Self {
            unit,
            weapon,
            emitter: (at, at),
            front: (at, at),
            ground: (at, at),
            radius: 30.0,
            speed: 400.0,
            charge: None,
            launch: None,
            strike: None,
            release: None,
            lost: None,
            seen: false,
            seed,
        }
    }

    /// Still charging, running or holding: not yet let go nor lost.
    fn live(&self) -> bool {
        self.release.is_none() && self.lost.is_none()
    }

    /// Whether it is done with at `time`: its print gone, its fade over, or a charge that
    /// was never let go.
    fn spent(&self, time: f32) -> bool {
        if let Some(lost) = self.lost {
            return time > lost + LOST_FADE;
        }
        if let Some(release) = self.release {
            return time > release + PRINT.max(RELEASE);
        }
        match (self.charge, self.launch, self.strike) {
            (Some((start, seconds)), None, None) => time > start + seconds + 1.5,
            _ => false,
        }
    }
}

/// The lenses at work and the pass that draws them.
pub(super) struct LensFx {
    lenses: Vec<Lens>,
    pass: LensPass,
}

impl LensFx {
    pub(super) fn new(gpu: &Gpu, layouts: &Layouts, passes: &Passes) -> Result<Self, GpuError> {
        Ok(Self {
            lenses: Vec::new(),
            pass: LensPass::new(gpu, layouts, passes)?,
        })
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        self.pass.destroy(gpu);
    }

    /// The live lens of `unit`'s weapon `weapon`, if it has one.
    fn live(&mut self, unit: u32, weapon: u8) -> Option<&mut Lens> {
        self.lenses
            .iter_mut()
            .rev()
            .find(|l| l.unit == unit && l.weapon == weapon && l.live())
    }

    /// A new lens, making room for it: a deliberate cosmetic cap, as many as are drawn;
    /// the oldest goes first.
    fn start(&mut self, lens: Lens) -> &mut Lens {
        if self.lenses.len() >= MAX_LENSES as usize {
            self.lenses.remove(0);
        }
        self.lenses.push(lens);
        let last = self.lenses.len() - 1;
        &mut self.lenses[last]
    }

    /// Hands the GPU this frame's lenses, `alpha` of the way from the last tick to this one.
    pub(super) fn upload(&mut self, time: f32, alpha: f32) {
        let none = -1.0;
        let lenses: Vec<GpuLens> = self
            .lenses
            .iter()
            .map(|l| {
                let at = |(a, b): (Vec3, Vec3)| a.lerp(b, alpha);
                let (emitter, front) = (at(l.emitter), at(l.front));
                let fade = l.lost.map_or(1.0, |lost| {
                    1.0 - ((time - lost) / LOST_FADE).clamp(0.0, 1.0)
                });
                GpuLens {
                    emitter: emitter.to_array(),
                    charge: l.charge.map_or(none, |c| c.0),
                    front: front.to_array(),
                    launch: l.launch.unwrap_or(none),
                    ground: at(l.ground).to_array(),
                    strike: l.strike.unwrap_or(none),
                    radius: l.radius,
                    charge_time: l.charge.map_or(1.0, |c| c.1),
                    release: l.release.unwrap_or(none),
                    seed: l.seed,
                    fade,
                    run: emitter.distance(front),
                    _pad: [0.0; 2],
                }
            })
            .collect();
        self.pass.upload(&lenses);
    }

    /// Inside `scene_over` (see `LensPass::draw`).
    pub(super) fn draw(
        &self,
        device: &ash::Device,
        cmd: ash::vk::CommandBuffer,
        layout: ash::vk::PipelineLayout,
        seen: ash::vk::DescriptorSet,
    ) {
        self.pass.draw(device, cmd, layout, seen);
    }
}

impl Renderer {
    /// A puff of dust a lens raises: thinner than a blast's, so many can overlap into a
    /// low haze and none reads as a ball of its own.
    fn lens_dust(&mut self, pos: Vec3, vel: Vec3, start: f32, life: f32, size: (f32, f32)) {
        let kept = self.effect_settings;
        self.effect_settings.dust_visibility = kept.dust_visibility * DUST_THIN;
        self.push_puff(PUFF_DUST, pos, vel, start, life, size);
        self.effect_settings = kept;
    }

    /// Weapon `weapon` of `blueprint`, when it is a gravitic lens.
    fn lens_weapon(&self, blueprint: BlueprintId, weapon: u8) -> Option<&Weapon> {
        self.blueprints
            .unit(blueprint)
            .weapons
            .get(weapon as usize)
            .filter(|w| w.lens.is_some())
    }

    /// Takes `event` when it is a gravitic lens's: true when that is all of it. Its charge
    /// and its shot are drawn here instead of a gun's glow and muzzle flash.
    pub(super) fn lens_event(&mut self, event: &SimEvent, time: f32) -> bool {
        match *event {
            SimEvent::WeaponCharging {
                unit,
                pos,
                blueprint,
                weapon,
                ..
            } => {
                let Some(w) = self.lens_weapon(blueprint, weapon) else {
                    return false;
                };
                let seconds = (w.charge_ticks as f32 * self.tick_seconds.max(0.02)).max(0.2);
                let at = Vec3::from(pos.to_f32());
                let seed = self.scatter.unit() * 97.0;
                let fx = &mut self.lens_fx;
                let lens = match fx.live(unit.0, weapon) {
                    // Charging again for a new mark before it was let go.
                    Some(l) if l.launch.is_none() && l.strike.is_none() => l,
                    // A front out or a well held: the gun does not charge while it is.
                    Some(_) => return true,
                    None => fx.start(Lens::new(unit.0, weapon, at, seed)),
                };
                lens.emitter = (at, at);
                lens.front = (at, at);
                lens.charge = Some((time, seconds));
                true
            }
            SimEvent::ShotFired {
                blueprint, weapon, ..
            } => self.lens_weapon(blueprint, weapon).is_some(),
            SimEvent::LensLaunched {
                unit, weapon, from, ..
            } => {
                let at = Vec3::from(from.to_f32());
                let seed = self.scatter.unit() * 97.0;
                let fx = &mut self.lens_fx;
                let lens = match fx.live(unit.0, weapon) {
                    Some(l) => l,
                    None => fx.start(Lens::new(unit.0, weapon, at, seed)),
                };
                lens.launch = Some(time);
                lens.emitter = (at, at);
                lens.front = (at, at);
                self.lens_snap(at, time);
                true
            }
            SimEvent::LensStruck {
                unit,
                weapon,
                pos,
                radius,
                ..
            } => {
                let radius = radius.to_f32().max(1.0);
                // The front is drawn gliding over the coming tick: it reaches the
                // target as the tick ends.
                let time = time + self.tick_seconds.max(0.02);
                let ground = self.lens_fx.live(unit.0, weapon).map(|l| {
                    l.strike = Some(time);
                    l.launch.get_or_insert(time);
                    l.radius = radius;
                    l.ground.1
                });
                let at = Vec3::from(pos.to_f32());
                let foot = ground
                    .filter(|g| g.truncate().distance(at.truncate()) < radius)
                    .unwrap_or_else(|| at.truncate().extend(self.ground_height(at.truncate())));
                self.lens_strike(foot, radius, time);
                true
            }
            SimEvent::LensReleased { unit, weapon, .. } => {
                self.gravitic_engine_fx.ground.release_trail(unit.0, time);
                let well = self.lens_fx.live(unit.0, weapon).map(|l| {
                    l.release = Some(time);
                    (l.strike.is_some(), l.ground.1, l.radius)
                });
                if let Some((true, foot, radius)) = well {
                    self.lens_letting_go(foot, radius, time);
                }
                true
            }
            _ => false,
        }
    }

    /// A new tick's lenses (`RenderFrame::lenses`): where each one's front and well now
    /// are, and what each throws up over the coming tick.
    pub(super) fn lens_tick(&mut self, frame: &RenderFrame, time: f32) {
        let fx = &mut self.lens_fx;
        for l in &mut fx.lenses {
            l.seen = false;
            // Held where it was last seen.
            l.emitter.0 = l.emitter.1;
            l.front.0 = l.front.1;
            l.ground.0 = l.ground.1;
        }
        for v in &frame.lenses {
            let (emitter, front, ground) = (
                Vec3::from(v.emitter),
                Vec3::from(v.front),
                Vec3::from(v.ground),
            );
            let lens = match fx.live(v.unit_id, v.weapon) {
                Some(l) if l.launch.is_some() || l.strike.is_some() => {
                    l.emitter.1 = emitter;
                    l.front.1 = front;
                    l.ground.1 = ground;
                    l
                }
                // First seen running (its launch missed or its charge seen alone): it
                // glides out from the emitter.
                found => {
                    let seed = (v.unit_id.wrapping_mul(2_654_435_761) >> 8) as f32 / 65536.0;
                    let lens = match found {
                        Some(l) => l,
                        None => fx.start(Lens::new(v.unit_id, v.weapon, emitter, seed % 97.0)),
                    };
                    lens.emitter = (emitter, emitter);
                    lens.front = (emitter, front);
                    lens.ground = (ground, ground);
                    lens.launch = Some(time - v.seconds);
                    lens
                }
            };
            lens.seen = true;
            lens.radius = v.radius.max(1.0);
            if let Some(w) = self
                .blueprints
                .unit(v.blueprint)
                .weapons
                .get(v.weapon as usize)
            {
                lens.speed = w.projectile_speed.to_f32().max(1.0);
            }
            if v.struck && lens.strike.is_none() {
                // Struck before it was seen: the well is laid standing, without its strike.
                lens.strike = Some(time - IMPLODE - FLASH - 1.0);
                lens.front.0 = lens.front.1;
                lens.ground.0 = lens.ground.1;
            }
        }
        let ground = &mut self.gravitic_engine_fx.ground;
        for l in &mut fx.lenses {
            if !l.seen && l.launch.is_some() && l.live() {
                l.lost = Some(time);
                ground.release_trail(l.unit, time);
            }
        }
        fx.lenses.retain(|l| !l.spent(time));
        let lenses = self.lens_fx.lenses.clone();
        for l in lenses.iter().filter(|l| l.live()) {
            self.lens_trail(l);
            match (l.launch, l.strike) {
                (None, _) => self.lens_drawing_in(l, time),
                (Some(_), None) => {
                    self.lens_front_dust(l, time);
                    self.lens_front_arcs(l, time);
                }
                (Some(_), Some(strike)) => {
                    self.lens_well_grit(l, strike, time);
                    self.lens_well_arcs(l, strike, time);
                }
            }
        }
    }

    /// The trail a running front presses into the ground under its path
    /// (`rift::TRAIL`, drawn by the terrain): laid as the front leaves, run out as far as
    /// the front has got (a tick ahead, as the front is drawn gliding), and its embers
    /// fed while the well holds.
    fn lens_trail(&mut self, l: &Lens) {
        let Some(launch) = l.launch else {
            return;
        };
        let from = l.emitter.1.truncate();
        let to = l.ground.1.truncate();
        let length = from.distance(to);
        let dt = self.tick_seconds.max(0.02);
        let ground = &mut self.gravitic_engine_fx.ground;
        if !ground.has_trail(l.unit) {
            ground.trail(l.unit, from, to, l.speed, TRAIL_WIDTH, launch);
        }
        let (run, fed) = match l.strike {
            None => {
                let run = l.front.1.truncate().distance(from) + l.speed * dt * 1.2;
                (run.min(length), TRAIL_RUNNING)
            }
            Some(_) => (length, TRAIL_HOLDING),
        };
        ground.trail_run(l.unit, to, run, fed);
    }

    /// A violet arc of a lens's: a short crooked stroke of the Fallen's lightning made
    /// small and kept violet (`fade_beam::LENS_ARC`).
    fn lens_arc(&mut self, from: Vec3, to: Vec3, start: f32, life: f32, width: f32) {
        let side = (to - from).cross(Vec3::Z).normalize_or(Vec3::X);
        let wander = from.distance(to) * 0.18;
        self.emp_kinks(
            from,
            to,
            wander,
            side,
            Vec3::Z,
            start,
            life,
            width,
            5,
            LENS_ARC,
        );
    }

    /// Over the coming tick of a running front: violet arcs crackling off it, back along
    /// its way and down to the ground under it, and its light on what it passes.
    fn lens_front_arcs(&mut self, l: &Lens, time: f32) {
        let dt = self.tick_seconds.max(0.02);
        let (was, now) = l.front;
        let back = (was - now).normalize_or(-Vec3::X);
        for _ in 0..FRONT_ARCS {
            let f = self.scatter.unit();
            let at = was.lerp(now, f);
            let born = time + f * dt;
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed() * 0.6,
            )
            .normalize_or(Vec3::Z);
            let reach = 3.0 + 5.0 * self.scatter.unit();
            let to = at + (out + back * 1.2).normalize_or(back) * reach;
            let life = 0.07 + 0.08 * self.scatter.unit();
            let width = 0.22 + 0.2 * self.scatter.unit();
            self.lens_arc(at, to, born, life, width);
        }
        // Now and then one down to the ground under it, when it runs low.
        let ground = self.ground_height(now.truncate());
        if now.z - ground < 16.0 && self.scatter.unit() < 0.5 {
            let f = self.scatter.unit();
            let at = was.lerp(now, f);
            let foot = at.truncate()
                + Vec2::from_angle(self.scatter.unit() * TAU) * 2.5 * self.scatter.unit();
            let to = foot.extend(self.ground_height(foot) + 0.2);
            self.lens_arc(at, to, time + f * dt, 0.12, 0.3);
        }
        // Its violet light on the ground and hulls it passes.
        self.emp_fx.flashes.push(Flash {
            pos: was.lerp(now, 0.5),
            start: time,
            life: dt * 1.3,
            color: VIOLET * 260.0,
            range: 26.0,
            flicker: 0.5,
        });
    }

    /// Over the coming tick of a held well: an arc or two crawling along its edge, and
    /// now and then one up from the edge onto what it holds.
    fn lens_well_arcs(&mut self, l: &Lens, strike: f32, time: f32) {
        if time < strike + IMPLODE {
            return;
        }
        let dt = self.tick_seconds.max(0.02);
        let foot = l.ground.1;
        let r = l.radius;
        let arcs = (self.scatter.unit() * (WELL_ARCS as f32 + 0.6)) as usize;
        for _ in 0..arcs.min(WELL_ARCS) {
            let a = self.scatter.unit() * TAU;
            let b = a + (0.25 + 0.35 * self.scatter.unit()) * self.scatter.signed().signum();
            let rim = |angle: f32, out: f32| foot.truncate() + Vec2::from_angle(angle) * r * out;
            let (pa, pb) = (rim(a, 0.97), rim(b, 0.97));
            let from = pa.extend(self.ground_height(pa) + 0.6);
            let to = if self.scatter.unit() < 0.3 {
                // Up onto the target.
                l.front.1 + Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.5) * 2.0
            } else {
                pb.extend(self.ground_height(pb) + 0.6)
            };
            let born = time + self.scatter.unit() * dt;
            let life = 0.1 + 0.1 * self.scatter.unit();
            let width = 0.3 + 0.2 * self.scatter.unit();
            self.lens_arc(from, to, born, life, width);
            if self.scatter.unit() < 0.5 {
                self.emp_fx.flashes.push(Flash {
                    pos: to + Vec3::Z * 2.0,
                    start: born,
                    life: life * 1.5,
                    color: VIOLET * 80.0,
                    range: r * 0.5,
                    flicker: 0.9,
                });
            }
        }
    }

    /// Over the coming tick of a charge: black grit and a little dust drawn in to the
    /// pinpoint from round it, more as it gathers.
    fn lens_drawing_in(&mut self, l: &Lens, time: f32) {
        let Some((start, seconds)) = l.charge else {
            return;
        };
        let dt = self.tick_seconds.max(0.02);
        let p = ((time - start) / seconds).clamp(0.0, 1.0);
        if time > start + seconds {
            return;
        }
        let at = l.emitter.1;
        let water = self.map_info.water_level.to_f32();
        let specks = 2 + (p * 6.0) as usize;
        for _ in 0..specks {
            let angle = self.scatter.unit() * TAU;
            let out = 7.0 + 9.0 * self.scatter.unit();
            let from = at
                + Vec2::from_angle(angle).extend(0.0) * out
                + Vec3::Z * (self.scatter.signed() * 3.0 - 1.5);
            let life = 0.35 + 0.3 * self.scatter.unit();
            // Carried on `vel` and braked by the air: it comes to rest on the pinpoint.
            let vel = (at - from) * (SPECK_DRAG / (1.0 - (-SPECK_DRAG * life).exp()));
            let born = time + dt * self.scatter.unit();
            let size = 0.22 + 0.25 * self.scatter.unit();
            self.push_warp(
                SPECK,
                from,
                vel,
                born,
                life,
                (size, size * 0.3),
                VIOLET.lerp(RED, 0.5) * 0.25,
                0.0,
            );
        }
        // Dust drawn off the ground toward its foot, low.
        if self.scatter.unit() < 0.4 + 0.5 * p {
            let angle = self.scatter.unit() * TAU;
            let foot = at.truncate() + Vec2::from_angle(angle) * (9.0 + 6.0 * self.scatter.unit());
            let ground = self.ground_height(foot);
            if ground > water + 0.05 {
                let from = foot.extend(ground + 0.4);
                let toward = (at.truncate() - foot).normalize_or(Vec2::X);
                let vel = (toward * (6.0 + 6.0 * p)).extend(2.0 + 3.0 * p);
                let size = 0.9 + 0.6 * self.scatter.unit();
                self.lens_dust(from, vel, time, 0.9, (size, size * 0.6));
            }
        }
    }

    /// The pinpoint let go: a deep pressure snap round the emitter (a ring of bent air
    /// running out, lens_fx.wgsl), the trees round it shoved, and dust thrown off the
    /// ground under it.
    fn lens_snap(&mut self, at: Vec3, time: f32) {
        self.tree_blasts.record(at, time, 22.0, 0.5, false);
        let ground = self.ground_height(at.truncate());
        if ground <= self.map_info.water_level.to_f32() + 0.05 {
            return;
        }
        for k in 0..14 {
            let angle = (k as f32 + self.scatter.unit()) / 14.0 * TAU;
            let out = Vec2::from_angle(angle);
            let from =
                (at.truncate() + out * (5.0 + 2.0 * self.scatter.unit())).extend(ground + 0.4);
            let vel = (out * (10.0 + 6.0 * self.scatter.unit())).extend(1.0);
            let size = 1.0 + 0.6 * self.scatter.unit();
            self.lens_dust(from, vel, time, 0.9, (size, size * 3.0));
        }
    }

    /// Over the coming tick of a running front: a line of dust kicked up off the ground
    /// under its path, less the higher it runs.
    fn lens_front_dust(&mut self, l: &Lens, time: f32) {
        let (from, to) = l.front;
        let span = from.truncate().distance(to.truncate());
        let water = self.map_info.water_level.to_f32();
        let dt = self.tick_seconds.max(0.02);
        let steps = ((span / DUST_STEP).ceil() as usize).min(24);
        let side = (to - from).truncate().normalize_or(Vec2::X).perp();
        for k in 0..steps {
            let f = (k as f32 + self.scatter.unit()) / steps.max(1) as f32;
            let p = from.lerp(to, f);
            let ground = self.ground_height(p.truncate());
            let high = (p.z - ground) / DUST_HEIGHT;
            if high >= 1.0 || ground <= water + 0.05 {
                continue;
            }
            // Patchy, not a row of beads: some stretches throw none, the rest are thrown
            // off to either side by different amounts.
            if self.scatter.unit() < 0.45 {
                continue;
            }
            let strength = 1.0 - high;
            let way = self.scatter.signed();
            let at = (p.truncate() + side * way * 3.0).extend(ground + 0.2);
            let out = side * way.signum() * (6.0 + 8.0 * self.scatter.unit());
            let vel = out.extend(0.4 + 1.2 * strength);
            let size = (0.8 + 2.2 * self.scatter.unit().powi(2)) * strength.sqrt();
            // As the front reaches this stretch over the coming tick.
            let born = time + f * dt;
            self.lens_dust(at, vel, born, 1.6, (size, size * 2.6));
        }
    }

    /// The front strikes on `foot`: a ring of bent air closes on the target (lens_fx.wgsl),
    /// a dark flash, then dust pushed out across the well and the trees bowed.
    fn lens_strike(&mut self, foot: Vec3, radius: f32, time: f32) {
        let push = time + IMPLODE + 0.06;
        // As the ring closes: arcs from out round the well's edge in onto the target, and
        // a violet flash with the dark one.
        for k in 0..7 {
            let angle = (k as f32 + self.scatter.unit()) / 7.0 * TAU;
            let rim = foot.truncate()
                + Vec2::from_angle(angle) * radius * (0.7 + 0.3 * self.scatter.unit());
            let from = rim.extend(self.ground_height(rim) + 0.8);
            let to = foot + Vec3::Z * (2.0 + 2.0 * self.scatter.unit());
            let born = time + IMPLODE * (0.3 + 0.6 * self.scatter.unit());
            self.lens_arc(from, to, born, 0.16, 0.45);
        }
        self.emp_fx.flashes.push(Flash {
            pos: foot + Vec3::Z * 5.0,
            start: time + IMPLODE,
            life: FLASH + 0.15,
            color: VIOLET * 700.0,
            range: radius * 1.6,
            flicker: 0.3,
        });
        self.tree_blasts
            .record(foot, push, radius * 1.3, 0.7, false);
        let water = self.map_info.water_level.to_f32();
        // Dust pushed out across the well along the ground, from near the middle out to
        // its edge, where it is pressed down and spills over, low.
        for k in 0..12 {
            let angle = (k as f32 + self.scatter.unit()) / 12.0 * TAU;
            let out = Vec2::from_angle(angle);
            let from = 0.25 + 0.55 * self.scatter.unit();
            let xy = foot.truncate() + out * radius * from;
            let ground = self.ground_height(xy);
            if ground <= water + 0.05 {
                continue;
            }
            // Out to about the edge before the air stops it (puffs.wgsl's drag on dust).
            let vel = (out * radius * (1.0 - from) * (2.3 + 0.5 * self.scatter.unit())).extend(0.6);
            let size = 1.5 + 1.0 * self.scatter.unit();
            let born = push + 0.05 * from;
            self.lens_dust(xy.extend(ground + 0.3), vel, born, 1.0, (size, size * 2.2));
        }
    }

    /// Over the coming tick of a held well: a little grit dragged in low off its edge.
    fn lens_well_grit(&mut self, l: &Lens, strike: f32, time: f32) {
        if time < strike + IMPLODE + FLASH || self.scatter.unit() < 0.4 {
            return;
        }
        let foot = l.ground.1;
        let angle = self.scatter.unit() * TAU;
        let out = Vec2::from_angle(angle);
        let xy = foot.truncate() + out * l.radius * (0.85 + 0.15 * self.scatter.unit());
        let ground = self.ground_height(xy);
        if ground <= self.map_info.water_level.to_f32() + 0.05 {
            return;
        }
        let vel = (-out * (3.0 + 2.0 * self.scatter.unit())).extend(0.4);
        let size = 0.7 + 0.5 * self.scatter.unit();
        self.lens_dust(xy.extend(ground + 0.3), vel, time, 1.4, (size, size * 1.6));
    }

    /// The well let go on `foot`: a soft pulse outward, and the grit landing in dust.
    fn lens_letting_go(&mut self, foot: Vec3, radius: f32, time: f32) {
        self.tree_blasts
            .record(foot, time, radius * 1.4, 0.25, false);
        let water = self.map_info.water_level.to_f32();
        for _ in 0..10 {
            let xy = foot.truncate()
                + Vec2::from_angle(self.scatter.unit() * TAU) * radius * self.scatter.unit().sqrt();
            let ground = self.ground_height(xy);
            if ground <= water + 0.05 {
                continue;
            }
            let size = 0.8 + 0.6 * self.scatter.unit();
            let vel = Vec3::new(self.scatter.signed(), self.scatter.signed(), 1.0) * 1.5;
            let born = time + 0.35 + 0.3 * self.scatter.unit();
            self.lens_dust(xy.extend(ground + 0.3), vel, born, 1.2, (size, size * 2.0));
        }
    }
}
