//! A Shatter gun's round (docs/STYLE.md "ARC fires no plasma").
//!
//! Rail flak: the rails throw a dense canister, and a timed fuse splits it just short
//! of the aircraft. The slug's path flashes white-hot like any rail shot; the split is a
//! hard orange-white flash that leaves a black flak puff hanging in the air, and a cone
//! of flechettes carries on along the canister's path into the target volume, each one
//! a hot metal streak that sparks where it strikes. No blue, no energy rings, nothing
//! that glows once the burst is over.

use super::{
    shatter_airburst, shatter_detail_scale, shatter_fragment_count, FadeBeam, PendingShatter, Renderer,
    PUFF_FIREBALL, PUFF_SMOKE, PUFF_SPARK, SHATTER_FRAGMENT_MAX_LIFE, SHATTER_FRAGMENT_MIN_LIFE,
};
use super::rail_fx::RAIL_FLASH;
use super::water_fx::PUFF_STEAM;
use glam::Vec3;

/// A flechette in flight: a thin hot streak along its velocity (puffs.wgsl).
pub(super) const PUFF_FLECHETTE: f32 = 37.0;
/// Effect kind of a powder flash (sprites.wgsl `weapon_color`: `WeaponColor::Orange`).
const FLASH_ORANGE: f32 = 1.0;

impl Renderer {
    pub(super) fn spawn_shatter_split_inner(
        &mut self,
        shot: &PendingShatter,
        target: Vec3,
        target_velocity: Vec3,
        beam_start: f32,
        split_start: f32,
        miss: bool,
    ) {
        let (burst, forward) = shatter_airburst(shot.muzzle, target, shot.dir, shot.splash);
        if shot.muzzle.distance(burst) > 2.0 {
            self.flak_slug(shot.muzzle, burst, shot.width, beam_start);
        }
        let scale = if miss { 0.45 } else { 1.0 };
        let detail = shatter_detail_scale(shot.impact) * scale;
        let impact = shot.impact * scale;

        // The fuse: a hard white core inside an orange powder flash.
        self.push_effect(burst.to_array(), split_start, 3.2 * impact, 0.07, RAIL_FLASH, 0.0);
        self.push_effect(burst.to_array(), split_start, 9.0 * impact, 0.16, FLASH_ORANGE, 0.0);
        // A small fireball that burns out into the black puff flak is known by.
        for _ in 0..3 {
            let dir = self.random_dir();
            self.push_puff(PUFF_FIREBALL, burst + dir * impact * 0.6, dir * 6.0 * detail,
                split_start, 0.32, (1.6 * impact, 3.6 * impact));
        }
        let puffs = if miss { 4 } else { 8 };
        let drift = (self.sky.wind_heading() * 1.6).extend(0.5);
        for _ in 0..puffs {
            let dir = self.random_dir();
            // Big from the start: the charge throws its smoke out at once, then it spreads.
            let life = 2.6 + self.scatter.unit() * 1.4;
            let grow = (11.0 + self.scatter.unit() * 4.0) * impact;
            self.push_puff(PUFF_SMOKE, burst + dir * impact * 1.8, dir * 9.0 * detail + drift,
                split_start + 0.02, life, (5.0 * impact, grow));
        }
        // The pressure of the charge, pale and quick: no coloured ring.
        if shot.shockwave > 0.0 && !miss {
            self.push_shockwave(burst.to_array(), split_start, (8.0 + shot.impact * 4.0) * shot.shockwave,
                0.22, (shot.shockwave * 0.3).min(0.45), FLASH_ORANGE, forward);
        }
        // Casing scraps thrown off the canister as it opens.
        let scraps = if miss { 4 } else { shot.bolts.max(3) + 2 };
        for _ in 0..scraps {
            let spray = (forward * 0.5 + self.random_dir() * 0.7).normalize_or_zero();
            let speed = (20.0 + self.scatter.unit() * 30.0) * detail;
            let life = 0.18 + self.scatter.unit() * 0.14;
            self.push_puff(PUFF_SPARK, burst, spray * speed, split_start, life, (0.3 * detail, 0.08 * detail));
        }

        // The flechette cone: it fills the target volume, one down the middle and the
        // rest spread out round it, each leading the aircraft by its own flight time.
        let n = shatter_fragment_count(shot.bolts, miss);
        let side = forward.cross(if forward.z.abs() > 0.9 { Vec3::Y } else { Vec3::Z }).normalize_or_zero();
        let up = side.cross(forward).normalize_or_zero();
        for i in 0..n {
            let angle = i as f32 * 2.399963 + self.scatter.signed() * 0.22;
            let radius = if i == 0 { 0.0 } else { (i as f32 / (n - 1) as f32).sqrt() * shot.splash };
            let depth = if i == 0 { 0.0 } else { self.scatter.signed() * shot.splash * 0.65 };
            let life = SHATTER_FRAGMENT_MIN_LIFE
                + self.scatter.unit() * (SHATTER_FRAGMENT_MAX_LIFE - SHATTER_FRAGMENT_MIN_LIFE);
            let mut end = target + target_velocity * life
                + (side * angle.cos() + up * angle.sin()) * radius + forward * depth;
            end.z = end.z.max(self.ground_height(end.truncate()) + 0.5);
            self.push_puff(PUFF_FLECHETTE, burst, (end - burst) / life, split_start, life,
                (0.5 * detail, 0.2 * detail));
            if miss {
                continue;
            }
            // Where it lands: a spark of struck metal and a wisp of smoke, not a blast.
            let arrival = split_start + life;
            let size = shot.impact * (1.8 + self.scatter.unit() * 1.4);
            self.push_effect(end.to_array(), arrival, size, 0.07, FLASH_ORANGE, 0.0);
            for _ in 0..3 {
                let velocity = self.random_dir() * (18.0 * detail) + forward * 10.0 * detail;
                let life = 0.16 + self.scatter.unit() * 0.1;
                self.push_puff(PUFF_SPARK, end, velocity, arrival, life, (0.22 * detail, 0.05 * detail));
            }
            if i % 3 == 0 {
                let life = 1.2 + self.scatter.unit() * 0.6;
                self.push_puff(PUFF_SMOKE, end, drift, arrival, life, (0.8 * shot.impact, 2.2 * shot.impact));
            }
        }
    }

    /// The canister's flight, muzzle to fuse: the white-hot line every ARC rail leaves,
    /// cooling fast, with a thin wake of vapour that hangs on the wind.
    pub(super) fn flak_slug(&mut self, from: Vec3, to: Vec3, width: f32, time: f32) {
        let line = (width * 0.35).clamp(0.22, 0.6);
        self.fade_beams.push(FadeBeam { from, to, start: time, life: 0.09, width: line * 1.5, laser: false, rail: true });
        self.fade_beams.push(FadeBeam { from, to, start: time, life: 0.4, width: line * 0.55, laser: false, rail: true });
        let wind = self.sky.wind_heading();
        let n = ((from.distance(to) / 14.0) as usize).clamp(2, 40);
        for k in 1..=n {
            let t = k as f32 / n as f32;
            let at = from + (to - from) * t;
            let drift = (wind * (1.0 + self.scatter.unit())).extend(0.3);
            let life = 0.9 + self.scatter.unit() * 0.8;
            let grow = width * (1.1 + self.scatter.unit());
            self.push_puff(PUFF_STEAM, at, drift, time + t * 0.02, life, (width * 0.35, grow));
        }
    }

    fn random_dir(&mut self) -> Vec3 {
        Vec3::new(self.scatter.signed(), self.scatter.signed(), self.scatter.signed()).normalize_or(Vec3::Z)
    }
}
