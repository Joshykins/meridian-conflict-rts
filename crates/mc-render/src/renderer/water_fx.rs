//! The sea answering what happens on it and under it. Shells and blasts that
//! land in the water throw it up (a column, droplets, spray) and send rings
//! out; a torpedo runs dark under the surface under a thin wake line,
//! and its hit is a tall white column over a flash seen through the water;
//! moving hulls cut a bow wave and leave white water behind; a dying ship
//! blows up a little, burns and steams as it goes down, bubbles as it sinks
//! and boils the surface once more when it settles.
//!
//! The water stays one quad: `shaders/water.wgsl` reads a short list each
//! frame (`upload_sea_fx`) of the rings still spreading, their foam and the
//! flash of a blast lighting the water around it, and of the wakes near the
//! camera with the path they took. Everything thrown up is a puff
//! (`PUFF_DROPLET`, `PUFF_SPRAY`, `PUFF_COLUMN`, `PUFF_BUBBLE`, `PUFF_STEAM`
//! in puffs.wgsl), and all of it dies at the surface: puffs are drawn after the
//! water, so a puff under it would show on top, unrefracted. Bubbles are the
//! one thing drawn under it, dimmed and tinted by their depth.

use super::{Renderer, PUFF_FIRE, PUFF_FIREBALL, PUFF_SMOKE, PUFF_SPARK};
use crate::camera::Camera;
use crate::gpu::Buffer;
use ash::vk;
use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};
use mc_data::{BlueprintId, MoveLayer};
use mc_sim::mirror::{
    HousePose, ProjectileInstance, SimEvent, UnitInstance, KIND_GHOST, KIND_PROP, KIND_WRECK,
    PROJECTILE_ENDS_SHIFT, PROJECTILE_TORPEDO, STATE_RADAR, UNIT_DIVE_MASK, UNIT_HOUSE_SHIFT,
    WRECK_SINKING,
};
use std::collections::HashMap;

/// Water thrown up: a blob of droplets or a torn sheet. It flies an arc and is
/// gone when it falls back into the sea.
pub(super) const PUFF_DROPLET: f32 = 23.0;
/// Fine spray that hangs over the water and drifts off on the wind.
pub(super) const PUFF_SPRAY: f32 = 24.0;
/// A column of white water standing on the surface. `vel.z` is how fast its top
/// leaves the water; the top rises and falls back like anything thrown.
pub(super) const PUFF_COLUMN: f32 = 25.0;
/// Air rising through the water, `vel.z` metres a second; gone at the surface.
pub(super) const PUFF_BUBBLE: f32 = 26.0;
/// White vapour off a hot hull meeting the water.
pub(super) const PUFF_STEAM: f32 = 27.0;

/// What the water shader gets at most: the rings nearest the camera, and the wakes.
const MOST_RIPPLES: usize = 64;
const MOST_WAKES: usize = 48;
/// Rings kept on the CPU side; the nearest `MOST_RIPPLES` go up each frame.
const KEPT_RIPPLES: usize = 256;
/// Points of a wake: the bow and the stern where they are now, then where the
/// stern was, newest first.
const WAKE_POINTS: usize = 8;
/// Seconds between the points a hull leaves behind, and how long white water lasts.
const WAKE_STEP: f32 = 1.3;
const WAKE_LIFE: f32 = 9.0;
/// Seconds the speed a wake is drawn with takes to follow the hull's: the sim's
/// tick-to-tick speed steps as a boat gets going or pulls up, and every stretch
/// of the wake is sized by it.
const WAKE_EASE: f32 = 0.3;
/// A hull that has moved its stern less than this since the last point leaves
/// no new one: a boat lying still lets its wake age where it lies.
const WAKE_MOVED: f32 = 0.3;
/// How fast bubbles come up, metres a second.
const BUBBLE_RISE: f32 = 2.2;

/// Mirrors `SeaRipple` in shaders/water.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuRipple {
    /// xy the centre; z where the flash is (under the surface for a torpedo).
    pos: [f32; 3],
    start: f32,
    /// Size in metres, life in seconds, flash (negative for an energy blast's blue), foam.
    params: [f32; 4],
}

/// Mirrors `SeaWake` in shaders/water.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuWake {
    /// Where the hull is this frame, then its heading as a unit vector.
    at: [f32; 4],
    /// Speed m/s, half length, half beam, kind (0 a hull on the surface, 1 dived, 2 a torpedo).
    shape: [f32; 4],
    /// A circle round everything the wake touches (xy, radius), then how strong it is.
    bound: [f32; 4],
    /// Bow, stern, then the path the stern took, newest first: xy, the speed it had
    /// there, and the age of the water there in seconds.
    trail: [[f32; 4]; WAKE_POINTS],
}

const _: () = assert!(std::mem::size_of::<GpuRipple>() == 32);
const _: () = assert!(std::mem::size_of::<GpuWake>() == 176);

/// Bytes of the list: counts, then the rings, then the wakes.
pub(super) const SEA_FX_BYTES: usize =
    16 + MOST_RIPPLES * std::mem::size_of::<GpuRipple>() + MOST_WAKES * std::mem::size_of::<GpuWake>();

/// A hull on the water, followed from tick to tick for its wake.
struct Hull {
    prev: Vec3,
    pos: Vec3,
    prev_heading: f32,
    heading: f32,
    /// Eased speed at the last tick and this one (`WAKE_EASE`), drawn between them.
    prev_speed: f32,
    speed: f32,
    half_length: f32,
    half_beam: f32,
    kind: f32,
    strength: f32,
    /// Stern positions left behind, oldest first: xy, speed there, render time.
    trail: Vec<[f32; 4]>,
    /// When a big hull last laid the foam of its standing bow wave.
    bow_foam: f32,
    seen: bool,
}

/// A hull with a weapon that charges before it fires, as of the last tick: where the
/// charging glow goes (`Renderer::weapon_charging`), which the sim reports by hull
/// position and weapon only.
pub(super) struct GunHull {
    pub(super) blueprint: u32,
    pub(super) pos: Vec3,
    pub(super) heading: f32,
    /// How its gun houses are turned, when its guns turn on houses of their own.
    pub(super) house: Option<HousePose>,
}

/// Metres a torpedo runs between the points of its path that are kept.
const TORPEDO_STEP: f32 = 10.0;
/// Seconds a torpedo's line lies on the water once its air is up (`life` in
/// water.wgsl's `sea_stir`).
const TORPEDO_LINE: f32 = 4.0;

/// A torpedo in the water, and the line its air leaves on the surface.
struct Torpedo {
    prev: Vec3,
    pos: Vec3,
    speed: f32,
    /// Seconds the air it lets out now takes to reach the surface.
    delay: f32,
    /// Where it ran, oldest first from the tube: xy, speed there, and when the
    /// air it let out there comes up (render time).
    path: Vec<[f32; 4]>,
    /// When it burst or ran out: its line still lies on the water a while.
    ended: Option<f32>,
}

pub(super) struct WaterFx {
    pub(super) buffer: Buffer,
    pub(super) set: vk::DescriptorSet,
    ripples: Vec<GpuRipple>,
    hulls: HashMap<u32, Hull>,
    /// Keyed by where each was at the end of the last tick (`trail_key`).
    torpedoes: HashMap<[u32; 3], Torpedo>,
    /// Sinking hulls, by unit id: when each last sent a ring out.
    sinking: HashMap<u32, f32>,
    /// Hulls whose guns charge before firing, as of the last tick (`note_gun_hulls`).
    pub(super) guns: Vec<GunHull>,
}

impl WaterFx {
    pub(super) fn new(buffer: Buffer, set: vk::DescriptorSet) -> WaterFx {
        buffer.write(0, &vec![0u8; buffer.size as usize]);
        WaterFx {
            buffer,
            set,
            ripples: Vec::new(),
            hulls: HashMap::new(),
            torpedoes: HashMap::new(),
            sinking: HashMap::new(),
            guns: Vec::new(),
        }
    }
}

fn heading_dir(heading: f32) -> Vec3 {
    Vec3::new(heading.cos(), heading.sin(), 0.0)
}

fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    let d = (b - a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    a + d * t
}

impl Renderer {
    pub(super) fn sea_level(&self) -> f32 {
        self.map_info.water_level.to_f32()
    }

    /// The water level, when `at` is on open water (or under it): the seabed
    /// there lies below the surface, and `at` is less than `above` over it.
    pub(super) fn at_sea(&self, at: Vec3, above: f32) -> Option<f32> {
        let water = self.sea_level();
        (at.z < water + above && self.ground_height(at.truncate()) < water - 0.3).then_some(water)
    }

    /// A spent casing dropping into the sea on its arc (puffs.wgsl `casing_flight`) from
    /// `from` at `vel`, thrown at `start`: a pin of white water, a droplet or two and, for
    /// every other one, a small ring. The shader loses the casing itself at the surface.
    pub(super) fn casing_splash(&mut self, from: Vec3, vel: Vec3, start: f32, life: f32, ring: bool) {
        let water = self.sea_level();
        let arc = |t: f32| {
            from + vel * ((1.0 - (-super::CASING_DRAG * t).exp()) / super::CASING_DRAG)
                - Vec3::Z * (super::CASING_FALL * t * t)
        };
        let Some(t) = (1..=(life / 0.02) as u32).map(|i| i as f32 * 0.02).find(|&t| arc(t).z <= water) else {
            return;
        };
        let hit = arc(t);
        if self.at_sea(hit, 1.0).is_none() {
            return;
        }
        let (surface, when) = (Vec3::new(hit.x, hit.y, water), start + t);
        self.push_puff(PUFF_COLUMN, surface, Vec3::Z * 2.4, when, 0.5, (0.05, 0.1));
        for _ in 0..2 {
            let throw = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 0.6
                + Vec3::Z * (1.4 + self.scatter.unit());
            self.push_puff(PUFF_DROPLET, surface + Vec3::Z * 0.05, throw, when, 0.6, (0.05, 0.08));
        }
        if ring {
            self.push_ripple(surface, when, 0.45, 0.9, 0.0, 0.2);
        }
    }

    /// True for a point under the water: fire and smoke never start there.
    pub(super) fn under_sea(&self, at: Vec3) -> bool {
        at.z < self.sea_level() - 0.05
    }

    /// A ring spreading on the water from `at` (xy; z is where its flash is),
    /// `size` metres at its strongest. `flash` lights the water around it for a
    /// moment (negative: an energy blast's blue); `foam` whitens its middle.
    pub(super) fn push_ripple(&mut self, at: Vec3, start: f32, size: f32, life: f32, flash: f32, foam: f32) {
        let fx = &mut self.water_fx;
        if fx.ripples.len() >= KEPT_RIPPLES {
            // The one closest to its end goes.
            if let Some((i, _)) = fx
                .ripples
                .iter()
                .enumerate()
                .min_by(|a, b| (a.1.start + a.1.params[1]).total_cmp(&(b.1.start + b.1.params[1])))
            {
                fx.ripples.swap_remove(i);
            }
        }
        fx.ripples.push(GpuRipple {
            pos: at.to_array(),
            start,
            params: [size, life, flash, foam],
        });
    }

