//! A bolt rifle's firing sequence (`Weapon::arc_charge`, the Paladin's twin rifles).
//! ARC (docs/STYLE.md "The electric bore"): the blue is only thin arcs and seams, never a
//! lit block or a white knot.
//!
//! - **Charge** (`SimEvent::WeaponCharging`): small arcs crawl along the three radiator
//!   blades' roots and jump between the blade edges and the core, starting by the housing
//!   and running forward, faster and denser as it builds; their light flickers with them.
//!   In the last part a small crackling knot of arcs curls round the muzzle collar.
//! - **Fire** (`SimEvent::ShotFired`): a short blue-white flash, a few forked strokes
//!   snapping forward out of the muzzle, sparks, ionised haze venting from the housing's
//!   flanks, and the blade seams flaring and cooling. The bolt itself and its burst where
//!   it hits are the ordinary ones (`plasma`, `discharge`).
//!
//! The arcs are laid a tick at a time on the gun as it is drawn then: the hull's place
//! and heading, the torso's yaw and the arm's pitch about the weapon's `pivot`, or, for a
//! gun on a house of its own (the Marlin's), the house's yaw and pitch about it. They are
//! lightning strokes among the bore's (`BoreFx::lightning`).

use super::water_fx::PUFF_STEAM;
use super::{Renderer, PUFF_BOLT, PUFF_SPARK};
use glam::Vec3;
use mc_data::{BlueprintId, Weapon};
use mc_sim::mirror::{HousePose, SimEvent, UnitInstance, KIND_GHOST, KIND_WRECK, UNIT_HOUSE_SHIFT};
use std::f32::consts::{PI, TAU};

/// Guns charging or cooling at once that get the sequence. A deliberate cosmetic cap: a
/// charge past it is not drawn (its shot and its flash still are), so a hundred Paladins
/// firing at once cost no more than this many guns' arcs. A gun has about 60 strokes alive
/// on average through its charge and at most about 130 at its top, so under 3200 in all
/// (of `MAX_PROJECTILES`, 16384, shared with every shot).
const MAX_GUNS: usize = 24;
/// Seconds the blade seams flare and cool after the shot.
const FLARE: f32 = 0.35;
/// The arcs' light: blue-violet, as the lightning is (sprites.wgsl fading-beam colour 3).
const ARC_LIGHT: Vec3 = Vec3::new(0.4, 0.5, 1.0);
/// The gun's layout in shares of its length from the breech (models `bolt_rifle`): the
/// bladed core, the blades' roots and their outer edges (the rear blades stand taller),
/// the collar, and the front of the housing, where it vents clear of the arm.
const BLADES: (f32, f32) = (0.37, 0.84);
const ROOT: f32 = 0.55;
const COLLAR: (f32, f32) = (0.87, 0.98);
/// The collar's radius over the gun's (models `collar`).
const COLLAR_RADIUS: f32 = 0.62;
const PORTS: (f32, f32) = (0.2, 0.33);
/// The blades stand up, and down either side (models `finned_core`), radians about the bore.
const BLADE_ANGLES: [f32; 3] = [0.0, 2.1, -2.1];
/// The gun's radius over its length (models: `r` 0.44 on a 4.1 gun).
const RADIUS: f32 = 0.107;

/// One gun in its sequence.
struct Gun {
    unit: u32,
    blueprint: u16,
    weapon: u8,
    /// The charge, start to end; `end` is cut short when the shot comes early.
    start: f32,
    end: f32,
    /// Time laid so far.
    laid: f32,
    /// When it fired, once it has.
    fired: Option<f32>,
    /// The blades' stretch as last laid, for the arc light.
    blades: (Vec3, Vec3),
}

#[derive(Default)]
pub(super) struct BoltRifleFx {
    guns: Vec<Gun>,
    last: f32,
    /// This tick's gun-house poses (`RenderFrame::houses`), for rifles on houses.
    houses: Vec<HousePose>,
}

/// Where a rifle is in the world: its muzzle, along the bore, and the gun's left and up.
struct Frame {
    muzzle: Vec3,
    dir: Vec3,
    left: Vec3,
    up: Vec3,
    length: f32,
}

impl Frame {
    /// The point `t` of the way from breech to muzzle, `radius` metres off the bore at
    /// `angle` round it (0 straight up, as the blades are turned).
    fn at(&self, t: f32, angle: f32, radius: f32) -> Vec3 {
        let (s, c) = angle.sin_cos();
        self.muzzle - self.dir * (1.0 - t) * self.length + (self.up * c - self.left * s) * radius
    }

