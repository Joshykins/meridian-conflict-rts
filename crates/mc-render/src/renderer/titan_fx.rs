//! The Behemoth's presentation: the weight of its footfalls, the size of its guns, and
//! what they leave behind.

use mc_data::Blueprints;

/// Whether blueprint `id` is a striding giant (`Motion::stride`).
pub(super) fn strides(blueprints: &Blueprints, id: u32) -> bool {
    blueprints
        .units
        .get(id as usize)
        .and_then(|bp| bp.motion)
        .is_some_and(|m| m.stride)
}

use super::water_fx::{PUFF_COLUMN, PUFF_SPRAY};
use super::{
    Renderer, PUFF_BOLT, PUFF_CLOD, PUFF_DUST, PUFF_SHOCK_SMOKE, PUFF_SPARK,
};
use glam::Vec3;

/// Seconds a giant's footprint lies on the ground.
pub(super) const FOOTPRINT_LIFE: f32 = 300.0;

/// A bore whose blast reaches this far lands as a cataclysm (the AEB-3).
pub(super) const CATACLYSM_SPLASH: f32 = 60.0;

/// A giant bore's fireball of ionised air (puffs.wgsl `PUFF_ARC_BALL`).
const PUFF_ARC_BALL: f32 = 35.0;

/// How fast a giant bore's hurricane turns at its heart, radians a second (the sky's
/// `conjure_storm` spin; the rim turns slower, sky.rs `VORTEX_SHEAR`).
const STORM_SPIN: f32 = 0.12;
/// Its bands, and how far round each winds from the heart to its rim, radians
/// (clouds_sim.wgsl `VORTEX_ARMS`, `VORTEX_WIND`).
const STORM_ARMS: usize = 5;
const STORM_WIND_UP: f32 = 4.0;

/// Metres of pressure front a giant's footfall throws out over the ground.
const STEP_REACH: f32 = 58.0;

impl Renderer {
    /// A giant's foot coming down (`Motion::stride`) at `plant` (on the ground), facing
    /// `forward`, its sole `sole` = (metres behind the ankle, ahead, across), at `start`.
    /// The ground takes the whole machine's weight at once: a pressure front runs out
    /// over the ground (trees lean away from it), a ring of dust bursts from under the
    /// sole and hangs, clods are thrown up; in the shallows, a sheet of spray instead.
    pub(super) fn giant_footfall(&mut self, plant: Vec3, forward: Vec3, sole: [f32; 3], start: f32) {
        let water = self.map_info.water_level.to_f32();
        let ground = self.ground_height(plant.truncate());
        let wet = ground < water - 0.3;
        let at = plant.truncate().extend(ground.max(water));
        let length = (sole[1] - sole[0]).max(4.0);
        let width = sole[2].max(3.0);
        let reach = length.max(width);
        // Everything scales with the sole: the first pass was sized for a 30 m foot.
        let big = (reach / 30.0).clamp(1.0, 6.0);
        let previous = self.effect_origin.replace(at);
        // Its own field reaches down round its feet: what its feet throw up is not cut by it.
        let outbound = std::mem::replace(&mut self.effect_outbound, true);
        // The pressure front: dust-coloured, low and wide. Its ground dust is laid by
        // `push_shockwave` in swept bands, so the ring reads as a wave, not dots.
        self.push_shockwave((at + Vec3::Z * 1.5).to_array(), start, STEP_REACH * big, 1.15 + 0.2 * big, 0.6, 1.0, Vec3::ZERO);
        let side = Vec3::new(-forward.y, forward.x, 0.0);
        if wet {
            // A sheet of water kicked out all round and a column where the foot went in.
            for k in 0..18 {
                let a = k as f32 / 18.0 * std::f32::consts::TAU + self.scatter.signed() * 0.15;
                let out = forward * a.cos() + side * a.sin();
                let pos = at + out * reach * 0.5 + Vec3::Z * 0.5;
                let vel = (out * (9.0 + self.scatter.unit() * 7.0) + Vec3::Z * (8.0 + self.scatter.unit() * 9.0)) * big.sqrt();
                let life = 1.6 + self.scatter.unit();
                self.push_puff(PUFF_SPRAY, pos, vel, start, life, (2.5 * big, 9.0 * big));
            }
            self.push_puff(PUFF_COLUMN, at, Vec3::Z * 14.0 * big.sqrt(), start, 2.2, (width * 0.6, width * 1.4));
            self.effect_outbound = outbound;
            self.effect_origin = previous;
            return;
        }
        // Dust bursting out from under the sole, heaviest to the sides it spreads to.
        for k in 0..20 {
            let a = k as f32 / 20.0 * std::f32::consts::TAU + self.scatter.signed() * 0.2;
            let out = (forward * a.cos() * 0.8 + side * a.sin()).normalize_or_zero();
            let pos = at + out * reach * (0.35 + self.scatter.unit() * 0.25) + Vec3::Z * 0.8;
            let vel = (out * (12.0 + self.scatter.unit() * 10.0) + Vec3::Z * (1.5 + self.scatter.unit() * 3.0)) * big.sqrt();
            let life = 2.8 + self.scatter.unit() * 1.8;
            let grow = (15.0 + self.scatter.unit() * 6.0) * big;
            self.push_puff(PUFF_DUST, pos, vel, start, life, (4.5 * big, grow));
        }
        // A low skirt of it that settles slowly round the foot.
        for k in 0..8 {
            let a = k as f32 / 8.0 * std::f32::consts::TAU + self.scatter.signed() * 0.3;
            let out = forward * a.cos() + side * a.sin();
            let pos = at + out * reach * 0.8 + Vec3::Z * 1.2;
            let vel = (out * (3.0 + self.scatter.unit() * 3.0) + Vec3::Z * 0.6) * big.sqrt();
            let life = 6.5 + self.scatter.unit() * 2.5;
            self.push_puff(PUFF_SHOCK_SMOKE, pos, vel, start + 0.1, life, (9.0 * big, 26.0 * big));
        }
        // Clods and grit thrown up from the edges of the sole.
        for _ in 0..16 {
            let a = self.scatter.unit() * std::f32::consts::TAU;
            let out = forward * a.cos() + side * a.sin();
            let pos = at + out * reach * 0.5 + Vec3::Z * 1.0;
            let vel = (out * (5.0 + self.scatter.unit() * 9.0) + Vec3::Z * (9.0 + self.scatter.unit() * 12.0)) * big.sqrt();
            let (life, size) = (1.6 + self.scatter.unit() * 0.8, (0.7 + self.scatter.unit() * 0.9) * big);
            self.push_puff(PUFF_CLOD, pos, vel, start, life * big.sqrt(), (size, 0.3));
        }
        self.effect_outbound = outbound;
        self.effect_origin = previous;
    }