    pub(super) fn push_bubbles(&mut self, at: Vec3, spread: f32, count: usize, start: f32, stagger: f32, size: f32) {
        let water = self.sea_level();
        for _ in 0..count {
            let p = at
                + Vec3::new(self.scatter.signed(), self.scatter.signed(), self.scatter.signed() * 0.5) * spread;
            if p.z > water - 0.3 {
                continue;
            }
            let rise = BUBBLE_RISE * (0.6 + 0.8 * self.scatter.unit()) * (0.7 + size.min(1.0) * 0.5);
            let drift = Vec3::new(self.scatter.signed(), self.scatter.signed(), rise);
            let life = ((water - p.z) / rise + 0.3).min(14.0);
            let s = size * (0.5 + self.scatter.unit());
            let when = start + self.scatter.unit() * stagger;
            self.push_puff(PUFF_BUBBLE, p, drift, when, life, (s * 0.7, s * 1.3));
        }
    }

    /// Water thrown up where something struck the surface at `at`: `scale` is
    /// about a metre for a bullet, three for a shell, ten for a torpedo. `tall`
    /// scales how high the column goes (a blast beside a hull throws it lower and wider).
    pub(super) fn water_splash(&mut self, at: Vec3, start: f32, scale: f32, tall: f32) {
        let s = scale.max(0.3);
        // The column: a thin spout for a bullet, a bushy tower of white water for a
        // shell, with a lower, wider sheath of thinner water round its foot.
        let spout = s < 1.2;
        let height = if spout { 0.7 + s * 1.3 } else { (3.0 + s * 2.4) * tall };
        let width = if spout { 0.18 + s * 0.3 } else { (0.9 + s * 0.7) * (2.0 - tall).max(1.0) };
        let v0 = (2.0 * 11.0 * height).sqrt();
        self.push_puff(PUFF_COLUMN, at, Vec3::Z * v0, start, 2.0 * v0 / 11.0 + 0.6, (width, width * 2.0));
        if !spout {
            let low = (2.0 * 11.0 * height * 0.5).sqrt();
            self.push_puff(
                PUFF_COLUMN,
                at,
                Vec3::Z * low,
                start + 0.03,
                2.0 * low / 11.0 + 0.8,
                (width * 1.8, width * 3.4),
            );
        }
        // Droplets thrown up with it, and a low crown of sheets out round its foot.
        let up = (4.0 + s * 5.0).min(34.0) as usize;
        for _ in 0..up {
            let vel = self.scatter.upward(0.62) * (2.5 + s * 2.4) * (0.55 + 0.8 * self.scatter.unit());
            let size = 0.14 + s * 0.08 * (0.6 + 0.8 * self.scatter.unit());
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * width * 0.4;
            self.push_puff(PUFF_DROPLET, at + off + Vec3::Z * 0.2, vel, start, 4.0, (size, size * 1.8));
        }
        let crown = (4.0 + s * 2.5).min(18.0) as usize;
        for i in 0..crown {
            let a = (i as f32 + self.scatter.unit()) * std::f32::consts::TAU / crown as f32;
            let out = Vec3::new(a.cos(), a.sin(), 0.0);
            let vel = out * (1.5 + s * 1.1) * (0.7 + 0.6 * self.scatter.unit()) + Vec3::Z * (2.0 + s * 1.6);
            let size = 0.16 + s * 0.1;
            self.push_puff(PUFF_DROPLET, at + out * width * 0.5 + Vec3::Z * 0.1, vel, start, 3.0, (size, size * 2.0));
        }
        // Spray that hangs where it was thrown and drifts off.
        let mist = (1.0 + s * 0.7).min(8.0) as usize;
        for i in 0..mist {
            let vel = self.scatter.upward(0.25) * (1.2 + s * 0.5);
            let h = height * (0.3 + 0.6 * self.scatter.unit());
            let life = 1.6 + s * 0.3 + self.scatter.unit();
            self.push_puff(
                PUFF_SPRAY,
                at + Vec3::Z * h,
                vel,
                start + 0.12 + i as f32 * 0.05,
                life,
                (0.5 + s * 0.35, 1.6 + s * 1.2),
            );
        }
        // The ring, with foam where the water came down.
        self.push_ripple(at, start, 1.4 + s * 1.3, 3.5 + s * 0.5, 0.0, (0.35 + s * 0.13).min(1.0));
    }

    /// Handles what the sea does about `event`. True when that is the whole of
    /// it and the land version (earth, dust, scorch) must not follow.
    pub(super) fn sea_effects_of(&mut self, event: &SimEvent, time: f32) -> bool {
        let blueprints = self.blueprints.clone();
        match event {
            SimEvent::ShotFired { pos, vel, travel, blueprint, weapon, .. } => {
                let unit = blueprints.unit(*blueprint);
                let w = &unit.weapons[*weapon as usize];
                if !w.torpedo {
                    // A warship's main battery stamps the sea under its muzzle; the gun's
                    // own flash and its wave in the air are the land effect's, after this.
                    if w.shockwave >= 1.5 && unit.motion.is_some_and(|m| m.layer == MoveLayer::Naval) {
                        let at = Vec3::from(pos.to_f32()) - Vec3::from(travel.to_f32());
                        let dir = Vec3::from(vel.to_f32()).normalize_or_zero();
                        self.battery_salvo(at, dir, w, time);
                    }
                    return false;
                }
                let at = Vec3::from(pos.to_f32());
                let dir = Vec3::from(vel.to_f32()).normalize_or_zero();
                // Dropped from an aircraft: nothing to see until it goes in (`torpedo_trails`).
                if at.z <= self.sea_level() {
                    self.torpedo_launch(at, dir, time);
                }
                true
            }
            SimEvent::Impact { pos, splash, color, after, on_unit, on_shield, blueprint, weapon, .. } => {
                if *on_shield {
                    return false;
                }
                let w = &blueprints.unit(*blueprint).weapons[*weapon as usize];
                let at = Vec3::from(pos.to_f32());
                let start = if w.hitscan { time } else { time + after.to_f32() * self.tick_seconds };
                let power = w.damage.to_f32().max(1.0).sqrt();
                let splash = splash.to_f32();
                if w.torpedo {
                    self.torpedo_hit(at, start, power, w.impact);
                    return true;
                }
                let Some(water) = self.at_sea(at, if *on_unit { 2.5 } else { 1.0 }) else {
                    return false;
                };
                let surface = Vec3::new(at.x, at.y, water);
                let scale = (0.55 + power * 0.09 + splash * 0.14) * w.impact.clamp(0.6, 2.0);
                let blue = *color == mc_data::WeaponColor::Blue;
                let shatter = super::is_shatter_gun(w);
                if *on_unit || w.hitscan || shatter {
                    // The hull (or the land effect's own bookkeeping) takes the hit: the water
                    // only gets what came off it at the waterline.
                    self.water_splash(surface, start, scale * 0.55, 0.7);
                    return false;
                }
                self.shell_in_water(surface, start, scale, power, splash, blue, w.shockwave);
                true
            }
            SimEvent::UnitDied { pos, blueprint, airborne: false, .. }
            | SimEvent::AircraftCrashed { pos, blueprint } => {
                let at = Vec3::from(pos.to_f32());
                let bp = blueprints.unit(*blueprint);
                let Some(water) = self.at_sea(at, 1.5) else {
                    return false;
                };
                let (r, h) = (bp.radius.to_f32(), bp.height.to_f32());
                if bp.has(mc_data::cat::COMMANDER) {
                    // The reactor is the event; the sea only answers it.
                    self.water_splash(Vec3::new(at.x, at.y, water), time + 0.05, 8.0, 1.0);
                    self.push_ripple(Vec3::new(at.x, at.y, water + 6.0), time, 60.0, 14.0, 3.0, 1.0);
                    return false;
                }
                if matches!(event, SimEvent::AircraftCrashed { .. }) {
                    self.aircraft_ditch(Vec3::new(at.x, at.y, water), r, time);
                    return true;
                }
                if at.z < water - 0.8 {
                    self.dived_death(at, r, time);
                } else {
                    self.ship_death(Vec3::new(at.x, at.y, water), r, h, time);
                }
                true
            }
            SimEvent::ShipSettled { pos, blueprint } => {
                let r = blueprints.unit(*blueprint).radius.to_f32();
                self.ship_settled(Vec3::from(pos.to_f32()), r, time);
                true
            }
            SimEvent::DivedLaunch { pos, blueprint, weapon } => {
                let w = &blueprints.unit(*blueprint).weapons[*weapon as usize];
                self.dived_launch(Vec3::from(pos.to_f32()), w, time);
                true
            }
            SimEvent::TorpedoIntercepted { pos } => {
                self.torpedo_intercepted(Vec3::from(pos.to_f32()), time);
                true
            }
            _ => false,
        }
    }