    fn radius(&self) -> f32 {
        self.length * RADIUS
    }

    /// How far out a blade's outer edge stands `t` along (rear blade tall, front one lower).
    fn blade_edge(&self, t: f32) -> f32 {
        let k = ((t - BLADES.0) / (BLADES.1 - BLADES.0)).clamp(0.0, 1.0);
        self.radius()
            * if k < 0.5 {
                1.0 - k * 0.3
            } else {
                0.8 - (k - 0.5) * 0.5
            }
    }
}

fn lerp_angle(a: f32, b: f32, f: f32) -> f32 {
    a + ((b - a + PI).rem_euclid(TAU) - PI) * f
}

impl Renderer {
    fn arc_rifle(&self, blueprint: BlueprintId, weapon: u8) -> Option<&Weapon> {
        self.blueprints
            .unit(blueprint)
            .weapons
            .get(weapon as usize)
            .filter(|w| w.arc_charge > 0.0)
    }

    /// Takes `event` when it is a bolt rifle's: true when that is all of it (its charge,
    /// drawn here instead of the usual glow at the muzzle). Its shot is also drawn as
    /// any other, so that returns false.
    pub(super) fn bolt_rifle_event(&mut self, event: &SimEvent, time: f32) -> bool {
        match event {
            SimEvent::WeaponCharging {
                unit,
                blueprint,
                weapon,
                ..
            } => {
                let Some(w) = self.arc_rifle(*blueprint, *weapon) else {
                    return false;
                };
                let seconds = (w.charge_ticks as f32 * self.tick_seconds.max(0.02)).max(0.2);
                let fx = &mut self.bore_fx.rifles;
                let full = fx.guns.len() >= MAX_GUNS;
                match fx
                    .guns
                    .iter_mut()
                    .find(|g| g.unit == unit.0 && g.weapon == *weapon)
                {
                    Some(g) => {
                        // Charging again while the last shot's seams still cool.
                        g.start = time;
                        g.end = time + seconds;
                        g.laid = g.laid.max(time);
                    }
                    None if !full => fx.guns.push(Gun {
                        unit: unit.0,
                        blueprint: blueprint.0,
                        weapon: *weapon,
                        start: time,
                        end: time + seconds,
                        laid: time,
                        fired: None,
                        blades: (Vec3::ZERO, Vec3::ZERO),
                    }),
                    None => {}
                }
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
                let Some(length) = self.arc_rifle(*blueprint, *weapon).map(|w| w.arc_charge) else {
                    return false;
                };
                let at = Vec3::from(pos.to_f32()) - Vec3::from(travel.to_f32());
                let dir = Vec3::from(vel.to_f32()).normalize_or(Vec3::X);
                // The charge on this gun is spent: its seams flare from now.
                if let Some(g) = self
                    .bore_fx
                    .rifles
                    .guns
                    .iter_mut()
                    .filter(|g| g.blueprint == blueprint.0 && g.weapon == *weapon)
                    .filter(|g| g.fired.is_none_or(|f| f < time))
                    .min_by(|a, b| {
                        let d = |g: &Gun| g.blades.1.distance_squared(at);
                        d(a).total_cmp(&d(b))
                    })
                    .filter(|g| g.blades.1.distance(at) < length * 3.0 + 10.0)
                {
                    g.fired = Some(time);
                    g.end = g.end.min(time);
                }
                let outbound = std::mem::replace(&mut self.effect_outbound, true);
                self.rifle_fire(at, dir, length, time);
                self.effect_outbound = outbound;
                false
            }
            _ => false,
        }
    }