    /// Where a giant bore's discharge lands (`splash` metres of blast): the heart goes out
    /// of whatever it hits. A flash that lights the clouds, a blue-white pressure front
    /// hundreds of metres across that bends the trees and stirs the sky, a lightning
    /// strike down from the cloud base into the hit with forks running out over the
    /// ground, a glassed crater, a fireball, debris, and a column of smoke that climbs
    /// and hangs.
    pub(super) fn cataclysm(&mut self, to: Vec3, splash: f32, start: f32) {
        let ground = self.ground_height(to.truncate());
        let at = to.truncate().extend(ground.max(to.z.min(ground + splash * 0.3)));
        let previous = self.effect_origin.replace(at);
        let outbound = std::mem::replace(&mut self.effect_outbound, true);
        // The pressure front, and a second slower one behind it.
        self.push_shockwave((at + Vec3::Z * 4.0).to_array(), start, splash * 3.2, 1.9, 1.0, 0.0, Vec3::ZERO);
        self.push_shockwave((at + Vec3::Z * 2.0).to_array(), start + 0.25, splash * 1.8, 2.6, 0.8, 1.0, Vec3::ZERO);
        self.sky.blast(at, splash * 5.0, 1.0, start);
        self.sky.strike(at + Vec3::Z * splash, start, 1.4, 2.2, true);
        // The flash: blinding for a moment, then a blue-white bloom that fades over seconds.
        self.push_effect(at.to_array(), start, splash * 1.1, 0.35, 0.0, 0.0);
        self.push_effect((at + Vec3::Z * splash * 0.2).to_array(), start + 0.05, splash * 0.7, 1.6, 0.0, 0.0);
        // A bolt from the sky into the hit: the channel's charge earths itself through
        // the air above it too, and forks run out over the ground round it.
        let sky = at + Vec3::new(self.scatter.signed() * 30.0, self.scatter.signed() * 30.0, 380.0);
        self.shell_discharge(sky, at, splash, 0.0, start + 0.08);
        for k in 0..10 {
            let a = k as f32 / 10.0 * std::f32::consts::TAU + self.scatter.signed() * 0.3;
            let out = Vec3::new(a.cos(), a.sin(), 0.0);
            let mut last = at + Vec3::Z * 1.0;
            let reach = splash * (0.8 + self.scatter.unit() * 0.7);
            let kinks = 6;
            for i in 1..=kinks {
                let t = i as f32 / kinks as f32;
                let turn = Vec3::new(-out.y, out.x, 0.0) * self.scatter.signed() * reach * 0.12;
                let mut next = at + out * reach * t + turn;
                next.z = self.ground_height(next.truncate()) + 0.6;
                let delay = t * 0.12 + self.scatter.unit() * 0.05;
                self.bore_fx.lightning(last, next, start + delay, 0.5 - t * 0.2, 2.2 * (1.2 - t * 0.7));
                last = next;
            }
        }
        // The fireball: a solid dome of ionised air that swells out of the hit over a
        // second and a half, white-hot, turning electric blue with cracks of light crawling
        // over it, lifting slowly off the ground as it churns, then going dark and thin.
        let r = splash * 0.9;
        for k in 0..46 {
            let born = (k as f32 / 46.0).powf(0.7) * 1.5;
            let (a, b) = (self.scatter.unit() * std::f32::consts::TAU, self.scatter.unit());
            let up = b.sqrt();
            let dir = Vec3::new(a.cos() * (1.0 - up * up).sqrt(), a.sin() * (1.0 - up * up).sqrt(), up);
            let reach = r * (0.25 + 0.55 * (born / 1.5));
            let pos = at + dir * reach + Vec3::Z * r * 0.15;
            let vel = dir * (6.0 + self.scatter.unit() * 6.0) + Vec3::Z * (5.0 + self.scatter.unit() * 5.0);
            let life = 5.5 + self.scatter.unit() * 3.0;
            let size = r * (0.32 + self.scatter.unit() * 0.18);
            self.push_puff(PUFF_ARC_BALL, pos, vel, start + born, life, (size, size * 1.7));
        }
        // Its white-hot heart, over in a moment.
        for _ in 0..8 {
            let (x, y, z) = (self.scatter.signed(), self.scatter.signed(), self.scatter.unit());
            let pos = at + Vec3::new(x, y, z) * r * 0.2 + Vec3::Z * r * 0.2;
            self.push_puff(PUFF_ARC_BALL, pos, Vec3::Z * 4.0, start, 1.6, (r * 0.4, r * 0.8));
        }
        // Debris and sparks thrown clear.
        for _ in 0..40 {
            let dir = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.4 + self.scatter.unit()).normalize_or_zero();
            let speed = 30.0 + self.scatter.unit() * 60.0;
            let (life, size, spark) =
                (2.5 + self.scatter.unit() * 1.5, 1.2 + self.scatter.unit() * 1.6, 0.9 + self.scatter.unit() * 0.6);
            self.push_puff(PUFF_CLOD, at + Vec3::Z * 2.0, dir * speed, start, life, (size, 0.3));
            self.push_puff(PUFF_SPARK, at + Vec3::Z * 2.0, dir * speed * 1.3, start, spark, (0.8, 0.3));
        }
        for _ in 0..24 {
            let dir = Vec3::new(self.scatter.signed(), self.scatter.signed(), self.scatter.unit()).normalize_or_zero();
            let speed = 50.0 + self.scatter.unit() * 50.0;
            self.push_puff(PUFF_BOLT, at + Vec3::Z * 3.0, dir * speed, start, 0.5, (2.0, 0.5));
        }
        // What the blast leaves: a glassed crater the width of its reach.
        self.add_crater_styled(at.truncate(), splash, 1.0, start, super::craters::CraterStyle::Glassed);
        self.effect_outbound = outbound;
        self.effect_origin = previous;
    }
}