    /// A main battery firing over the water (`Weapon::shockwave` of 1.5 and up on a naval
    /// hull): the muzzle blast stamps a pressure ring on the sea under the gun, lit by the
    /// flash, lifts a low sheet of white water where it presses down, and throws spray out
    /// from the hull along the barrel. `muzzle` is the gun as it is drawn.
    fn battery_salvo(&mut self, muzzle: Vec3, dir: Vec3, w: &mc_data::Weapon, time: f32) {
        let Some(water) = self.at_sea(muzzle, 60.0) else {
            return;
        };
        let power = w.damage.to_f32().max(1.0).sqrt();
        let blue = w.color == mc_data::WeaponColor::Blue;
        let h = Vec3::new(dir.x, dir.y, 0.0).normalize_or(Vec3::X);
        let side = Vec3::new(-h.y, h.x, 0.0);
        // About six and a half for the Leviathan's batteries.
        let s = (0.6 + power * 0.06) * w.shockwave;
        // Under the muzzle and a little ahead of it, where the blast meets the sea.
        let foot = Vec3::new(muzzle.x, muzzle.y, water) + h * (2.0 + s * 0.6);
        // The pressure ring, and the flash lighting the water from the gun's height.
        let flash = (1.2 + s * 0.35) * if blue { -1.0 } else { 1.0 };
        let lit = Vec3::new(foot.x, foot.y, water + (muzzle.z - water) * 0.5);
        self.push_ripple(lit, time, 4.0 + s * 3.2, 0.7, flash, 0.0);
        self.push_ripple(foot, time + 0.02, 5.0 + s * 3.6, 4.5 + s * 0.3, 0.0, 0.85);
        let tint = if blue { 0.0 } else { 1.0 };
        self.push_effect((foot + Vec3::Z * 1.2).to_array(), time, 2.0 + s * 0.9, 0.11, tint, 0.0);
        // A low sheet of white water lifted where the blast presses down.
        let lift = 1.5 + s * 0.35;
        let v0 = (2.0 * 11.0 * lift).sqrt();
        self.push_puff(PUFF_COLUMN, foot, Vec3::Z * v0, time + 0.03, 2.0 * v0 / 11.0 + 0.5, (1.5 + s * 0.5, 3.0 + s * 1.4));
        // The spray sheet: out along the barrel and fanned to either side, low and fast.
        let sheets = (4.0 + s * 1.2) as usize;
        for i in 0..sheets {
            let a = self.scatter.signed() * 0.6;
            let out = h * a.cos() + side * a.sin();
            let speed = (8.0 + s * 2.2) * (0.6 + 0.6 * self.scatter.unit());
            let from = foot + out * (1.0 + self.scatter.unit() * 2.0) + Vec3::Z * (0.4 + self.scatter.unit() * 0.8);
            let vel = out * speed + Vec3::Z * (1.0 + s * 0.25 * self.scatter.unit());
            let life = 1.1 + s * 0.08 + self.scatter.unit() * 0.5;
            let when = time + 0.02 + i as f32 * 0.012;
            self.push_puff(PUFF_SPRAY, from, vel, when, life, (1.0 + s * 0.3, 3.0 + s * 1.1));
        }
        let drops = (10.0 + s * 3.0).min(40.0) as usize;
        for _ in 0..drops {
            let a = self.scatter.signed() * 0.8;
            let out = h * a.cos() + side * a.sin();
            let speed = (6.0 + s * 2.0) * (0.5 + 0.8 * self.scatter.unit());
            let vel = out * speed + Vec3::Z * (2.0 + s * 0.5 * self.scatter.unit());
            let size = 0.15 + s * 0.04 * (0.6 + 0.8 * self.scatter.unit());
            let from = foot + side * self.scatter.signed() * (1.0 + s * 0.4) + Vec3::Z * 0.2;
            self.push_puff(PUFF_DROPLET, from, vel, time + 0.02, 2.5, (size, size * 2.0));
        }
        // Mist left hanging over the water where the sheet went out, drifting on.
        for i in 0..3 {
            let from = foot + h * (3.0 + s * 1.2 * i as f32) + Vec3::Z * (1.0 + s * 0.2);
            let when = time + 0.25 + i as f32 * 0.1;
            self.push_puff(PUFF_SPRAY, from, h * 2.0 + Vec3::Z * 0.6, when, 1.8 + s * 0.1, (1.5 + s * 0.4, 4.0 + s * 1.2));
        }
    }

    /// A missile leaving a dived hull at `at`, under the water: the ejection charge's
    /// flash seen through the sea, a rush of air up from the hatch, then the surface
    /// boiling and thrown up as the missile breaches. Its motor lights in the open later
    /// (`MissileIgnited`).
    fn dived_launch(&mut self, at: Vec3, w: &mc_data::Weapon, time: f32) {
        let water = self.sea_level();
        let depth = (water - at.z).max(0.5);
        let surface = Vec3::new(at.x, at.y, water);
        // About two and a half for the Kraken's missiles.
        let s = (0.8 + w.damage.to_f32().max(1.0).sqrt() * 0.03).min(3.0);
        self.push_ripple(at, time, 5.0 + s * 2.0, 2.0, 2.0 + s, 0.0);
        self.push_bubbles(at + Vec3::Z * 0.5, 1.2 + s * 0.4, (14.0 + s * 6.0) as usize, time, 0.35, 0.5);
        // The boil, then the breach: it comes up at a few metres a second.
        let up = time + depth / 9.0;
        self.push_ripple(surface, up - 0.15, 4.0 + s * 2.0, 3.0, 0.0, 0.7);
        self.water_splash(surface, up, 1.6 + s * 0.9, 1.4);
        // A column of spray standing where it came through, blown out as the motor takes over.
        let tall = 4.0 + s * 2.5;
        let v0 = (2.0 * 11.0 * tall).sqrt();
        self.push_puff(PUFF_COLUMN, surface, Vec3::Z * v0, up + 0.02, 2.0 * v0 / 11.0 + 0.6, (0.8 + s * 0.3, 2.0 + s * 0.8));
        for i in 0..5 {
            let vel = Vec3::new(self.scatter.signed(), self.scatter.signed(), 3.0 + self.scatter.unit() * 3.0);
            let from = surface + Vec3::Z * (1.0 + i as f32 * 1.2);
            self.push_puff(PUFF_SPRAY, from, vel, up + 0.1 + i as f32 * 0.06, 1.8, (0.8 + s * 0.3, 2.4 + s * 0.9));
        }
        self.push_ripple(surface, up + 0.1, 6.0 + s * 3.0, 5.0, 0.0, 0.9);
    }

    /// An interceptor meeting a torpedo under the water: a flash seen through the sea, a
    /// burst of air, and a small ring and spout where it comes up. A tenth of a torpedo hit.
    fn torpedo_intercepted(&mut self, at: Vec3, time: f32) {
        let water = self.sea_level();
        let under = Vec3::new(at.x, at.y, at.z.min(water - 1.0));
        let surface = Vec3::new(at.x, at.y, water);
        self.push_ripple(under, time, 5.0, 2.5, 3.0, 0.0);
        self.push_bubbles(under, 1.6, 22, time, 0.4, 0.55);
        let up = time + (water - under.z).max(0.0) / (BUBBLE_RISE * 1.5);
        self.push_ripple(surface, up, 4.5, 3.5, 0.0, 0.7);
        let v0 = (2.0f32 * 11.0 * 3.0).sqrt();
        self.push_puff(PUFF_COLUMN, surface, Vec3::Z * v0, up, 2.0 * v0 / 11.0 + 0.5, (0.9, 2.0));
        for _ in 0..10 {
            let vel = self.scatter.upward(0.6) * (3.0 + self.scatter.unit() * 4.0);
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 1.2;
            self.push_puff(PUFF_DROPLET, surface + off + Vec3::Z * 0.2, vel, up, 2.0, (0.2, 0.45));
        }
        self.push_puff(PUFF_SPRAY, surface + Vec3::Z * 1.0, Vec3::Z * 0.8, up + 0.08, 1.6, (0.8, 2.6));
    }

    /// Once a tick: where the hulls whose guns charge before they fire are and how their
    /// houses are turned, for the charging glow (`Renderer::weapon_charging`).
    pub(super) fn note_gun_hulls(&mut self, units: &[UnitInstance], houses: &[HousePose]) {
        let blueprints = self.blueprints.clone();
        let guns = &mut self.water_fx.guns;
        guns.clear();
        for u in units {
            if u.owner_flags & (KIND_WRECK | KIND_GHOST | KIND_PROP) != 0 {
                continue;
            }
            let bp = blueprints.unit(BlueprintId(u.blueprint as u16));
            if !bp.weapons.iter().any(|w| w.charge_ticks > 0) {
                continue;
            }
            let house = (u._pad3[1] >> UNIT_HOUSE_SHIFT)
                .checked_sub(1)
                .and_then(|i| houses.get(i as usize).copied());
            guns.push(GunHull { blueprint: u.blueprint, pos: Vec3::from(u.pos), heading: u.heading, house });
        }
    }

    /// A shell or blast landing in open water: a splash and rings instead of
    /// earth and scorch, a short orange flash on the surface for a charge that
    /// goes off, and the flash caught by the water round it.
    #[allow(clippy::too_many_arguments)]
    fn shell_in_water(
        &mut self,
        at: Vec3,
        start: f32,
        scale: f32,
        power: f32,
        splash: f32,
        blue: bool,
        shockwave: f32,
    ) {
        let tint = if blue { 0.0 } else { 1.0 };
        let bursts = splash > 0.0 || power > 6.0;
        if bursts {
            let core = (1.0 + power * 0.2 + splash * 0.12) * scale.min(3.0) * 0.5;
            self.push_effect((at + Vec3::Z * 0.7).to_array(), start, core, 0.1, tint, 0.0);
            // Lit from a metre over the water, so the glint shows on the rings round it.
            let flash = (0.6 + power * 0.09 + splash * 0.12) * if blue { -1.0 } else { 1.0 };
            self.push_ripple(at + Vec3::Z * 1.2, start, 1.4 + scale * 1.3, 0.8, flash, 0.0);
            // The fire is out as soon as it lit; what hangs after is smoke and spray.
            self.push_puff(
                PUFF_FIRE,
                at + Vec3::Z * 0.5,
                Vec3::Z * 3.0,
                start,
                0.22,
                (0.4 + splash * 0.08, 0.9 + splash * 0.15),
            );
            for i in 0..1 + (splash > 2.0) as usize {
                let vel = self.scatter.upward(0.6) * (1.5 + splash * 0.2);
                self.push_puff(
                    PUFF_SMOKE,
                    at + Vec3::Z * (1.0 + i as f32),
                    vel,
                    start + 0.05,
                    1.2 + splash * 0.05,
                    (0.5 + power * 0.04, 1.4 + power * 0.1 + splash * 0.15),
                );
            }
        }
        if shockwave > 0.0 {
            self.push_shockwave(
                at.to_array(),
                start,
                (14.0 + power * 1.6 + splash * 1.5) * shockwave,
                0.5,
                shockwave.min(1.0) * 0.6,
                tint,
                Vec3::ZERO,
            );
        }
        self.water_splash(at, start, scale, 1.0);
    }

    /// Tubes firing under the water: a burp of air rising from the muzzle and a
    /// little white water where it breaks the surface.
    fn torpedo_launch(&mut self, at: Vec3, dir: Vec3, time: f32) {
        let water = self.sea_level();
        self.push_bubbles(at + dir * 1.5, 0.9, 16, time, 0.25, 0.35);
        let depth = (water - at.z).max(0.0);
        let surface = Vec3::new(at.x + dir.x * 2.0, at.y + dir.y * 2.0, water);
        let up = time + depth / (BUBBLE_RISE * 1.2);
        self.push_ripple(surface, up, 2.2, 3.5, 0.0, 0.55);
        for _ in 0..4 {
            let vel = self.scatter.upward(0.7) * (1.5 + self.scatter.unit() * 2.0);
            self.push_puff(PUFF_DROPLET, surface + Vec3::Z * 0.1, vel, up, 1.5, (0.12, 0.22));
        }
        self.push_puff(PUFF_SPRAY, surface + Vec3::Z * 0.4, Vec3::Z * 0.6, up, 1.4, (0.5, 1.6));
    }