    /// Where `u`'s rifle `weapon` (`w`) is drawn `f` of the way through the tick.
    fn rifle_frame(&self, u: &UnitInstance, weapon: u8, w: &Weapon, f: f32) -> Frame {
        let pos = Vec3::from(u.prev_pos).lerp(Vec3::from(u.pos), f);
        let heading = lerp_angle(u.prev_heading, u.heading, f);
        // A gun on a house of its own (a warship's) has its pose in the houses list.
        let house = (u.status[1] >> UNIT_HOUSE_SHIFT)
            .checked_sub(1)
            .and_then(|i| self.bore_fx.rifles.houses.get(i as usize))
            .filter(|_| w.mount)
            .and_then(|h| h.pose.get(weapon as usize));
        let yaw = match house {
            Some(p) => lerp_angle(p[0], p[1], f),
            None if w.turret_turn > 0 => lerp_angle(u.prev_turret_yaw, u.turret_yaw, f),
            None => 0.0,
        };
        // The arm pitches the gun about its pivot (the shoulder), both guns together.
        let pitch = match house {
            Some(p) => p[2] + (p[3] - p[2]) * f,
            None if w.pivot.is_some() => u.arm_pitch[0] + (u.arm_pitch[1] - u.arm_pitch[0]) * f,
            None => 0.0,
        };
        let muzzle = Vec3::from(w.muzzle.to_f32());
        let pivot = w.pivot.map_or(muzzle, |p| Vec3::from(p.to_f32()));
        let rot_z = |v: Vec3, a: f32| {
            let (s, c) = a.sin_cos();
            Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
        };
        let rot_xz = |v: Vec3, a: f32| {
            let (s, c) = a.sin_cos();
            Vec3::new(v.x * c - v.z * s, v.y, v.x * s + v.z * c)
        };
        // A house turns about its own pivot; a torso about the unit's middle.
        let place = |local: Vec3| {
            let on_hull = if house.is_some() {
                pivot + rot_z(rot_xz(local - pivot, pitch), yaw)
            } else {
                rot_z(pivot + rot_xz(local - pivot, pitch), yaw)
            };
            pos + rot_z(on_hull, heading)
        };
        let turn = |v: Vec3| rot_z(rot_z(rot_xz(v, pitch), yaw), heading);
        Frame {
            muzzle: place(muzzle),
            dir: turn(Vec3::X),
            left: turn(Vec3::Y),
            up: turn(Vec3::Z),
            length: w.arc_charge,
        }
    }

    /// Once a tick, before the bore's strokes are written: the next tick of every rifle's
    /// charge and of its seams cooling, on the gun as it is then.
    pub(super) fn bolt_rifle_tick(
        &mut self,
        units: &[UnitInstance],
        houses: &[HousePose],
        time: f32,
    ) {
        let fx = &mut self.bore_fx.rifles;
        fx.houses.clear();
        fx.houses.extend_from_slice(houses);
        if time + 1.0 < fx.last {
            // The clock went back (a restaged backdrop).
            fx.guns.clear();
        }
        fx.last = time;
        fx.guns
            .retain(|g| time < g.end.max(g.fired.map_or(0.0, |f| f + FLARE)) + 0.3);
        let tick = self.tick_seconds.max(0.02);
        let blueprints = self.blueprints.clone();
        let outbound = std::mem::replace(&mut self.effect_outbound, true);
        for i in 0..self.bore_fx.rifles.guns.len() {
            let g = &self.bore_fx.rifles.guns[i];
            let (unit, blueprint, weapon) = (g.unit, g.blueprint, g.weapon);
            let (start, end, fired) = (g.start, g.end, g.fired);
            let from = g.laid.max(time);
            let until = time + tick;
            self.bore_fx.rifles.guns[i].laid = until;
            let Some(u) = units
                .iter()
                .find(|u| u.unit_id == unit && u.owner_flags & (KIND_WRECK | KIND_GHOST) == 0)
            else {
                continue;
            };
            let Some(w) = blueprints
                .unit(BlueprintId(blueprint))
                .weapons
                .get(weapon as usize)
            else {
                continue;
            };
            // In steps of a twentieth of a second, each on the gun where it is drawn then.
            let steps = ((until - from) / 0.05).ceil().max(1.0) as usize;
            for k in 0..steps {
                let when = from + (until - from) * k as f32 / steps as f32;
                let gun = self.rifle_frame(u, weapon, w, ((when - time) / tick).clamp(0.0, 1.0));
                if (start..end).contains(&when) {
                    let f = (when - start) / (end - start).max(0.01);
                    self.rifle_charge_step(&gun, f, when);
                }
                if let Some(age) = fired.map(|t| when - t).filter(|a| (0.0..FLARE).contains(a)) {
                    self.rifle_seams(&gun, 1.0 - age / FLARE, when);
                }
                self.bore_fx.rifles.guns[i].blades =
                    (gun.at(BLADES.0, 0.0, 0.0), gun.at(BLADES.1, 0.0, 0.0));
            }
        }
        self.effect_outbound = outbound;
    }

    /// A kinked arc from `a` to `b`, `wander` off the line at most.
    fn rifle_arc(
        &mut self,
        (a, b): (Vec3, Vec3),
        gun: &Frame,
        wander: f32,
        (when, life): (f32, f32),
        width: f32,
    ) {
        let kinks = ((a.distance(b) / (gun.radius() * 1.2)) as usize).clamp(3, 6);
        let mut last = a;
        for k in 1..=kinks {
            let t = k as f32 / kinks as f32;
            let jitter = if k == kinks {
                Vec3::ZERO
            } else {
                (gun.left * self.scatter.signed() + gun.up * self.scatter.signed()) * wander
            };
            let next = a.lerp(b, t) + jitter;
            self.bore_fx.lightning(last, next, when, life, width);
            last = next;
        }
    }

