//! The capital rail guns (`Weapon::heavy_rail`): the Resolute's spinal rail cannon and
//! the Zenith. Still ARC (docs/STYLE.md "ARC fires no plasma"): a slug on a current, the
//! air it tears white-hot and cooling to orange, vapour, shock. Only far bigger than any
//! other rail, and with layers no ordinary rail has:
//!
//! - **Charge** (`SimEvent::WeaponCharging`): arcs crawl along the rails, jump the slot
//!   between them and curl round the outside of them, faster as the charge builds; the
//!   rail tips spit brush discharge into the air; sparks drip off the rails and snap
//!   across the slot; the light is the arcs' own, a blue-violet strobe that jumps along
//!   the barrel with them (no steady glow: a knot of white light at the muzzle drowned
//!   the arcs); in the last part of the charge the air round the muzzle condenses into a
//!   ring.
//! - **Fire**: a colossal white flash, a shock front out along the bore and a ring of
//!   vapour thrown out flat across it, a long vapour cone, rail spatter and molten drops,
//!   the breech venting behind; the rails glow white and cool to orange; the air under a
//!   gun near the ground is thrown into dust (the shockwave's own ground dust, trees);
//!   the clouds round the muzzle are shoved about.
//! - **Path**: a thick ionised channel along the whole flight that cools white, orange,
//!   dull red over seconds; a vapour tube that hangs and drifts off downwind; vapour
//!   collars every so often along it; where it crosses the cloud layer the cloud is
//!   torn (`Sky::blast`, as the nukes stir it: disturbed, not cleared); where it runs low
//!   it raises dust or spray under it; the slug lights what it passes.
//! - **Impact**: a white flash, a big shock front, a crater, earth and dust thrown up and
//!   smoke that trails off downwind for half a minute; on a hull a burst of sparks,
//!   fragments and spall out the far side.
//!
//! The ordinary rail's effects are not drawn for these guns (`heavy_rail_event` takes the
//! whole event; `rail_wakes` skips their slugs through `heavy_rail_owns`). A light one,
//! under `FIRES_HEAVY` (the Resolute's turrets), only charges like them and its rails
//! glow and cool after the shot; the shot itself is an ordinary rail's.
//! `MERIDIAN_HEAVY_RAIL=0` draws them as ordinary rails instead (to compare looks and cost).

use super::craters::CraterStyle;
use super::nuke_fx::PUFF_STRATEGIC_TRAIL;
use super::rail_fx::RAIL_FLASH;
use super::water_fx::PUFF_STEAM;
use super::{
    Renderer, PUFF_BOLT, PUFF_CLOD, PUFF_DUST, PUFF_FIREBALL, PUFF_SHARD, PUFF_SHOCK_DUST,
    PUFF_SMOKE, PUFF_SPARK, PUFF_TREE_SMOKE,
};
use glam::{Vec2, Vec3};
use mc_data::BlueprintId;
use mc_sim::mirror::{
    HousePose, ProjectileInstance, SimEvent, UnitInstance, KIND_GHOST, KIND_WRECK, PROJECTILE_BEAM,
    PROJECTILE_ENDS_SHIFT, PROJECTILE_FADE_BEAM, PROJECTILE_RAIL, PROJECTILE_STARTS_SHIFT,
    UNIT_HOUSE_SHIFT,
};
use std::mem::size_of;

/// Fading-beam colours (sprites.wgsl): lightning, the white flash of a rail's path, and
/// the capital channel that cools white, orange, red.
const BOLT: u32 = 3;
const FLASH: u32 = 5;
const CHANNEL: u32 = 6;
/// The colour of a charge's arc light: blue-violet, as the arcs are (sprites.wgsl colour 3).
const ARC_LIGHT: Vec3 = Vec3::new(0.4, 0.5, 1.0);
/// Metres of cloud layer over its base (sky.rs `CLOUD_DECK`).
const CLOUD_DECK: f32 = 380.0;
/// Seconds the channel glows, and the vapour tube hangs.
const CHANNEL_LIFE: f32 = 5.5;
const TUBE_LIFE: f32 = 16.0;
/// Metres between vapour collars along the path.
const COLLAR_STEP: f32 = 150.0;
/// Most strokes kept (arcs and channel pieces of every gun firing).
const MAX_STROKES: usize = 3000;
/// The least `heavy_rail` whose firing, path and hit are drawn as a capital gun's. A
/// lighter gun's slug is too small to be told from an ordinary rail's (`heavy_shot_of`):
/// it charges like a capital gun and its rails glow after, but it fires as a plain rail.
const FIRES_HEAVY: f32 = 0.15;

fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("MERIDIAN_HEAVY_RAIL").map_or(true, |v| v != "0"))
}

struct Stroke {
    from: Vec3,
    to: Vec3,
    start: f32,
    life: f32,
    width: f32,
    color: u32,
}

/// A gun charging: its arcs and glow are laid a tick at a time on the barrel as it is then.
struct Charge {
    blueprint: u16,
    weapon: u8,
    /// Where the sim said the hull was, then where it was last found.
    near: Vec3,
    unit: Option<u32>,
    start: f32,
    end: f32,
    /// Charge time laid so far.
    laid: f32,
    /// The muzzle as last laid, for its light.
    muzzle: Vec3,
    breech: Vec3,
    /// The vapour ring has formed at the muzzle.
    ring: bool,
    /// The gun's `heavy_rail` size, for its light.
    scale: f32,
}

/// A gun that has just fired: its rails glow and cool, the breech vents (laid on the next
/// tick, when the hull can be found).
struct Fired {
    blueprint: u16,
    weapon: u8,
    near: Vec3,
    /// The unit, when its charge was seen.
    unit: Option<u32>,
    time: f32,
    /// Rails, for their light while they cool.
    rails: Option<(Vec3, Vec3)>,
}

/// A slug in flight and the channel it leaves.
#[derive(Clone, Copy)]
struct Shot {
    muzzle: Vec3,
    dir: Vec3,
    speed: f32,
    fired: f32,
    reach: f32,
    scale: f32,
    /// Metres along the path laid so far, and when the slug got there.
    laid: f32,
    laid_at: f32,
    next_collar: f32,
    next_cloud: f32,
    clouds: u8,
    /// When it struck, once it has.
    ended: Option<f32>,
}

#[derive(Default)]
pub(super) struct HeavyRailFx {
    charges: Vec<Charge>,
    fired: Vec<Fired>,
    shots: Vec<Shot>,
    strokes: Vec<Stroke>,
    last: f32,
    /// This tick's gun-house poses (`RenderFrame::houses`), for guns on houses of their own.
    houses: Vec<HousePose>,
}

/// Where a gun's rails run in the world: breech, muzzle and the way the barrel points, and
/// the open stretches of rail the charge crawls along.
#[derive(Clone, Copy)]
struct Barrel {
    breech: Vec3,
    muzzle: Vec3,
    dir: Vec3,
    side: Vec3,
    up: Vec3,
    /// Half the gap between the rails, metres.
    gap: f32,
    /// Where the arcs crawl: share of the way from breech to muzzle, half the stretch
    /// (the same share), and how far over the bore axis the rail tops are there.
    spots: [(f32, f32, f32); 6],
    /// How far over the bore axis the fired rails' heat runs: on the rail tops, or in
    /// the rails themselves (a turret gun's), where only its light shows.
    heat_lift: f32,
}

impl Barrel {
    fn length(&self) -> f32 {
        self.breech.distance(self.muzzle)
    }
    /// A point `t` of the way from breech to muzzle on rail `rail` (-1 or 1), `lift` over the axis.
    fn on_rail(&self, t: f32, rail: f32, lift: f32) -> Vec3 {
        self.breech.lerp(self.muzzle, t) + self.side * self.gap * rail + self.up * lift
    }
}

