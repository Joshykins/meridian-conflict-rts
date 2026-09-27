//! The great guns (`Weapon::great_gun`): the Culverin, a supergun that shells bases from
//! across the map. A conventional gun (docs/STYLE.md), only so big that each shot is an
//! event on the whole map:
//!
//! - **Fire**: an orange-white flash out of the muzzle with a jet of burning propellant
//!   after it, a shock front out along the bore and a ring blast flattening the ground
//!   round the emplacement (dust, trees), a bank of grey muzzle smoke that rolls out
//!   ahead and hangs for most of a reload, the clouds overhead shoved about.
//! - **Flight**: the shell climbs kilometres, up through the clouds and over everything,
//!   and leaves a smoke trail seen from the strategic view (the nukes' reserved trail
//!   puffs, `NukeFx::lay_trail`); where it crosses the cloud layer the cloud is stirred.
//! - **Hit**: a white flash and an orange fireball, a column of earth thrown straight up
//!   (the hit's signature: nothing else throws one), a ring of dust rolling out, clods
//!   thrown far, a crater, and a dark plume that rises and drifts off for most of a
//!   minute. Heard (and felt) across the map through the weapon's `far` sound.
//!
//! The ordinary shell effects are not drawn for these guns (`great_gun_event` takes the
//! whole event). `MERIDIAN_GREAT_GUN=0` draws them as ordinary shells.

use super::craters::CraterStyle;
use super::nuke_fx::PUFF_STRATEGIC_TRAIL;
use super::water_fx::PUFF_STEAM;
use super::{
    Puff, Renderer, PUFF_CLOD, PUFF_DUST, PUFF_FIREBALL, PUFF_SHOCK_DUST, PUFF_SMOKE, PUFF_SPARK,
    PUFF_TREE_SMOKE,
};
use glam::{Vec2, Vec3};
use mc_data::BlueprintId;
use mc_sim::mirror::{ProjectileInstance, SimEvent};

/// Effect kinds (sprites.wgsl): powder orange, white-hot, the ground disc.
const ORANGE: f32 = 1.0;
const WHITE_HOT: f32 = 4.0;
const GROUND_DISC: f32 = 5.0;
/// Metres of trail between two of its puffs, and how long it hangs.
const TRAIL_STEP: f32 = 34.0;
const TRAIL_LIFE: f32 = 30.0;
/// Shells in the air before their trails are laid in longer steps. A trail is some
/// 600 puffs at `TRAIL_STEP`, and every trail shares the missiles' reserved slots
/// (`NUKE_PUFF_SLOTS`): a battery firing filled them and the oldest trails, still
/// hanging, were dropped half-way along.
const TRAIL_SHELLS: f32 = 4.0;
/// No trail this near the muzzle: the muzzle smoke is there.
const TRAIL_CLEAR: f32 = 90.0;
/// Depth of the cloud layer over its base (as the heavy rail takes it), and the most
/// stirs one shell makes going up and coming down.
const CLOUD_DECK: f32 = 380.0;
const MOST_STIRS: u8 = 12;
/// Metres a shell can be off where it was last seen and still be taken for it.
const MATCH: f32 = 8.0;

fn enabled() -> bool {
    std::env::var("MERIDIAN_GREAT_GUN").map_or(true, |v| v != "0")
}

#[derive(Default)]
pub(super) struct GreatGunFx {
    shells: Vec<Shell>,
    /// The render clock at the last tick, to notice it going back.
    last: f32,
}

/// One shell in the air.
#[derive(Clone, Copy)]
struct Shell {
    /// Where it left the muzzle.
    muzzle: Vec3,
    /// Where it was last seen, and when.
    at: Vec3,
    seen: f32,
    /// The trail is laid up to here.
    laid: Vec3,
    /// Metres since the last cloud stir, and how many it has made.
    since_stir: f32,
    stirs: u8,
    scale: f32,
}

impl Renderer {
    fn great_weapon(&self, blueprint: BlueprintId, weapon: u8) -> Option<f32> {
        let w = self
            .blueprints
            .unit(blueprint)
            .weapons
            .get(weapon as usize)?;
        (w.great_gun > 0.0 && enabled()).then_some(w.great_gun)
    }