/// A bore's wind-up this long or longer is a giant's (the AEB-3): the charge shows
/// outside the gun as it builds (`Renderer::bore_charge`).
pub(super) const GIANT_CHARGE: f32 = 3.0;

impl Renderer {
    /// A jagged lightning path from `from` to `to` in `kinks` pieces, `wander` metres off
    /// the straight line at most (widest in the middle), from `start` for `life` seconds.
    fn arc(&mut self, from: Vec3, to: Vec3, kinks: usize, wander: f32, start: f32, life: f32, width: f32) {
        let along = (to - from).normalize_or_zero();
        let side = along.cross(Vec3::Z).normalize_or(Vec3::X);
        let up = side.cross(along).normalize_or_zero();
        let mut last = from;
        for k in 1..=kinks {
            let t = k as f32 / kinks as f32;
            let taper = (t * std::f32::consts::PI).sin();
            let (a, b) = (self.scatter.signed(), self.scatter.signed());
            let next = if k == kinks { to } else { from + (to - from) * t + (side * a + up * b) * wander * taper };
            self.bore_fx.lightning(last, next, start, life, width);
            last = next;
        }
    }

    /// A giant bore charging for `seconds` from `time`, its muzzle at `muzzle`: the charge
    /// leaks out as it builds. Arcs crawl and snap round the muzzle, then earth themselves
    /// to the ground round the machine's feet, more and heavier toward the shot, and the
    /// air round the muzzle glows.
    pub(super) fn bore_charge(&mut self, muzzle: Vec3, blueprint: u32, seconds: f32, time: f32) {
        // The machine: the nearest of that blueprint to the muzzle.
        let body = self
            .water_fx
            .guns
            .iter()
            .filter(|g| g.blueprint == blueprint)
            .min_by(|a, b| {
                a.pos.distance_squared(muzzle).total_cmp(&b.pos.distance_squared(muzzle))
            })
            .map_or(muzzle.truncate().extend(self.ground_height(muzzle.truncate())), |g| g.pos);
        let previous = self.effect_origin.replace(muzzle);
        let outbound = std::mem::replace(&mut self.effect_outbound, true);
        let arcs = (seconds * 7.0) as usize;
        for i in 0..arcs {
            // Denser toward the shot: most of the arcs come in the last second or two.
            let f = (i as f32 / arcs as f32).sqrt();
            let start = time + f * seconds * 0.97;
            let heavy = f * f;
            // Round the muzzle: short snapping arcs off the bore's lip.
            let (a, b, c) = (self.scatter.signed(), self.scatter.signed(), self.scatter.signed());
            let off = Vec3::new(a, b, c).normalize_or_zero() * (3.0 + heavy * 6.0);
            self.arc(muzzle, muzzle + off, 3, 1.2, start, 0.12, 0.35 + heavy * 0.5);
            // Every other one earths itself: muzzle to the ground round the feet.
            if i % 2 == 1 && f > 0.3 {
                let (d, e) = (self.scatter.signed(), self.scatter.signed());
                let mut ground = body + Vec3::new(d, e, 0.0) * (20.0 + heavy * 30.0);
                ground.z = self.ground_height(ground.truncate()) + 0.3;
                let kinks = 6 + (heavy * 6.0) as usize;
                self.arc(muzzle, ground, kinks, 4.0 + heavy * 5.0, start, 0.16 + heavy * 0.1, 0.5 + heavy * 0.9);
                self.push_effect(ground.to_array(), start, 2.0 + heavy * 5.0, 0.2, 0.0, 0.0);
                self.push_puff(PUFF_SPARK, ground, Vec3::Z * 8.0, start, 0.4, (0.6, 0.2));
            }
            self.push_effect(muzzle.to_array(), start, 2.5 + heavy * 9.0, 0.25, 0.0, 0.0);
        }
        self.effect_outbound = outbound;
        self.effect_origin = previous;
    }
}