    /// A twentieth of a second of a charge `f` of the way done.
    fn rifle_charge_step(&mut self, gun: &Frame, f: f32, when: f32) {
        let r = gun.radius();
        // The arcs creep forward from the housing as the charge builds.
        let front = BLADES.0 + (BLADES.1 - BLADES.0) * (0.25 + 0.9 * f).min(1.0);
        let arcs = 2 + (f * f * 5.0 + self.scatter.unit() * (1.0 + f)) as usize;
        let width = r * (0.18 + 0.14 * f);
        for _ in 0..arcs {
            // On one face of a blade or the other, not in its plate.
            let blade = BLADE_ANGLES[(self.scatter.unit() * 3.0) as usize % 3]
                + if self.scatter.unit() < 0.5 { 0.3 } else { -0.3 };
            let t = BLADES.0 + (front - BLADES.0) * self.scatter.unit();
            let life = 0.07 + self.scatter.unit() * 0.07;
            let roll = self.scatter.unit() * 0.04;
            let root = gun.at(t, blade, r * ROOT);
            let to = if self.scatter.unit() < 0.45 {
                // Crawling along the blade's root, by its seam.
                let t2 = (t + (0.05 + 0.08 * self.scatter.unit())).min(front);
                gun.at(t2, blade, r * ROOT)
            } else {
                // From the blade's edge down to the core between the blades.
                let t2 = t + self.scatter.signed() * 0.04;
                let side = if self.scatter.unit() < 0.5 {
                    1.05
                } else {
                    -1.05
                };
                let edge = gun.at(t2, blade, gun.blade_edge(t2) * 0.95);
                self.rifle_arc(
                    (edge, root),
                    gun,
                    r * 0.12,
                    (when + roll, life),
                    width * 0.8,
                );
                gun.at(t2 + 0.03, blade + side, r * 0.42)
            };
            self.rifle_arc((root, to), gun, r * 0.1, (when + roll, life), width);
        }
        // Late on, a knot of arcs curling round the collar and spitting off its face.
        if f > 0.6 {
            let knot = ((f - 0.6) / 0.4).clamp(0.0, 1.0);
            for _ in 0..1 + (knot * 2.5 + self.scatter.unit()) as usize {
                let a = self.scatter.unit() * TAU;
                let span = 0.8 + self.scatter.unit() * 1.4;
                let t = COLLAR.0 + (COLLAR.1 - COLLAR.0) * self.scatter.unit();
                let ring = r * COLLAR_RADIUS * 1.1;
                let (from, apex, to) = (
                    gun.at(t, a, ring),
                    gun.at(t, a + span * 0.5, ring * (1.2 + 0.3 * knot)),
                    gun.at(t, a + span, ring),
                );
                let life = 0.05 + self.scatter.unit() * 0.06;
                self.rifle_arc((from, apex), gun, r * 0.1, (when, life), width * 0.8);
                self.rifle_arc((apex, to), gun, r * 0.1, (when, life), width * 0.8);
            }
            if self.scatter.unit() < knot * 0.6 {
                let out = (gun.dir
                    + (gun.left * self.scatter.signed() + gun.up * self.scatter.signed()) * 0.8)
                    .normalize_or(gun.dir);
                let tip = gun.muzzle + out * r * (0.6 + 1.0 * knot);
                let from = gun.at(1.0, self.scatter.unit() * TAU, r * COLLAR_RADIUS * 0.8);
                self.rifle_arc((from, tip), gun, r * 0.2, (when, 0.05), width * 0.8);
            }
        }
        // Now and then a spark drips off a blade.
        if self.scatter.unit() < 0.2 + 0.4 * f {
            let blade = BLADE_ANGLES[(self.scatter.unit() * 3.0) as usize % 3];
            let at = gun.at(
                BLADES.0 + (front - BLADES.0) * self.scatter.unit(),
                blade,
                r * 0.8,
            );
            let vel = (gun.up * self.scatter.signed() + gun.left * self.scatter.signed()) * 3.0
                - Vec3::Z * 2.0;
            self.push_puff(PUFF_BOLT, at, vel, when, 0.2, (0.12, 0.04));
        }
    }

