//! An Arc Howitzer's firing sequence (`Weapon::howitzer`): the Trebuchet's gun, and the
//! Leviathan's Arc Cannons, three of the same tube to a house. A plasma cell charged in
//! the breech and thrown; ARC (docs/STYLE.md "The electric bore"): the blue is arcs, seams
//! and the plasma itself, never a lit block.
//!
//! - **Charge** (`SimEvent::WeaponCharging`), built up the whole way to the shot:
//!   - the two plasma cells on the breech housing light along their seams and arcs
//!     crawl over them;
//!   - arcs run from the cells up the six radiator blades' roots, creeping forward, and
//!     jump from blade edge to blade edge;
//!   - on a house of several tubes, arcs jump the gap between neighbouring tubes' blades,
//!     sooner and more often;
//!   - late on, arcs curl round the collar and spit off the muzzle, a knot of plasma
//!     forms in the bore with sparks drawn in to it, and now and then an arc runs the
//!     whole tube;
//!   - its light strobes along the tubes with the arcs, over a steady glow at the muzzle.
//! - **Fire** (`SimEvent::ShotFired`, instead of an ordinary shot's flash): a white-hot
//!   core and a blue bloom, a plasma jet out along the bore, a shock front, a ring of
//!   vapour thrown out flat across the muzzle, forks of lightning snapping forward, blue
//!   sparks; the blade seams flare and cool, the cells vent ionised haze, and arcs die
//!   off along the tube for a moment after. A warship's battery still stamps the sea
//!   under its muzzles (`sea_effects_of`).
//!
//! The arcs are laid a tick at a time on the tubes as they are drawn then (the hull's
//! place and heading, the house's or turret's yaw and pitch about the weapon's `pivot`),
//! as lightning strokes among the bore's (`BoreFx::lightning`). The tube's layout, in
//! shares of its length and radii of `HowitzerLook::radius`, is the model's
//! (models `siege_howitzer`).

use super::hull_frame::DrawnHull;
use super::water_fx::PUFF_STEAM;
use super::{Renderer, PUFF_BOLT, PUFF_PLASMA, PUFF_SPARK};
use glam::Vec3;
use mc_data::{BlueprintId, HowitzerLook, Weapon};
use mc_sim::mirror::{HousePose, SimEvent, UnitInstance, KIND_GHOST, KIND_WRECK, UNIT_HOUSE_SHIFT};
use std::f32::consts::{PI, TAU};

/// Guns (a weapon on a hull) in their sequence at once. A deliberate cosmetic cap: a
/// charge past it is not drawn (its shot still is), so a fleet firing costs no more than
/// this many guns' arcs. A tube has about 40 strokes alive at the top of its charge, a
/// Leviathan's nine some 300 (strokes share `MAX_PROJECTILES`, 32768, with every shot).
const MAX_GUNS: usize = 16;
/// Tubes of one gun drawn (a house holds three).
const MAX_TUBES: usize = 4;
/// Seconds the blade seams flare and the arcs die off after the shot.
const AFTER: f32 = 0.9;
/// The arcs' light: blue-violet, as the lightning is (sprites.wgsl fading-beam colour 3),
/// and the plasma knot's, whiter.
const ARC_LIGHT: Vec3 = Vec3::new(0.4, 0.5, 1.0);
const KNOT_LIGHT: Vec3 = Vec3::new(0.55, 0.75, 1.0);
/// Effect kinds (sprites.wgsl): blue energy, white-hot.
const BLUE: f32 = 0.0;
const WHITE_HOT: f32 = 4.0;
/// The tube in shares of its length from the breech (models `siege_howitzer`): the
/// plasma cells on the housing's flanks, the bladed core, the blades' stretch, the
/// collar, and where the housing vents.
const CELLS: (f32, f32) = (0.03, 0.24);
const CORE: (f32, f32) = (0.3, 0.84);
const BLADES: (f32, f32) = (0.34, 0.76);
const COLLAR: (f32, f32) = (0.86, 0.97);
/// In radii: a cell's centre off the bore (out to the side, down), its own radius, the
/// core's, a blade's root and its edge (tall at the back, lower at the front), the collar's.
const CELL_SIDE: f32 = 1.38;
const CELL_DROP: f32 = -0.35;
const CELL_R: f32 = 0.42;
const CORE_R: f32 = 0.72;
const ROOT_R: f32 = 0.68;
const COLLAR_R: f32 = 0.9;
/// Share of a gun's charge before its tubes start to arc to each other.
const BRIDGE_FROM: f32 = 0.3;

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
    /// The first tube's breech and muzzle, and the middle one's muzzle, as last laid (for
    /// the light, and to find the gun a shot left).
    tube: (Vec3, Vec3),
    mouth: Vec3,
}