impl Renderer {
    /// A giant bore's storm (`Bore::storm`), struck at `at` from `start`: for `seconds` it
    /// spreads to `radius` as the sim's does (`DischargeStorm::reach`). The sky's own
    /// clouds wheel into a hurricane far wider than it over the strike (`conjure_storm`
    /// with a spin), lightning crawls along its bands inside the cloud and falls out of
    /// them all over the storm, thickest at its spreading edge, and whatever is struck
    /// flashes, melts and goes up as vapour. It is drawn a tick at a time as it goes
    /// (`storms_tick`): scheduled all at once, its thousands of particles overran the
    /// ring and the battle's own ate them.
    pub(super) fn discharge_storm(&mut self, at: Vec3, radius: f32, seconds: f32, start: f32) {
        let cloud_reach = (radius * 2.8).max(900.0);
        self.sky.conjure_storm(at.truncate(), cloud_reach, seconds + 45.0, STORM_SPIN);
        // The storm's cloud base (it hangs a little lower than the fair-weather deck).
        let base = (self.sky.cloud_base_at(at.truncate()) - 40.0).max(at.z + 80.0);
        // The field itself: a blue-white pressure dome swelling slowly to the storm's edge
        // over its whole life, and a brighter inner one behind it.
        let previous = self.effect_origin.replace(at);
        let outbound = std::mem::replace(&mut self.effect_outbound, true);
        self.push_shockwave((at + Vec3::Z * 3.0).to_array(), start, radius, seconds, 1.0, 0.0, Vec3::ZERO);
        self.push_shockwave((at + Vec3::Z * 3.0).to_array(), start + 0.3, radius * 0.55, seconds * 0.7, 1.0, 0.0, Vec3::ZERO);
        self.effect_outbound = outbound;
        self.effect_origin = previous;
        self.giant_fx.storms.push(StormFx { at, radius, start, end: start + seconds, cloud_reach, base, done: false });
    }

    /// Once a tick: each giant bore's storm, this tick's part of it (`discharge_storm`).
    pub(super) fn storms_tick(&mut self, time: f32) {
        let tick = self.tick_seconds.max(0.02);
        let mut storms = std::mem::take(&mut self.giant_fx.storms);
        // The thunderhead hangs on after the storm, raining itself out.
        storms.retain(|s| time < s.end + 14.0);
        for storm in &mut storms {
            if time < storm.start {
                continue;
            }
            self.storm_step(storm, time, tick);
        }
        storms.append(&mut self.giant_fx.storms);
        self.giant_fx.storms = storms;
    }