    /// The blade seams after the shot: white-blue along each blade's root, `heat` 1 to 0.
    fn rifle_seams(&mut self, gun: &Frame, heat: f32, when: f32) {
        let r = gun.radius();
        for blade in BLADE_ANGLES {
            let (a, b) = (
                gun.at(BLADES.0, blade, r * ROOT),
                gun.at(BLADES.1, blade, r * ROOT),
            );
            self.bore_fx
                .lightning(a, b, when, 0.07, r * (0.05 + 0.2 * heat * heat));
        }
    }

    /// The shot leaving the muzzle at `at` along `dir`, from a gun `length` metres long.
    fn rifle_fire(&mut self, at: Vec3, dir: Vec3, length: f32, time: f32) {
        let r = length * RADIUS;
        let left = Vec3::Z.cross(dir).normalize_or(Vec3::Y);
        let up = dir.cross(left).normalize_or(Vec3::Z);
        // A short blue-white flash, hard and small.
        self.push_effect((at + dir * r).to_array(), time, r * 1.6, 0.07, 0.0, 1.0);
        // Forked strokes snapping forward out of the muzzle, a few metres.
        let gun = Frame {
            muzzle: at,
            dir,
            left,
            up,
            length,
        };
        for _ in 0..3 {
            let out = (dir * 1.4 + (left * self.scatter.signed() + up * self.scatter.signed()))
                .normalize_or(dir);
            let reach = length * (0.35 + 0.35 * self.scatter.unit());
            let tip = at + out * reach;
            let life = 0.08 + self.scatter.unit() * 0.06;
            let width = r * 0.22;
            self.rifle_arc((at, tip), &gun, reach * 0.12, (time, life), width);
            // A fork off it.
            let root = at.lerp(tip, 0.4 + 0.3 * self.scatter.unit());
            let away = (out + (left * self.scatter.signed() + up * self.scatter.signed()) * 1.2)
                .normalize_or(out);
            let fork = (root, root + away * reach * 0.45);
            self.rifle_arc(fork, &gun, reach * 0.08, (time, life * 0.7), width * 0.6);
        }
        // Sparks thrown forward.
        for _ in 0..8 {
            let spray = (dir * 2.0
                + Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.signed(),
                ))
            .normalize_or(dir);
            let speed = 25.0 + self.scatter.unit() * 35.0;
            let life = 0.15 + self.scatter.unit() * 0.2;
            self.push_puff(PUFF_SPARK, at, spray * speed, time, life, (0.3, 0.1));
        }
        // Ionised haze venting from the housing's flanks, drifting up and back.
        for k in 0..8 {
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            let t = PORTS.0 + (PORTS.1 - PORTS.0) * self.scatter.unit();
            let port = gun.at(t, side * PI * 0.5, r * 1.2);
            let vel = left * side * (3.0 + self.scatter.unit() * 3.0)
                + Vec3::Z * (1.5 + self.scatter.unit() * 1.5)
                - dir * 2.0;
            let life = 1.0 + self.scatter.unit() * 0.8;
            let when = time + 0.02 + 0.03 * k as f32;
            self.push_puff(PUFF_STEAM, port, vel, when, life, (r * 0.6, r * 2.6));
        }
    }

    /// Every frame: the arcs' flicker along the blades while they charge, and the flash's.
    pub(super) fn bolt_rifle_lights(&mut self, time: f32) {
        for g in &self.bore_fx.rifles.guns {
            if (g.start..g.end).contains(&time) {
                let f = (time - g.start) / (g.end - g.start).max(0.01);
                // Arc light strobes: dark between strikes, more strikes as it builds.
                let step = (time * 30.0).floor();
                let roll = |salt: f32| {
                    ((step * 12.9898 + salt * 78.233).sin() * 43758.545)
                        .fract()
                        .abs()
                };
                if roll(g.start + g.weapon as f32) < 0.3 + 0.6 * f {
                    let at = g.blades.0.lerp(g.blades.1, roll(g.start + 5.3));
                    let power = 60.0 * (0.3 + 0.7 * f) * (0.6 + 0.4 * roll(g.start + 2.1));
                    self.lights
                        .lamp(at, Vec3::Z, ARC_LIGHT * power, 5.0 + 5.0 * f, 180.0, 1.0);
                }
            }
            if let Some(age) = g
                .fired
                .map(|t| time - t)
                .filter(|a| (0.0..0.12).contains(a))
            {
                let k = 1.0 - age / 0.12;
                let muzzle = g.blades.1 + (g.blades.1 - g.blades.0) * 0.35;
                self.lights.lamp(
                    muzzle,
                    Vec3::Z,
                    Vec3::new(0.6, 0.8, 1.0) * 400.0 * k * k,
                    14.0,
                    180.0,
                    1.0,
                );
            }
        }
    }
}
