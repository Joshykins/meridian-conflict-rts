//! A gravitic shock mortar's charge as it is drawn (`SimEvent::WeaponCharging`, the
//! Exarch's shoulder mortar; its sound is the weapon's `charge`), laid each step on the gun
//! as it is then drawn, so it rides the gun as it turns and pitches. While the crystal
//! between the tines winds up (the mirror's `flung_pair`) and the gun draws back:
//!
//! - the crystal lights from within, violet going lilac, its light on the gun growing;
//! - violet motes are drawn in to it from all round, spiralling, more and faster as it fills;
//! - from a third of the way, light runs up the tines' inner faces from the crystal to the
//!   points, and lilac arcs snap across the gap between the tines, walking out to the points;
//! - over the last half, a ring of bent light closes on the mouth, the view bent in toward
//!   it, and a knot of light grows between the points until the shot lets it go.
//!
//! Presentation only; the renderer's own clock.

use glam::Vec3;
use mc_data::{BlueprintId, Weapon};
use mc_sim::mirror::{HousePose, UnitInstance, KIND_GHOST, KIND_WRECK};

use super::super::gravitic_engine_fx::{LILAC, VIOLET, WHITE};
use super::super::hull_frame::GunFrame;
use super::super::Renderer;
use super::{mote_speed, shell_gun, Glow, Pull, ARC, LENS, MOTE};
use crate::gpu_consts::puff;

const GLOW: f32 = puff::WARP_GLOW as f32;
/// Seconds between the steps a charge is laid in, each on the gun as it is then.
const STEP: f32 = 0.05;
/// Guns charging at once that are drawn. A deliberate cosmetic cap: a charge past it is not
/// drawn (its shot still is).
const MAX_CHARGES: usize = 12;
/// Where the crystal hangs between the tines: this share of the way from the trunnions
/// (the weapon's `pivot`) out to the points (its `muzzle`), as `gravitic.rs` builds it.
pub(super) const CRYSTAL: f32 = 0.37;
/// How far the charge has come when the ring of bent light starts closing on the mouth.
const RING_AT: f32 = 0.55;

/// A gravitic shock mortar charging.
#[derive(Clone, Copy)]
pub(super) struct Charge {
    unit: u32,
    blueprint: BlueprintId,
    weapon: u8,
    gun: u16,
    start: f32,
    due: f32,
    /// Time laid so far.
    laid: f32,
    /// Its ring of bent light has been laid (once, at `RING_AT`).
    ringed: bool,
}

/// A mortar's bore in the world: the mouth between the points and the way out of it, the
/// way to the upper tine, and the crystal.
struct Bore {
    mouth: Vec3,
    dir: Vec3,
    up: Vec3,
    crystal: Vec3,
}

impl Bore {
    fn of(gun: &GunFrame, w: &Weapon) -> Self {
        let pivot = w.pivot.map_or(Vec3::ZERO, |p| Vec3::from(p.to_f32()));
        let muzzle = Vec3::from(w.muzzle.to_f32());
        let (mouth, dir) = gun.mouth(w, muzzle);
        let up = gun.place(muzzle + Vec3::Z) - mouth;
        Self {
            mouth,
            dir,
            up: (up - dir * up.dot(dir)).normalize_or(Vec3::Z),
            crystal: gun.place(pivot.lerp(muzzle, CRYSTAL)),
        }
    }
}

impl Renderer {
    /// A gun began charging (`WeaponCharging`). True when it is a gravitic shock mortar,
    /// whose charge this draws (and nothing else should).
    pub(in crate::renderer) fn gravitic_mortar_charging(
        &mut self,
        unit: u32,
        (blueprint, weapon, gun): (BlueprintId, u8, u16),
        time: f32,
    ) -> bool {
        let w = self.gun_weapon(blueprint, weapon, gun);
        if !shell_gun(w) {
            return false;
        }
        let due = time + w.charge_ticks as f32 * self.tick_seconds;
        let fx = &mut self.plasma_fx.shells;
        fx.charges
            .retain(|c| !(c.unit == unit && c.blueprint == blueprint && c.gun == gun));
        if fx.charges.len() < MAX_CHARGES {
            fx.charges.push(Charge {
                unit,
                blueprint,
                weapon,
                gun,
                start: time,
                due,
                laid: time,
                ringed: false,
            });
        }
        true
    }

    /// The next tick of every mortar's charge, on its gun as it is drawn (from
    /// `upload_sim`); `houses` is this tick's `RenderFrame::houses`. A charge whose shot
    /// never came (the target died) just stops at its end.
    pub(in crate::renderer) fn gravitic_mortar_tick(
        &mut self,
        units: &[UnitInstance],
        houses: &[HousePose],
        time: f32,
    ) {
        let tick = self.tick_seconds.max(0.02);
        self.plasma_fx.shells.charges.retain(|c| time < c.due);
        let blueprints = self.blueprints.clone();
        for i in 0..self.plasma_fx.shells.charges.len() {
            let c = self.plasma_fx.shells.charges[i];
            let until = (time + tick).min(c.due);
            if c.laid >= until {
                continue;
            }
            let live = |u: &&UnitInstance| {
                u.unit_id == c.unit && u.owner_flags & (KIND_WRECK | KIND_GHOST) == 0
            };
            let Some(u) = units.iter().find(live).copied() else {
                continue;
            };
            let w = self.gun_weapon(c.blueprint, c.weapon, c.gun).clone();
            let mesh = &blueprints.unit(c.blueprint).visual.mesh;
            let from = c.laid.max(time);
            let steps = ((until - from) / STEP).ceil().max(1.0) as usize;
            let span = (until - from) / steps as f32;
            for k in 0..steps {
                let when = from + (until - from) * k as f32 / steps as f32;
                let along = ((when - time) / tick).clamp(0.0, 1.0);
                let gun = GunFrame::of(
                    &u,
                    &w,
                    c.gun as usize,
                    houses,
                    mesh,
                    w.turret_turn > 0,
                    along,
                );
                let bore = Bore::of(&gun, &w);
                let f = ((when - c.start) / (c.due - c.start).max(0.01)).clamp(0.0, 1.0);
                self.gravitic_mortar_step(&bore, f, when, span);
                let charge = &mut self.plasma_fx.shells.charges[i];
                if f >= RING_AT && !charge.ringed {
                    charge.ringed = true;
                    let left = (c.due - when).max(0.1);
                    self.gravitic_mortar_ring(&bore, when, left);
                }
            }
            self.plasma_fx.shells.charges[i].laid = until;
        }
    }