    fn storm_step(&mut self, storm: &mut StormFx, now: f32, tick: f32) {
        use std::f32::consts::TAU;
        let StormFx { at, radius, start, end, cloud_reach, base, .. } = *storm;
        let seconds = (end - start).max(0.1);
        let f = ((now - start) / seconds).clamp(0.0, 1.0);
        let raging = now < end;
        let grow = 2.0 * f - f * f;
        let r = radius * (0.2 + 0.8 * grow);
        let water = self.map_info.water_level.to_f32();
        let previous = self.effect_origin.replace(at);
        let outbound = std::mem::replace(&mut self.effect_outbound, true);
        // The hurricane's bands, as the sky draws them (clouds_sim `rest_state` wound
        // round by `vortex_warp_in`): band `k`'s crest `s` of the way out to the storm's
        // rim, turned as far as the air there has wheeled since the strike.
        let reach = cloud_reach;
        let turned = STORM_SPIN * (now - start);
        let arm = |k: usize, s: f32| {
            (k as f32 * TAU - STORM_WIND_UP * s) / STORM_ARMS as f32
                + turned / (1.0 + crate::sky::VORTEX_SHEAR * s)
        };
        if !raging {
            if !storm.done {
                // Spent: the last of the charge goes in one blinding flash.
                storm.done = true;
                let heart = at + Vec3::Z * 22.0;
                self.push_effect(heart.to_array(), now, radius * 0.5, 0.6, 0.0, 0.0);
                self.push_shockwave((at + Vec3::Z * 3.0).to_array(), now, radius * 1.2, 1.4, 1.0, 0.0, Vec3::ZERO);
                self.sky.strike(Vec3::new(at.x, at.y, base), now, 1.5, 4.0, true);
            }
            self.effect_outbound = outbound;
            self.effect_origin = previous;
            return;
        }
        // It fades over its last quarter.
        let strength = 1.0 - ((f - 0.75) / 0.25).clamp(0.0, 1.0) * 0.7;
        // A point inside the thunderhead along arm `k`, `s` of the way out: low in the
        // billows, so the stroke is seen from under the deck and lights it from within.
        let in_cloud = |this: &mut Self, k: usize, s: f32| {
            let a = arm(k, s) + this.scatter.signed() * 0.1;
            let lift = (1.0 - s) * 120.0 + this.scatter.signed() * 30.0;
            Vec3::new(at.x + a.cos() * s * reach, at.y + a.sin() * s * reach, base + 50.0 + lift)
        };
        // Lightning crawling out along the arms inside the cloud, lighting them as it goes.
        let crawlers = if strength > 0.6 { 3 } else { 2 };
        for _ in 0..crawlers {
            let k = (self.scatter.unit() * STORM_ARMS as f32) as usize % STORM_ARMS;
            let s0 = 0.05 + self.scatter.unit() * 0.6;
            let span = 0.18 + self.scatter.unit() * 0.22;
            let lag = self.scatter.unit() * tick;
            let width = (3.0 + self.scatter.unit() * 3.0) * strength;
            let mut last = in_cloud(self, k, s0);
            for j in 1..=5 {
                let next = in_cloud(self, k, s0 + span * j as f32 / 5.0);
                self.arc(last, next, 4, last.distance(next) * 0.12, now + lag, 0.2, width);
                if j % 2 == 1 {
                    self.push_effect(next.to_array(), now + lag, 90.0 * strength, 0.25, 0.0, 0.0);
                }
                last = next;
            }
        }
        // Out of the cloud to the ground, straight down out of the arm over the storm's
        // spreading edge, and here and there inside it.
        let bolts = if strength > 0.6 { 5 } else { 3 };
        let mut thunder = self.scatter.unit() < 0.35;
        for b in 0..bolts {
            let k = (self.scatter.unit() * STORM_ARMS as f32) as usize % STORM_ARMS;
            let (d, a) = if b < 3 {
                (r * (0.85 + self.scatter.unit() * 0.15), arm(k, (r / reach).min(1.0)) + self.scatter.signed() * 0.3)
            } else {
                (r * self.scatter.unit().sqrt() * 0.8, self.scatter.unit() * TAU)
            };
            let mut ground = at + Vec3::new(a.cos(), a.sin(), 0.0) * d;
            ground.z = self.ground_height(ground.truncate()).max(water) + 0.3;
            let (dx, dy) = (self.scatter.signed(), self.scatter.signed());
            let sky = Vec3::new(ground.x + dx * 40.0, ground.y + dy * 40.0, base + 60.0 + self.scatter.unit() * 90.0);
            let width = (5.0 + self.scatter.unit() * 4.0) * strength;
            let lag = self.scatter.unit() * tick;
            self.arc(sky, ground, 9, 34.0, now + lag, 0.24, width);
            self.arc(sky, ground, 9, 34.0, now + lag + 0.12, 0.14, width * 0.5);
            self.push_effect(ground.to_array(), now + lag, 36.0 + 24.0 * strength, 0.35, 0.0, 0.0);
            // Where it leaves the cloud, the cloud lights up round it.
            self.push_effect(sky.to_array(), now + lag, 110.0 * strength, 0.3, 0.0, 0.0);
            if std::mem::take(&mut thunder) {
                self.sky.strike(sky, now + lag, strength, 3.0, true);
            }
            if self.scatter.unit() < 0.12 {
                let size = 10.0 + self.scatter.unit() * 14.0;
                self.bore_fx.melt(ground.truncate(), size, now, 90.0);
            }
            if self.scatter.unit() < 0.5 {
                let rise = Vec3::Z * (10.0 + self.scatter.unit() * 10.0);
                let life = 5.0 + self.scatter.unit() * 4.0;
                self.push_puff(super::water_fx::PUFF_STEAM, ground + Vec3::Z * 3.0, rise, now + 0.1, life, (8.0, 34.0));
            }
            for _ in 0..3 {
                let (sx, sy) = (self.scatter.signed(), self.scatter.signed());
                self.push_puff(PUFF_BOLT, ground, Vec3::new(sx, sy, 1.4) * 28.0, now + lag, 0.5, (1.6, 0.4));
            }
        }
        // Arcs crawling round the edge over the ground, and the wall of vapour it drives.
        let a = self.scatter.unit() * TAU;
        let span = 0.3 + self.scatter.unit() * 0.25;
        let mut last = at + Vec3::new(a.cos(), a.sin(), 0.0) * r;
        last.z = self.ground_height(last.truncate()).max(water) + 1.0;
        for j in 1..=4 {
            let b = a + span * j as f32 / 4.0;
            let mut next = at + Vec3::new(b.cos(), b.sin(), 0.0) * r * (0.92 + self.scatter.unit() * 0.16);
            next.z = self.ground_height(next.truncate()).max(water) + 1.0;
            self.arc(last, next, 3, r * 0.03, now, 0.25, 2.6 * strength);
            last = next;
        }
        let out = Vec3::new(a.cos(), a.sin(), 0.0);
        let mut wall = at + out * r;
        wall.z = self.ground_height(wall.truncate()).max(water) + 4.0;
        self.push_puff(PUFF_SHOCK_SMOKE, wall, out * 18.0 + Vec3::Z * 5.0, now, 6.0, (16.0, 55.0));
        // Its heart: a channel of light from the ground up into the hurricane's middle,
        // every tick, lighting the heart of the cloud from within like the beam's end.
        let heart = at + Vec3::Z * 2.0;
        let up = Vec3::new(at.x, at.y, base + 120.0);
        self.arc(heart, up, 12, 14.0, now, 0.28, 14.0 * (1.0 - f) + 3.0);
        // (Light reaches seven radii; drawn much bigger the flash sprite reads as a bubble.)
        self.push_effect(up.to_array(), now, 90.0 * strength, tick * 1.6, 0.0, 0.0);
        self.push_effect((up + Vec3::Z * 160.0).to_array(), now, 75.0 * strength, tick * 1.6, 0.0, 0.0);
        self.effect_outbound = outbound;
        self.effect_origin = previous;
    }