    /// A torpedo going off against a hull under the waterline: a flash seen
    /// through the water, then the water above it thrown up as a tall white
    /// column with a skirt of spray, bubbles boiling up, rings and foam. The
    /// fire is small; the sea takes most of it.
    fn torpedo_hit(&mut self, at: Vec3, start: f32, power: f32, impact: f32) {
        let water = self.sea_level();
        let surface = Vec3::new(at.x, at.y, water);
        let s = (power * 0.55 * impact.clamp(0.8, 2.5)).clamp(6.0, 16.0);
        // Under the water: the flash, lighting the surface from below.
        let under = Vec3::new(at.x, at.y, at.z.min(water - 1.5));
        self.push_ripple(under, start, s * 1.3, 9.0 + s * 0.3, 5.0 + s * 0.25, 1.0);
        self.push_bubbles(under, s * 0.3, 30, start, 0.6, 0.6);
        // The column: one tall jet and two shoulders beside it, a beat later.
        let tall = 12.0 + s * 1.5;
        for (k, (off, h, w)) in [(0.0, tall, s * 0.7), (0.35, tall * 0.62, s * 0.55), (0.35, tall * 0.5, s * 0.5)]
            .into_iter()
            .enumerate()
        {
            let a = self.scatter.unit() * std::f32::consts::TAU;
            let foot = surface + Vec3::new(a.cos(), a.sin(), 0.0) * off * s;
            let v0 = (2.0 * 11.0 * h).sqrt();
            self.push_puff(
                PUFF_COLUMN,
                foot,
                Vec3::Z * v0,
                start + 0.03 + k as f32 * 0.07,
                2.0 * v0 / 11.0 + 0.9,
                (w, w * 2.4),
            );
        }
        // Water thrown high out of it, and falling back all round.
        for _ in 0..40 {
            let vel = self.scatter.upward(0.72) * (8.0 + s * 1.2) * (0.5 + 0.7 * self.scatter.unit());
            let size = 0.35 + s * 0.06 * (0.5 + self.scatter.unit());
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * s * 0.3;
            self.push_puff(PUFF_DROPLET, surface + off + Vec3::Z * 0.5, vel, start + 0.04, 6.0, (size, size * 2.2));
        }
        // The skirt: spray rolling out low over the water from the foot.
        for i in 0..18 {
            let a = (i as f32 + self.scatter.unit()) * std::f32::consts::TAU / 18.0;
            let out = Vec3::new(a.cos(), a.sin(), 0.0);
            let (when, life) = (start + 0.08 + self.scatter.unit() * 0.1, 2.6 + self.scatter.unit() * 1.2);
            self.push_puff(
                PUFF_SPRAY,
                surface + out * s * 0.35 + Vec3::Z * 1.2,
                out * (6.0 + s * 0.6) + Vec3::Z * 1.0,
                when,
                life,
                (s * 0.35, s * 1.1),
            );
        }
        // Mist hanging where the column was, drifting off as it falls.
        for i in 0..7 {
            let h = tall * (0.35 + 0.6 * self.scatter.unit());
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * s * 0.3;
            let drift = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.5) * 1.5;
            let life = 3.4 + self.scatter.unit() * 1.2;
            self.push_puff(
                PUFF_SPRAY,
                surface + off + Vec3::Z * h,
                drift,
                start + 0.5 + i as f32 * 0.12,
                life,
                (s * 0.4, s * 1.4),
            );
        }
        // The blast venting through the surface: brief, orange, low.
        self.push_effect((surface + Vec3::Z * 1.5).to_array(), start + 0.02, s * 0.45, 0.12, 1.0, 0.0);
        for i in 0..3 {
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * s * 0.2;
            let rise = Vec3::Z * (3.0 + self.scatter.unit() * 3.0);
            self.push_puff(
                PUFF_FIRE,
                surface + off + Vec3::Z * 1.0,
                rise,
                start + 0.03 * i as f32,
                0.45,
                (s * 0.12, s * 0.3),
            );
        }
        for i in 0..4 {
            let vel = Vec3::new(self.scatter.signed() * 0.8 + 0.8, self.scatter.signed() * 0.8, 2.4);
            self.push_puff(
                PUFF_SMOKE,
                surface + Vec3::Z * (2.0 + i as f32 * 0.8),
                vel,
                start + 0.35 + i as f32 * 0.25,
                3.0,
                (s * 0.15, s * 0.6),
            );
        }
    }

    /// A ship going up: the land death made smaller (no dust ring, no clods, no
    /// blast on the ground), the rest of the blast thrown into the sea round it.
    fn ship_death(&mut self, at: Vec3, r: f32, h: f32, time: f32) {
        let core = at + Vec3::Z * h * 0.45;
        self.push_effect(core.to_array(), time, r * 1.5, 0.2, 1.0, 0.0);
        self.push_effect(core.to_array(), time, r * 3.2, 0.7, 2.0, 0.25);
        self.push_shockwave(at.to_array(), time, 18.0 + r * 5.0, 0.6, 0.7, 1.0, Vec3::ZERO);
        // Secondary blasts along the hull.
        for i in 0..3 {
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed() * 0.35, self.scatter.unit() * 0.5) * r * 0.8;
            let delay = 0.08 + 0.14 * i as f32 + self.scatter.unit() * 0.05;
            let size = r * (1.4 + self.scatter.unit());
            self.push_effect((core + off).to_array(), time + delay, size, 0.5, 2.0, 0.35);
            self.push_ripple(at + off.truncate().extend(h * 0.3), time + delay, r * 0.5, 4.0, 1.2, 0.5);
        }
        if r > 30.0 {
            self.capital_ship_death(at, r, h, time);
        }
        // Burning fragments; they die where they meet the water.
        for _ in 0..30 {
            let vel = self.scatter.upward(0.15) * (12.0 + self.scatter.unit() * 24.0) * (0.85 + r * 0.05);
            let life = 0.5 + self.scatter.unit() * 1.1;
            self.push_puff(PUFF_SPARK, core, vel, time, life, (0.22 + r * 0.035, 0.05));
        }
        self.push_puff(PUFF_FIREBALL, core, Vec3::Z * 4.0, time, 1.0, (r * 0.5, r * 1.3));
        for i in 0..8 {
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed() * 0.4, self.scatter.unit()) * r * 0.5;
            let vel = self.scatter.upward(0.5) * (3.0 + self.scatter.unit() * 5.0);
            let life = 1.0 + self.scatter.unit() * 0.6;
            self.push_puff(
                PUFF_FIRE,
                core + off,
                vel,
                time + 0.03 * i as f32,
                life,
                (r * 0.45, r * 1.2),
            );
        }
        for i in 0..10 {
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed() * 0.4, 0.0) * r * 0.45 + Vec3::Z * h * 0.5;
            let vel = Vec3::new(self.scatter.signed() * 0.9 + 0.9, self.scatter.signed() * 0.9, 3.2 + self.scatter.unit() * 2.0);
            self.push_puff(PUFF_SMOKE, at + off, vel, time + 0.15 + i as f32 * 0.25, 3.6, (r * 0.4, r * 1.8));
        }
        // The sea: water blown up along both sides and round the ends, a skirt of spray, rings.
        for k in 0..4 {
            let a = (k as f32 + self.scatter.signed() * 0.3) * std::f32::consts::FRAC_PI_2;
            let foot = at + Vec3::new(a.cos(), a.sin() * 0.45, 0.0) * r * 0.75;
            self.water_splash(foot, time + 0.02 + k as f32 * 0.05, 2.0 + r * 0.15, 0.55);
        }
        for i in 0..20 {
            let a = (i as f32 + self.scatter.unit()) * std::f32::consts::TAU / 20.0;
            let out = Vec3::new(a.cos(), a.sin(), 0.0);
            let life = 2.4 + self.scatter.unit();
            self.push_puff(
                PUFF_SPRAY,
                at + out * r * 0.7 + Vec3::Z * 1.0,
                out * (8.0 + r * 0.6) + Vec3::Z * 0.8,
                time + 0.05,
                life,
                (r * 0.25, r * 0.8),
            );
        }
        self.push_ripple(at + Vec3::Z * h * 0.45, time, r * 1.3, 9.0, 3.5, 1.0);
    }

    /// The extra beats of a capital ship going up (a hull over 30 m): its magazines go
    /// off along the hull after the first blast, each throwing the sea up beside it,
    /// and the middle stands up in a tall column over a wide ring. The event carries no
    /// heading, so the hull's line is taken at random.
    fn capital_ship_death(&mut self, at: Vec3, r: f32, h: f32, time: f32) {
        let a = self.scatter.unit() * std::f32::consts::TAU;
        let axis = Vec3::new(a.cos(), a.sin(), 0.0);
        let perp = Vec3::new(-axis.y, axis.x, 0.0);
        for (k, delay) in [0.8f32, 1.6].into_iter().enumerate() {
            let sign = if k == 0 { 1.0 } else { -1.0 };
            let at2 = at + axis * r * (0.35 + 0.25 * k as f32) * sign;
            let core = at2 + Vec3::Z * h * 0.4;
            self.push_effect(core.to_array(), time + delay, r * 1.1, 0.16, 1.0, 0.0);
            self.push_effect(core.to_array(), time + delay, r * 2.2, 0.6, 2.0, 0.3);
            self.push_shockwave(at2.to_array(), time + delay, 20.0 + r * 3.0, 0.55, 0.6, 1.0, Vec3::ZERO);
            self.push_puff(PUFF_FIREBALL, core, Vec3::Z * 5.0, time + delay, 0.9, (r * 0.35, r * 0.9));
            for _ in 0..16 {
                let vel = self.scatter.upward(0.2) * (14.0 + self.scatter.unit() * 22.0) * (0.85 + r * 0.04);
                let life = 0.5 + self.scatter.unit() * 1.0;
                self.push_puff(PUFF_SPARK, core, vel, time + delay, life, (0.2 + r * 0.03, 0.05));
            }
            for i in 0..4 {
                let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.2 + Vec3::Z * h * 0.5;
                let vel = Vec3::new(self.scatter.signed() * 0.9 + 0.9, self.scatter.signed() * 0.9, 3.5 + self.scatter.unit() * 2.0);
                self.push_puff(PUFF_SMOKE, at2 + off, vel, time + delay + 0.1 + i as f32 * 0.2, 4.0, (r * 0.3, r * 1.5));
            }
            for s in [-1.0f32, 1.0] {
                let foot = at2 + perp * s * r * 0.3;
                self.water_splash(foot, time + delay + 0.05, 3.0 + r * 0.12, 0.8);
            }
            self.push_ripple(at2 + Vec3::Z * h * 0.4, time + delay, r * 0.9, 7.0, 2.5, 0.9);
        }
        // The middle: the sea over the keel thrown up after the first blast.
        self.water_splash(at, time + 0.35, 3.0 + r * 0.14, 1.6);
        self.push_ripple(at + Vec3::Z * h * 0.45, time + 0.3, r * 2.4, 12.0, 0.0, 1.0);
    }

    /// A dived boat breaking up: a flash under the water, a rush of air and a
    /// boil on the surface over it. Nothing burns down there.
    fn dived_death(&mut self, at: Vec3, r: f32, time: f32) {
        let water = self.sea_level();
        let surface = Vec3::new(at.x, at.y, water);
        let depth = (water - at.z).max(0.5);
        self.push_ripple(at, time, r * 1.4, 10.0, 5.5, 0.0);
        self.push_bubbles(at, r * 0.5, 50, time, 1.2, 0.7);
        let up = time + depth / (BUBBLE_RISE * 1.3);
        self.push_ripple(surface, up, r * 1.2, 8.0, 0.0, 1.0);
        let dome = 4.0 + r * 0.35;
        let v0 = (2.0 * 11.0 * dome).sqrt();
        self.push_puff(PUFF_COLUMN, surface, Vec3::Z * v0, up, 2.0 * v0 / 11.0 + 1.0, (r * 0.8, r * 1.8));
        for _ in 0..18 {
            let vel = self.scatter.upward(0.6) * (4.0 + self.scatter.unit() * 6.0);
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.4;
            self.push_puff(PUFF_DROPLET, surface + off, vel, up, 3.0, (0.3, 0.7));
        }
        for _ in 0..6 {
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.5;
            let vel = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.6) * 2.5;
            self.push_puff(PUFF_SPRAY, surface + off + Vec3::Z * 1.5, vel, up + 0.1, 2.8, (r * 0.25, r * 0.9));
        }
    }

    /// A dead aircraft going into the sea (aircraft_crash.rs): a tall burst of white
    /// water, a short flare of burning fuel snuffed out in steam, rings, and a trail
    /// of air coming up from it as it sinks nose down to the seabed at 3.5 m/s.
    fn aircraft_ditch(&mut self, at: Vec3, r: f32, time: f32) {
        let depth = (at.z - self.ground_height(at.truncate())).max(0.5);
        self.water_splash(at, time, 3.0 + r * 0.55, 1.35);
        for k in 0..3 {
            let a = (k as f32 + self.scatter.unit() * 0.6) * std::f32::consts::TAU / 3.0;
            let foot = at + Vec3::new(a.cos(), a.sin(), 0.0) * r * 0.6;
            self.water_splash(foot, time + 0.06 + k as f32 * 0.04, 1.6 + r * 0.2, 0.7);
        }
        let core = at + Vec3::Z * r * 0.3;
        self.push_effect(core.to_array(), time, r * 1.2, 0.18, 1.0, 0.0);
        self.push_shockwave(at.to_array(), time, 12.0 + r * 3.0, 0.5, 0.55, 1.0, Vec3::ZERO);
        self.push_puff(PUFF_FIREBALL, core, Vec3::Z * 3.0, time, 0.45, (r * 0.35, r * 0.8));
        for i in 0..6 {
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.4;
            let rise = Vec3::new(self.scatter.signed(), self.scatter.signed(), 2.4 + self.scatter.unit() * 1.6);
            self.push_puff(PUFF_STEAM, at + off + Vec3::Z * 0.6, rise, time + 0.1 + i as f32 * 0.12, 3.2, (r * 0.25, r * 1.1));
        }
        for _ in 0..16 {
            let vel = self.scatter.upward(0.25) * (8.0 + self.scatter.unit() * 14.0);
            self.push_puff(PUFF_SPARK, core, vel, time, 0.6, (0.2 + r * 0.02, 0.05));
        }
        self.push_ripple(at + Vec3::Z * r * 0.3, time, 8.0 + r * 1.6, 9.0, 2.0, 1.0);
        self.push_ripple(at, time + 0.35, 5.0 + r, 7.0, 0.0, 0.6);
        // The air it takes down with it comes up behind it all the way to the bottom.
        let steps = (depth / 6.0).ceil().clamp(1.0, 8.0) as usize;
        for k in 0..steps {
            let d = depth * (k as f32 + 0.5) / steps as f32;
            let when = time + 0.3 + d / 3.5;
            self.push_bubbles(at - Vec3::Z * d, r * 0.35, 12, when, 1.2, 0.55);
        }
        let bottom = time + 0.3 + depth / 3.5;
        let up = bottom + depth / (BUBBLE_RISE * 1.3);
        self.push_ripple(at, up, r * 0.7, 5.0, 0.0, 0.5);
    }

    /// A hull come to rest on the seabed: the air still in it goes up in a rush
    /// and boils the surface over it one last time.
    fn ship_settled(&mut self, at: Vec3, r: f32, time: f32) {
        let water = self.sea_level();
        let depth = (water - at.z).max(0.5);
        self.push_bubbles(at + Vec3::Z * 1.0, r * 0.45, 36, time, 1.5, 0.7);
        let surface = Vec3::new(at.x, at.y, water);
        let up = time + depth / (BUBBLE_RISE * 1.3);
        self.push_ripple(surface, up, r * 0.8, 6.0, 0.0, 0.7);
        for _ in 0..8 {
            let vel = self.scatter.upward(0.7) * (1.5 + self.scatter.unit() * 2.5);
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.3;
            let when = up + self.scatter.unit() * 0.8;
            self.push_puff(PUFF_DROPLET, surface + off, vel, when, 1.6, (0.2, 0.4));
        }
        for i in 0..3 {
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.3;
            self.push_puff(PUFF_SPRAY, surface + off + Vec3::Z * 0.5, Vec3::Z * 0.6, up + i as f32 * 0.3, 2.2, (r * 0.12, r * 0.45));
        }
    }

    /// Once a tick: follow the hulls on the water and the torpedoes in it for
    /// their wakes, throw spray off fast bows, trail bubbles behind torpedoes,
    /// and keep a sinking hull burning, steaming and bubbling as it goes down.
    pub(super) fn sea_tick(
        &mut self,
        units: &[UnitInstance],
        projectiles: &[ProjectileInstance],
        time: f32,
        camera: &Camera,
    ) {
        let blueprints = self.blueprints.clone();
        let water = self.sea_level();
        let tick = self.tick_seconds.max(0.02);
        let reach = camera.distance * 2.5 + 300.0;
        let focus = camera.focus.truncate();
        for hull in self.water_fx.hulls.values_mut() {
            hull.seen = false;
        }
        let hidden = KIND_WRECK
            | KIND_GHOST
            | KIND_PROP
            | STATE_RADAR
            | ((mc_sim::tables::flag::IN_FACTORY | mc_sim::tables::flag::UNDER_CONSTRUCTION) as u32) << 8;
        let mut sinking_now = Vec::new();
        for u in units {
            if u.owner_flags & KIND_WRECK != 0 && u._pad == WRECK_SINKING {
                sinking_now.push(*u);
                continue;
            }
            if u.owner_flags & hidden != 0 || u.build < 1.0 {
                continue;
            }
            let bp = blueprints.unit(BlueprintId(u.blueprint as u16));
            let Some(motion) = bp.motion else { continue };
            let (from, to) = (Vec3::from(u.prev_pos), Vec3::from(u.pos));
            let naval = motion.layer == MoveLayer::Naval;
            // A hover over the water throws a lighter wake of its own.
            let hover = motion.layer == MoveLayer::Hover && self.at_sea(to, 3.0).is_some();
            if !naval && !hover {
                continue;
            }
            let r = bp.radius.to_f32();
            let dive = u._pad3[0] & UNIT_DIVE_MASK;
            let dived = naval && (dive > 160 || to.z < water - 1.0);
            let raw_speed = (to - from).truncate().length() / tick;
            let (kind, strength) = if dived {
                (1.0, 0.2 * (1.0 - (water - to.z) / 12.0).clamp(0.0, 1.0))
            } else if hover {
                (0.0, 0.45)
            } else {
                (0.0, 1.0)
            };
            // A big hull is fuller in the beam and shoulders more water aside: its white
            // water is wider for its length.
            let big = ((r - 20.0) / 40.0).clamp(0.0, 1.0);
            let (half_length, half_beam) =
                if hover { (r * 0.8, r * 0.6) } else { (r * 0.95, r * (0.22 + 0.06 * big)) };
            let track = self.water_fx.hulls.entry(u.unit_id).or_insert_with(|| Hull {
                prev: from,
                pos: to,
                prev_heading: u.prev_heading,
                heading: u.heading,
                prev_speed: raw_speed,
                speed: raw_speed,
                half_length,
                half_beam,
                kind,
                strength,
                trail: Vec::new(),
                bow_foam: f32::MIN,
                seen: true,
            });
            track.prev = from;
            track.pos = to;
            track.prev_heading = u.prev_heading;
            track.heading = u.heading;
            track.prev_speed = track.speed;
            track.speed += (raw_speed - track.speed) * (1.0 - (-tick / WAKE_EASE).exp());
            let speed = track.speed;
            track.kind = kind;
            track.strength = strength;
            track.half_length = half_length;
            track.half_beam = half_beam;
            track.seen = true;
            track.trail.retain(|p| time - p[3] < WAKE_LIFE);
            // Laid where the stern is drawn now (the tick's start, `alpha` 0), with the
            // speed it is drawn with, so a new point lands on the live stern's own.
            let stern = from - heading_dir(u.prev_heading) * half_length;
            let due = track.trail.last().is_none_or(|p| {
                time - p[3] >= WAKE_STEP && Vec2::new(p[0], p[1]).distance(stern.truncate()) >= WAKE_MOVED
            });
            if due {
                track.trail.push([stern.x, stern.y, track.prev_speed, time]);
                if track.trail.len() > WAKE_POINTS - 2 {
                    track.trail.remove(0);
                }
            }
            // A big hull's standing bow wave: foam laid at the stem every half second.
            let bow_due = time - track.bow_foam >= 0.5;
            if bow_due {
                track.bow_foam = time;
            }
            // Spray off the bow of anything quick, and the fast boat's rooster tail. A
            // big hull shoulders the sea aside at a walking pace: its bow wave stands up
            // well below the speed a boat needs to throw spray.
            let brisk = 10.0 * (1.0 - 0.6 * big);
            if dived || speed < brisk || to.truncate().distance(focus) > reach {
                continue;
            }
            let fwd = heading_dir(u.heading);
            let right = Vec3::new(fwd.y, -fwd.x, 0.0);
            let quick = ((speed - brisk) / 30.0).clamp(0.0, 1.0);
            if r > 30.0 && !hover && bow_due {
                // The standing wave: a foam ring held ahead of the stem, taller with speed,
                // and white water peeling off both shoulders of the bow.
                let stand = (speed / 15.0).clamp(0.3, 1.2);
                let stem = to + fwd * (half_length + 1.0);
                let stem = Vec3::new(stem.x, stem.y, water);
                self.push_ripple(stem, time, half_beam * (0.9 + 0.7 * stand), 1.3, 0.0, 0.55 * stand);
                for side in [-1.0f32, 1.0] {
                    let shoulder = to + fwd * half_length * 0.7 + right * side * half_beam * 0.9;
                    let shoulder = Vec3::new(shoulder.x, shoulder.y, water + 0.3);
                    let throw = right * side * (1.5 + speed * 0.15) + fwd * speed * 0.3 + Vec3::Z * (1.0 + stand * 1.5);
                    let s = half_beam * 0.25 * stand;
                    self.push_puff(PUFF_SPRAY, shoulder, throw * 0.5, time, 0.6, (s, s * 2.6));
                }
            }
            for side in [-1.0f32, 1.0] {
                if self.scatter.unit() > 0.35 + 0.6 * quick {
                    continue;
                }
                let t = self.scatter.unit();
                let at = from.lerp(to, t) + fwd * half_length * 0.6 + right * side * half_beam * 1.1;
                let at = Vec3::new(at.x, at.y, water + 0.3);
                let start = time + t * tick;
                let throw = right * side * (2.0 + speed * 0.12) + fwd * speed * 0.35 + Vec3::Z * (1.5 + quick * 2.0);
                // Short and small: spray laid every tick and left behind would bead
                // the wake into a row of puffs. The white water is the shader's.
                if self.scatter.unit() < 0.3 * quick + 0.2 * big {
                    self.push_puff(PUFF_SPRAY, at, throw * 0.5, start, 0.45, (half_beam * 0.3, half_beam * 0.8));
                }
                for _ in 0..1 + (quick * 2.0 + big * 3.0) as usize {
                    let vel = throw + Vec3::new(self.scatter.signed(), self.scatter.signed(), self.scatter.unit()) * (1.5 + big * 1.5);
                    let size = 0.12 + half_beam * 0.05;
                    self.push_puff(PUFF_DROPLET, at, vel, start, 2.0 + big, (size, 0.3 + half_beam * 0.04));
                }
            }
            if quick > 0.3 && !hover {
                let t = self.scatter.unit();
                let stern = from.lerp(to, t) - fwd * half_length * 0.95;
                let stern = Vec3::new(stern.x, stern.y, water + 0.2);
                let start = time + t * tick;
                let tail = -fwd * speed * 0.18 + Vec3::Z * (3.0 + quick * 4.0);
                for _ in 0..2 {
                    let vel = tail + Vec3::new(self.scatter.signed(), self.scatter.signed(), self.scatter.unit()) * 1.8;
                    self.push_puff(PUFF_DROPLET, stern, vel, start, 2.2, (0.16, 0.4));
                }
            }
        }
        self.water_fx.hulls.retain(|_, h| h.seen);
        self.sinking_hulls(&sinking_now, time, camera);
        self.torpedo_trails(projectiles, time);
    }

    fn sinking_hulls(&mut self, hulls: &[UnitInstance], time: f32, camera: &Camera) {
        let blueprints = self.blueprints.clone();
        let water = self.sea_level();
        let tick = self.tick_seconds.max(0.02);
        let reach = camera.distance * 2.5 + 400.0;
        let alive: Vec<u32> = hulls.iter().map(|u| u.unit_id).collect();
        self.water_fx.sinking.retain(|id, _| alive.contains(id));
        for u in hulls {
            let (from, to) = (Vec3::from(u.prev_pos), Vec3::from(u.pos));
            if to.distance(camera.focus) > reach {
                continue;
            }
            let bp = blueprints.unit(BlueprintId(u.blueprint as u16));
            let (r, h) = (bp.radius.to_f32(), bp.height.to_f32());
            let progress = u.health.clamp(0.0, 1.0);
            let fwd = heading_dir(u.heading);
            let pitch = u.arm_pitch[1];
            // Fire dies down as the hull goes; steam where it is still hot and meets the sea.
            let heat = (1.0 - progress * 1.6).clamp(0.0, 1.0);
            let mut crossing = false;
            for k in [-0.75f32, 0.0, 0.75] {
                let t = self.scatter.unit();
                let centre = from.lerp(to, t);
                let deck = centre + fwd * k * r + Vec3::Z * (h * 0.55 + k * r * pitch.sin());
                let above = deck.z - water;
                let start = time + t * tick;
                let side = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.15;
                if above > 0.6 {
                    if self.scatter.unit() < 0.7 * heat {
                        let rise = Vec3::new(0.5, 0.2, 2.0 + self.scatter.unit() * 2.0);
                        self.push_puff(PUFF_FIRE, deck + side, rise, start, 0.9, (r * 0.1, r * 0.28));
                    }
                    if self.scatter.unit() < 0.3 + 0.4 * heat {
                        let drift = Vec3::new(1.4 + self.scatter.signed() * 0.6, 0.5 + self.scatter.signed() * 0.6, 2.6 + self.scatter.unit() * 1.5);
                        self.push_puff(PUFF_SMOKE, deck + side + Vec3::Z, drift, start, 4.2, (r * 0.15, r * 0.75));
                    }
                }
                if above.abs() < h * 0.7 {
                    crossing = true;
                    let at = Vec3::new(deck.x + side.x, deck.y + side.y, water + 0.3);
                    if self.scatter.unit() < 0.25 + 0.6 * heat {
                        let drift = Vec3::new(self.scatter.signed(), self.scatter.signed(), 1.5);
                        self.push_puff(PUFF_STEAM, at, drift, start, 2.6, (r * 0.1, r * 0.5));
                    }
                    if self.scatter.unit() < 0.5 {
                        let vel = self.scatter.upward(0.5) * (1.5 + self.scatter.unit() * 2.5);
                        self.push_puff(PUFF_DROPLET, at, vel, start, 1.2, (0.18, 0.35));
                    }
                }
                if above < -0.5 && self.scatter.unit() < 0.8 {
                    self.push_bubbles(deck, r * 0.25, 2, start, tick, 0.45);
                }
            }
            // Rings and foam round it while it is going through the surface, fainter as it goes.
            let last = self.water_fx.sinking.get(&u.unit_id).copied().unwrap_or(f32::MIN);
            let every = if crossing { 0.7 } else { 1.8 };
            if time - last >= every {
                self.water_fx.sinking.insert(u.unit_id, time);
                let foam = if crossing { 0.9 } else { 0.35 };
                let size = r * if crossing { 0.75 } else { 0.4 };
                self.push_ripple(Vec3::new(to.x, to.y, water), time, size, 5.0, 0.0, foam * (1.0 - progress * 0.6));
                if !crossing {
                    // Air still escaping, reaching the surface in gulps.
                    self.push_bubbles(to + Vec3::Z * h * 0.3, r * 0.4, 6, time, 0.6, 0.6);
                }
            }
        }
    }

    /// Follows each torpedo along its run, from the tube it left, for the line
    /// its air leaves on the water; once it is gone the line lies there and fades.
    fn torpedo_trails(&mut self, projectiles: &[ProjectileInstance], time: f32) {
        let tick = self.tick_seconds.max(0.02);
        let water = self.sea_level();
        // Air from deeper takes longer to come up, so the line grows out behind a
        // torpedo from over the boat. Held short: it still reads as leaving the tubes.
        let delay = |at: Vec3| ((water - at.z).max(0.0) / (BUBBLE_RISE * 2.5)).min(1.2);
        let mut was = std::mem::take(&mut self.water_fx.torpedoes);
        let mut now = HashMap::new();
        for p in projectiles {
            if p.color & PROJECTILE_TORPEDO == 0 {
                continue;
            }
            let (from, to) = (Vec3::from(p.prev_pos), Vec3::from(p.pos));
            // One dropped from the air leaves no line until it is in the water; the
            // tick it goes in, it throws up a splash and its first gulp of air.
            if to.z > water {
                continue;
            }
            if from.z > water {
                let t = ((from.z - water) / (from.z - to.z).max(1e-3)).clamp(0.0, 1.0);
                let entry = from.lerp(to, t);
                let at = Vec3::new(entry.x, entry.y, water);
                self.water_splash(at, time, 1.4, 0.9);
                self.push_ripple(at, time, 5.0, 4.0, 0.0, 0.8);
                self.torpedo_launch(Vec3::new(at.x, at.y, water - 1.0), (to - from).normalize_or_zero(), time);
            }
            let from = Vec3::new(from.x, from.y, from.z.min(water));
            let ends = ((p.color >> PROJECTILE_ENDS_SHIFT) & 0xFF) as f32 / 255.0;
            let span = if ends > 0.0 { ends } else { 1.0 };
            let mut run = was
                .remove(&super::trail_key(from))
                .filter(|r| r.ended.is_none())
                .unwrap_or_else(|| Torpedo {
                    prev: from,
                    pos: to,
                    speed: 0.0,
                    delay: delay(from),
                    path: vec![[from.x, from.y, 0.0, time + delay(from)]],
                    ended: None,
                });
            run.prev = from;
            run.pos = to;
            run.speed = from.distance(to) / (tick * span);
            run.delay = delay(to);
            if run.path[0][2] <= 0.0 {
                run.path[0][2] = run.speed;
            }
            let last = *run.path.last().unwrap();
            let far = Vec2::new(last[0], last[1]).distance(to.truncate()) >= TORPEDO_STEP;
            if far || ends > 0.0 {
                // Never earlier than the air behind it: the line fills in toward the head.
                run.path.push([to.x, to.y, run.speed, (time + run.delay).max(last[3])]);
            }
            if ends > 0.0 {
                run.ended = Some(time);
            }
            now.insert(super::trail_key(to), run);
        }
        // Ones that ran out of sight of the list, and the lines of spent ones.
        for (key, mut run) in was {
            if run.ended.is_none() {
                let last = *run.path.last().unwrap();
                run.path.push([run.pos.x, run.pos.y, run.speed, (time + run.delay).max(last[3])]);
                run.ended = Some(time);
            }
            now.insert(key, run);
        }
        now.retain(|_, run| {
            // Keep one point past the end of the line so its tail fades rather than jumps.
            while run.path.len() > 2 && time - run.path[1][3] > TORPEDO_LINE {
                run.path.remove(0);
            }
            run.ended.is_none() || time - run.path.last().unwrap()[3] < TORPEDO_LINE
        });
        self.water_fx.torpedoes = now;
    }

    /// Once a frame: the rings and wakes nearest the camera, for the water shader.
    pub(super) fn upload_sea_fx(&mut self, time: f32, alpha: f32, camera: &Camera) {
        let focus = camera.focus.truncate();
        let alpha = alpha.clamp(0.0, 1.0);
        let fx = &mut self.water_fx;
        fx.ripples.retain(|r| time < r.start + r.params[1]);
        let mut ripples: Vec<GpuRipple> = fx.ripples.iter().filter(|r| r.start <= time + 1.5).copied().collect();
        if ripples.len() > MOST_RIPPLES {
            ripples.sort_by(|a, b| {
                let da = Vec2::new(a.pos[0], a.pos[1]).distance_squared(focus);
                let db = Vec2::new(b.pos[0], b.pos[1]).distance_squared(focus);
                da.total_cmp(&db)
            });
            ripples.truncate(MOST_RIPPLES);
        }
        let mut wakes: Vec<GpuWake> = Vec::new();
        for hull in fx.hulls.values() {
            let pos = hull.prev.lerp(hull.pos, alpha);
            let speed = hull.prev_speed + (hull.speed - hull.prev_speed) * alpha;
            let heading = lerp_angle(hull.prev_heading, hull.heading, alpha);
            let fwd = heading_dir(heading);
            let bow = pos + fwd * hull.half_length;
            let stern = pos - fwd * hull.half_length;
            // The water at the stern was stirred when the bow went through it.
            let passing = (2.0 * hull.half_length / speed.max(1.0)).min(1.0);
            let mut trail = [[0.0f32; 4]; WAKE_POINTS];
            trail[0] = [bow.x, bow.y, speed, 0.0];
            trail[1] = [stern.x, stern.y, speed, passing];
            let mut n = 2;
            for p in hull.trail.iter().rev() {
                if n >= WAKE_POINTS {
                    break;
                }
                trail[n] = [p[0], p[1], p[2], time - p[3] + passing];
                n += 1;
            }
            for i in n..WAKE_POINTS {
                trail[i] = trail[n - 1];
                trail[i][2] = 0.0;
            }
            // Round every point, as far out as its arms have spread by now.
            let mut radius = hull.half_length + hull.half_beam * 4.0;
            for p in &trail {
                // Arms are gone after four and a half seconds; the white water keeps widening.
                let arms = p[3].min(4.5) * (p[2] * 0.34 + 1.1) + (0.5 + p[3].min(4.5) * 1.1) * 3.0;
                let churn = (hull.half_beam * 1.1 + p[3] * 1.2) * 3.0;
                let spread = arms.max(churn) + hull.half_beam;
                radius = radius.max(Vec2::new(p[0], p[1]).distance(pos.truncate()) + spread);
            }
            wakes.push(GpuWake {
                at: [pos.x, pos.y, fwd.x, fwd.y],
                shape: [speed, hull.half_length, hull.half_beam, hull.kind],
                bound: [pos.x, pos.y, radius, hull.strength],
                trail,
            });
        }
        for run in fx.torpedoes.values() {
            let live = run.ended.is_none();
            let head = if live { run.prev.lerp(run.pos, alpha) } else { run.pos };
            let dir = (run.pos - run.prev).truncate().normalize_or_zero();
            // Newest first: where it is now, its air not up yet, then its path back
            // toward the tubes, thinned to fit and always keeping the oldest point.
            let mut trail = [[0.0f32; 4]; WAKE_POINTS];
            let mut n = 0;
            if live {
                trail[0] = [head.x, head.y, run.speed, -run.delay];
                n = 1;
            }
            let room = WAKE_POINTS - n;
            let len = run.path.len();
            let take = len.min(room);
            for k in 0..take {
                let i = if len <= room { len - 1 - k } else { (len - 1) - (k * (len - 1) + (room - 1) / 2) / (room - 1) };
                let p = run.path[i];
                trail[n] = [p[0], p[1], p[2], time - p[3]];
                n += 1;
            }
            if n < 2 {
                continue;
            }
            for i in n..WAKE_POINTS {
                trail[i] = trail[n - 1];
                trail[i][2] = 0.0;
            }
            // Round every point, as wide as its line has spread by now.
            let spread = (0.35 + TORPEDO_LINE * 0.35) * 3.0;
            let mut radius = spread;
            for p in &trail[..n] {
                radius = radius.max(Vec2::new(p[0], p[1]).distance(head.truncate()) + spread);
            }
            wakes.push(GpuWake {
                at: [head.x, head.y, dir.x, dir.y],
                shape: [run.speed, 1.5, 0.3, 2.0],
                bound: [head.x, head.y, radius, 0.75],
                trail,
            });
        }
        if wakes.len() > MOST_WAKES {
            wakes.sort_by(|a, b| {
                let da = Vec2::new(a.at[0], a.at[1]).distance(focus) - a.bound[2];
                let db = Vec2::new(b.at[0], b.at[1]).distance(focus) - b.bound[2];
                da.total_cmp(&db)
            });
            wakes.truncate(MOST_WAKES);
        }
        let mut counts = [ripples.len() as u32, wakes.len() as u32, 0, 0];
        #[cfg(test)]
        if sea_shots::OFF.load(std::sync::atomic::Ordering::Relaxed) {
            counts = [0; 4];
        }
        fx.buffer.write(0, bytemuck::cast_slice(&counts));
        fx.buffer.write(16, bytemuck::cast_slice(&ripples));
        fx.buffer.write(
            (16 + MOST_RIPPLES * std::mem::size_of::<GpuRipple>()) as u64,
            bytemuck::cast_slice(&wakes),
        );
    }
}