#[derive(Default)]
pub(super) struct ArcHowitzerFx {
    guns: Vec<Gun>,
    /// This tick's gun-house poses (`RenderFrame::houses`), for guns on houses of their own.
    houses: Vec<HousePose>,
    last: f32,
}

/// Where one tube is in the world: its muzzle, the way it points, its left and up, and
/// its size.
#[derive(Clone, Copy)]
struct Tube {
    muzzle: Vec3,
    dir: Vec3,
    left: Vec3,
    up: Vec3,
    length: f32,
    radius: f32,
}

impl Tube {
    /// The point `t` of the way from breech to muzzle, `off` radii off the bore at `angle`
    /// round it (0 straight up, as the blades are turned in the model).
    fn at(&self, t: f32, angle: f32, off: f32) -> Vec3 {
        let (s, c) = angle.sin_cos();
        self.axis(t) + (self.up * c - self.left * s) * off * self.radius
    }

    fn axis(&self, t: f32) -> Vec3 {
        self.muzzle - self.dir * (1.0 - t) * self.length
    }

    /// A point on the seam along the top of the cell on `side` (1 left, -1 right).
    fn cell(&self, t: f32, side: f32) -> Vec3 {
        self.axis(t)
            + self.left * side * CELL_SIDE * self.radius
            + self.up * (CELL_DROP + CELL_R) * self.radius
    }

    /// How far out a blade's edge stands `t` along the tube, in radii.
    fn blade_edge(t: f32) -> f32 {
        let k = ((t - BLADES.0) / (BLADES.1 - BLADES.0)).clamp(0.0, 1.0);
        if k < 0.2 {
            0.6 + 0.7 * k / 0.2
        } else {
            1.3 - 0.32 * (k - 0.2) / 0.7
        }
        .min(1.3)
    }
}

/// The six blades' angles about the bore (models: `(k + 0.5) / 6` of a turn).
fn blade(k: usize) -> f32 {
    (k % 6) as f32 * TAU / 6.0 + TAU / 12.0
}

fn lerp_angle(a: f32, b: f32, f: f32) -> f32 {
    a + ((b - a + PI).rem_euclid(TAU) - PI) * f
}

impl Renderer {
    fn arc_howitzer(&self, blueprint: BlueprintId, weapon: u8) -> Option<(&Weapon, HowitzerLook)> {
        let w = self
            .blueprints
            .unit(blueprint)
            .weapons
            .get(weapon as usize)?;
        Some((w, w.howitzer?))
    }