    /// A giant gun's spent casing kicked out of its breech (`SimEvent::SabotThrown`) from
    /// `from` at `vel` m/s. The sim flies the case itself and the mirror draws it tumbling;
    /// this is only the puff of propellant smoke that comes out of the port with it.
    pub(super) fn throw_sabot(&mut self, from: Vec3, vel: Vec3, start: f32) {
        let previous = self.effect_origin.replace(from);
        let outbound = std::mem::replace(&mut self.effect_outbound, true);
        let dir = vel.normalize_or_zero();
        for k in 0..4 {
            let (a, b, c) = (self.scatter.signed(), self.scatter.signed(), self.scatter.signed());
            let push = dir * (10.0 + k as f32 * 5.0) + Vec3::new(a, b, c) * 4.0;
            let life = 1.4 + self.scatter.unit() * 0.8;
            self.push_puff(PUFF_SHOCK_SMOKE, from, push, start, life, (5.0, 18.0));
        }
        self.effect_outbound = outbound;
        self.effect_origin = previous;
    }

    /// A spent casing coming down (`SimEvent::SabotLanded`): cold metal, so no fire, only
    /// the thump of it: a ring of dust thrown out low, clods, and a dust cloud settling.
    pub(super) fn sabot_burst(&mut self, at: Vec3, splash: f32, start: f32) {
        let previous = self.effect_origin.replace(at);
        self.push_shockwave((at + Vec3::Z * 1.0).to_array(), start, splash * 1.6, 0.5, 0.35, 1.0, Vec3::ZERO);
        for _ in 0..8 {
            let a = self.scatter.unit() * std::f32::consts::TAU;
            let out = Vec3::new(a.cos(), a.sin(), 0.15);
            let life = 2.5 + self.scatter.unit() * 1.5;
            self.push_puff(PUFF_DUST, at + out * splash * 0.2, out * 14.0, start, life, (splash * 0.15, splash * 0.45));
        }
        for _ in 0..10 {
            let (x, y) = (self.scatter.signed(), self.scatter.signed());
            let dir = Vec3::new(x, y, 1.0).normalize_or_zero();
            let (speed, life) = (15.0 + self.scatter.unit() * 25.0, 1.4 + self.scatter.unit());
            self.push_puff(PUFF_CLOD, at + Vec3::Z, dir * speed, start, life, (0.9, 0.3));
        }
        let grow = splash * (0.8 + self.scatter.unit() * 0.3);
        self.push_puff(PUFF_DUST, at + Vec3::Z * 3.0, Vec3::Z * 2.0, start + 0.2, 6.0, (splash * 0.3, grow));
        self.effect_origin = previous;
    }
}

