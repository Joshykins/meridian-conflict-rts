//! A warship's spinal bore (models `SpinalBore`: the Dominion's spinal AEB), the biggest
//! gun a ship carries, shown as the event it is:
//!
//! - **Charge**: the charge climbs the spine coil by coil (the coils' own light is the
//!   shader's, `titan_charge`), arcs crawling round each lit ring and leaping on to the
//!   next, faster and heavier toward the shot; a ball of ionised air gathers in the bore's
//!   mouth and, in the last second or so, arcs snap off the prow into the open air. It is
//!   drawn a tick at a time on the hull as it is then, since the ship turns and pitches
//!   onto its mark while it charges.
//! - **Fire**: the charge tears down the coils at once, a blinding flash and a pressure
//!   ring out of the mouth along the bore, and the cloud deck is punched open where the
//!   channel crosses it. The channel, the strike and the storm are the giant bore's
//!   (`bore_fx`, `titan_fx`).
//!
//! Presentation only. A giant bore on anything else keeps `bore_charge`.

use super::titan_fx::PUFF_ARC_BALL;
use super::water_fx::GunHull;
use super::{Renderer, PUFF_SPARK};
use crate::models::SpinalBore;
use glam::Vec3;
use mc_sim::mirror::UnitInstance;

/// A spinal bore charging (`Renderer::spinal_charges_tick`).
#[derive(Clone, Copy)]
struct SpinalCharge {
    blueprint: u32,
    /// Where the ship was last seen: it is found again each tick as the nearest of its
    /// blueprint.
    near: Vec3,
    start: f32,
    end: f32,
}

#[derive(Default)]
pub(super) struct SpinalBores {
    charges: Vec<SpinalCharge>,
}

/// The hull's pitch as the sim lays it (a spinal gun's slot 0, `hull_pitched`), for a
/// capital ship; zero for anything else.
pub(super) fn hull_pitch(u: &UnitInstance, mesh: &str) -> f32 {
    if crate::models::capital_rig(mesh).is_some() {
        u.arm_pitch[1]
    } else {
        0.0
    }
}

/// A hull as entity.wgsl `vs_main` draws it `f` of the way through the tick: at its
/// place, turned by its heading, pitched by a capital ship's spinal lay (`hull_pitch`)
/// and rolled by its bank. Effects on its gun houses are laid in it: laid level, a
/// pitched ship's guns charge and fire well off their barrels.
#[derive(Clone, Copy)]
pub(super) struct DrawnHull {
    pos: Vec3,
    heading: f32,
    pitch: f32,
    bank: f32,
}

