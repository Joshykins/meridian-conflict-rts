//! Capital ships' drives and lamps (`UnitBlueprint::is_capital_ship`: the Bastion and
//! the spacecraft to come). Stern engines at `models::aircraft_exhausts`, belly lift jets
//! at `models::lift_jets`, lamps at `models::capital_lamps`, all by mesh, with every size
//! scaled by the hull's radius against the Bastion's (160 m).
//!
//! Nothing here comes from the sim but where the ship is. Its state is read from
//! its motion: height over the ground or sea, climb and sink rate, speed and how
//! hard it is changing, and the ramp closing on the ground (it only closes when
//! the ship is about to leave). From that:
//!
//! - The stern engines burn whenever the ship is off the ground, a short faint
//!   burn hanging still and a long white-hot plume with shock diamonds under
//!   throttle. They wind down over a couple of seconds after touchdown and light
//!   while the ramp closes for take-off. Their light falls on the hull and, low
//!   down, on the ground behind.
//! - The lift jets fire straight down below ~190 m, hardest in the last 40 m and
//!   while sinking or climbing. Where each one meets the ground: a hot glow and a
//!   light, dust (or spray) rolling out in a ring, and the trees pushed over.
//!   Touchdown and lift-off each throw one big ring and a pressure front.
//! - Low down, the stern plumes also blow dust off the ground behind the ship.
//! - Lamps: nav lights and strobes always, landing floods from 250 m down that search
//!   the ground and settle on the pad (beams seen through the air at night), amber
//!   beacons while the ramp moves, the hold lit down the open ramp.
//!
//! Emitters are capped per ship per tick (about 80 puffs low over the ground, 50 more
//! on the touchdown or lift-off tick; a dozen lights), and ships far from the camera
//! draw only the plumes and lamps.

use super::water_fx::{PUFF_DROPLET, PUFF_SPRAY};
use super::{Puff, Renderer, PUFF_CLOUD_WISP, PUFF_RING, PUFF_SHOCK_DUST};
use crate::models::CapitalLamps;
use glam::{Vec2, Vec3};
use mc_sim::mirror::UnitInstance;
use std::collections::HashMap;
use std::mem::size_of;

/// A drive plume: a ribbon from the nozzle along `vel` (axis times length), carried
/// with the ship (`appearance.xyz`), `appearance.w` its heat (0 idle to 1 full).
pub(super) const PUFF_THRUST: f32 = 30.0;
/// A soft drive glow carried with the ship: a nozzle mouth, or where a lift jet
/// meets the ground (`appearance.w` below zero: heated ground, warmer at the rim).
pub(super) const PUFF_THRUST_GLOW: f32 = 31.0;

/// Radius (m) of the hull every size here is tuned for: the Bastion's.
const REFERENCE_RADIUS: f32 = 160.0;
/// Height of a lift jet mouth over the ground where the jets begin to reach it (reference hull).
const JET_REACH: f32 = 190.0;

/// What a capital ship's hull carries for these effects, looked up by mesh.
#[derive(Clone, Copy)]
struct Kit {
    /// Stern engine mouths and belly lift jets, model space (+X forward, +Y left).
    nozzles: &'static [[f32; 3]],
    lift_jets: &'static [[f32; 3]],
    lamps: Option<&'static CapitalLamps>,
    /// Size against the reference hull.
    k: f32,
    /// Full cruise speed, m/s.
    cruise: f32,
    /// Half length and half width of the hull on the ground, metres.
    half: (f32, f32),
}
/// Base colour of the dust the wash throws up (linear RGB, shaded by the puff).
const WASH_DUST: [f32; 3] = [0.43, 0.40, 0.35];
/// Seconds between pushes on the trees under a low ship.
const GUST_EVERY: f32 = 0.45;

#[derive(Clone, Copy)]
struct Light {
    from: [Vec3; 2],
    to: [Vec3; 2],
    color: Vec3,
    range: f32,
}

#[derive(Default)]
struct Ship {
    /// Velocity last tick, m/s.
    vel: Vec3,
    /// Smoothed 0..1: how hard the stern engines are pushing.
    throttle: f32,
    /// Smoothed 0..1: stern engines alight at all.
    burn: f32,
    /// Smoothed 0..1: lift jets.
    lift: f32,
    /// Render time of the tick this ship was last seen at.
    seen: f32,
    /// When the trees under it were last pushed.
    gust: f32,
    lights: Vec<Light>,
    /// Where its lamps are this tick (`capital_lights` places them each frame).
    lamps: Option<Lamps>,
}