impl Renderer {
    /// The giants' part of a sim event: sabots thrown and landing. Everything else about
    /// the event is drawn as usual.
    pub(super) fn giant_event(&mut self, event: &mc_sim::SimEvent, time: f32) {
        match event {
            mc_sim::SimEvent::StormCollapsed { pos } => {
                self.storm_collapsed(Vec3::from(pos.to_f32()), time);
            }
            mc_sim::SimEvent::SabotThrown { from, vel, .. } => {
                self.throw_sabot(Vec3::from(from.to_f32()), Vec3::from(vel.to_f32()), time);
            }
            mc_sim::SimEvent::SabotLanded { pos, blueprint, weapon } => {
                let Some(sabot) = self.blueprints.unit(*blueprint).weapons.get(*weapon as usize).and_then(|w| w.sabot)
                else {
                    return;
                };
                self.sabot_burst(Vec3::from(pos.to_f32()), sabot.splash.to_f32(), time);
            }
            _ => {}
        }
    }
}

/// A giant bore feeding its storm: from the shot until the storm is spent, the gun keeps
/// discharging into it down a live channel (`Renderer::storm_beams`).
struct StormBeam {
    blueprint: u32,
    weapon: u8,
    /// Where the machine was last seen: it is found again each tick as the nearest of
    /// its blueprint.
    near: Vec3,
    target: Vec3,
    end: f32,
    /// When it began and how far the storm it feeds spreads.
    start: f32,
    radius: f32,
}

/// A giant bore's storm being drawn (`Renderer::storms_tick`).
#[derive(Clone, Copy)]
pub(super) struct StormFx {
    at: Vec3,
    radius: f32,
    start: f32,
    end: f32,
    /// How far the thunderhead over it reaches, and its base.
    cloud_reach: f32,
    base: f32,
    /// Its closing flash is drawn.
    done: bool,
}

#[derive(Default)]
pub(super) struct GiantFx {
    beams: Vec<StormBeam>,
    storms: Vec<StormFx>,
}

impl Renderer {
    /// Starts the channel a giant bore feeds its storm down: the shot left `muzzle`, landed at
    /// `target`, and the storm lasts until `end`.
    pub(super) fn feed_storm(&mut self, muzzle: Vec3, target: Vec3, blueprint: u32, weapon: u8, start: f32, end: f32, radius: f32) {
        self.giant_fx.beams.push(StormBeam { blueprint, weapon, near: muzzle, target, end, start, radius });
    }

    /// Where weapon `weapon` of the gun hull `hull` has its muzzle now, and which way it
    /// points: turned and pitched in its gun house as the sim has it.
    fn house_muzzle(&self, hull: &super::water_fx::GunHull, weapon: u8) -> Option<(Vec3, Vec3)> {
        let w = self.blueprints.units.get(hull.blueprint as usize)?.weapons.get(weapon as usize)?;
        let pose = hull.house.filter(|_| (weapon as usize) < mc_data::MAX_HOUSES).map(|h| h.pose[weapon as usize]);
        let (yaw, pitch) = pose.map_or((0.0, 0.0), |p| (p[1], p[3]));
        let pivot = w.pivot.map_or(Vec3::ZERO, |p| Vec3::from(p.to_f32()));
        let rot_z = |v: Vec3, a: f32| {
            let (s, c) = a.sin_cos();
            Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
        };
        let rot_xz = |v: Vec3, a: f32| {
            let (s, c) = a.sin_cos();
            Vec3::new(v.x * c - v.z * s, v.y, v.x * s + v.z * c)
        };
        let local = Vec3::from(w.muzzle.to_f32());
        let on_hull = pivot + rot_z(rot_xz(local - pivot, pitch), yaw);
        let muzzle = hull.pos + rot_z(on_hull, hull.heading);
        let dir = rot_z(rot_z(rot_xz(Vec3::X, pitch), yaw), hull.heading);
        Some((muzzle, dir))
    }