#[cfg(test)]
mod sea_shots {
    use super::super::{FrameInput, Renderer, SceneDesc, Target};
    use crate::camera::Camera;
    use crate::overlay::Overlay;
    use bytemuck::Zeroable;
    use glam::{Vec2, Vec3};
    use mc_core::{Fx, FxVec3};
    use mc_data::{BlueprintId, WeaponColor};
    use mc_sim::mirror::*;
    use std::sync::Arc;

    const TICK: f32 = 0.1;
    /// Set to draw the water with empty lists, to measure what they cost.
    pub(super) static OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    fn fx3(v: Vec3) -> FxVec3 {
        FxVec3::new(Fx::from_f32(v.x), Fx::from_f32(v.y), Fx::from_f32(v.z))
    }

    fn hull(bp: BlueprintId, id: u32, prev: Vec3, pos: Vec3, heading: f32, radius: f32) -> UnitInstance {
        let mut u = UnitInstance::zeroed();
        u.prev_pos = prev.to_array();
        u.pos = pos.to_array();
        u.prev_heading = heading;
        u.heading = heading;
        u.blueprint = bp.0 as u32;
        u.health = 1.0;
        u.build = 1.0;
        u.radius = radius;
        u.unit_id = id;
        u
    }

    fn impact(at: Vec3, bp: BlueprintId, weapon: u8, splash: f32, on_unit: bool, after: f32) -> SimEvent {
        SimEvent::Impact {
            pos: fx3(at),
            target_motion: FxVec3::ZERO,
            splash: Fx::from_f32(splash),
            color: WeaponColor::Orange,
            after: Fx::from_f32(after),
            on_unit,
            on_shield: false,
            blueprint: bp,
            weapon,
        }
    }