    /// Takes `event` when it is an Arc Howitzer's: true when that is all of it (its charge
    /// instead of the usual glow at the muzzle, its firing instead of a shot's flash).
    pub(super) fn arc_howitzer_event(&mut self, event: &SimEvent, time: f32) -> bool {
        match event {
            SimEvent::WeaponCharging {
                unit,
                blueprint,
                weapon,
                ..
            } => {
                let Some((w, _)) = self.arc_howitzer(*blueprint, *weapon) else {
                    return false;
                };
                let seconds = (w.charge_ticks as f32 * self.tick_seconds.max(0.02)).max(0.3);
                let fx = &mut self.bore_fx.howitzers;
                let full = fx.guns.len() >= MAX_GUNS;
                match fx
                    .guns
                    .iter_mut()
                    .find(|g| g.unit == unit.0 && g.weapon == *weapon)
                {
                    Some(g) => {
                        // Charging again while the last shot's arcs still die off.
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
                        tube: (Vec3::ZERO, Vec3::ZERO),
                        mouth: Vec3::ZERO,
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
                let Some((_, look)) = self.arc_howitzer(*blueprint, *weapon) else {
                    return false;
                };
                // A warship's battery still stamps the sea under its muzzles.
                self.sea_effects_of(event, time);
                let at = Vec3::from(pos.to_f32()) - Vec3::from(travel.to_f32());
                let dir = Vec3::from(vel.to_f32()).normalize_or(Vec3::X);
                // The charge on this gun is spent: its seams flare and its arcs die off from now.
                let reach = look.length * 1.5 + 12.0;
                let barrels = {
                    let (w, _) = self
                        .arc_howitzer(*blueprint, *weapon)
                        .expect("checked above");
                    w.muzzles.len().max(1)
                };
                if let Some(g) = self
                    .bore_fx
                    .howitzers
                    .guns
                    .iter_mut()
                    .filter(|g| g.blueprint == blueprint.0 && g.weapon == *weapon)
                    .min_by(|a, b| {
                        let d = |g: &Gun| g.mouth.distance_squared(at);
                        d(a).total_cmp(&d(b))
                    })
                    .filter(|g| g.mouth.distance(at) < reach)
                {
                    g.fired = Some(time);
                    g.end = g.end.min(time);
                }
                let outbound = std::mem::replace(&mut self.effect_outbound, true);
                self.howitzer_fire(at, dir, look, barrels, time);
                self.effect_outbound = outbound;
                true
            }
            _ => false,
        }
    }

    /// Where the tubes of `u`'s gun `weapon` are drawn `f` of the way through the tick.
    fn howitzer_tubes(&self, u: &UnitInstance, weapon: u8, f: f32) -> Vec<Tube> {
        let bp = self.blueprints.unit(BlueprintId(u.blueprint as u16));
        let Some(w) = bp.weapons.get(weapon as usize) else {
            return Vec::new();
        };
        let Some(look) = w.howitzer else {
            return Vec::new();
        };
        let hull = DrawnHull::of(u, &bp.visual.mesh, f);
        let rot_z = |v: Vec3, a: f32| {
            let (s, c) = a.sin_cos();
            Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
        };
        let rot_xz = |v: Vec3, a: f32| {
            let (s, c) = a.sin_cos();
            Vec3::new(v.x * c - v.z * s, v.y, v.x * s + v.z * c)
        };
        // A gun on a house of its own (a warship's turret) has its pose in the houses list;
        // a turret gun (the Trebuchet's) turns with the unit's turret and pitches with its
        // arm; anything else lies fixed in the hull.
        let house = (u.status[1] >> UNIT_HOUSE_SHIFT)
            .checked_sub(1)
            .and_then(|i| self.bore_fx.howitzers.houses.get(i as usize))
            .filter(|_| w.mount)
            .and_then(|h| h.pose.get(weapon as usize));
        let turret = house.is_none() && weapon == 0 && w.turret_turn > 0 && w.pivot.is_some();
        let (yaw, pitch) = match house {
            Some(p) => (lerp_angle(p[0], p[1], f), p[2] + (p[3] - p[2]) * f),
            None if turret => (
                lerp_angle(u.prev_turret_yaw, u.turret_yaw, f),
                u.arm_pitch[0] + (u.arm_pitch[1] - u.arm_pitch[0]) * f,
            ),
            None => (0.0, 0.0),
        };
        let pivot = w.pivot.map(|p| Vec3::from(p.to_f32()));
        let place = |local: Vec3| {
            let on_hull = match pivot {
                // A house turns about its pivot; what is in it pitches there too.
                Some(p) if house.is_some() => p + rot_z(rot_xz(local - p, pitch), yaw),
                // The turret turns about the unit's middle, the gun about its trunnion.
                Some(p) if turret => rot_z(p + rot_xz(local - p, pitch), yaw),
                _ => local,
            };
            hull.place(on_hull)
        };
        let mouths: Vec<Vec3> = if w.muzzles.is_empty() {
            vec![Vec3::from(w.muzzle.to_f32())]
        } else {
            w.muzzles.iter().map(|m| Vec3::from(m.to_f32())).collect()
        };
        mouths
            .into_iter()
            .take(MAX_TUBES)
            .map(|m| {
                // An aft house's muzzles are given as if it faced forward, mirrored (as the sim does).
                let local = if w.rear {
                    Vec3::new(-m.x, -m.y, m.z)
                } else {
                    m
                };
                // Down the bore: from the trunnion out to the muzzle, level across.
                let bore = pivot.map_or(Vec3::X, |p| {
                    let d = local - p;
                    Vec3::new(d.x, 0.0, d.z).normalize_or(Vec3::X)
                });
                let muzzle = place(local);
                let dir = (place(local + bore) - muzzle).normalize_or(Vec3::X);
                let left = Vec3::Z.cross(dir).normalize_or(Vec3::Y);
                Tube {
                    muzzle,
                    dir,
                    left,
                    up: dir.cross(left).normalize_or(Vec3::Z),
                    length: look.length,
                    radius: look.radius,
                }
            })
            .collect()
    }

    /// Once a tick, before the bore's strokes are written: the next tick of every gun's
    /// charge, and of its arcs dying off after the shot, on its tubes as they are then.
    pub(super) fn arc_howitzer_tick(
        &mut self,
        units: &[UnitInstance],
        houses: &[HousePose],
        time: f32,
    ) {
        let fx = &mut self.bore_fx.howitzers;
        if time + 1.0 < fx.last {
            // The clock went back (a restaged backdrop).
            fx.guns.clear();
        }
        fx.last = time;
        fx.houses.clear();
        fx.houses.extend_from_slice(houses);
        fx.guns
            .retain(|g| time < g.end.max(g.fired.map_or(0.0, |f| f + AFTER)) + 0.3);
        let tick = self.tick_seconds.max(0.02);
        let outbound = std::mem::replace(&mut self.effect_outbound, true);
        for i in 0..self.bore_fx.howitzers.guns.len() {
            let g = &self.bore_fx.howitzers.guns[i];
            let (unit, weapon) = (g.unit, g.weapon);
            let (start, end, fired) = (g.start, g.end, g.fired);
            let from = g.laid.max(time);
            let until = time + tick;
            self.bore_fx.howitzers.guns[i].laid = until;
            let Some(u) = units
                .iter()
                .find(|u| u.unit_id == unit && u.owner_flags & (KIND_WRECK | KIND_GHOST) == 0)
            else {
                continue;
            };
            // In steps of a twentieth of a second, each on the tubes where they are drawn then.
            let steps = ((until - from) / 0.05).ceil().max(1.0) as usize;
            for k in 0..steps {
                let when = from + (until - from) * k as f32 / steps as f32;
                let tubes = self.howitzer_tubes(u, weapon, ((when - time) / tick).clamp(0.0, 1.0));
                let (Some(first), Some(middle)) = (tubes.first(), tubes.get(tubes.len() / 2))
                else {
                    break;
                };
                {
                    let g = &mut self.bore_fx.howitzers.guns[i];
                    g.tube = (first.axis(0.0), first.muzzle);
                    g.mouth = middle.muzzle;
                }
                if (start..end).contains(&when) {
                    let f = (when - start) / (end - start).max(0.01);
                    self.howitzer_charge_step(&tubes, f, end - start, when);
                }
                if let Some(age) = fired.map(|t| when - t).filter(|a| (0.0..AFTER).contains(a)) {
                    self.howitzer_after(&tubes, 1.0 - age / AFTER, when);
                }
            }
        }
        self.effect_outbound = outbound;
    }

    /// A kinked arc from `a` to `b` round `tube`, `wander` metres off the line at most.
    fn howitzer_arc(
        &mut self,
        (a, b): (Vec3, Vec3),
        tube: &Tube,
        wander: f32,
        (when, life): (f32, f32),
        width: f32,
    ) {
        let kinks = ((a.distance(b) / (tube.radius * 0.9)) as usize).clamp(3, 7);
        let mut last = a;
        for k in 1..=kinks {
            let t = k as f32 / kinks as f32;
            let taper = (t * PI).sin().sqrt();
            let jitter = if k == kinks {
                Vec3::ZERO
            } else {
                (tube.left * self.scatter.signed() + tube.up * self.scatter.signed())
                    * wander
                    * taper
            };
            let next = a.lerp(b, t) + jitter;
            self.bore_fx.lightning(last, next, when, life, width);
            last = next;
        }
    }

    /// A twentieth of a second of a charge `f` of the way done, `seconds` long, on every tube.
    fn howitzer_charge_step(&mut self, tubes: &[Tube], f: f32, seconds: f32, when: f32) {
        let n = tubes.len();
        // A house of several tubes shares the storm between them.
        let share = 1.0 / (n as f32).sqrt();
        for tube in tubes {
            self.tube_charge_step(tube, f, seconds, share, when);
        }
        // Arcs jumping the gap between neighbouring tubes, blade edge to blade edge.
        if n > 1 && f > BRIDGE_FROM {
            let g = (f - BRIDGE_FROM) / (1.0 - BRIDGE_FROM);
            for pair in tubes.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let jumps = (g * g * 3.0 + self.scatter.unit() * (0.4 + g)) as usize;
                // Which of each tube's blades faces the other: b lies to a's left or right.
                let b_left = (b.muzzle - a.muzzle).dot(a.left) > 0.0;
                let (face_a, face_b) = if b_left {
                    (blade(4), blade(1))
                } else {
                    (blade(1), blade(4))
                };
                for _ in 0..jumps {
                    let t = BLADES.0 + (BLADES.1 - BLADES.0) * self.scatter.unit();
                    let t2 = (t + self.scatter.signed() * 0.05).clamp(BLADES.0, BLADES.1);
                    let from = a.at(
                        t,
                        face_a + self.scatter.signed() * 0.25,
                        Tube::blade_edge(t),
                    );
                    let to = b.at(
                        t2,
                        face_b + self.scatter.signed() * 0.25,
                        Tube::blade_edge(t2),
                    );
                    let life = 0.06 + self.scatter.unit() * 0.08;
                    let width = a.radius * (0.25 + 0.3 * g);
                    let wander = from.distance(to) * 0.22;
                    let roll = self.scatter.unit() * 0.04;
                    self.howitzer_arc((from, to), &a, wander, (when + roll, life), width);
                }
            }
        }
    }

    /// One tube's share of a charge step (`share` of a lone tube's arcs).
    fn tube_charge_step(&mut self, tube: &Tube, f: f32, seconds: f32, share: f32, when: f32) {
        let r = tube.radius;
        let width = r * (0.2 + 0.25 * f);
        let many = |this: &mut Self, base: f32| (base * share + this.scatter.unit()) as usize;
        // The cells: their seams light (a thin bright line that thickens), arcs crawl over them.
        for side in [1.0, -1.0] {
            let (a, b) = (tube.cell(CELLS.0, side), tube.cell(CELLS.1, side));
            self.bore_fx
                .lightning(a, b, when, 0.07, r * (0.06 + 0.2 * f));
            // The cells' own glow, swelling and flickering.
            if self.scatter.unit() < 0.5 {
                let t = CELLS.0 + (CELLS.1 - CELLS.0) * self.scatter.unit();
                let flicker = 0.7 + 0.3 * self.scatter.unit();
                self.push_effect(
                    tube.cell(t, side).to_array(),
                    when,
                    r * (0.4 + 0.9 * f) * flicker,
                    0.1,
                    BLUE,
                    0.0,
                );
            }
            for _ in 0..many(self, 0.8 + 1.6 * f) {
                let t = CELLS.0 + (CELLS.1 - CELLS.0) * self.scatter.unit();
                let from = tube.cell(t, side);
                let to = tube.cell(
                    (t + self.scatter.signed() * 0.07).clamp(CELLS.0, CELLS.1),
                    side,
                ) + tube.left * side * r * 0.25 * self.scatter.unit();
                let life = 0.05 + self.scatter.unit() * 0.06;
                self.howitzer_arc((from, to), tube, r * 0.2, (when, life), width * 0.8);
            }
        }
        // From the cells up the blade roots, the front creeping forward as it builds.
        let front = BLADES.0 + (BLADES.1 - BLADES.0) * (0.15 + 0.95 * f).min(1.0);
        for _ in 0..many(self, 1.0 + 5.0 * f * f) {
            let k = (self.scatter.unit() * 6.0) as usize;
            let a = blade(k)
                + if self.scatter.unit() < 0.5 {
                    0.28
                } else {
                    -0.28
                };
            let t = BLADES.0 + (front - BLADES.0) * self.scatter.unit();
            let life = 0.06 + self.scatter.unit() * 0.08;
            let roll = self.scatter.unit() * 0.04;
            let root = tube.at(t, a, ROOT_R);
            let to = match self.scatter.unit() {
                // Along the blade's root, by its seam.
                x if x < 0.4 => tube.at(
                    (t + 0.04 + 0.08 * self.scatter.unit()).min(front),
                    a,
                    ROOT_R,
                ),
                // Out to the blade's edge.
                x if x < 0.75 => tube.at(
                    t + self.scatter.signed() * 0.03,
                    a,
                    Tube::blade_edge(t) * 0.95,
                ),
                // Across to the next blade's edge.
                _ => {
                    let next = blade(k + 1)
                        + if self.scatter.unit() < 0.5 {
                            -0.28
                        } else {
                            0.28
                        };
                    let t2 = t + self.scatter.signed() * 0.04;
                    let edge = tube.at(t, a, Tube::blade_edge(t) * 0.95);
                    let other = tube.at(t2, next, Tube::blade_edge(t2) * 0.95);
                    self.howitzer_arc((edge, other), tube, r * 0.25, (when + roll, life), width);
                    edge
                }
            };
            self.howitzer_arc((root, to), tube, r * 0.12, (when + roll, life), width);
        }
        // A cell feeding the core: now and then an arc from a cell's front into the blades.
        if self.scatter.unit() < (0.15 + 0.5 * f) * share {
            let side = if self.scatter.unit() < 0.5 { 1.0 } else { -1.0 };
            let from = tube.cell(CELLS.1, side);
            let to = tube.at(
                CORE.0 + 0.05,
                if side > 0.0 { blade(4) } else { blade(1) },
                CORE_R,
            );
            self.howitzer_arc((from, to), tube, r * 0.3, (when, 0.08), width * 1.2);
        }
        // Late on: arcs curling round the collar and spitting off the muzzle, a knot of
        // plasma swelling in the bore with sparks drawn in to it.
        if f > 0.35 {
            let knot = ((f - 0.35) / 0.65).clamp(0.0, 1.0);
            for _ in 0..many(self, 1.0 + 3.0 * knot) {
                let a = self.scatter.unit() * TAU;
                let span = 0.8 + self.scatter.unit() * 1.6;
                let t = COLLAR.0 + (COLLAR.1 - COLLAR.0) * self.scatter.unit();
                let ring = COLLAR_R * 1.05;
                let (from, apex, to) = (
                    tube.at(t, a, ring),
                    tube.at(t, a + span * 0.5, ring * (1.25 + 0.4 * knot)),
                    tube.at(t, a + span, ring),
                );
                let life = 0.05 + self.scatter.unit() * 0.06;
                self.howitzer_arc((from, apex), tube, r * 0.1, (when, life), width);
                self.howitzer_arc((apex, to), tube, r * 0.1, (when, life), width);
            }
            if self.scatter.unit() < 0.2 + knot * 0.7 {
                let out = (tube.dir
                    + (tube.left * self.scatter.signed() + tube.up * self.scatter.signed()) * 0.9)
                    .normalize_or(tube.dir);
                let tip = tube.muzzle + out * r * (1.0 + 3.0 * knot) * (0.6 + self.scatter.unit());
                let from = tube.at(1.0, self.scatter.unit() * TAU, COLLAR_R * 0.8);
                self.howitzer_arc((from, tip), tube, r * 0.3, (when, 0.06), width * 0.9);
            }
            // The knot: a blue glow in the mouth, growing, a hard white heart in the last part.
            let pulse = 0.85 + 0.15 * self.scatter.unit();
            self.push_effect(
                (tube.muzzle + tube.dir * r * 0.3).to_array(),
                when,
                r * (0.5 + 2.2 * knot * knot) * pulse,
                0.09,
                BLUE,
                0.0,
            );
            if knot > 0.6 {
                self.push_effect(
                    (tube.muzzle + tube.dir * r * 0.2).to_array(),
                    when,
                    r * (knot - 0.6) * 2.2 * pulse,
                    0.08,
                    WHITE_HOT,
                    0.0,
                );
            }
            // Sparks drawn in to the mouth from a ring ahead of it, faster as it tops out.
            for _ in 0..many(self, 1.0 + 3.0 * knot) {
                let a = self.scatter.unit() * TAU;
                let (s, c) = a.sin_cos();
                let off = (tube.up * c - tube.left * s) * r * (2.5 + 3.0 * self.scatter.unit())
                    + tube.dir * r * (0.5 + 2.5 * self.scatter.unit());
                let life = 0.2 + 0.15 * (1.0 - knot);
                self.push_puff(
                    PUFF_BOLT,
                    tube.muzzle + off,
                    -off / life,
                    when,
                    life,
                    (r * 0.14, 0.05),
                );
            }
        }
        // In the last part, now and then an arc runs the whole tube, cells to collar.
        if f > 0.6 && self.scatter.unit() < (f - 0.6) * 1.2 * share {
            let a = blade((self.scatter.unit() * 6.0) as usize);
            let (from, to) = (
                tube.at(CELLS.0 + 0.05, a, 1.25),
                tube.at(COLLAR.1, a, COLLAR_R),
            );
            self.howitzer_arc((from, to), tube, r * 0.5, (when, 0.1), width * 1.4);
        }
        // Now and then a spark drips off a blade; a long charge drips more by the end.
        if self.scatter.unit() < (0.2 + 0.5 * f) * share * (seconds / 2.4).clamp(0.8, 1.5) {
            let at = tube.at(
                BLADES.0 + (front - BLADES.0) * self.scatter.unit(),
                blade((self.scatter.unit() * 6.0) as usize),
                1.1,
            );
            let vel = (tube.up * self.scatter.signed() + tube.left * self.scatter.signed()) * 4.0
                - Vec3::Z * 3.0;
            self.push_puff(PUFF_SPARK, at, vel, when, 0.35, (r * 0.2, 0.05));
        }
    }

    /// After the shot, `heat` 1 to 0: the blade seams flare and cool, arcs die off along
    /// the tube, and at first the cells vent haze.
    fn howitzer_after(&mut self, tubes: &[Tube], heat: f32, when: f32) {
        let share = 1.0 / (tubes.len() as f32).sqrt();
        for tube in tubes {
            let r = tube.radius;
            for k in 0..6 {
                let (a, b) = (
                    tube.at(BLADES.0 + 0.03, blade(k), ROOT_R),
                    tube.at(BLADES.1 - 0.03, blade(k), ROOT_R),
                );
                self.bore_fx
                    .lightning(a, b, when, 0.07, r * (0.04 + 0.22 * heat * heat));
            }
            // The last of the charge crawling off the blades, thinning out.
            for _ in 0..((heat * heat * 4.0 * share) + self.scatter.unit() * heat) as usize {
                let a = blade((self.scatter.unit() * 6.0) as usize);
                let t = BLADES.0 + (BLADES.1 - BLADES.0) * self.scatter.unit();
                let from = tube.at(t, a, ROOT_R);
                let to = tube.at(t + self.scatter.signed() * 0.06, a, Tube::blade_edge(t));
                let life = 0.05 + self.scatter.unit() * 0.05;
                self.howitzer_arc((from, to), tube, r * 0.15, (when, life), r * 0.12 * heat);
            }
            // The cells vent: ionised haze out of the housing's flanks, drifting up and back.
            if heat > 0.75 {
                for side in [1.0, -1.0] {
                    let t = CELLS.0 + (CELLS.1 - CELLS.0) * self.scatter.unit();
                    let port = tube.cell(t, side) + tube.left * side * r * 0.4;
                    let vel = tube.left * side * (2.0 + 3.0 * self.scatter.unit()) * r
                        + Vec3::Z * (1.5 + 1.5 * self.scatter.unit()) * r
                        - tube.dir * 1.5 * r;
                    let life = 1.2 + self.scatter.unit() * 0.9;
                    self.push_puff(PUFF_STEAM, port, vel, when, life, (r * 0.7, r * 3.2));
                }
            }
        }
    }

    /// One tube's shot leaving the muzzle at `at` along `dir`: the flash, the plasma jet,
    /// the shock, vapour, lightning and sparks. `barrels` tubes fire together, so each
    /// throws a share of the shock.
    fn howitzer_fire(
        &mut self,
        at: Vec3,
        dir: Vec3,
        look: HowitzerLook,
        barrels: usize,
        time: f32,
    ) {
        let r = look.radius;
        // How big the gun is: the Trebuchet's tube is about 10 m, a battleship's twice that.
        let k = (look.length / 10.0).clamp(0.6, 3.0);
        let share = 1.0 / (barrels as f32).sqrt();
        let left = Vec3::Z.cross(dir).normalize_or(Vec3::Y);
        let up = dir.cross(left).normalize_or(Vec3::Z);
        let tube = Tube {
            muzzle: at,
            dir,
            left,
            up,
            length: look.length,
            radius: r,
        };
        // The flash: a hard white-hot core, a blue bloom round it, and the plasma lit ahead.
        self.push_effect(
            (at + dir * r).to_array(),
            time,
            3.5 * k,
            0.09,
            WHITE_HOT,
            0.0,
        );
        self.push_effect(
            (at + dir * r * 2.0).to_array(),
            time,
            11.0 * k,
            0.16,
            BLUE,
            0.35,
        );
        self.push_effect(
            (at + dir * 7.0 * k).to_array(),
            time + 0.02,
            6.0 * k,
            0.14,
            BLUE,
            0.0,
        );
        // The shock front out along the bore.
        self.push_shockwave(
            (at + dir * 3.0 * k).to_array(),
            time,
            55.0 * k * share,
            0.6,
            0.8,
            BLUE,
            dir,
        );
        // The plasma jet: glowing blue gas blown out along the bore, fast and short-lived,
        // spreading as it slows.
        for i in 0..14 {
            let t = i as f32 / 13.0;
            let speed = (25.0 + 140.0 * t) * k;
            let jitter = (left * self.scatter.signed() + up * self.scatter.signed()) * speed * 0.1;
            let life = 0.18 + 0.25 * (1.0 - t) + self.scatter.unit() * 0.08;
            self.push_puff(
                PUFF_PLASMA,
                at + dir * r,
                dir * speed + jitter,
                time + 0.008 * i as f32,
                life,
                (r * 1.2 * k, r * (3.0 + 3.0 * t) * k),
            );
        }
        // Vapour thrown out flat across the muzzle, and a cone of it blown ahead.
        for i in 0..16 {
            let a = (i as f32 + self.scatter.unit() * 0.6) / 16.0 * TAU;
            let out = left * a.cos() + up * a.sin();
            let speed = 35.0 * k * (0.8 + 0.4 * self.scatter.unit());
            let life = 1.0 + self.scatter.unit() * 0.7;
            self.push_puff(
                PUFF_STEAM,
                at + out * r + dir * r,
                out * speed + dir * speed * 0.3,
                time,
                life,
                (1.4 * k, 5.0 * k),
            );
        }
        for i in 0..8 {
            let t = i as f32 / 7.0;
            let speed = (30.0 + 110.0 * t) * k;
            let jitter = (left * self.scatter.signed() + up * self.scatter.signed()) * speed * 0.12;
            self.push_puff(
                PUFF_STEAM,
                at + dir * r * 2.0,
                dir * speed + jitter,
                time + 0.015 * i as f32,
                1.1 + t * 1.3,
                (1.6 * k, (5.0 + 5.0 * t) * k),
            );
        }
        // Forks of lightning snapping forward out of the muzzle.
        for _ in 0..4 {
            let out = (dir * 1.3 + (left * self.scatter.signed() + up * self.scatter.signed()))
                .normalize_or(dir);
            let reach = look.length * (0.5 + 0.6 * self.scatter.unit());
            let tip = at + out * reach;
            let life = 0.09 + self.scatter.unit() * 0.07;
            let width = r * 0.3;
            self.howitzer_arc((at, tip), &tube, reach * 0.12, (time, life), width);
            let root = at.lerp(tip, 0.35 + 0.35 * self.scatter.unit());
            let away = (out + (left * self.scatter.signed() + up * self.scatter.signed()) * 1.2)
                .normalize_or(out);
            let fork = (root, root + away * reach * 0.45);
            self.howitzer_arc(fork, &tube, reach * 0.08, (time, life * 0.7), width * 0.6);
        }
        // Blue sparks thrown forward in a cone, and a few heavy ones that fall away.
        for _ in 0..22 {
            let spray = (dir * 2.2
                + Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.signed(),
                ) * 0.6)
                .normalize_or(dir);
            let speed = (40.0 + self.scatter.unit() * 90.0) * k;
            let life = 0.2 + self.scatter.unit() * 0.3;
            let kind = if self.scatter.unit() < 0.6 {
                PUFF_BOLT
            } else {
                PUFF_SPARK
            };
            self.push_puff(kind, at, spray * speed, time, life, (0.35 * k, 0.1));
        }
    }