    /// Once a tick: every giant bore still feeding a storm discharges into it again. A
    /// straight white-blue channel from the muzzle to the storm's heart, jagged strokes
    /// twisting round it, and the muzzle blazing. A machine that is gone feeds nothing.
    pub(super) fn storm_beams(&mut self, time: f32) {
        let tick = self.tick_seconds.max(0.02);
        let mut beams = std::mem::take(&mut self.giant_fx.beams);
        beams.retain(|b| time < b.end);
        let mut kept = Vec::with_capacity(beams.len());
        for mut beam in beams {
            let hull = self
                .water_fx
                .guns
                .iter()
                .filter(|g| g.blueprint == beam.blueprint && g.pos.truncate().distance(beam.near.truncate()) < 400.0)
                .min_by(|a, b| {
                    a.pos.distance_squared(beam.near).total_cmp(&b.pos.distance_squared(beam.near))
                })
                .map(|g| super::water_fx::GunHull { blueprint: g.blueprint, pos: g.pos, heading: g.heading, house: g.house });
            // Not in the list this tick (it is filled after the events): try again next tick.
            let Some(hull) = hull else {
                kept.push(beam);
                continue;
            };
            let Some((muzzle, _dir)) = self.house_muzzle(&hull, beam.weapon) else {
                kept.push(beam);
                continue;
            };
            beam.near = hull.pos;
            let heart = beam.target + Vec3::Z * 6.0;
            let previous = self.effect_origin.replace(muzzle);
            let outbound = std::mem::replace(&mut self.effect_outbound, true);
            let life = tick * 1.35;
            self.bore_fx.column(muzzle, heart, time, life, 7.0);
            for _ in 0..3 {
                let w = 1.5 + self.scatter.unit() * 2.5;
                self.arc(muzzle, heart, 16, 22.0, time, life, w);
            }
            let (glow, blaze) = (26.0 + self.scatter.unit() * 10.0, 40.0 + self.scatter.unit() * 15.0);
            self.push_effect(muzzle.to_array(), time, glow, life, 0.0, 0.0);
            self.push_effect(heart.to_array(), time, blaze, life, 0.0, 0.0);
            self.storm_ball(&beam, time);
            self.effect_outbound = outbound;
            self.effect_origin = previous;
            kept.push(beam);
        }
        self.giant_fx.beams = kept;
    }

    /// A storm that died early (`SimEvent::StormCollapsed`): the channel into it and the
    /// strokes still to come go with it.
    pub(super) fn storm_collapsed(&mut self, at: Vec3, time: f32) {
        self.giant_fx.beams.retain(|b| b.target.truncate().distance(at.truncate()) > 30.0);
        for s in &mut self.giant_fx.storms {
            if s.at.truncate().distance(at.truncate()) < 30.0 {
                s.end = s.end.min(time);
            }
        }
        self.bore_fx.cancel_near(at, 900.0, time);
    }
}

impl Renderer {
    /// The ball of lightning the gun feeds, once a tick while it does: a dome of white-hot
    /// ionised air standing on the hit, swelling as the storm spreads, and lightning
    /// cracking over its skin all the while, blinding where the channel pours into it.
    fn storm_ball(&mut self, beam: &StormBeam, time: f32) {
        let span = (beam.end - beam.start).max(0.1);
        let f = ((time - beam.start) / span).clamp(0.0, 1.0);
        // As big as the storm's heart: a fifth of its reach at once, a third at the end.
        let r = beam.radius * (0.2 + 0.14 * (2.0 * f - f * f));
        let centre = beam.target + Vec3::Z * r * 0.35;
        for _ in 0..6 {
            let (a, b) = (self.scatter.unit() * std::f32::consts::TAU, self.scatter.unit());
            let up = b.sqrt();
            let dir = Vec3::new(a.cos() * (1.0 - up * up).sqrt(), a.sin() * (1.0 - up * up).sqrt(), up);
            let pos = centre + dir * r * (0.25 + self.scatter.unit() * 0.45);
            let vel = dir * (4.0 + self.scatter.unit() * 5.0) + Vec3::Z * 3.0;
            let size = r * (0.5 + self.scatter.unit() * 0.25);
            let life = 1.4 + self.scatter.unit() * 0.6;
            self.push_puff(PUFF_ARC_BALL, pos, vel, time, life, (size, size * 1.25));
        }
        // Cracks: lightning running over the skin from one point of it to another.
        for _ in 0..3 {
            let pick = |s: &mut Self| {
                let (a, b) = (s.scatter.unit() * std::f32::consts::TAU, s.scatter.unit());
                let up = 0.15 + b * 0.85;
                centre + Vec3::new(a.cos() * (1.0 - up * up).sqrt(), a.sin() * (1.0 - up * up).sqrt(), up) * r * 1.02
            };
            let (from, to) = (pick(self), pick(self));
            let width = 2.0 + self.scatter.unit() * 3.0;
            self.arc(from, to, 8, r * 0.12, time, 0.16, width);
        }
        self.push_effect(centre.to_array(), time, r * 1.6, 0.25, 0.0, 0.0);
    }
}