    /// Draws `event` when it is a great gun's: true when that is all of it.
    pub(super) fn great_gun_event(&mut self, event: &SimEvent, time: f32) -> bool {
        match event {
            SimEvent::ShotFired {
                pos,
                vel,
                travel,
                blueprint,
                weapon,
                ..
            } => {
                let Some(scale) = self.great_weapon(*blueprint, *weapon) else {
                    return false;
                };
                let w = &self.blueprints.unit(*blueprint).weapons[*weapon as usize];
                let barrel = (w.muzzle.to_f32()[0] - w.pivot.map_or(0.0, |p| p.to_f32()[0])).abs();
                let at = Vec3::from(pos.to_f32()) - Vec3::from(travel.to_f32());
                let dir = Vec3::from(vel.to_f32()).normalize_or(Vec3::Z);
                self.great_gun.shells.push(Shell {
                    muzzle: at,
                    at: Vec3::from(pos.to_f32()),
                    seen: time,
                    laid: at,
                    since_stir: 0.0,
                    stirs: 0,
                    scale,
                });
                let outbound = std::mem::replace(&mut self.effect_outbound, true);
                self.great_muzzle(at, dir, barrel, scale, time);
                self.effect_outbound = outbound;
                true
            }
            SimEvent::Impact {
                pos,
                after,
                on_shield,
                blueprint,
                weapon,
                ..
            } => {
                let Some(scale) = self.great_weapon(*blueprint, *weapon) else {
                    return false;
                };
                let at = Vec3::from(pos.to_f32());
                let start = time + after.to_f32() * self.tick_seconds;
                // The shell is down: its trail ends where it landed.
                if let Some(i) = self.great_shell_near(at, 120.0) {
                    let s = self.great_gun.shells.swap_remove(i);
                    self.lay_shell_trail(s, at, start);
                }
                if *on_shield {
                    // The dome takes it (the ordinary shield hit), with the shell's flash.
                    self.push_effect(at.to_array(), start, 70.0 * scale, 0.3, WHITE_HOT, 0.0);
                    return false;
                }
                self.great_impact(at, scale, start);
                true
            }
            _ => false,
        }
    }