    /// Stages sea effects on twin_shoals and writes PPMs. `SEA_SCENES` picks
    /// from shells, torpedo, death, subdeath, wakes; `SEA_CAM` = dist,yaw,tilt;
    /// `SEA_OUT` the folder. Prints the GPU pass times for the last 20 frames.
    #[test]
    #[ignore = "requires Vulkan and maps/twin_shoals.mcmap"]
    fn sea_shots() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let map = Arc::new(mc_map::MapFile::open(root.join("maps/twin_shoals.mcmap")).expect("open map"));
        let blueprints = Arc::new(mc_data::Blueprints::load(&root.join("data")).unwrap());
        let (w, h) = if std::env::var("SEA_BIG").is_ok() { (2560u32, 1440u32) } else { (1600u32, 900u32) };
        let mut renderer = Renderer::new(
            Target::Headless { width: w, height: h },
            SceneDesc {
                map: map.clone(),
                blueprints: blueprints.clone(),
                pool: Arc::new(mc_jobs::Pool::new(2)),
                team_colors: [[0.1, 0.6, 0.9]; 8],
            },
        )
        .unwrap();
        let out = std::path::PathBuf::from(
            std::env::var("SEA_OUT").unwrap_or_else(|_| root.join("artifacts/sea").display().to_string()),
        );
        std::fs::create_dir_all(&out).unwrap();
        let water = map.info().water_level.to_f32();
        // Open water at least 14 m deep, nearest the first start position.
        let size = Vec2::from(map.info().size_metres().to_f32());
        let from = map.start_positions().first().map_or(size * 0.5, |p| Vec2::from(p.to_f32()));
        let deep = |p: Vec2| {
            (-3..=3).all(|i| {
                (-3..=3).all(|j| {
                    let q = p + Vec2::new(i as f32 * 40.0, j as f32 * 40.0);
                    q.x > 0.0 && q.y > 0.0 && q.x < size.x && q.y < size.y && renderer.ground_height(q) < water - 14.0
                })
            })
        };
        let mut sea = from;
        'find: for ring in 0..200 {
            for s in 0..(ring * 8).max(1) {
                let a = std::f32::consts::TAU * s as f32 / (ring * 8).max(1) as f32;
                let p = from + Vec2::new(a.cos(), a.sin()) * 60.0 * ring as f32;
                if deep(p) {
                    sea = p;
                    break 'find;
                }
            }
        }
        println!("sea at {:.0},{:.0}, seabed {:.1}, water {water}", sea.x, sea.y, renderer.ground_height(sea));
        let id = |key: &str| blueprints.id_of(key).unwrap_or_else(|| panic!("no {key}"));
        let (frigate, boat, sub) = (id("aster_t1_frigate"), id("aster_t1_attack_boat"), id("aster_t1_submarine"));
        let at = |x: f32, y: f32, z: f32| Vec3::new(sea.x + x, sea.y + y, water + z);
        let overlay = Overlay::default();
        let cam: Vec<f32> = std::env::var("SEA_CAM")
            .unwrap_or_else(|_| "170,0.5,0".into())
            .split(',')
            .map(|v| v.trim().parse().unwrap())
            .collect();
        OFF.store(std::env::var("SEA_FX_OFF").is_ok(), std::sync::atomic::Ordering::Relaxed);
        let scenes = std::env::var("SEA_SCENES").unwrap_or_else(|_| "shells,torpedo,death,subdeath,wakes".into());
        let mut base = 100.0f32;
        for scene in scenes.split(',') {
            let captures: Vec<f32> = match std::env::var("SEA_TIMES") {
                Ok(times) => times.split(',').map(|v| v.trim().parse().unwrap()).collect(),
                Err(_) => match scene {
                "shells" => vec![1.0, 2.05, 3.0],
                "torpedo" => vec![1.8, 3.42, 3.6, 3.8, 4.5, 6.0],
                "death" => vec![0.32, 0.8, 2.0, 5.0, 9.0, 13.5, 15.0],
                "subdeath" => vec![0.25, 0.6, 1.6, 3.0],
                "wakestart" => vec![0.6, 1.5, 3.0, 5.0, 6.5, 9.0],
                _ => vec![3.0, 6.0],
                },
            };
            let end = captures.iter().copied().fold(0.0, f32::max);
            let mut camera = Camera::new(size, Vec2::new(w as f32, h as f32));
            // SEA_FOCUS: metres off the middle of the scene to look at.
            let off: Vec<f32> = std::env::var("SEA_FOCUS")
                .map(|v| v.split(',').map(|n| n.trim().parse().unwrap()).collect())
                .unwrap_or_else(|_| vec![0.0, 0.0]);
            camera.focus = Vec3::new(sea.x + off[0], sea.y + off[1], water);
            camera.distance = cam[0];
            camera.yaw = cam[1];
            camera.tilt = cam[2];
            let mut next = 0;
            let mut sums: Vec<f32> = Vec::new();
            let mut frames = 0;
            let settle_tick = 123u32;
            let seabed = renderer.ground_height(sea);
            let torpedo_from = at(-120.0, 30.0, -2.4);
            // On the side of the frigate's hull that faces the boat.
            let torpedo_to = at(37.0, 0.3, -1.8);
            let torpedo_ticks = (torpedo_from.distance(torpedo_to) / (48.0 * TICK)) as u32;
            let mut t = 0.0f32;
            while next < captures.len() {
                let k = (t / TICK).round() as u32;
                let ticked = (t / TICK - k as f32).abs() < 0.01;
                let mut frame = RenderFrame::default();
                frame.props_dead = vec![0; map.props().len().div_ceil(32)];
                let ts = k as f32 * TICK;
                match scene {
                    "shells" => {
                        let y = -40.0 + ts * 5.0;
                        frame.units.push(hull(frigate, 1, at(-60.0, y - 0.5, 0.0), at(-60.0, y, 0.0), 1.57, 15.0));
                        if k % 4 == 1 {
                            let a = k as f32 * 2.39;
                            let r = 25.0 + (k as f32 * 7.3) % 45.0;
                            frame.events.push(impact(at(a.cos() * r, a.sin() * r, 0.0), frigate, 0, 3.0, false, 0.4));
                        }
                        // A stream of machine-gun rounds walking across the water.
                        let x = -20.0 + (k % 20) as f32 * 3.0;
                        frame.events.push(impact(at(x, -30.0 + (k % 3) as f32, 0.0), boat, 0, 0.0, false, 0.5));
                        if k % 10 == 5 {
                            frame.events.push(impact(at(-60.0 + 3.2, y, 0.8), frigate, 0, 3.0, true, 0.3));
                        }
                    }
                    "torpedo" => {
                        let mut s = hull(sub, 2, torpedo_from, torpedo_from, 0.0, 10.0);
                        s.pos[2] = water - 3.0;
                        s.prev_pos[2] = water - 3.0;
                        s._pad3[0] = 255 | UNIT_DIVE_GOAL;
                        frame.units.push(s);
                        frame.units.push(hull(frigate, 1, at(40.0, -0.6, 0.0), at(40.0, 0.0, 0.0), 1.57, 15.0));
                        let dir = (torpedo_to - torpedo_from).normalize();
                        let step = dir * 48.0 * TICK;
                        if k == 2 {
                            frame.events.push(SimEvent::ShotFired {
                                pos: fx3(torpedo_from),
                                vel: fx3(step),
                                travel: fx3(Vec3::ZERO),
                                color: WeaponColor::Orange,
                                owner: 0,
                                blueprint: sub,
                                weapon: 0,
                            });
                        }
                        if k >= 2 && k <= 2 + torpedo_ticks {
                            let n = (k - 2) as f32;
                            let mut p = ProjectileInstance::zeroed();
                            p.prev_pos = (torpedo_from + step * n).to_array();
                            p.pos = (torpedo_from + step * (n + 1.0)).to_array();
                            p.color = 1 | PROJECTILE_TORPEDO | if k == 2 { PROJECTILE_FRESH } else { 0 };
                            p.size = 0.6;
                            if k == 2 + torpedo_ticks {
                                p.color |= 128 << PROJECTILE_ENDS_SHIFT;
                                frame.events.push(impact(torpedo_to, sub, 0, 3.0, true, 0.5));
                            }
                            frame.projectiles.push(p);
                        }
                    }
                    "death" => {
                        let pos = at(0.0, 0.0, 0.0);
                        if k < 2 {
                            frame.units.push(hull(frigate, 1, pos, pos, 0.4, 15.0));
                        }
                        if k == 2 {
                            frame.events.push(SimEvent::UnitDied {
                                pos: fx3(pos),
                                blueprint: frigate,
                                owner: 0,
                                airborne: false,
                            });
                        }
                        if k >= 3 {
                            let progress = |k: u32| ((k.min(settle_tick) - 3) as f32 / (settle_tick - 3) as f32).clamp(0.0, 1.0);
                            let depth = |p: f32| water - (water - seabed - 1.0) * p * p.sqrt();
                            let (p0, p1) = (progress(k.saturating_sub(1).max(3)), progress(k));
                            let mut u = hull(frigate, 1, Vec3::new(pos.x, pos.y, depth(p0)), Vec3::new(pos.x, pos.y, depth(p1)), 0.4, 15.0);
                            u.owner_flags = KIND_WRECK;
                            u.health = p1;
                            if k < settle_tick {
                                u._pad = WRECK_SINKING;
                                u.arm_pitch = [p0.min(0.3) * 0.8, p1.min(0.3) * 0.8, 0.0, 0.0];
                            }
                            frame.units.push(u);
                        }
                        if k == settle_tick {
                            frame.events.push(SimEvent::ShipSettled {
                                pos: fx3(Vec3::new(pos.x, pos.y, seabed + 1.0)),
                                blueprint: frigate,
                            });
                        }
                    }
                    "subdeath" => {
                        let pos = at(0.0, 0.0, -3.0);
                        if k == 2 {
                            frame.events.push(SimEvent::UnitDied {
                                pos: fx3(pos),
                                blueprint: sub,
                                owner: 0,
                                airborne: false,
                            });
                        }
                    }
                    "wakestart" => {
                        // Both hulls pull away from rest, run, and stop at five seconds.
                        let travel = |t: f32, top: f32, accel: f32| {
                            let t = t.clamp(0.0, 5.0);
                            let ramp = top / accel;
                            if t < ramp { 0.5 * accel * t * t } else { 0.5 * top * ramp + top * (t - ramp) }
                        };
                        let fx = |t: f32| at(-100.0 + travel(t, 22.0, 8.0), 20.0, 0.0);
                        frame.units.push(hull(frigate, 1, fx(ts - TICK), fx(ts), 0.0, 15.0));
                        let bx = |t: f32| at(-100.0 + travel(t, 48.0, 30.0), -30.0, 0.0);
                        frame.units.push(hull(boat, 3, bx(ts - TICK), bx(ts), 0.0, 6.0));
                    }
                    "stress" => {
                        // A busy sea: sixteen boats circling, six shells landing every tick.
                        for b in 0..16u32 {
                            let r = 40.0 + b as f32 * 9.0;
                            let c = |a: f32| at(a.cos() * r, a.sin() * r, 0.0);
                            let (a0, a1) = ((ts - TICK) * 0.4 + b as f32, ts * 0.4 + b as f32);
                            frame.units.push(hull(boat, 10 + b, c(a0), c(a1), a1 + 1.57, 6.0));
                        }
                        for i in 0..6u32 {
                            let a = (k * 6 + i) as f32 * 2.39;
                            let r = ((k * 6 + i) as f32 * 17.3) % 160.0;
                            frame.events.push(impact(at(a.cos() * r, a.sin() * r, 0.0), frigate, 0, 3.0, false, 0.3));
                        }
                    }
                    _ => {
                        // A fast boat on a circle, a frigate straight across, a dived boat.
                        let a0 = (ts - TICK) * 0.5;
                        let a1 = ts * 0.5;
                        let c = |a: f32| at(a.cos() * 80.0, a.sin() * 80.0, 0.0);
                        frame.units.push(hull(boat, 3, c(a0), c(a1), a1 + 1.57, 6.0));
                        let fx = |t: f32| at(-120.0 + t * 22.0, 40.0, 0.0);
                        frame.units.push(hull(frigate, 1, fx(ts - TICK), fx(ts), 0.0, 15.0));
                        let sx = |t: f32| at(-100.0 + t * 20.0, -60.0, -5.6);
                        let mut s = hull(sub, 2, sx(ts - TICK), sx(ts), 0.0, 10.0);
                        s._pad3[0] = 255 | UNIT_DIVE_GOAL;
                        frame.units.push(s);
                        // An enemy boat, dived, as a sonar contact would show it.
                        let ex = |t: f32| at(-60.0 + t * 20.0, -95.0, -5.6);
                        let mut e = hull(sub, 4, ex(ts - TICK), ex(ts), 0.0, 10.0);
                        e.owner_flags = 1;
                        e._pad3[0] = 255 | UNIT_DIVE_GOAL;
                        frame.units.push(e);
                    }
                }
                renderer
                    .render(&FrameInput {
                        camera: &camera,
                        time: base + t,
                        alpha: (t / TICK - k as f32).max(0.0),
                        sim: ticked.then_some(&frame),
                        ghosts: &[],
                        marks: &[],
                        ranges: &[],
                        ranges_drawn: 0,
                        overlay: &overlay,
                        build_grid: false,
                    })
                    .unwrap();
                if t + 20.0 * 0.05 > end {
                    frames += 1;
                    for (i, (_, ms)) in renderer.stats.gpu_passes.iter().enumerate() {
                        if sums.len() <= i {
                            sums.push(0.0);
                        }
                        sums[i] += ms;
                    }
                }
                if t + 0.001 >= captures[next] {
                    let pixels = renderer.read_pixels().unwrap();
                    let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
                    for p in pixels.chunks_exact(4) {
                        ppm.extend_from_slice(&p[..3]);
                    }
                    std::fs::write(out.join(format!("{scene}_{:.2}.ppm", captures[next])), ppm).unwrap();
                    next += 1;
                }
                t += 0.05;
                let _ = end;
            }
            let names: Vec<_> = renderer.stats.gpu_passes.iter().map(|p| p.0).collect();
            let mean: Vec<String> =
                names.iter().zip(&sums).map(|(n, ms)| format!("{n} {:.3}", ms / frames.max(1) as f32)).collect();
            println!("{scene}: {}", mean.join(", "));
            base += 100.0;
        }
    }
}