/// The ship's frame: origin, forward, left, up (world).
type Frame = (Vec3, Vec3, Vec3, Vec3);

fn place(f: &Frame, p: [f32; 3]) -> Vec3 {
    f.0 + f.1 * p[0] + f.2 * p[1] + f.3 * p[2]
}

/// A ship's lamps as of the last tick.
#[derive(Clone, Copy)]
struct Lamps {
    /// The frame at the start and the end of the tick.
    frame: [Frame; 2],
    /// Metres over the ground or sea, and that surface's height.
    height: f32,
    surface: f32,
    /// The ramp, last tick and this (0 shut, 1 down).
    ramp: [f32; 2],
    landed: bool,
    seed: f32,
    fittings: &'static CapitalLamps,
    k: f32,
}

/// Below this height (m) the landing lights are on.
const FLOODS_ON: f32 = 250.0;
const PUFF_LAMP: f32 = 32.0;
const PUFF_LAMP_CONE: f32 = 33.0;
/// Seconds from one strobe double-flash to the next (puffs.wgsl `lamp_flare` agrees).
const STROBE_PERIOD: f32 = 1.3;
/// Beacon turn rate, radians a second (puffs.wgsl `lamp_flare` agrees).
const BEACON_TURN: f32 = 7.0;

#[derive(Default)]
pub(super) struct CapitalFx {
    ships: HashMap<u32, Ship>,
}

fn approach(from: f32, to: f32, up: f32, down: f32) -> f32 {
    from + (to - from) * if to > from { up } else { down }
}

impl Renderer {
    /// A capital ship's drives and lamps this tick (called from `aircraft_trails`).
    pub(super) fn capital_drives(&mut self, u: &UnitInstance, time: f32, focus: Vec2) {
        let bp = self.blueprints.unit(mc_data::BlueprintId(u.blueprint as u16));
        let mesh = bp.visual.mesh.as_str();
        let radius = bp.radius.to_f32().max(1.0);
        let hull = (bp.hull.0.to_f32(), bp.hull.1.to_f32());
        let kit = Kit {
            nozzles: crate::models::aircraft_exhausts(mesh),
            lift_jets: crate::models::lift_jets(mesh),
            lamps: crate::models::capital_lamps(mesh),
            k: radius / REFERENCE_RADIUS,
            cruise: bp.motion.map_or(78.0, |m| m.speed.to_f32()).max(1.0),
            half: if hull.0 > 1.0 { (hull.0 * 0.9, hull.1 * 0.9) } else { (radius, radius * 0.45) },
        };
        // Ground torn up by the wash is earthier than a blast's grey pressure dust.
        let saved = (self.effect_origin, self.effect_settings);
        self.effect_origin = Some(Vec3::from(u.pos));
        self.effect_settings = mc_data::EffectSettings { dust_color: Some(WASH_DUST), ..saved.1 };
        self.drives(u, &kit, time, focus);
        (self.effect_origin, self.effect_settings) = saved;
    }