impl DrawnHull {
    pub(super) fn of(u: &UnitInstance, mesh: &str, f: f32) -> Self {
        let turn = (u.heading - u.prev_heading + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        let pitch = if crate::models::capital_rig(mesh).is_some() {
            u.arm_pitch[0] + (u.arm_pitch[1] - u.arm_pitch[0]) * f
        } else {
            0.0
        };
        Self {
            pos: Vec3::from(u.prev_pos).lerp(Vec3::from(u.pos), f),
            heading: u.prev_heading + turn * f,
            pitch,
            bank: u._pad2[0] + (u._pad2[1] - u._pad2[0]) * f,
        }
    }

    /// A direction in the hull's frame (+X forward) in the world.
    pub(super) fn turn(&self, v: Vec3) -> Vec3 {
        let (s, c) = self.bank.sin_cos();
        let rolled = Vec3::new(v.x, v.y * c - v.z * s, v.y * s + v.z * c);
        rot_z(rot_xz(rolled, self.pitch), self.heading)
    }

    /// A point on the hull's model (metres, +X forward) in the world.
    pub(super) fn place(&self, local: Vec3) -> Vec3 {
        self.pos + self.turn(local)
    }
}

fn rot_z(v: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

fn rot_xz(v: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(v.x * c - v.z * s, v.y, v.x * s + v.z * c)
}

/// A point on the ship's model (metres, +X forward) where it is in the world.
pub(super) fn on_hull(hull: &GunHull, local: Vec3) -> Vec3 {
    hull.pos + rot_z(rot_xz(local, hull.pitch), hull.heading)
}

/// Where the ship hull `hull` carries its spinal bore's anchors in the world: the model's
/// record and the ship's frame.
struct Frame {
    bore: &'static SpinalBore,
    hull: GunHull,
}

impl Frame {
    fn at(&self, p: [f32; 3]) -> Vec3 {
        on_hull(&self.hull, Vec3::from(p))
    }

    /// The bore's axis in the world.
    fn axis(&self) -> Vec3 {
        (self.at(self.bore.muzzle) - self.at(self.bore.breech)).normalize_or(Vec3::X)
    }

    /// A point `turn` of the way round stage `i`'s section as it shows outside the hull
    /// (0 at the top of the port flank, round under the belly, 1 at the top of the
    /// starboard flank): on the hull's skin (`SpinalBore::skin`) at every stage but the
    /// last, which is its ring in the open mouth.
    fn rim(&self, i: usize, turn: f32) -> Vec3 {
        let c0 = self.bore.coils[i];
        if i + 1 == self.bore.coils.len() {
            let (s, c) = (turn * std::f32::consts::TAU).sin_cos();
            let r = self.bore.coil_radius;
            return self.at([c0[0], c0[1] + s * r, c0[2] + c * r]);
        }
        let [half, top, bottom] = self.bore.skin;
        let flank = bottom - top;
        let perimeter = 2.0 * flank + 2.0 * half;
        let along = turn.rem_euclid(1.0) * perimeter;
        let (y, drop) = if along < flank {
            (half, top + along)
        } else if along < flank + 2.0 * half {
            (half - (along - flank), bottom)
        } else {
            (-half, bottom - (along - flank - 2.0 * half))
        };
        self.at([c0[0], c0[1] + y, c0[2] - drop])
    }
}

impl Renderer {
    /// The spinal bore of the ship nearest `near` of blueprint `blueprint`, in its frame
    /// this tick.
    fn spinal_frame(&self, blueprint: u32, near: Vec3, reach: f32) -> Option<Frame> {
        let mesh = self
            .blueprints
            .units
            .get(blueprint as usize)?
            .visual
            .mesh
            .as_str();
        let bore = crate::models::spinal_bore(mesh)?;
        let hull = self
            .water_fx
            .guns
            .iter()
            .filter(|g| g.blueprint == blueprint && g.pos.distance(near) < reach)
            .min_by(|a, b| {
                a.pos
                    .distance_squared(near)
                    .total_cmp(&b.pos.distance_squared(near))
            })?;
        Some(Frame {
            bore,
            hull: hull.clone(),
        })
    }

    /// Whether blueprint `blueprint`'s weapon `weapon` is a spinal bore: its charge and its
    /// shot are drawn here instead of as a giant's on the ground.
    pub(super) fn is_spinal_bore(&self, blueprint: u32, weapon: u8) -> bool {
        weapon == 0
            && self
                .blueprints
                .units
                .get(blueprint as usize)
                .is_some_and(|bp| {
                    bp.weapons.first().is_some_and(|w| w.bore.is_some())
                        && crate::models::spinal_bore(&bp.visual.mesh).is_some()
                })
    }

    /// A spinal bore starts to charge for `seconds`, its muzzle at `muzzle` now.
    pub(super) fn spinal_charge(&mut self, muzzle: Vec3, blueprint: u32, seconds: f32, time: f32) {
        let charges = &mut self.giant_fx.spinal.charges;
        // A fresh charge on the same ship replaces the old one (a retarget).
        charges.retain(|c| c.blueprint != blueprint || c.near.distance(muzzle) > 600.0);
        charges.push(SpinalCharge {
            blueprint,
            near: muzzle,
            start: time,
            end: time + seconds,
        });
    }

    /// Once a tick: each spinal bore still charging, this tick's part of its charge.
    pub(super) fn spinal_charges_tick(&mut self, time: f32) {
        let tick = self.tick_seconds.max(0.02);
        let mut charges = std::mem::take(&mut self.giant_fx.spinal.charges);
        charges.retain(|c| time < c.end + 0.2);
        for charge in &mut charges {
            let Some(frame) = self.spinal_frame(charge.blueprint, charge.near, 600.0) else {
                continue;
            };
            charge.near = frame.hull.pos;
            let f = ((time - charge.start) / (charge.end - charge.start).max(0.1)).clamp(0.0, 1.0);
            self.spinal_charge_step(&frame, f, time, tick);
        }
        charges.append(&mut self.giant_fx.spinal.charges);
        self.giant_fx.spinal.charges = charges;
    }

    fn spinal_charge_step(&mut self, frame: &Frame, f: f32, time: f32, tick: f32) {
        let rings = frame.bore.coils.len();
        // The stages come up in turn, as the coils' light does (`titan_charge`).
        let lit = ((f * rings as f32) as usize + 1).min(rings);
        let heavy = f * f;
        // Each arc outlives its tick by a few: the charge crackles on without gaps.
        let life = tick * 3.5;
        let muzzle = frame.at(frame.bore.muzzle);
        let previous = self.effect_origin.replace(muzzle);
        let outbound = std::mem::replace(&mut self.effect_outbound, true);
        let r = frame.bore.coil_radius;
        // Over each lit stage: arcs crawling over the skin in its gap, more and heavier
        // toward the shot.
        for i in 0..lit {
            for _ in 0..1 + (heavy * 2.0) as usize {
                let a = self.scatter.unit();
                let b = a + 0.12 + self.scatter.unit() * 0.25;
                let (from, to) = (frame.rim(i, a), frame.rim(i, b));
                let width = 1.4 + heavy * 2.4;
                self.arc(from, to, 6, r * 0.3, time, life, width);
            }
            self.push_effect(
                frame.rim(i, 0.5).to_array(),
                time,
                r * (1.2 + heavy),
                life,
                0.0,
                0.0,
            );
        }
        // The front of the charge leaps on to the next stage.
        let front = lit - 1;
        if front + 1 < rings {
            let turn = self.scatter.unit();
            let (from, to) = (frame.rim(front, turn), frame.rim(front + 1, turn + 0.05));
            self.arc(from, to, 10, r * 0.8, time, life, 2.0 + heavy * 3.0);
        }
        // Strokes running the lit length of the spine, more often toward the shot.
        if lit > 1 && self.scatter.unit() < 0.25 + heavy * 0.6 {
            let turn = self.scatter.unit();
            let (from, to) = (frame.rim(0, turn), frame.rim(front, turn));
            self.arc(from, to, 18, r * 1.2, time, life * 1.4, 2.0 + heavy * 3.5);
        }
        self.push_effect(
            frame.rim(front, 0.5).to_array(),
            time,
            r * (2.0 + heavy * 3.0),
            life,
            0.0,
            0.0,
        );
        // The mouth: a ball of ionised air that swells toward the shot, arcs snapping
        // round its lip.
        let axis = frame.axis();
        if f > 0.15 {
            let g = (f - 0.15) / 0.85;
            let size = r * (0.8 + g * g * 2.6);
            let jitter = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            );
            self.push_puff(
                PUFF_ARC_BALL,
                muzzle + axis * size * 0.4 + jitter * size * 0.15,
                axis * 2.0,
                time,
                life * 1.5,
                (size, size * 1.25),
            );
            self.push_effect(muzzle.to_array(), time, size * 3.0, life, 0.0, 0.0);
            let (a, b) = (self.scatter.unit(), self.scatter.unit());
            let lip = rings - 1;
            self.arc(
                frame.rim(lip, a),
                frame.rim(lip, b),
                6,
                r * 0.4,
                time,
                life,
                1.5 + heavy * 2.5,
            );
        }
        // The last stretch: arcs snap off the prow into the air, longer toward the shot.
        if f > 0.6 {
            for _ in 0..1 + ((f - 0.6) * 5.0) as usize {
                let reach = r * (3.0 + (f - 0.6) * 25.0) * (0.5 + self.scatter.unit());
                let dir = (axis
                    + Vec3::new(
                        self.scatter.signed(),
                        self.scatter.signed(),
                        self.scatter.signed(),
                    ) * 1.1)
                    .normalize_or(axis);
                let end = muzzle + dir * reach;
                self.arc(muzzle, end, 10, reach * 0.2, time, life, 1.8 + heavy * 2.5);
                self.push_puff(PUFF_SPARK, end, dir * 12.0, time, 0.3, (0.8, 0.3));
            }
        }
        self.effect_outbound = outbound;
        self.effect_origin = previous;
    }

    /// A spinal bore fires from `from` (its muzzle) at `to`: the charge tears down the
    /// coils, the mouth blazes and throws a pressure ring along the bore, and the channel
    /// punches the cloud deck open where it crosses it.
    pub(super) fn spinal_fire(&mut self, from: Vec3, to: Vec3, blueprint: u32, start: f32) {
        self.giant_fx
            .spinal
            .charges
            .retain(|c| c.blueprint != blueprint || c.near.distance(from) > 600.0);
        let axis = (to - from).normalize_or(Vec3::X);
        let previous = self.effect_origin.replace(from);
        let outbound = std::mem::replace(&mut self.effect_outbound, true);
        if let Some(frame) = self.spinal_frame(blueprint, from, 600.0) {
            let r = frame.bore.coil_radius;
            let rings = frame.bore.coils.len();
            // The whole charge down the spine at once: strokes ring to ring along it.
            for k in 0..3 {
                let turn = k as f32 / 3.0 + self.scatter.unit() * 0.2;
                let mut last = frame.rim(0, turn);
                for i in 1..rings {
                    let next = frame.rim(i, turn + self.scatter.signed() * 0.1);
                    self.arc(last, next, 4, r * 0.4, start, 0.35, 3.0);
                    last = next;
                }
                self.arc(
                    last,
                    frame.at(frame.bore.muzzle),
                    4,
                    r * 0.3,
                    start,
                    0.35,
                    3.0,
                );
            }
            for i in 0..rings {
                self.push_effect(
                    frame.at(frame.bore.coils[i]).to_array(),
                    start,
                    r * 3.0,
                    0.3,
                    0.0,
                    0.0,
                );
            }
        }
        // The mouth: a blinding flash, then a bloom that fades, and the pressure ring
        // thrown out along the bore.
        self.push_effect(from.to_array(), start, 140.0, 0.3, 0.0, 0.0);
        self.push_effect(from.to_array(), start + 0.05, 90.0, 1.4, 0.0, 0.0);
        self.push_shockwave(
            (from + axis * 20.0).to_array(),
            start,
            160.0,
            1.1,
            1.0,
            0.0,
            axis,
        );
        self.push_shockwave(
            (from + axis * 60.0).to_array(),
            start + 0.08,
            90.0,
            1.6,
            0.8,
            0.0,
            axis,
        );
        for _ in 0..24 {
            let dir = (axis * 1.5
                + Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.signed(),
                ))
            .normalize_or(axis);
            let speed = 60.0 + self.scatter.unit() * 120.0;
            self.push_puff(PUFF_SPARK, from, dir * speed, start, 0.6, (1.4, 0.4));
        }
        // Where the channel crosses the cloud deck, it is punched open.
        let length = from.distance(to);
        let steps = (length / 120.0).ceil().max(1.0) as usize;
        let mut punched = 0;
        for s in 0..=steps {
            let p = from.lerp(to, s as f32 / steps as f32);
            let base = self.sky.cloud_base_at(p.truncate());
            if (p.z - base).abs() < 260.0 && punched < 6 {
                self.sky.blast(p, 320.0, 1.0, start);
                punched += 1;
            }
        }
        self.effect_outbound = outbound;
        self.effect_origin = previous;
    }
}