    /// `span` seconds of a mortar's charge `f` of the way done, on `b`.
    fn gravitic_mortar_step(&mut self, b: &Bore, f: f32, when: f32, span: f32) {
        let life = span * 2.0;
        // The crystal lit from within, violet going lilac.
        let s = 0.5 + 1.1 * f;
        let flicker = 0.85 + 0.15 * self.scatter.unit();
        self.push_lit(
            GLOW,
            b.crystal,
            Vec3::ZERO,
            when,
            life,
            (s, s),
            VIOLET.lerp(LILAC, f * f) * (0.4 + 2.6 * f * f) * flicker,
            0.0,
        );
        // Motes drawn in to it from all round, spiralling, more and faster as it fills.
        let motes = (1.0 + f * 4.0 + self.scatter.unit()) as usize;
        for _ in 0..motes {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            )
            .normalize_or(Vec3::Z);
            let d = 3.5 + 4.5 * self.scatter.unit();
            let from = b.crystal + out * d;
            let flight = 0.45 - 0.2 * f;
            let speed = mote_speed(d, flight);
            let swirl = b.dir.cross(out).normalize_or_zero() * speed * 0.35;
            let roll = self.scatter.unit();
            let late = self.scatter.unit() * span;
            self.push_lit(
                MOTE,
                from,
                -out * speed + swirl,
                when + late,
                flight,
                (0.22, 0.08),
                VIOLET.lerp(LILAC, roll) * (2.5 + 3.0 * f),
                0.0,
            );
        }
        // Light running up the tines' inner faces from the crystal to the points.
        if f > 0.3 {
            for _ in 0..(1 + (f * 3.0) as usize) {
                let face = b.up
                    * if self.scatter.unit() < 0.5 {
                        0.42
                    } else {
                        -0.42
                    };
                let (from, to) = (b.crystal + face, b.mouth + face * 0.6);
                let flight = 0.35 - 0.15 * f;
                let speed = mote_speed(from.distance(to), flight);
                let late = self.scatter.unit() * span;
                self.push_lit(
                    MOTE,
                    from,
                    (to - from).normalize_or(b.dir) * speed,
                    when + late,
                    flight,
                    (0.16, 0.06),
                    LILAC * (3.0 + 4.0 * f),
                    0.0,
                );
            }
        }
        // Lilac arcs snapping across the gap, walking out to the points as it fills.
        if f > 0.25 && self.scatter.unit() < 0.25 + 0.9 * f {
            let out = ((f - 0.25) / 0.75 + 0.15 * self.scatter.signed()).clamp(0.0, 1.0);
            let p = b.crystal.lerp(b.mouth, out);
            let from = p + b.up * 0.5;
            let to = p - b.up * 0.5 + b.dir * 0.3 * self.scatter.signed();
            let roll = self.scatter.unit();
            self.push_lit(
                ARC,
                from,
                to - from,
                when,
                0.05 + 0.06 * roll,
                (0.16, 0.16),
                LILAC.lerp(WHITE, 0.3 * roll) * (3.0 + 4.0 * f),
                0.0,
            );
        }
        // A knot of light growing between the points over the last third.
        if f > 0.65 {
            let k = (f - 0.65) / 0.35;
            let s = 0.3 + 0.7 * k;
            self.push_lit(
                GLOW,
                b.mouth,
                Vec3::ZERO,
                when,
                life,
                (s, s),
                LILAC.lerp(WHITE, k) * (0.5 + 2.5 * k),
                0.0,
            );
        }
        // Its light on the gun and the shoulder.
        self.plasma_fx.shells.light(Glow {
            pos: b.crystal,
            color: VIOLET.lerp(LILAC, f) * (15.0 + 260.0 * f * f),
            range: 5.0 + 9.0 * f,
            start: when,
            life: span * 1.6,
        });
    }

    /// The ring of bent light closing on the mouth over the charge's last `left` seconds,
    /// and the view bent in toward it, strongest at the shot.
    fn gravitic_mortar_ring(&mut self, b: &Bore, when: f32, left: f32) {
        self.push_lit(
            LENS,
            b.mouth,
            Vec3::ZERO,
            when,
            left + 0.05,
            (3.4, 0.5),
            VIOLET * 6.0,
            0.0,
        );
        self.plasma_fx.shells.pulls.push(Pull {
            at: b.mouth,
            radius: 3.5,
            start: when,
            life: left + 0.3,
            peak: left / (left + 0.3),
        });
    }
}