    fn drives(&mut self, u: &UnitInstance, kit: &Kit, time: f32, focus: Vec2) {
        let k = kit.k;
        let nozzles = kit.nozzles;
        let dt = self.tick_seconds.max(0.02);
        let from = Vec3::from(u.prev_pos);
        let to = Vec3::from(u.pos);
        let travel = to - from;
        if travel.length() > 60.0 {
            // A jump (spawned, or the view restaged): nothing to go on this tick.
            self.capital_fx.ships.remove(&u.unit_id);
            return;
        }
        let vel = travel / dt;
        let water = self.map_info.water_level.to_f32();
        let ground_at = |r: &Renderer, p: Vec3| r.ground_height(p.truncate());
        let (ground, ground_before) = (ground_at(self, to), ground_at(self, from));
        let surface = ground.max(water);
        let height = to.z - surface;
        let height_before = from.z - ground_before.max(water);
        let wet = ground < water - 0.2;
        let landed = height <= 1.0;
        let touched = landed && height_before > 1.0;
        let lifted = !landed && height_before <= 1.0;
        // The ramp only swings shut on the ground when the ship is about to leave.
        let spool = if landed && u.deploy < u.prev_deploy - 1e-4 { 1.0 - u.deploy } else { 0.0 };

        let fx = &mut self.capital_fx;
        if fx.ships.len() > 64 {
            fx.ships.retain(|_, s| time - s.seen < 10.0);
        }
        let fresh = !fx.ships.contains_key(&u.unit_id);
        let ship = fx.ships.entry(u.unit_id).or_default();
        let accel = if fresh { 0.0 } else { (vel - ship.vel).length() / dt };
        ship.vel = vel;
        ship.seen = time;
        let speed = vel.truncate().length();
        let climb = vel.z;
        let push = (speed / kit.cruise * 0.75 + accel / 9.0 * 0.55 + climb.abs() / 30.0 * 0.25).clamp(0.0, 1.0);
        let burn_goal = if !landed {
            1.0
        } else if spool > 0.0 {
            0.25 + 0.5 * spool
        } else {
            0.0
        };
        let closeness = (1.0 - (height - 40.0 * k) / ((JET_REACH - 40.0) * k)).clamp(0.0, 1.0);
        let lift_goal = if landed {
            if spool > 0.0 { 0.35 + 0.65 * spool } else { 0.0 }
        } else {
            let moving = (climb.abs() / 14.0).min(1.0);
            (closeness * (0.6 + 0.4 * moving)).max(if climb.abs() > 3.0 { 0.3 } else { 0.0 })
        };
        if fresh {
            ship.burn = burn_goal;
            ship.lift = lift_goal;
            ship.throttle = push;
        } else {
            // Light fast, wind down over a couple of seconds.
            ship.burn = approach(ship.burn, burn_goal, 0.5, 0.07);
            ship.lift = approach(ship.lift, lift_goal, 0.45, 0.09);
            ship.throttle = approach(ship.throttle, push, 0.3, 0.12);
        }
        let (burn, lift, throttle) = (ship.burn, ship.lift, ship.throttle);
        let gust_due = time - ship.gust >= GUST_EVERY;
        ship.lights.clear();
        // The ship's frame at both ends of the tick, as `aircraft_trails` builds it.
        let frame = |k: f32| {
            let turn = (u.heading - u.prev_heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            let yaw = u.prev_heading + turn * k;
            let bank = u._pad2[0] + (u._pad2[1] - u._pad2[0]) * k;
            let pitch = u.arm_pitch[0] + (u.arm_pitch[1] - u.arm_pitch[0]) * k;
            let horizontal = Vec3::new(yaw.cos(), yaw.sin(), 0.0);
            let forward = horizontal * pitch.cos() + Vec3::Z * pitch.sin();
            let up = Vec3::Z * pitch.cos() - horizontal * pitch.sin();
            let left = Vec3::new(-yaw.sin(), yaw.cos(), 0.0);
            let rolled_left = left * bank.cos() + up * bank.sin();
            let rolled_up = up * bank.cos() - left * bank.sin();
            (from.lerp(to, k), forward, rolled_left, rolled_up)
        };
        let (f0, f1) = (frame(0.0), frame(1.0));
        let near = to.truncate().distance(focus) < 4000.0;
        ship.lamps = kit.lamps.map(|fittings| Lamps {
            frame: [f0, f1],
            height,
            surface,
            ramp: [u.prev_deploy, u.deploy],
            landed,
            seed: (u.unit_id % 97) as f32 * 0.13,
            fittings,
            k,
        });
        if near {
            self.capital_flares(u.unit_id, &f0, height, landed, [u.prev_deploy, u.deploy], vel, time);
        }
        if burn < 0.02 && lift < 0.02 && !touched {
            return;
        }


        // Stern engines: one ribbon per nozzle per slice of the tick, so a turn bends
        // the plume, and a glow at each mouth.
        if burn >= 0.02 {
            let heat = (0.18 + 0.82 * throttle) * burn;
            let reach = (38.0 + 120.0 * throttle) * (0.35 + 0.65 * burn) * k;
            let slices = (travel.length() / 4.0).ceil().clamp(1.0, 6.0) as usize;
            for i in 0..slices {
                let k = (i as f32 + 0.5) / slices as f32;
                let f = frame(k);
                let start = time + k * dt;
                for &port in nozzles {
                    let at = place(&f, port) - f.1 * 1.0;
                    // A little wander, so the four plumes are not ruled lines.
                    let sway = f.2 * self.scatter.signed() * 0.012 + f.3 * self.scatter.signed() * 0.012;
                    let axis = (-f.1 + sway).normalize_or_zero();
                    // The tip flickers: each ribbon a little longer or shorter.
                    let length = reach * (0.9 + 0.2 * self.scatter.unit());
                    self.push_drive(PUFF_THRUST, at, axis * length, start, dt * 1.5 / slices as f32,
                        (8.2 * k, 8.2 * k), vel, heat);
                }
            }
            if speed > 1.0 && !landed {
                // Moisture torn off where the plumes cross cloud (the shader keeps
                // only what is born in cloud); it hangs on the breeze behind.
                let breeze = (self.sky.wind_heading() * 10.0).extend(1.8);
                for &port in nozzles {
                    let at = place(&f0, port) + f0.3 * 9.0 * k;
                    self.push_puff(PUFF_CLOUD_WISP, at, vel * 0.22 + breeze, time, 5.5, (9.0 * k, 32.0 * k));
                }
            }
            for &port in nozzles {
                let at = place(&f0, port) - f0.1 * 2.5;
                let size = (13.0 + 9.0 * heat) * (0.4 + 0.6 * burn) * k;
                self.push_drive(PUFF_THRUST_GLOW, at, Vec3::ZERO, time, dt * 1.6, (size, size * 1.1), vel, heat);
            }
            // Light off the plumes: each pair's, from the mouths down the flame.
            let color = Vec3::new(0.45, 0.68, 1.0) * (1500.0 + 4000.0 * throttle) * burn * k * k;
            // One light per side (port nozzles, starboard nozzles), not one per nozzle.
            for port_side in [true, false] {
                let side: Vec<[f32; 3]> = nozzles.iter().copied().filter(|p| (p[1] >= 0.0) == port_side).collect();
                if side.is_empty() {
                    continue;
                }
                let mouth = |f: &Frame| side.iter().map(|&p| place(f, p)).sum::<Vec3>() / side.len() as f32;
                let (m0, m1) = (mouth(&f0), mouth(&f1));
                ship_light(&mut self.capital_fx, u.unit_id, Light {
                    from: [m0 - f0.1 * 4.0 * k, m1 - f1.1 * 4.0 * k],
                    to: [m0 - f0.1 * reach * 0.55, m1 - f1.1 * reach * 0.55],
                    color,
                    range: (110.0 + 60.0 * throttle) * k,
                });
            }
        }
        if !near {
            return;
        }

        // Stern plumes low over the ground: dust blown off it behind the ship.
        let (stern_x, stern_z, stern_y) = nozzles.iter().fold((0.0f32, 0.0f32, 0.0f32), |a, p| {
            (a.0.min(p[0]), a.1 + p[2] / nozzles.len() as f32, a.2 + p[1].abs() / nozzles.len() as f32)
        });
        let stern_gap = height + stern_z;
        let wash = (1.0 - (stern_gap - 50.0 * k) / (70.0 * k)).clamp(0.0, 1.0) * burn * (0.35 + 0.65 * throttle);
        if wash > 0.05 && !nozzles.is_empty() {
            let aft = -Vec3::new(f1.1.x, f1.1.y, 0.0).normalize_or_zero();
            let side = Vec3::new(-aft.y, aft.x, 0.0);
            for pair in [-stern_y, stern_y] {
                let stern = place(&f1, [stern_x, pair, 0.0]);
                for _ in 0..(1 + (wash * 2.0) as usize) {
                    let back = (20.0 + self.scatter.unit() * (40.0 + 60.0 * wash)) * k;
                    let mut at = stern + aft * back + side * self.scatter.signed() * 18.0 * k;
                    let floor = self.ground_height(at.truncate()).max(water);
                    at.z = floor + 2.0;
                    let vel = (aft * (18.0 + 30.0 * wash) + side * self.scatter.signed() * 5.0) * k + Vec3::Z * 1.5;
                    let kind = if wet { PUFF_SPRAY } else { PUFF_SHOCK_DUST };
                    let life = 2.0 + self.scatter.unit() * 1.2;
                    let start = time + self.scatter.unit() * dt;
                    self.push_puff(kind, at, vel, start, life,
                        ((10.0 + 6.0 * wash) * k, (28.0 + 26.0 * wash) * k));
                }
            }
        }

        // Lift jets: fired straight down, and what they do where they meet the ground.
        let mut strongest = 0.0f32;
        if lift >= 0.02 {
            let heat = lift;
            for &port in kit.lift_jets {
                let mouth = place(&f0, port);
                let down = -f0.3;
                let gap = (mouth.z - surface).max(0.0);
                let length = ((25.0 + 95.0 * lift) * k).min(gap / down.z.abs().max(0.3) + 6.0 * k);
                self.push_drive(PUFF_THRUST, mouth + down * 0.5, down * length, time, dt * 1.5,
                    (8.0 * k, 8.0 * k), vel, heat);
                self.push_drive(PUFF_THRUST_GLOW, mouth + down * 2.0 * k, Vec3::ZERO, time, dt * 1.6,
                    ((9.0 + 5.0 * heat) * k, (11.0 + 6.0 * heat) * k), vel, heat);
                // The ground under the jet.
                let g = (1.0 - gap / (JET_REACH * k)).clamp(0.0, 1.0);
                let g = g * g * lift;
                strongest = strongest.max(g);
                if g < 0.03 {
                    continue;
                }
                let hit = Vec3::new(mouth.x, mouth.y, surface);
                let size = (22.0 + 48.0 * g) * k;
                self.push_drive(PUFF_THRUST_GLOW, hit + Vec3::Z * 3.0, Vec3::ZERO, time, dt * 1.7,
                    (size, size * 1.08), Vec3::new(vel.x, vel.y, 0.0), -g);
                let m1 = place(&f1, port);
                let hit1 = Vec3::new(m1.x, m1.y, surface);
                ship_light(&mut self.capital_fx, u.unit_id, Light {
                    from: [hit + Vec3::Z * 6.0, hit1 + Vec3::Z * 6.0],
                    to: [mouth + down * 6.0, m1 - f1.3 * 6.0],
                    color: Vec3::new(0.5, 0.72, 1.0) * 3500.0 * g * k * k,
                    range: (50.0 + 60.0 * g) * k,
                });
                // Dust (or spray) driven out along the ground in a ring.
                let n = (3.0 + 7.0 * g).round() as usize;
                for _ in 0..n {
                    let a = self.scatter.unit() * std::f32::consts::TAU;
                    let out = Vec3::new(a.cos(), a.sin(), 0.0);
                    let r = (5.0 + 10.0 * g + self.scatter.unit() * 6.0) * k;
                    let at = hit + out * r + Vec3::Z * 2.5;
                    let speed = (22.0 + 62.0 * g + self.scatter.unit() * 14.0) * k;
                    let start = time + self.scatter.unit() * dt;
                    let life = 2.4 + 2.0 * g + self.scatter.unit() * 0.8;
                    if wet {
                        self.push_puff(PUFF_SPRAY, at, out * speed * 0.7 + Vec3::Z * 2.0, start, life,
                            ((5.0 + 5.0 * g) * k, (16.0 + 22.0 * g) * k));
                    } else {
                        self.push_puff(PUFF_SHOCK_DUST, at, out * speed + Vec3::Z * (0.5 + 2.0 * g), start, life,
                            ((8.0 + 8.0 * g) * k, (26.0 + 40.0 * g) * k));
                    }
                }
                if wet && self.scatter.unit() < g {
                    // Water torn off the surface under the jet.
                    for _ in 0..3 {
                        let a = self.scatter.unit() * std::f32::consts::TAU;
                        let out = Vec3::new(a.cos(), a.sin(), 0.0);
                        let vel = out * (14.0 + 20.0 * g) + Vec3::Z * (6.0 + 10.0 * g);
                        self.push_puff(PUFF_DROPLET, hit + out * 8.0 + Vec3::Z, vel, time, 1.6, (2.5, 5.0));
                    }
                }
            }
        }

        // The trees under it lean away from the wash, pushed again every half second.
        if strongest > 0.08 && gust_due {
            if let Some(ship) = self.capital_fx.ships.get_mut(&u.unit_id) {
                ship.gust = time;
            }
            let centre = Vec3::new(to.x, to.y, surface);
            self.tree_blasts.record(centre, time, (50.0 + 70.0 * strongest) * k, 0.35 * strongest, false);
        }

        // Touchdown and lift-off: the whole underside's air let go at once.
        if touched || lifted {
            let centre = Vec3::new(to.x, to.y, surface);
            let big = if touched { 1.0 } else { 0.8 };
            self.tree_blasts.record(centre, time, kit.half.0 * big, 0.6 * big, false);
            let fwd = Vec3::new(f1.1.x, f1.1.y, 0.0).normalize_or_zero();
            let side = Vec3::new(-fwd.y, fwd.x, 0.0);
            for _ in 0..48 {
                // Out from round the hull's outline, a stretched ring.
                let a = self.scatter.unit() * std::f32::consts::TAU;
                let rim = fwd * a.cos() * kit.half.0 + side * a.sin() * kit.half.1;
                let out = (fwd * a.cos() * 0.6 + side * a.sin()).normalize_or_zero();
                let mut at = centre + rim * (0.8 + self.scatter.unit() * 0.35);
                at.z = self.ground_height(at.truncate()).max(water) + 3.0;
                let vel = out * (45.0 + self.scatter.unit() * 45.0) * big * k + Vec3::Z * 3.0;
                let life = 4.0 + self.scatter.unit() * 2.2;
                let start = time + self.scatter.unit() * dt * 0.5;
                if wet {
                    self.push_puff(PUFF_SPRAY, at, vel * 0.7, start, life, (10.0 * k, 36.0 * k));
                } else {
                    self.push_puff(PUFF_SHOCK_DUST, at, vel, start, life, (26.0 * k, 95.0 * big * k));
                }
            }
        }
    }

    /// A drive puff: like `push_puff_with_motion`, with the heat in `appearance.w`.
    #[allow(clippy::too_many_arguments)]
    fn push_drive(&mut self, kind: f32, pos: Vec3, vel: Vec3, start: f32, life: f32, size: (f32, f32),
        motion: Vec3, heat: f32) {
        let origin = self.effect_origin.unwrap_or(pos);
        if self.live_effect_barriers.iter().any(|b| b.crosses(origin, pos)) {
            return;
        }
        let p = Puff {
            appearance: [motion.x, motion.y, motion.z, heat],
            origin: origin.to_array(),
            opacity: 1.0,
            pos: pos.to_array(),
            start,
            vel: vel.to_array(),
            life,
            params: [size.0, size.1, kind, self.scatter.unit()],
        };
        self.puffs.write((self.puff_cursor * size_of::<Puff>()) as u64, bytemuck::bytes_of(&p));
        self.puff_cursor = (self.puff_cursor + 1) % PUFF_RING;
    }

    /// The drives' and the lamps' light this frame, `alpha` of the way through the tick.
    pub(super) fn capital_lights(&mut self, time: f32, alpha: f32) {
        let now = self.last_tick_time;
        let dark = self.sky.darkness();
        let mut lamps = Vec::new();
        for ship in self.capital_fx.ships.values() {
            if now - ship.seen > 0.01 {
                continue;
            }
            for l in &ship.lights {
                let flicker = 0.93 + 0.07 * ((now + alpha * 0.1) * 61.0 + l.range).sin();
                self.lights.beam(
                    l.from[0].lerp(l.from[1], alpha),
                    l.to[0].lerp(l.to[1], alpha),
                    l.color * flicker,
                    l.range,
                );
            }
            lamps.extend(ship.lamps);
        }
        for l in lamps {
            let f = blend(&l.frame, alpha);
            let (fit, k) = (l.fittings, l.k);
            // Landing lights: a white cone each onto the ground under it.
            let on = floods_on(&l);
            if on > 0.0 {
                for (i, &at) in fit.floods.iter().enumerate() {
                    let pos = place(&f, at);
                    let aim = flood_aim(&f, i, fit.floods.len(), l.height, time, l.seed);
                    let reach = flood_reach(pos, aim, l.surface);
                    // A smaller hull carries smaller, dimmer lamps: a narrower cone, so the
                    // pool on the ground stays in proportion to the ship.
                    let color = Vec3::new(1.0, 0.97, 0.9) * 10.0 * (reach + 30.0 * k).powi(2) * on * flood_scale(k);
                    self.lights.lamp(pos, aim, color, (reach * 1.35 + 40.0 * k).min(560.0), 22.0 * flood_scale(k), 0.02);
                }
            }
            if dark > 0.05 {
                let nav = 260.0 * k * k;
                self.lights.lamp(place(&f, fit.nav_port), Vec3::Z, Vec3::new(1.0, 0.06, 0.03) * nav, 24.0 * k, 180.0, 1.0);
                self.lights.lamp(place(&f, fit.nav_starboard), Vec3::Z, Vec3::new(0.06, 1.0, 0.25) * nav, 24.0 * k, 180.0, 1.0);
            }
            if strobe_on(time) {
                for &at in fit.strobes {
                    self.lights.lamp(place(&f, at), Vec3::Z, Vec3::splat(2600.0 * k * k), 75.0 * k, 180.0, 1.0);
                }
            }
            // The ramp (or doors) moving: amber beacons sweep round, lighting the ground about it.
            if (l.ramp[1] - l.ramp[0]).abs() > 1e-4 {
                for (i, &at) in fit.beacons.iter().enumerate() {
                    let a = time * BEACON_TURN + i as f32 * std::f32::consts::PI;
                    let aim = f.1 * a.cos() + f.2 * a.sin() - f.3 * 0.35;
                    self.lights.lamp(place(&f, at), aim, Vec3::new(1.0, 0.5, 0.06) * 9000.0 * k * k, 120.0 * k, 20.0, 0.12);
                }
            }
            // The hold lit, spilling down the open ramp onto the ground.
            let open = l.ramp[0] + (l.ramp[1] - l.ramp[0]) * alpha;
            if let Some((hold, lip_x)) = fit.hold.filter(|_| l.landed && open > 0.05) {
                let lamp = place(&f, hold);
                let mut lip = place(&f, [lip_x - 12.0 * k, 0.0, 0.0]);
                lip.z = l.surface;
                let warm = Vec3::new(1.0, 0.82, 0.58);
                self.lights.lamp(lamp, lip - lamp, warm * 26000.0 * open * k * k, 190.0 * k, 30.0, 0.25);
                let inside = [hold[0] + 20.0 * k, hold[1], hold[2] - 14.0 * k];
                self.lights.lamp(place(&f, inside), Vec3::Z, warm * 2600.0 * open * k * k, 48.0 * k, 180.0, 1.0);
            }
        }
    }

    /// The lamps' flares and light shafts this tick: nav lights, strobes, landing lights
    /// (and their beams through the air at night), beacons while the ramp moves.
    #[allow(clippy::too_many_arguments)]
    fn capital_flares(&mut self, id: u32, f: &Frame, height: f32, landed: bool, ramp: [f32; 2], vel: Vec3, time: f32) {
        let dt = self.tick_seconds.max(0.02);
        let life = dt * 1.6;
        let dark = self.sky.darkness();
        let Some(l) = self.capital_fx.ships.get(&id).and_then(|s| s.lamps) else { return };
        let (fit, k) = (l.fittings, l.k);
        // Flares keep a floor size, so a small hull's lamps still read.
        let flare = |m: f32| (m * k).max(m * 0.4);
        let red = Vec3::new(1.0, 0.05, 0.03) * 7.0;
        let green = Vec3::new(0.05, 1.0, 0.22) * 7.0;
        self.push_drive(PUFF_LAMP, place(f, fit.nav_port), red, time, life, (flare(3.5), flare(3.5)), vel, 0.0);
        self.push_drive(PUFF_LAMP, place(f, fit.nav_starboard), green, time, life, (flare(3.5), flare(3.5)), vel, 0.0);
        for &at in fit.strobes {
            self.push_drive(PUFF_LAMP, place(f, at), Vec3::splat(12.0), time, life, (flare(7.0), flare(7.0)), vel, 1.0);
        }
        if (ramp[1] - ramp[0]).abs() > 1e-4 {
            for (i, &at) in fit.beacons.iter().enumerate() {
                let amber = Vec3::new(1.0, 0.45, 0.04) * 10.0;
                // The shader turns the flare; each beacon half a turn from the other.
                self.push_drive(PUFF_LAMP, place(f, at), amber, time, life, (flare(5.0), flare(5.0)), vel, 2.0 + (i % 2) as f32 * 0.5);
            }
        }
        let on = floods_on(&l);
        if on > 0.0 {
            for (i, &at) in fit.floods.iter().enumerate() {
                let pos = place(f, at);
                self.push_drive(PUFF_LAMP, pos, Vec3::new(1.0, 0.94, 0.82) * 9.0 * on, time, life, (flare(6.0), flare(6.0)), vel, 0.0);
                if dark > 0.1 {
                    // The beam seen through the air, strongest where the wash has raised dust.
                    let aim = flood_aim(f, i, fit.floods.len(), height, time, l.seed);
                    let reach = flood_reach(pos, aim, l.surface);
                    let width = reach * 0.4 * flood_scale(k) + 4.0 * k;
                    let dust = if landed { 0.6 } else { 0.6 + 0.4 * (1.0 - height / FLOODS_ON).clamp(0.0, 1.0) };
                    self.push_drive(PUFF_LAMP_CONE, pos, aim * reach, time, life, (width, width), vel, dark * on * dust);
                }
            }
        }
        // The hold's light down the open ramp, a soft shaft at night.
        if let Some((hold, lip_x)) = fit.hold.filter(|_| landed && ramp[1] > 0.05 && dark > 0.1) {
            let lamp = place(f, hold);
            let mut lip = place(f, [lip_x - 12.0 * k, 0.0, 0.0]);
            lip.z = l.surface;
            self.push_drive(PUFF_LAMP_CONE, lamp, lip - lamp, time, life, (30.0 * k, 30.0 * k), vel, dark * ramp[1] * 0.7);
        }
    }
}

fn blend(frames: &[Frame; 2], k: f32) -> Frame {
    let (a, b) = (&frames[0], &frames[1]);
    (
        a.0.lerp(b.0, k),
        a.1.lerp(b.1, k).normalize_or_zero(),
        a.2.lerp(b.2, k).normalize_or_zero(),
        a.3.lerp(b.3, k).normalize_or_zero(),
    )
}

/// Share of a full-size (Bastion) landing light's cone and brightness for a hull of scale `k`.
fn flood_scale(k: f32) -> f32 {
    k.sqrt().clamp(0.35, 1.0)
}

/// How far the landing lights are on: from 250 m down, dimmed once it sits on the ground.
fn floods_on(l: &Lamps) -> f32 {
    if l.landed {
        0.6
    } else {
        ((FLOODS_ON - l.height) / 50.0).clamp(0.0, 1.0)
    }
}

/// A landing light's aim: searching the ground in slow loops while it is high, settling
/// onto the pad under the hull as it comes down.
fn flood_aim(f: &Frame, i: usize, n: usize, height: f32, time: f32, seed: f32) -> Vec3 {
    let search = ((height - 50.0) / 150.0).clamp(0.0, 1.0);
    let a = time * 0.55 + i as f32 * 1.7 + seed;
    // The first half lean forward, the rest aft.
    let lean = if i < n.div_ceil(2) { 0.18 } else { -0.18 };
    let side = if i % 2 == 0 { 0.12 } else { -0.12 };
    let local = Vec3::new(lean + 0.34 * a.sin() * search, side + 0.3 * (a * 1.3).cos() * search, -1.0);
    (f.1 * local.x + f.2 * local.y + f.3 * local.z).normalize_or_zero()
}

/// How far a light at `pos` aimed along `aim` reaches before the ground stops it.
fn flood_reach(pos: Vec3, aim: Vec3, surface: f32) -> f32 {
    ((pos.z - surface) / (-aim.z).max(0.2)).clamp(5.0, 450.0)
}

/// Whether the strobes are lit at `time`: a double flash every `STROBE_PERIOD`.
fn strobe_on(time: f32) -> bool {
    let ph = (time / STROBE_PERIOD).fract();
    ph < 0.05 || (0.12..0.16).contains(&ph)
}

fn ship_light(fx: &mut CapitalFx, id: u32, light: Light) {
    if let Some(ship) = fx.ships.get_mut(&id) {
        if ship.lights.len() < 8 {
            ship.lights.push(light);
        }
    }
}