    /// Every frame: the arcs' strobing light along each gun while it charges, the knot's
    /// glow at the muzzle, and the shot's flash.
    pub(super) fn arc_howitzer_lights(&mut self, time: f32) {
        for g in &self.bore_fx.howitzers.guns {
            let (breech, muzzle) = g.tube;
            let size = breech.distance(muzzle).max(1.0) / 10.0;
            if (g.start..g.end).contains(&time) {
                let f = (time - g.start) / (g.end - g.start).max(0.01);
                // Arc light strobes: dark between strikes, more strikes as it builds.
                let step = (time * 30.0).floor();
                let roll = |salt: f32| {
                    ((step * 12.9898 + salt * 78.233).sin() * 43758.545)
                        .fract()
                        .abs()
                };
                if roll(g.start + g.weapon as f32) < 0.35 + 0.6 * f {
                    let at = breech.lerp(g.mouth, roll(g.start + 5.3));
                    let power =
                        260.0 * size * size * (0.3 + 0.7 * f) * (0.6 + 0.4 * roll(g.start + 2.1));
                    self.lights.lamp(
                        at,
                        Vec3::Z,
                        ARC_LIGHT * power,
                        (7.0 + 9.0 * f) * size,
                        180.0,
                        1.0,
                    );
                }
                // The knot's glow in the mouth, steady and swelling over the last part.
                let knot = ((f - 0.35) / 0.65).clamp(0.0, 1.0);
                if knot > 0.0 {
                    self.lights.lamp(
                        g.mouth,
                        Vec3::Z,
                        KNOT_LIGHT * 700.0 * size * size * knot * knot,
                        (6.0 + 14.0 * knot) * size,
                        180.0,
                        1.0,
                    );
                }
            }
            if let Some(age) = g
                .fired
                .map(|t| time - t)
                .filter(|a| (0.0..0.18).contains(a))
            {
                let k = 1.0 - age / 0.18;
                self.lights.lamp(
                    g.mouth,
                    Vec3::Z,
                    KNOT_LIGHT * 1600.0 * size * k * k,
                    30.0 * size,
                    180.0,
                    1.0,
                );
            }
        }
    }
}