/// White-hot, then orange, then a dull red, as sprites.wgsl colours the channel.
fn heat_rgb(heat: f32) -> Vec3 {
    let h = heat.clamp(0.0, 1.0);
    let warm = Vec3::new(0.5, 0.05, 0.01).lerp(Vec3::new(1.0, 0.36, 0.06), smooth(0.0, 0.5, h));
    warm.lerp(Vec3::new(1.0, 0.95, 0.88), smooth(0.8, 0.98, h))
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Renderer {
    fn heavy_weapon(&self, blueprint: BlueprintId, weapon: u8) -> Option<f32> {
        let w = self
            .blueprints
            .unit(blueprint)
            .weapons
            .get(weapon as usize)?;
        (w.heavy_rail > 0.0 && enabled()).then_some(w.heavy_rail)
    }

    /// Draws `event` when it is a capital rail gun's: true when that is all of it.
    pub(super) fn heavy_rail_event(&mut self, event: &SimEvent, time: f32) -> bool {
        match event {
            SimEvent::WeaponCharging {
                pos,
                blueprint,
                weapon,
                ..
            } => {
                let Some(scale) = self.heavy_weapon(*blueprint, *weapon) else {
                    return false;
                };
                let w = &self.blueprints.unit(*blueprint).weapons[*weapon as usize];
                let seconds = w.charge_ticks as f32 * self.tick_seconds.max(0.02);
                let near = Vec3::from(pos.to_f32()) - Vec3::Z * w.muzzle.z.to_f32();
                // A fresh charge replaces one still running on the same gun.
                self.heavy_rail.charges.retain(|c| {
                    !(c.blueprint == blueprint.0
                        && c.weapon == *weapon
                        && c.near.truncate().distance(near.truncate()) < 30.0)
                });
                self.heavy_rail.charges.push(Charge {
                    blueprint: blueprint.0,
                    weapon: *weapon,
                    near,
                    unit: None,
                    start: time,
                    end: time + seconds.max(0.3),
                    laid: time,
                    muzzle: Vec3::from(pos.to_f32()),
                    breech: Vec3::from(pos.to_f32()),
                    ring: false,
                    scale,
                });
                true
            }
            SimEvent::ShotFired {
                pos,
                vel,
                travel,
                blueprint,
                weapon,
                ..
            } => {
                let Some(scale) = self.heavy_weapon(*blueprint, *weapon) else {
                    return false;
                };
                let w = &self.blueprints.unit(*blueprint).weapons[*weapon as usize];
                let at = Vec3::from(pos.to_f32()) - Vec3::from(travel.to_f32());
                let v = Vec3::from(vel.to_f32());
                let dir = v.normalize_or(Vec3::X);
                let speed = v.length() / self.tick_seconds.max(0.02);
                let reach = w.range_max.to_f32() * 1.15 + 100.0;
                // The charge is spent: the one on this gun whose muzzle the slug leaves, and
                // with it the unit (a muzzle can be further from the hull's centre than the
                // hull can be looked for from). Without one, near the muzzle.
                let charge = self
                    .heavy_rail
                    .charges
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.blueprint == blueprint.0 && c.weapon == *weapon)
                    .map(|(i, c)| (i, c.muzzle.distance(at)))
                    .filter(|&(_, d)| d < 40.0)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(i, _)| i);
                let (near, unit) = match charge {
                    Some(i) => {
                        let c = self.heavy_rail.charges.swap_remove(i);
                        (c.near, c.unit)
                    }
                    None => (
                        Vec3::from(pos.to_f32()) - Vec3::Z * w.muzzle.z.to_f32(),
                        None,
                    ),
                };
                self.heavy_rail.fired.push(Fired {
                    blueprint: blueprint.0,
                    weapon: *weapon,
                    near,
                    unit,
                    time,
                    rails: None,
                });
                if scale < FIRES_HEAVY {
                    return false;
                }
                self.heavy_rail.shots.push(Shot {
                    muzzle: at,
                    dir,
                    speed,
                    fired: time,
                    reach,
                    scale,
                    laid: 0.0,
                    laid_at: time,
                    next_collar: COLLAR_STEP * 0.5,
                    next_cloud: 0.0,
                    clouds: 0,
                    ended: None,
                });
                let outbound = std::mem::replace(&mut self.effect_outbound, true);
                // A longer gun throws a bigger blast: the Resolute's keel rail is four times
                // the Zenith's barrel.
                let size = (self.heavy_rail_length(*blueprint, *weapon) / 90.0)
                    .sqrt()
                    .clamp(0.8, 1.9);
                self.heavy_muzzle(at, dir, scale * size, time);
                self.effect_outbound = outbound;
                true
            }
            SimEvent::Impact {
                pos,
                target_motion,
                after,
                on_unit,
                on_shield,
                blueprint,
                weapon,
                ..
            } => {
                let Some(scale) = self
                    .heavy_weapon(*blueprint, *weapon)
                    .filter(|&s| s >= FIRES_HEAVY)
                else {
                    return false;
                };
                let at = Vec3::from(pos.to_f32());
                let start = time + after.to_f32() * self.tick_seconds;
                // The channel runs all the way in, however the slug's last stretch was drawn.
                let dir = self.finish_channel(at, start);
                if *on_shield {
                    // The dome takes it: its own hit, much heavier (the ordinary handler), and
                    // a white flash where the slug stopped.
                    self.push_effect(at.to_array(), start, 40.0 * scale, 0.3, RAIL_FLASH, 1.0);
                    return false;
                }
                self.heavy_impact(
                    at,
                    dir,
                    Vec3::from(target_motion.to_f32()),
                    *on_unit,
                    scale,
                    start,
                );
                true
            }
            _ => false,
        }
    }

    /// Metres of rail from breech to muzzle on `blueprint`'s gun `weapon` (its model's anchors).
    fn heavy_rail_length(&self, blueprint: BlueprintId, weapon: u8) -> f32 {
        let bp = self.blueprints.unit(blueprint);
        let mesh = bp.visual.mesh.as_str();
        if let Some(r) = crate::models::turret_rail(mesh, weapon as usize) {
            return r.muzzle - r.breech;
        }
        if let Some(r) = crate::models::spinal_rail(mesh) {
            return (r.muzzle[0] - r.breech[0]).abs();
        }
        if mesh == "anti_ship_rail" {
            let z = &crate::models::ZENITH_RAIL;
            return (z.muzzle[0] - z.breech[0]).abs();
        }
        bp.weapons
            .first()
            .map_or(60.0, |w| w.muzzle.to_f32()[0].abs().max(20.0))
    }

    /// True for a slug of a capital rail gun, whose path `heavy_rail_tick` draws.
    pub(super) fn heavy_rail_owns(&self, p: &ProjectileInstance) -> bool {
        self.heavy_shot_of(p).is_some()
    }

    fn heavy_shot_of(&self, p: &ProjectileInstance) -> Option<usize> {
        // A capital slug is a heavy trace (mirror `look`: 5 and up); a turret's rail on the
        // same line is not taken for it.
        if p.color & (PROJECTILE_RAIL | PROJECTILE_FADE_BEAM | PROJECTILE_BEAM) != PROJECTILE_RAIL
            || p.size < 3.0
        {
            return None;
        }
        let pos = Vec3::from(p.pos);
        self.heavy_rail.shots.iter().position(|s| {
            if s.ended.is_some() {
                return false;
            }
            let rel = pos - s.muzzle;
            let along = rel.dot(s.dir);
            let off = (rel - s.dir * along).length();
            (-40.0..s.reach).contains(&along) && off < 14.0 + along * 0.02
        })
    }

    /// Once a tick, after the fading beams: the charges, the paths the slugs flew this
    /// tick, the rails of guns that just fired, and every stroke written for the GPU.
    pub(super) fn heavy_rail_tick(
        &mut self,
        units: &[UnitInstance],
        houses: &[HousePose],
        projectiles: &[ProjectileInstance],
        time: f32,
    ) {
        if !enabled() {
            return;
        }
        if time + 1.0 < self.heavy_rail.last {
            // The clock went back (a restaged backdrop): nothing of the old world is left.
            self.heavy_rail = HeavyRailFx::default();
        }
        self.heavy_rail.last = time;
        self.heavy_rail.houses.clear();
        self.heavy_rail.houses.extend_from_slice(houses);
        self.heavy_charges(units, time);
        self.heavy_fired(units, time);
        for p in projectiles {
            if let Some(i) = self.heavy_shot_of(p) {
                self.heavy_path(i, p, time);
            }
        }
        // A slug that is no longer seen has gone past its reach or into something.
        let tick = self.tick_seconds.max(0.02);
        for s in &mut self.heavy_rail.shots {
            if s.ended.is_none() && time - s.laid_at > tick * 3.0 && time - s.fired > tick * 3.0 {
                s.ended = Some(s.laid_at);
            }
        }
        self.heavy_rail.shots.retain(|s| time < s.fired + TUBE_LIFE);
        self.heavy_rail.fired.retain(|f| time < f.time + 6.0);
        self.heavy_rail.strokes.retain(|s| time < s.start + s.life);
        if self.heavy_rail.strokes.len() > MAX_STROKES {
            let extra = self.heavy_rail.strokes.len() - MAX_STROKES;
            self.heavy_rail.strokes.drain(..extra);
        }
        let size = size_of::<ProjectileInstance>();
        for s in &self.heavy_rail.strokes {
            let i = self.projectile_count as usize;
            if i >= super::MAX_PROJECTILES {
                break;
            }
            let inst = ProjectileInstance {
                prev_pos: s.from.to_array(),
                color: PROJECTILE_FADE_BEAM | s.color,
                pos: s.to.to_array(),
                size: s.width,
                wake: s.start,
                plasma: s.life,
                _pad: [0.0; 2],
                aim: [0.0; 4],
                prev_aim: [0.0; 4],
            };
            self.projectiles
                .write((i * size) as u64, bytemuck::bytes_of(&inst));
            self.projectile_count += 1;
        }
    }

    /// Every frame: the charge's light, the slug's as it flies, the channel's and the
    /// rails' as they cool.
    pub(super) fn heavy_rail_lights(&mut self, time: f32) {
        if !enabled() {
            return;
        }
        for c in &self.heavy_rail.charges {
            let f = ((time - c.start) / (c.end - c.start).max(0.01)).clamp(0.0, 1.0);
            if time < c.start || time > c.end + 0.05 {
                continue;
            }
            // Arc light, not a glow: it strobes, dark between strikes more often early on,
            // and each strike lights from wherever on the barrel it jumped.
            let step = (time * 30.0).floor();
            let roll = |salt: f32| {
                ((step * 12.9898 + salt * 78.233).sin() * 43758.545)
                    .fract()
                    .abs()
            };
            let struck = roll(c.start) < 0.35 + 0.55 * f;
            let strobe = if struck {
                0.55 + 0.45 * roll(c.start + 3.1)
            } else {
                0.06
            };
            let s = c.scale.sqrt();
            let at = c.breech.lerp(c.muzzle, 0.15 + 0.85 * roll(c.start + 7.7));
            self.lights.lamp(
                at,
                Vec3::Z,
                ARC_LIGHT * 2400.0 * s * (0.3 + 0.7 * f) * strobe,
                (12.0 + 40.0 * f) * s.max(0.5),
                180.0,
                1.0,
            );
            // The rails themselves, faintly, all along.
            self.lights.beam(
                c.breech,
                c.muzzle,
                ARC_LIGHT * 150.0 * s * f * strobe,
                (15.0 + 30.0 * f) * s.max(0.5),
            );
        }
        for s in &self.heavy_rail.shots {
            let flown = (time - s.fired) * s.speed;
            let end = s
                .ended
                .map_or(s.reach, |e| (e - s.fired) * s.speed)
                .min(s.reach);
            if time >= s.fired && flown < end {
                // The slug: a white-hot point tearing past, lighting the ground it passes over.
                let head = s.muzzle + s.dir * flown;
                self.lights.lamp(
                    head,
                    Vec3::Z,
                    Vec3::new(1.0, 0.95, 0.88) * 90_000.0 * s.scale,
                    220.0 * s.scale,
                    180.0,
                    1.0,
                );
            }
            let age = time - s.fired;
            if age < 0.0 || age > CHANNEL_LIFE {
                continue;
            }
            let heat = 1.0 - age / CHANNEL_LIFE;
            let tail = s.muzzle + s.dir * flown.min(end).max(0.0);
            // In a few pieces, so each lights only the ground near it.
            let pieces = 4;
            for k in 0..pieces {
                let (a, b) = (
                    s.muzzle.lerp(tail, k as f32 / pieces as f32),
                    s.muzzle.lerp(tail, (k + 1) as f32 / pieces as f32),
                );
                let power = 5000.0 * heat.powf(1.6) * s.scale;
                self.lights
                    .beam(a, b, heat_rgb(heat) * power, 70.0 + 80.0 * heat);
            }
        }
        for f in &self.heavy_rail.fired {
            let Some((a, b)) = f.rails else { continue };
            let age = time - f.time;
            if !(0.0..4.0).contains(&age) {
                continue;
            }
            let heat = 1.0 - age / 4.0;
            self.lights
                .beam(a, b, heat_rgb(heat) * 2200.0 * heat * heat, 50.0);
        }
    }

    /// Where the gun of `blueprint`'s `weapon` on `u` is, `f` of the way through the tick.
    fn heavy_barrel(&self, u: &UnitInstance, weapon: u8, f: f32) -> Option<Barrel> {
        let bp = self.blueprints.unit(BlueprintId(u.blueprint as u16));
        let w = bp.weapons.get(weapon as usize)?;
        let pos = Vec3::from(u.prev_pos).lerp(Vec3::from(u.pos), f);
        let turn = (u.heading - u.prev_heading + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        let heading = u.prev_heading + turn * f;
        let lerp_angle = |a: f32, b: f32| {
            a + ((b - a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI)
                * f
        };
        // A gun on a house of its own (a warship's turret) has its pose in the houses list;
        // otherwise only the first weapon's turn and elevation are in the instance, and a
        // gun fixed in the hull (`turret_turn` 0, the spinal cannon) lies along the keel.
        let house = (u.status[1] >> UNIT_HOUSE_SHIFT)
            .checked_sub(1)
            .and_then(|i| self.heavy_rail.houses.get(i as usize))
            .filter(|_| w.mount)
            .and_then(|h| h.pose.get(weapon as usize));
        let fixed = w.turret_turn == 0 || (weapon != 0 && house.is_none());
        let (yaw, pitch) = match house {
            Some(p) => (lerp_angle(p[0], p[1]), p[2] + (p[3] - p[2]) * f),
            None if fixed => (0.0, 0.0),
            None => (
                lerp_angle(u.prev_turret_yaw, u.turret_yaw),
                u.arm_pitch[0] + (u.arm_pitch[1] - u.arm_pitch[0]) * f,
            ),
        };
        let rot_z = |v: Vec3, a: f32| {
            let (s, c) = a.sin_cos();
            Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
        };
        let rot_xz = |v: Vec3, a: f32| {
            let (s, c) = a.sin_cos();
            Vec3::new(v.x * c - v.z * s, v.y, v.x * s + v.z * c)
        };
        let mesh = bp.visual.mesh.as_str();
        // A spacecraft's hull pitches to lay its spinal gun and carries everything on it
        // (entity.wgsl `capital_ship`: the hull's pitch is in slot 0).
        let hull = if crate::models::capital_rig(mesh).is_some() {
            u.arm_pitch[0] + (u.arm_pitch[1] - u.arm_pitch[0]) * f
        } else {
            0.0
        };
        let world = |local: Vec3| pos + rot_z(rot_xz(local, hull), heading);
        let muzzle = Vec3::from(w.muzzle.to_f32());
        let even = |n: usize| (n as f32 + 0.5) / 6.0;
        let (breech, muzzle, gap, spots, heat_lift) = if let (Some(r), Some(p)) =
            (crate::models::turret_rail(mesh, weapon as usize), w.pivot)
        {
            // A turret rail cannon (models `TurretRail`): the gun's frame from its trunnion,
            // the arcs on the rail tops where they can be seen, not in the bore.
            let pivot = Vec3::from(p.to_f32());
            let place = |x: f32| world(pivot + rot_z(rot_xz(Vec3::new(x, 0.0, 0.0), pitch), yaw));
            let span = (r.muzzle - r.breech).max(1.0);
            let spots = r
                .arcs
                .map(|x| ((x - r.breech) / span, r.arc_half / span, r.rail_top));
            (place(r.breech), place(r.muzzle), r.rail_y, spots, 0.0)
        } else if let Some(r) = crate::models::spinal_rail(mesh).filter(|_| w.turret_turn == 0) {
            // A spinal gun (models `SpinalRail`): the rails along the keel, the arcs in
            // the open lengths between its collars, on top of the rails. The whole hull
            // pitches to lay it (`combat::spinal_gun`), and the rails with it.
            let (b, m) = (Vec3::from(r.breech), Vec3::from(r.muzzle));
            let span = (m.x - b.x).max(1.0);
            let lift = r.arcs[0][2] - r.muzzle[2];
            let spots = r.arcs.map(|a| ((a[0] - b.x) / span, 14.0 / span, lift));
            (
                world(b),
                world(m),
                (lift * 0.75).clamp(1.0, 6.0),
                spots,
                lift,
            )
        } else if mesh == "anti_ship_rail" {
            // The Zenith (models `ZENITH_RAIL`): barrel frame along the bore from the
            // trunnion; bare rails out past the jacket, arcs over the jacket behind them.
            let z = &crate::models::ZENITH_RAIL;
            let pivot = Vec3::from(z.pivot);
            let place = |x: f32| world(pivot + rot_z(rot_xz(Vec3::new(x, 0.0, 0.0), pitch), yaw));
            let (b, m) = (z.breech[0], z.muzzle[0]);
            let span = (m - b).max(1.0);
            let bare = z.rails[0];
            let spots = z.along.map(|a| {
                let t = (a[0] - b) / span;
                let lift = if a[0] < bare - 0.5 {
                    z.jacket_radius
                } else {
                    0.0
                };
                (t, 9.0 / span, lift)
            });
            (place(b), place(m), z.rails[2], spots, spots[5].2)
        } else {
            let length;
            let (b, m) = match w.pivot {
                Some(p) if !fixed => {
                    let pivot = Vec3::from(p.to_f32());
                    let place = |local: Vec3| pivot + rot_z(rot_xz(local - pivot, pitch), yaw);
                    length = muzzle.distance(pivot) * 1.35;
                    // The rails run back past the trunnions, a third of the barrel again.
                    (
                        world(place(pivot - (muzzle - pivot) * 0.35)),
                        world(place(muzzle)),
                    )
                }
                _ => {
                    // Along the keel, most of the hull's length.
                    length = (bp.radius.to_f32() * 1.5).max(20.0);
                    (world(muzzle - Vec3::X * length), world(muzzle))
                }
            };
            let gap = (length * 0.018).clamp(0.8, 5.0);
            (
                b,
                m,
                gap,
                [0, 1, 2, 3, 4, 5].map(|n| (even(n), 0.08, 0.0)),
                0.0,
            )
        };
        let dir = (muzzle - breech).normalize_or(Vec3::X);
        let side = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
        let up = side.cross(dir).normalize_or(Vec3::Z);
        Some(Barrel {
            breech,
            muzzle,
            dir,
            side,
            up,
            gap,
            spots,
            heat_lift,
        })
    }

    fn heavy_find(
        &self,
        units: &[UnitInstance],
        blueprint: u16,
        near: Vec3,
        unit: Option<u32>,
    ) -> Option<usize> {
        let live = |u: &UnitInstance| {
            u.blueprint == blueprint as u32 && u.owner_flags & (KIND_WRECK | KIND_GHOST) == 0
        };
        if let Some(id) = unit {
            if let Some(i) = units.iter().position(|u| live(u) && u.unit_id == id) {
                return Some(i);
            }
        }
        units
            .iter()
            .enumerate()
            .filter(|(_, u)| {
                live(u) && Vec3::from(u.pos).truncate().distance(near.truncate()) < 120.0
            })
            .min_by(|a, b| {
                let da = Vec3::from(a.1.pos).truncate().distance(near.truncate());
                let db = Vec3::from(b.1.pos).truncate().distance(near.truncate());
                da.total_cmp(&db)
            })
            .map(|(i, _)| i)
    }

    /// The next tick of every charge on its barrel as it is now.
    fn heavy_charges(&mut self, units: &[UnitInstance], time: f32) {
        let tick = self.tick_seconds.max(0.02);
        self.heavy_rail.charges.retain(|c| time < c.end + 0.5);
        for i in 0..self.heavy_rail.charges.len() {
            let c = &self.heavy_rail.charges[i];
            let (blueprint, weapon, near, unit, start, end, laid) = (
                c.blueprint,
                c.weapon,
                c.near,
                c.unit,
                c.start,
                c.end,
                c.laid,
            );
            let until = (time + tick).min(end);
            if laid >= until {
                continue;
            }
            let Some(row) = self.heavy_find(units, blueprint, near, unit) else {
                continue;
            };
            let u = units[row];
            {
                let c = &mut self.heavy_rail.charges[i];
                c.unit = Some(u.unit_id);
                c.near = Vec3::from(u.pos);
                c.laid = until;
            }
            let scale = self
                .heavy_weapon(BlueprintId(blueprint), weapon)
                .unwrap_or(1.0);
            // In steps of a twentieth of a second, each on the barrel where it is drawn then.
            let steps = ((until - laid.max(time)) / 0.05).ceil().max(1.0) as usize;
            let outbound = std::mem::replace(&mut self.effect_outbound, true);
            for k in 0..steps {
                let when = laid.max(time) + (until - laid.max(time)) * k as f32 / steps as f32;
                let Some(barrel) =
                    self.heavy_barrel(&u, weapon, ((when - time) / tick).clamp(0.0, 1.0))
                else {
                    break;
                };
                let f = ((when - start) / (end - start).max(0.01)).clamp(0.0, 1.0);
                self.charge_step(&barrel, f, when, scale);
                if f > 0.5 && !self.heavy_rail.charges[i].ring {
                    self.heavy_rail.charges[i].ring = true;
                    self.condense_ring(&barrel, when, end - when + 2.0, scale);
                }
                let c = &mut self.heavy_rail.charges[i];
                c.muzzle = barrel.muzzle;
                c.breech = barrel.breech;
            }
            self.effect_outbound = outbound;
        }
    }

    /// A twentieth of a second of a charge `f` of the way done.
    fn charge_step(&mut self, b: &Barrel, f: f32, when: f32, scale: f32) {
        let length = b.length();
        let (side, up) = (b.side, b.up);
        // Arcs stay thick enough to read on the smaller guns: by the square root of their size.
        let size = scale.sqrt() * (length / 80.0).clamp(0.6, 2.2);
        // Arcs crawling along the open rails: a few at first, a storm of them at the end.
        // Early on they crawl near the breech; by the end the whole rail is alive.
        let arcs = 2 + (f * f * 12.0 + self.scatter.unit()) as usize;
        let reach = ((0.3 + 0.8 * f) * b.spots.len() as f32)
            .ceil()
            .min(b.spots.len() as f32) as usize;
        for _ in 0..arcs {
            let (t, half, lift) =
                b.spots[(self.scatter.unit() * reach as f32) as usize % b.spots.len()];
            let rail = if self.scatter.unit() < 0.5 { -1.0 } else { 1.0 };
            let t0 = t + self.scatter.signed() * half;
            let across = self.scatter.unit() < 0.5;
            let from = b.on_rail(t0, rail, lift);
            let to = if across {
                // Jumping the slot between the rails.
                b.on_rail(t0 + self.scatter.signed() * half * 0.3, -rail, lift)
            } else {
                b.on_rail(t0 + self.scatter.signed() * half * 1.4, rail, lift)
            };
            let life = 0.07 + self.scatter.unit() * 0.1;
            let width = (0.5 + f * 1.2) * size;
            let wander = (from.distance(to) * 0.14).max(b.gap * 0.4);
            let roll = self.scatter.unit();
            self.heavy_bolt(from, to, wander, side, up, when + roll * 0.04, life, width);
        }
        // Late in the charge, now and then an arc runs the whole length of the gun.
        if f > 0.5 && self.scatter.unit() < f * 0.5 {
            let rail = if self.scatter.unit() < 0.5 { -1.0 } else { 1.0 };
            let (t0, _, l0) = b.spots[0];
            let (t1, _, l1) = b.spots[b.spots.len() - 1];
            let (from, to) = (b.on_rail(t0, rail, l0), b.on_rail(t1, rail, l1));
            let width = (0.6 + f) * size;
            self.heavy_long_bolt(
                from,
                to,
                (length * 0.015).max(b.gap),
                side,
                up,
                when,
                0.12,
                width,
            );
        }
        // Arcs curling round the rails: off the outside of one and back onto it further on.
        for _ in 0..(f * 4.0 + self.scatter.unit()) as usize {
            let (t, half, lift) =
                b.spots[(self.scatter.unit() * reach as f32) as usize % b.spots.len()];
            let rail = if self.scatter.unit() < 0.5 { -1.0 } else { 1.0 };
            let t0 = t + self.scatter.signed() * half;
            let from = b.on_rail(t0, rail, lift);
            let to = b.on_rail(t0 + self.scatter.signed() * half, rail, lift);
            let bulge = b.gap * (1.0 + f * 1.6) * (0.6 + self.scatter.unit());
            let apex = from.lerp(to, 0.5)
                + (side * rail + up * self.scatter.signed()).normalize_or(up) * bulge;
            let width = (0.35 + f * 0.8) * size;
            let (life, roll) = (
                0.06 + self.scatter.unit() * 0.08,
                self.scatter.unit() * 0.04,
            );
            let wander = bulge * 0.35;
            self.bolt_kinks(from, apex, wander, side, up, when + roll, life, width, 3);
            self.bolt_kinks(apex, to, wander, side, up, when + roll, life, width, 3);
        }
        // Brush discharge off the rail tips: the charge bleeding into the air at the
        // muzzle in short forks, more and longer as it tops out.
        let (_, _, tip_lift) = b.spots[b.spots.len() - 1];
        for rail in [-1.0, 1.0] {
            if self.scatter.unit() > 0.25 + 0.6 * f {
                continue;
            }
            let tip = b.on_rail(1.0, rail, tip_lift);
            let reach = b.gap * (1.2 + 3.5 * f) * (0.5 + self.scatter.unit());
            let out = (b.dir * (0.3 + self.scatter.unit())
                + side * rail * self.scatter.unit()
                + up * self.scatter.signed())
            .normalize_or(b.dir);
            let width = (0.2 + f * 0.5) * size;
            let life = 0.05 + self.scatter.unit() * 0.06;
            self.bolt_kinks(
                tip,
                tip + out * reach,
                reach * 0.25,
                side,
                up,
                when,
                life,
                width,
                3,
            );
        }
        // Sparks dripping off the rails, and some snapping across the slot.
        for _ in 0..(1.0 + f * 5.0) as usize {
            let (t, half, lift) =
                b.spots[(self.scatter.unit() * b.spots.len() as f32) as usize % b.spots.len()];
            let rail = if self.scatter.unit() < 0.5 { -1.0 } else { 1.0 };
            let at = b.on_rail(t + self.scatter.signed() * half, rail, lift);
            let vel = if self.scatter.unit() < 0.3 {
                // Across the slot, at the other rail.
                -side * rail * b.gap * (4.0 + 6.0 * f) + up * self.scatter.signed() * 2.0
            } else {
                (side * self.scatter.signed() + up * self.scatter.unit()) * (6.0 + 14.0 * f)
            };
            let kind = if self.scatter.unit() < 0.5 {
                PUFF_SPARK
            } else {
                PUFF_BOLT
            };
            let roll = self.scatter.unit();
            self.push_puff(kind, at, vel, when, 0.3 + roll * 0.5, (0.5 * scale, 0.1));
        }
    }

    /// The air round the muzzle condensing into a ring as the field builds: a torus of
    /// vapour (the seamless tube puffs.wgsl `strategic_trail` draws), hanging until the
    /// shot blows through it.
    fn condense_ring(&mut self, b: &Barrel, when: f32, life: f32, scale: f32) {
        let r = (b.length() * 0.09).clamp(6.0, 24.0) * scale;
        let centre = b.muzzle + b.dir * r * 0.3;
        let n = 14;
        for k in 0..n {
            let a = k as f32 / n as f32 * std::f32::consts::TAU;
            let out = b.side * a.cos() + b.up * a.sin();
            let along = -b.side * a.sin() + b.up * a.cos();
            let half = r * std::f32::consts::PI / n as f32;
            self.push_puff(
                PUFF_STRATEGIC_TRAIL,
                centre + out * r,
                along * half,
                when,
                life,
                (r * 0.5, r * 2.2),
            );
        }
    }

    /// A jagged bolt from `from` to `to`, wandering `wander` metres off the line.
    fn heavy_bolt(
        &mut self,
        from: Vec3,
        to: Vec3,
        wander: f32,
        side: Vec3,
        up: Vec3,
        start: f32,
        life: f32,
        width: f32,
    ) {
        self.bolt_kinks(from, to, wander, side, up, start, life, width, 4);
    }

    /// The same run the length of a gun: more kinks, a fork or two.
    fn heavy_long_bolt(
        &mut self,
        from: Vec3,
        to: Vec3,
        wander: f32,
        side: Vec3,
        up: Vec3,
        start: f32,
        life: f32,
        width: f32,
    ) {
        let kinks = ((from.distance(to) / 12.0) as usize).clamp(6, 18);
        self.bolt_kinks(from, to, wander, side, up, start, life, width, kinks);
        for _ in 0..2 {
            let t = 0.2 + self.scatter.unit() * 0.6;
            let root = from.lerp(to, t);
            let tip =
                root + (side * self.scatter.signed() + up * self.scatter.unit()) * wander * 4.0;
            self.bolt_kinks(
                root,
                tip,
                wander * 0.6,
                side,
                up,
                start,
                life * 0.7,
                width * 0.5,
                3,
            );
        }
    }

    fn bolt_kinks(
        &mut self,
        from: Vec3,
        to: Vec3,
        wander: f32,
        side: Vec3,
        up: Vec3,
        start: f32,
        life: f32,
        width: f32,
        kinks: usize,
    ) {
        let mut last = from;
        for k in 1..=kinks {
            let t = k as f32 / kinks as f32;
            let taper = (t * std::f32::consts::PI).sin().sqrt();
            let jitter = if k == kinks {
                Vec3::ZERO
            } else {
                (side * self.scatter.signed() + up * self.scatter.signed()) * wander * taper
            };
            let next = from.lerp(to, t) + jitter;
            self.heavy_rail.strokes.push(Stroke {
                from: last,
                to: next,
                start,
                life,
                width,
                color: BOLT,
            });
            last = next;
        }
    }

    /// The gun going off: everything at the muzzle the moment the slug leaves.
    fn heavy_muzzle(&mut self, at: Vec3, dir: Vec3, s: f32, time: f32) {
        let side = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
        let up = side.cross(dir).normalize_or(Vec3::Z);
        // The flash: a colossal white bloom, a hard core, and the plug of air ahead of it lit.
        self.push_effect(at.to_array(), time, 60.0 * s, 0.32, RAIL_FLASH, 1.0);
        // For a moment brighter than anything else on the field (sprites.wgsl kind 4).
        self.push_effect(at.to_array(), time, 70.0 * s, 0.2, 4.0, 0.0);
        self.push_effect(
            (at + dir * 4.0 * s).to_array(),
            time,
            26.0 * s,
            0.14,
            RAIL_FLASH,
            0.0,
        );
        self.push_effect(
            (at + dir * 45.0 * s).to_array(),
            time + 0.02,
            34.0 * s,
            0.22,
            RAIL_FLASH,
            0.8,
        );
        // The shock front, out along the bore.
        self.push_shockwave(
            (at + dir * 20.0 * s).to_array(),
            time,
            320.0 * s,
            1.2,
            0.55,
            1.0,
            dir,
        );
        // The shock disc: a ring of vapour standing across the bore (a torus of the
        // seamless tube), thickening as it hangs, and vapour thrown out flat through it.
        let r = 26.0 * s;
        let n = 20;
        for k in 0..n {
            let a = k as f32 / n as f32 * std::f32::consts::TAU;
            let out = side * a.cos() + up * a.sin();
            let along = -side * a.sin() + up * a.cos();
            let half = r * std::f32::consts::PI / n as f32;
            self.push_puff(
                PUFF_STRATEGIC_TRAIL,
                at + dir * 8.0 * s + out * r,
                along * half,
                time,
                6.0,
                (r * 0.45, r * 2.6),
            );
        }
        for k in 0..24 {
            let a = (k as f32 + self.scatter.unit() * 0.6) / 24.0 * std::f32::consts::TAU;
            let out = side * a.cos() + up * a.sin();
            let speed = 170.0 * s * (0.85 + 0.3 * self.scatter.unit());
            let life = 1.4 + self.scatter.unit() * 0.8;
            self.push_puff(
                PUFF_STEAM,
                at + out * 6.0 * s,
                out * speed,
                time,
                life,
                (8.0 * s, 26.0 * s),
            );
        }
        // The vapour cone blown out ahead of the slug.
        for i in 0..18 {
            let t = i as f32 / 17.0;
            let speed = (90.0 + 420.0 * t) * s;
            let jitter = (side * self.scatter.signed() + up * self.scatter.signed()) * speed * 0.08;
            let life = 1.4 + t * 1.8 + self.scatter.unit() * 0.6;
            self.push_puff(
                PUFF_STEAM,
                at + dir * 3.0 * s,
                dir * speed + jitter,
                time + 0.01 * i as f32,
                life,
                (5.0 * s, (14.0 + 18.0 * t) * s),
            );
        }
        // Rail spatter: metal torn off the rails, thrown out fast in a cone, and molten
        // drops that arc down after.
        for _ in 0..70 {
            let spray = (dir * 2.2
                + Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.signed(),
                ) * 0.55)
                .normalize_or_zero();
            let speed = (110.0 + self.scatter.unit() * 230.0) * s;
            let life = 0.25 + self.scatter.unit() * 0.45;
            self.push_puff(PUFF_SPARK, at, spray * speed, time, life, (0.9 * s, 0.2));
        }
        for _ in 0..24 {
            let spray = (dir + (side * self.scatter.signed() + up * self.scatter.signed()) * 0.8)
                .normalize_or_zero();
            let speed = (18.0 + self.scatter.unit() * 45.0) * s;
            let roll = self.scatter.unit();
            self.push_puff(
                PUFF_SPARK,
                at,
                spray * speed,
                time,
                1.2 + roll * 1.0,
                (1.3 * s, 0.35),
            );
        }
        // Arcs snapping off the muzzle as the current breaks.
        for _ in 0..8 {
            let out = (side * self.scatter.signed() + up * self.scatter.signed() + dir * 0.4)
                .normalize_or_zero();
            let to = at + out * (10.0 + self.scatter.unit() * 20.0) * s;
            let roll = self.scatter.unit();
            self.heavy_bolt(at, to, 3.0 * s, side, up, time + roll * 0.05, 0.12, 0.9 * s);
        }
        // Over the sea, the blast presses the water flat and throws spray; near the
        // ground the shockwave raises the dust itself and the flash lights it.
        if let Some(water) = self.at_sea(at, 120.0) {
            let surface = Vec3::new(at.x, at.y, water);
            self.push_ripple(surface + Vec3::Z * 2.0, time, 90.0 * s, 5.0, 3.0, 0.9);
            self.water_splash(surface, time + 0.05, 9.0 * s, 0.6);
        }
        // The clouds round it are shoved about (only if it fires in or over them).
        self.sky.blast(at + dir * 60.0 * s, 200.0 * s, 1.2, time);
    }

    /// The rails of guns that fired last tick: white-hot, cooling to orange, venting.
    fn heavy_fired(&mut self, units: &[UnitInstance], time: f32) {
        for i in 0..self.heavy_rail.fired.len() {
            if self.heavy_rail.fired[i].rails.is_some() {
                continue;
            }
            let (blueprint, weapon, near, unit, fired) = {
                let f = &self.heavy_rail.fired[i];
                (f.blueprint, f.weapon, f.near, f.unit, f.time)
            };
            let Some(row) = self.heavy_find(units, blueprint, near, unit) else {
                // Gone (or not seen): nothing to glow; don't look again.
                self.heavy_rail.fired[i].rails = Some((near, near));
                continue;
            };
            let Some(b) = self.heavy_barrel(&units[row], weapon, 0.5) else {
                continue;
            };
            self.heavy_rail.fired[i].rails = Some((b.breech, b.muzzle));
            let s = self
                .heavy_weapon(BlueprintId(blueprint), weapon)
                .unwrap_or(1.0);
            let outbound = std::mem::replace(&mut self.effect_outbound, true);
            // The open rails glow white-hot and cool to orange (where they are in the open:
            // from the first stretch as high as the last to the muzzle).
            let lift = b.heat_lift;
            let (t0, half0, _) = b
                .spots
                .iter()
                .copied()
                .find(|sp| (sp.2 - lift).abs() < 0.01)
                .unwrap_or(b.spots[0]);
            for rail in [-1.0, 1.0] {
                let (a, z) = (
                    b.on_rail(t0 - half0, rail, lift),
                    b.on_rail(1.0, rail, lift),
                );
                self.heavy_rail.strokes.push(Stroke {
                    from: a,
                    to: z,
                    start: fired,
                    life: 4.0,
                    width: b.gap * 0.8,
                    color: CHANNEL,
                });
            }
            // The breech vents behind.
            let (side, up) = (b.side, b.up);
            for i in 0..10 {
                let jitter = (side * self.scatter.signed() + up * self.scatter.signed()) * 12.0 * s;
                let speed = (40.0 + 16.0 * i as f32) * s;
                let roll = self.scatter.unit();
                self.push_puff(
                    PUFF_STEAM,
                    b.breech,
                    -b.dir * speed + jitter,
                    fired + 0.02 * i as f32,
                    1.6 + roll,
                    (3.0 * s, 12.0 * s),
                );
            }
            self.push_effect(b.breech.to_array(), fired, 18.0 * s, 0.2, RAIL_FLASH, 0.5);
            // Heat shimmer off the rails as they cool: vapour rising for a few seconds.
            for _ in 0..18 {
                let at = b.on_rail(
                    t0 + self.scatter.unit() * (1.0 - t0),
                    if self.scatter.unit() < 0.5 { -1.0 } else { 1.0 },
                    lift,
                );
                let when = fired + 0.2 + self.scatter.unit() * 2.5;
                let roll = self.scatter.unit();
                self.push_puff(
                    PUFF_STEAM,
                    at,
                    Vec3::Z * (3.0 + roll * 3.0),
                    when,
                    2.0,
                    (1.5 * s, 7.0 * s),
                );
            }
            self.effect_outbound = outbound;
            let _ = time;
        }
    }

    /// The stretch slug `i` flew this tick: the channel, the vapour tube, collars, and what
    /// it does to the cloud and the ground it passes.
    fn heavy_path(&mut self, i: usize, p: &ProjectileInstance, time: f32) {
        let (prev, pos) = (Vec3::from(p.prev_pos), Vec3::from(p.pos));
        // As sprites.wgsl flies it: over part of the tick on its last stretch, and from part
        // of the way along for a shot that leaves during the tick (`rail_wakes`).
        let ends = ((p.color >> PROJECTILE_ENDS_SHIFT) & 0xFF) as f32 / 255.0;
        let span = if ends > 0.0 { ends } else { 1.0 };
        let s0 = (((p.color >> PROJECTILE_STARTS_SHIFT) & 0xFF) as f32 / 255.0 / span).min(1.0);
        let tick = self.tick_seconds;
        let shot = self.heavy_rail.shots[i];
        // From where the channel was laid to, so it is one line from the muzzle.
        let from_along = shot.laid;
        let to_along = (pos - shot.muzzle).dot(shot.dir).max(from_along);
        let t_from = time + s0 * span * tick;
        let t_to = time + span * tick;
        let _ = prev;
        self.lay_channel(i, from_along, to_along, t_from, t_to);
        let s = &mut self.heavy_rail.shots[i];
        s.laid_at = time;
        if ends > 0.0 {
            s.ended = Some(t_to);
        }
    }

    /// The channel from `a` to `b` metres along shot `i`, the slug passing them at `ta`, `tb`.
    fn lay_channel(&mut self, i: usize, a: f32, b: f32, ta: f32, tb: f32) {
        let shot = self.heavy_rail.shots[i];
        let length = b - a;
        if length < 1.0 {
            return;
        }
        let at = |d: f32| shot.muzzle + shot.dir * d;
        let when = |d: f32| ta + (tb - ta) * ((d - a) / length);
        let s = shot.scale;
        let (side, up) = {
            let side = shot.dir.cross(Vec3::Z).normalize_or(Vec3::Y);
            (side, side.cross(shot.dir).normalize_or(Vec3::Z))
        };
        let outbound = std::mem::replace(&mut self.effect_outbound, true);
        // The channel: a thick core that cools over seconds, a broad sheath that cools
        // sooner, and the white flash of the slug passing.
        let pieces = ((length / 30.0).ceil() as usize).max(1);
        for k in 0..pieces {
            let (d0, d1) = (
                a + length * k as f32 / pieces as f32,
                a + length * (k + 1) as f32 / pieces as f32,
            );
            let (p0, p1) = (at(d0), at(d1));
            let start = when(d1);
            self.heavy_rail.strokes.push(Stroke {
                from: p0,
                to: p1,
                start,
                life: CHANNEL_LIFE,
                width: 2.0 * s,
                color: CHANNEL,
            });
            self.heavy_rail.strokes.push(Stroke {
                from: p0,
                to: p1,
                start,
                life: 0.9,
                width: 6.0 * s,
                color: CHANNEL,
            });
            self.heavy_rail.strokes.push(Stroke {
                from: p0,
                to: p1,
                start,
                life: 0.3,
                width: 6.0 * s,
                color: FLASH,
            });
            // The slug itself, a white-hot knot tearing along the front of it.
            if k % 2 == 1 {
                self.push_effect(p1.to_array(), start, 11.0 * s, 0.07, RAIL_FLASH, 0.0);
            }
        }
        // The vapour tube, seamless (puffs.wgsl `strategic_trail`), hanging and drifting.
        let step = 24.0;
        let mut d = (a / step).ceil() * step;
        while d <= b {
            let p = at(d);
            if p.z > self.ground_height(p.truncate()) + 1.0 {
                self.push_puff(
                    PUFF_STRATEGIC_TRAIL,
                    p,
                    shot.dir * step,
                    when(d),
                    TUBE_LIFE,
                    (16.0 * s, 100.0 * s),
                );
            }
            d += step;
        }
        // Vapour collars: the shock standing off the slug, left behind as rings.
        let mut next = self.heavy_rail.shots[i].next_collar;
        while next <= b {
            if next >= a {
                let centre = at(next);
                let start = when(next);
                for k in 0..10 {
                    let ang = (k as f32 + self.scatter.unit() * 0.5) / 10.0 * std::f32::consts::TAU;
                    let out = side * ang.cos() + up * ang.sin();
                    let life = 1.8 + self.scatter.unit() * 0.9;
                    self.push_puff(
                        PUFF_STEAM,
                        centre + out * 3.0 * s,
                        out * 45.0 * s + shot.dir * 25.0,
                        start,
                        life,
                        (3.0 * s, 13.0 * s),
                    );
                }
                self.push_effect(centre.to_array(), start, 14.0 * s, 0.12, RAIL_FLASH, 0.9);
            }
            next += COLLAR_STEP;
        }
        self.heavy_rail.shots[i].next_collar = next;
        // Where it runs low: the ground under it thrown up in its wake, or the sea.
        let mut d = (a / 40.0).ceil() * 40.0;
        let wind = self.sky.wind_heading();
        while d <= b {
            let p = at(d);
            let ground = self.ground_height(p.truncate());
            let h = p.z - ground;
            if h < 80.0 && d > 30.0 {
                let press = 1.0 - h / 80.0;
                let start = when(d) + 0.05;
                if let Some(water) = self.at_sea(p, 80.0) {
                    let surface = Vec3::new(p.x, p.y, water);
                    self.push_ripple(surface, start, 22.0 * s * press, 3.5, 0.0, 0.8 * press);
                    for side_k in [-1.0, 1.0] {
                        let out = (side * side_k).with_z(0.0).normalize_or_zero();
                        self.push_puff(
                            super::water_fx::PUFF_SPRAY,
                            surface + Vec3::Z,
                            out * 18.0 * press + Vec3::Z * 6.0,
                            start,
                            2.2,
                            (3.0, 12.0 * press),
                        );
                    }
                } else {
                    for side_k in [-1.0, 1.0] {
                        let out = (side * side_k).with_z(0.0).normalize_or_zero();
                        let foot = Vec3::new(p.x, p.y, ground + 1.0);
                        let vel = out * (22.0 + 10.0 * self.scatter.unit()) * press
                            + Vec3::Z * 4.0
                            + (wind * 2.0).extend(0.0);
                        self.push_puff(
                            PUFF_SHOCK_DUST,
                            foot,
                            vel,
                            start,
                            2.8,
                            (4.0 * press + 1.0, 16.0 * press + 4.0),
                        );
                    }
                }
            }
            d += 40.0;
        }
        // Through the cloud layer: the cloud torn along the way.
        let mut d = a.max(self.heavy_rail.shots[i].next_cloud);
        while d <= b && self.heavy_rail.shots[i].clouds < 10 {
            let p = at(d);
            let base = self.sky.cloud_base_at(p.truncate());
            if p.z > base - 20.0 && p.z < base + CLOUD_DECK + 40.0 {
                self.sky.blast(p, 70.0 * s, 1.0, when(d));
                self.heavy_rail.shots[i].clouds += 1;
                d += 90.0;
            } else {
                d += 20.0;
            }
        }
        self.heavy_rail.shots[i].next_cloud = d;
        self.effect_outbound = outbound;
        let shot = &mut self.heavy_rail.shots[i];
        shot.laid = b;
    }

    /// The channel laid on to where the slug struck; its direction, for the impact.
    fn finish_channel(&mut self, at: Vec3, start: f32) -> Vec3 {
        let found = self
            .heavy_rail
            .shots
            .iter()
            .enumerate()
            .rev()
            .find(|(_, s)| {
                let rel = at - s.muzzle;
                let along = rel.dot(s.dir);
                (rel - s.dir * along).length() < 25.0 + along * 0.03
                    && along > -10.0
                    && s.laid <= along + 60.0
            });
        let Some((i, shot)) = found.map(|(i, s)| (i, *s)) else {
            return Vec3::NEG_Z;
        };
        let along = (at - shot.muzzle).dot(shot.dir);
        if along > shot.laid {
            let ta = shot.fired + shot.laid / shot.speed.max(1.0);
            self.lay_channel(i, shot.laid, along, ta.min(start), start);
        }
        self.heavy_rail.shots[i].ended = Some(start);
        shot.dir
    }

    /// The slug striking at `at`, flying along `dir`.
    fn heavy_impact(
        &mut self,
        at: Vec3,
        dir: Vec3,
        motion: Vec3,
        on_unit: bool,
        s: f32,
        start: f32,
    ) {
        let ground = self.ground_height(at.truncate());
        let airborne = at.z - ground > 25.0;
        // On a hull the burst is on the face the slug came in through, not buried in it.
        let at = if on_unit { at - dir * 8.0 * s } else { at };
        // The flash and the shock, bigger than anything else a gun does.
        self.push_effect(at.to_array(), start, 75.0 * s, 0.45, RAIL_FLASH, 1.0);
        self.push_effect(at.to_array(), start, 30.0 * s, 0.16, RAIL_FLASH, 0.0);
        self.push_effect(at.to_array(), start, 50.0 * s, 0.2, 4.0, 0.0);
        self.push_shockwave(at.to_array(), start, 280.0 * s, 1.5, 0.7, 1.0, Vec3::ZERO);
        self.sky.blast(at, 260.0 * s, 1.5, start);
        let side = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
        let up = side.cross(dir).normalize_or(Vec3::Z);
        if on_unit || airborne {
            // Into a hull: a burst of sparks, the far side spalling out along the slug's
            // line, fragments tumbling away and smoke carried off with the target.
            let carry = motion / self.tick_seconds.max(0.02) * 0.6;
            for _ in 0..90 {
                let spray = (Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.signed(),
                ) - dir * 0.3)
                    .normalize_or_zero();
                let speed = (40.0 + self.scatter.unit() * 150.0) * s;
                let roll = self.scatter.unit();
                self.push_puff(
                    PUFF_SPARK,
                    at,
                    spray * speed + carry,
                    start,
                    0.4 + roll * 0.9,
                    (2.4 * s, 0.4),
                );
            }
            for _ in 0..50 {
                let spray = (dir * 2.0
                    + (side * self.scatter.signed() + up * self.scatter.signed()) * 0.7)
                    .normalize_or_zero();
                let speed = (160.0 + self.scatter.unit() * 260.0) * s;
                let roll = self.scatter.unit();
                self.push_puff(
                    PUFF_SPARK,
                    at,
                    spray * speed + carry,
                    start,
                    0.3 + roll * 0.5,
                    (3.0 * s, 0.5),
                );
            }
            for _ in 0..30 {
                let spray = (Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.signed() * 0.6 + 0.2,
                ) + dir * 0.6)
                    .normalize_or_zero();
                let speed = (20.0 + self.scatter.unit() * 50.0) * s;
                let roll = self.scatter.unit();
                self.push_puff(
                    PUFF_SHARD,
                    at,
                    spray * speed + carry,
                    start,
                    2.5 + roll,
                    (3.0 * s, 1.0),
                );
            }
            for i in 0..8 {
                let out = (dir * 0.8
                    + Vec3::new(
                        self.scatter.signed(),
                        self.scatter.signed(),
                        self.scatter.signed(),
                    ) * 0.5)
                    .normalize_or_zero();
                self.push_puff(
                    PUFF_STEAM,
                    at,
                    out * (60.0 + 30.0 * i as f32) * s + carry,
                    start + 0.02 * i as f32,
                    2.0,
                    (6.0 * s, 26.0 * s),
                );
            }
            // What was in the way goes up: a short burst of fire out of the hole.
            for i in 0..7 {
                let out = (Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.signed(),
                ) - dir * 0.8)
                    .normalize_or_zero();
                let roll = self.scatter.unit();
                self.push_puff(
                    PUFF_FIREBALL,
                    at + out * 6.0 * s,
                    out * (10.0 + 14.0 * roll) * s + carry,
                    start + 0.03 * i as f32,
                    0.9 + 0.4 * roll,
                    (14.0 * s, 40.0 * s),
                );
            }
            for i in 0..6 {
                let out = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.3)
                    .normalize_or_zero();
                self.push_puff(
                    PUFF_SMOKE,
                    at + out * 6.0,
                    out * 6.0 + carry * 0.5,
                    start + 0.1 + 0.05 * i as f32,
                    4.0,
                    (10.0 * s, 30.0 * s),
                );
            }
            if airborne {
                return;
            }
        }
        if let Some(water) = self.at_sea(at, 4.0) {
            // Into the sea: a column of white water, a ring running out, steam off the slug.
            let surface = Vec3::new(at.x, at.y, water);
            self.water_splash(surface, start, 16.0 * s, 1.8);
            self.push_ripple(surface + Vec3::Z * 3.0, start, 110.0 * s, 7.0, 3.0, 1.0);
            for i in 0..12 {
                let out = Vec3::new(self.scatter.signed(), self.scatter.signed(), 1.5)
                    .normalize_or_zero();
                self.push_puff(
                    PUFF_STEAM,
                    surface,
                    out * (20.0 + 8.0 * i as f32) * s,
                    start + 0.03 * i as f32,
                    4.0,
                    (8.0 * s, 34.0 * s),
                );
            }
            return;
        }
        // Into the ground: a white disc of the blast on it, a crater, earth and dust thrown
        // up and out, then smoke trailing off downwind for half a minute.
        self.push_effect(at.to_array(), start, 95.0 * s, 0.8, 5.0, RAIL_FLASH);
        self.add_crater_styled(at.truncate(), 46.0 * s, 0.4, start, CraterStyle::Blast);
        for _ in 0..70 {
            let vel = (self.scatter.upward(0.35) - dir.with_z(0.0) * 0.4).normalize_or_zero()
                * (35.0 + self.scatter.unit() * 90.0)
                * s;
            let size = (0.8 + self.scatter.unit() * 1.2) * s;
            let roll = self.scatter.unit();
            self.push_puff(
                PUFF_CLOD,
                at + Vec3::Z,
                vel,
                start,
                2.2 + roll * 2.0,
                (size, 0.3),
            );
        }
        for _ in 0..40 {
            let vel = self.scatter.upward(0.25) * (40.0 + self.scatter.unit() * 90.0) * s;
            let roll = self.scatter.unit();
            self.push_puff(
                PUFF_SPARK,
                at + Vec3::Z,
                vel,
                start,
                0.5 + roll * 0.6,
                (1.1 * s, 0.25),
            );
        }
        // Earth thrown up in jets, and the base surge rolling out.
        for i in 0..12 {
            let vel = (Vec3::Z * 2.5
                + Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0))
            .normalize_or_zero()
                * (35.0 + self.scatter.unit() * 45.0)
                * s;
            let roll = self.scatter.unit();
            self.push_puff(
                PUFF_DUST,
                at + Vec3::Z * 2.0,
                vel,
                start + 0.02 * i as f32,
                3.5 + roll * 2.0,
                (7.0 * s, 30.0 * s),
            );
        }
        for k in 0..22 {
            let a = (k as f32 + self.scatter.unit()) / 22.0 * std::f32::consts::TAU;
            let out = Vec3::new(a.cos(), a.sin(), 0.0);
            let vel = out * (45.0 + self.scatter.unit() * 30.0) * s + Vec3::Z * 3.0;
            let roll = self.scatter.unit();
            self.push_puff(
                PUFF_DUST,
                at + out * 8.0 + Vec3::Z * 2.0,
                vel,
                start + 0.08,
                4.5 + roll * 2.0,
                (8.0 * s, 32.0 * s),
            );
        }
        let wind: Vec2 = self.sky.wind_heading();
        for i in 0..10 {
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 20.0 * s;
            let carry =
                (wind * (3.0 + self.scatter.unit() * 2.5)).extend(3.0 + self.scatter.unit() * 3.0);
            let life = 24.0 + self.scatter.unit() * 14.0;
            self.push_puff(
                PUFF_TREE_SMOKE,
                at + off + Vec3::Z * 4.0,
                carry,
                start + 0.6 + 0.35 * i as f32,
                life,
                (12.0 * s, 55.0 * s),
            );
        }
    }
}