    /// The shell whose last sighting is nearest `at` and within `reach`.
    fn great_shell_near(&self, at: Vec3, reach: f32) -> Option<usize> {
        self.great_gun
            .shells
            .iter()
            .enumerate()
            .map(|(i, s)| (i, s.at.distance(at)))
            .filter(|(_, d)| *d < reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// Once a tick: each shell in the air found among this tick's projectiles (by where
    /// it was last seen), its trail laid on to where it is now.
    pub(super) fn great_gun_tick(&mut self, projectiles: &[ProjectileInstance], time: f32) {
        if !enabled() {
            return;
        }
        if time + 1.0 < self.great_gun.last {
            // The clock went back (a restaged backdrop): nothing of the old world is left.
            self.great_gun = GreatGunFx::default();
        }
        self.great_gun.last = time;
        if self.great_gun.shells.is_empty() {
            return;
        }
        for i in 0..self.great_gun.shells.len() {
            let s = self.great_gun.shells[i];
            let found = projectiles
                .iter()
                .filter(|p| {
                    Vec3::from(p.prev_pos).distance(s.at) < MATCH
                        || Vec3::from(p.pos).distance(s.at) < MATCH
                })
                .map(|p| Vec3::from(p.pos))
                .filter(|p| *p != s.at)
                .min_by(|a, b| a.distance(s.at).total_cmp(&b.distance(s.at)));
            let Some(now) = found else {
                continue;
            };
            let laid = self.lay_shell_trail(s, now, time);
            let shell = &mut self.great_gun.shells[i];
            shell.at = now;
            shell.seen = time;
            shell.laid = laid.0;
            shell.since_stir = laid.1;
            shell.stirs = laid.2;
        }
        // Not seen for a while: gone into something the sim did not report as a hit.
        let tick = self.tick_seconds.max(0.02);
        self.great_gun.shells.retain(|s| time - s.seen < tick * 4.0);
    }

    /// The trail from where shell `s` has it laid to `to`, reached at `time`: seamless
    /// strategic trail puffs, and the cloud stirred where it crosses the layer. Returns
    /// where it is laid to, the metres since the last stir and the stirs made.
    fn lay_shell_trail(&mut self, s: Shell, to: Vec3, time: f32) -> (Vec3, f32, u8) {
        // Longer steps the more shells are up (each puff's tent spans its step, so the
        // line stays whole), so every trail fits the reserved slots for its whole life.
        let step = TRAIL_STEP * (self.great_gun.shells.len() as f32 / TRAIL_SHELLS).max(1.0);
        let span = to - s.laid;
        let len = span.length();
        let count = (len / step).floor() as usize;
        if count == 0 {
            return (s.laid, s.since_stir, s.stirs);
        }
        let dir = span / len;
        let tick = self.tick_seconds.max(0.02);
        let (mut since, mut stirs) = (s.since_stir, s.stirs);
        for k in 1..=count.min(400) {
            let p = s.laid + dir * step * k as f32;
            let born = time - tick * (1.0 - k as f32 / count as f32);
            since += step;
            let base = self.sky.cloud_base_at(p.truncate());
            if stirs < MOST_STIRS
                && since > 120.0
                && p.z > base - 20.0
                && p.z < base + CLOUD_DECK + 40.0
            {
                self.sky.blast(p, 110.0 * s.scale, 1.2, born);
                since = 0.0;
                stirs += 1;
            }
            if p.distance(s.muzzle) < TRAIL_CLEAR || p.z < self.ground_height(p.truncate()) + 1.0 {
                continue;
            }
            self.nuke_fx.lay_trail(Puff {
                origin: p.to_array(),
                opacity: 1.0,
                pos: p.to_array(),
                start: born,
                // A tent a step either side (puffs.wgsl `strategic_trail`): no seams.
                vel: (dir * step).to_array(),
                life: TRAIL_LIFE,
                params: [
                    5.0 * s.scale,
                    46.0 * s.scale,
                    PUFF_STRATEGIC_TRAIL,
                    self.scatter.unit(),
                ],
                appearance: [-1.0, -1.0, -1.0, 0.6],
            });
        }
        (s.laid + dir * step * count as f32, since, stirs)
    }

    /// The gun going off at `at`, fired along `dir` from a barrel `barrel` metres long.
    fn great_muzzle(&mut self, at: Vec3, dir: Vec3, barrel: f32, s: f32, time: f32) {
        let side = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
        let up = side.cross(dir).normalize_or(Vec3::Z);
        // The flash: a white-hot core, an orange bloom round it, and the plug of burning
        // propellant thrown out ahead lighting up after it.
        self.push_effect(at.to_array(), time, 60.0 * s, 0.18, WHITE_HOT, 0.0);
        self.push_effect(at.to_array(), time, 130.0 * s, 0.42, ORANGE, 1.0);
        self.push_effect(
            (at + dir * 40.0 * s).to_array(),
            time + 0.03,
            90.0 * s,
            0.36,
            ORANGE,
            0.6,
        );
        for i in 0..9 {
            let t = i as f32 / 8.0;
            let roll = self.scatter.unit();
            let jitter = (side * self.scatter.signed() + up * self.scatter.signed()) * 12.0 * s;
            self.push_puff(
                PUFF_FIREBALL,
                at + dir * (4.0 + 50.0 * t) * s,
                dir * (60.0 + 110.0 * t) * s + jitter,
                time + 0.015 * i as f32,
                0.5 + 0.4 * roll,
                ((10.0 + 8.0 * t) * s, (26.0 + 30.0 * t) * s),
            );
        }
        // The shock front out along the bore, and the ring blast round the emplacement
        // that flattens the ground (the shockwave's own dust and bent trees).
        self.push_shockwave(
            (at + dir * 25.0 * s).to_array(),
            time,
            520.0 * s,
            1.3,
            0.8,
            ORANGE,
            dir,
        );
        let base = at - dir * barrel;
        let ground = Vec3::new(base.x, base.y, self.ground_height(base.truncate()) + 2.0);
        self.push_shockwave(
            ground.to_array(),
            time + 0.02,
            260.0 * s,
            1.1,
            0.7,
            ORANGE,
            Vec3::ZERO,
        );
        for k in 0..56 {
            let a = (k as f32 + self.scatter.unit() * 0.7) / 56.0 * std::f32::consts::TAU;
            let out = Vec3::new(a.cos(), a.sin(), 0.0);
            let roll = self.scatter.unit();
            self.push_puff(
                PUFF_SHOCK_DUST,
                ground + out * 30.0 * s,
                out * (50.0 + 20.0 * roll) * s + Vec3::Z * 3.0,
                time + 0.04,
                3.0 + 1.0 * roll,
                (14.0 * s, 42.0 * s),
            );
        }
        // The muzzle smoke: a grey bank rolling out ahead of the gun, spreading as it
        // slows, and hanging on the wind for most of a reload.
        let wind: Vec2 = self.sky.wind_heading();
        for i in 0..22 {
            let t = i as f32 / 21.0;
            let roll = self.scatter.unit();
            let spread = (side * self.scatter.signed() + up * self.scatter.signed()) * 0.3;
            self.push_puff(
                PUFF_SMOKE,
                at + dir * 6.0 * s,
                (dir + spread).normalize_or(dir) * (20.0 + 130.0 * t) * s
                    + (wind * 2.0).extend(1.5),
                time + 0.02 + 0.01 * i as f32,
                8.0 + 6.0 * roll,
                ((8.0 + 6.0 * t) * s, (40.0 + 40.0 * t) * s),
            );
        }
        // A ring of smoke thrown out round the muzzle, across the bore.
        for k in 0..14 {
            let a = (k as f32 + self.scatter.unit() * 0.5) / 14.0 * std::f32::consts::TAU;
            let out = side * a.cos() + up * a.sin();
            let roll = self.scatter.unit();
            self.push_puff(
                PUFF_SMOKE,
                at + out * 4.0 * s,
                out * (50.0 + 20.0 * roll) * s + dir * 15.0 * s,
                time + 0.02,
                5.0 + 3.0 * roll,
                (6.0 * s, 30.0 * s),
            );
        }
        // Burning grains of propellant thrown out in a cone.
        for _ in 0..60 {
            let spray = (dir * 2.0
                + Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.signed(),
                ) * 0.6)
                .normalize_or_zero();
            let speed = (80.0 + self.scatter.unit() * 200.0) * s;
            let life = 0.4 + self.scatter.unit() * 0.8;
            self.push_puff(PUFF_SPARK, at, spray * speed, time, life, (1.2 * s, 0.3));
        }
        // The clouds over it are shoved about.
        self.sky.blast(at + dir * 120.0 * s, 260.0 * s, 1.3, time);
    }

    /// A shell landing at `at`.
    fn great_impact(&mut self, at: Vec3, s: f32, start: f32) {
        // The flash: white-hot, then the orange of the burst and its disc on the ground.
        self.push_effect(at.to_array(), start, 70.0 * s, 0.22, WHITE_HOT, 0.0);
        self.push_effect(
            (at + Vec3::Z * 10.0).to_array(),
            start,
            120.0 * s,
            0.6,
            ORANGE,
            1.0,
        );
        self.push_shockwave(
            at.to_array(),
            start,
            440.0 * s,
            1.6,
            0.8,
            ORANGE,
            Vec3::ZERO,
        );
        self.sky.blast(at, 300.0 * s, 1.5, start);
        if let Some(water) = self.at_sea(at, 4.0) {
            // Into the sea: a tall column of white water, a ring running out, steam.
            let surface = Vec3::new(at.x, at.y, water);
            self.water_splash(surface, start, 18.0 * s, 2.4);
            self.push_ripple(surface + Vec3::Z * 3.0, start, 140.0 * s, 8.0, 3.0, 1.0);
            for i in 0..14 {
                let out = Vec3::new(self.scatter.signed(), self.scatter.signed(), 2.0)
                    .normalize_or_zero();
                self.push_puff(
                    PUFF_STEAM,
                    surface,
                    out * (25.0 + 9.0 * i as f32) * s,
                    start + 0.03 * i as f32,
                    5.0,
                    (10.0 * s, 40.0 * s),
                );
            }
            return;
        }
        self.push_effect(at.to_array(), start, 170.0 * s, 1.0, GROUND_DISC, ORANGE);
        self.add_crater_styled(at.truncate(), 40.0 * s, 0.5, start, CraterStyle::Blast);
        // The fireball, rolling up off the ground.
        for i in 0..12 {
            let out = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 0.6;
            let roll = self.scatter.unit();
            self.push_puff(
                PUFF_FIREBALL,
                at + out * 20.0 * s + Vec3::Z * 4.0,
                (out + Vec3::Z) * (18.0 + 22.0 * roll) * s,
                start + 0.02 * i as f32,
                1.4 + 0.8 * roll,
                (18.0 * s, 58.0 * s),
            );
        }
        // Its signature: a column of earth thrown straight up, tall and narrow, falling
        // back as a curtain.
        for i in 0..18 {
            let t = i as f32 / 17.0;
            let lean = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 0.12;
            let roll = self.scatter.unit();
            self.push_puff(
                PUFF_DUST,
                at + Vec3::Z * 3.0,
                (Vec3::Z + lean) * (50.0 + 150.0 * t) * s,
                start + 0.01 * i as f32,
                4.0 + 3.0 * roll,
                ((6.0 + 6.0 * t) * s, (30.0 + 20.0 * t) * s),
            );
        }
        // The base surge: a ring of dust rolling out along the ground.
        for k in 0..30 {
            let a = (k as f32 + self.scatter.unit()) / 30.0 * std::f32::consts::TAU;
            let out = Vec3::new(a.cos(), a.sin(), 0.0);
            let roll = self.scatter.unit();
            self.push_puff(
                PUFF_DUST,
                at + out * 10.0 * s + Vec3::Z * 2.0,
                out * (60.0 + 35.0 * roll) * s + Vec3::Z * 3.0,
                start + 0.1,
                5.0 + 2.0 * roll,
                (10.0 * s, 40.0 * s),
            );
        }
        // Earth and stone thrown far, sparks off the burst.
        for _ in 0..110 {
            let vel = self.scatter.upward(0.3) * (40.0 + self.scatter.unit() * 130.0) * s;
            let size = (0.9 + self.scatter.unit() * 1.6) * s;
            let roll = self.scatter.unit();
            self.push_puff(
                PUFF_CLOD,
                at + Vec3::Z,
                vel,
                start,
                2.5 + roll * 2.5,
                (size, 0.3),
            );
        }
        for _ in 0..50 {
            let vel = self.scatter.upward(0.25) * (50.0 + self.scatter.unit() * 110.0) * s;
            let roll = self.scatter.unit();
            self.push_puff(
                PUFF_SPARK,
                at + Vec3::Z,
                vel,
                start,
                0.5 + roll * 0.7,
                (1.2 * s, 0.25),
            );
        }
        // The plume: a column of dark smoke climbing off the hit and leaning off
        // downwind, hanging for most of a minute; each puff starts higher up the column
        // than the last, so it stands rather than lying on the crater.
        let wind: Vec2 = self.sky.wind_heading();
        for i in 0..14 {
            let t = i as f32 / 13.0;
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 10.0 * s;
            let rise = 9.0 + self.scatter.unit() * 6.0;
            let carry = (wind * (2.0 + self.scatter.unit() * 2.0)).extend(rise);
            let life = 30.0 + self.scatter.unit() * 16.0;
            self.push_puff(
                PUFF_TREE_SMOKE,
                at + off + Vec3::Z * (12.0 + 90.0 * t) * s,
                carry,
                start + 0.5 + 0.3 * i as f32,
                life,
                ((14.0 + 10.0 * t) * s, (50.0 + 40.0 * t) * s),
            );
        }
    }
}
