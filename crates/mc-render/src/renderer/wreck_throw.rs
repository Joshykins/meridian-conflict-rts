//! A wreck's turret coming down where it was thrown (wreck.wgsl `turret_throw` flies
//! it): dust rolling out round it, earth and sparks kicked up, and for a heavy one a
//! shove through the ground, a dent where it struck and dust along the furrow it skids.
//!
//! Presentation only. The throw is worked out here as the shader works it out, from the
//! wreck's id and its model, so the dust goes up where and when the turret is drawn
//! hitting the ground. One that comes down in the sea throws up a splash instead.

use super::water_fx::{PUFF_COLUMN, PUFF_SPRAY};
use super::wreck_fx::{hash, HullFrame};
use super::{Renderer, PUFF_CLOD, PUFF_SHOCK_DUST, PUFF_SPARK};
use crate::camera::Camera;
use crate::gpu_consts::wreck::{
    LANDING_DITCHED, LANDING_MASK, LANDING_SANK, THROW_GRAVITY, THROW_HEAVY, THROW_LIGHT,
    THROW_SALT,
};
use glam::{Vec2, Vec3};
use mc_sim::mirror::{UnitInstance, KIND_WRECK, WRECK_COUNT_SHIFT, WRECK_INNER, WRECK_POSED};

/// How a turret is thrown off its wreck: wreck.wgsl `turret_throw`, which see.
struct TurretThrow {
    heft: f32,
    /// Where its pivot comes down, model metres, and seconds in the air.
    land: Vec3,
    flight: f32,
    skid: f32,
}

impl TurretThrow {
    fn of(id: u32, pivot: Vec3, height: f32, reach: f32) -> TurretThrow {
        let r1 = hash(id, THROW_SALT);
        let r2 = hash(id, THROW_SALT + 1);
        let r3 = hash(id, THROW_SALT + 2);
        let heft = smoothstep(THROW_LIGHT, THROW_HEAVY, reach);
        let side = std::f32::consts::FRAC_PI_2;
        let away = if r3 > 0.5 { side } else { -side } + (r2 - 0.5) * 1.1;
        let thrown = reach * lerp(1.0, 0.8, heft) * (0.95 + r1 * 0.25);
        let land = Vec3::new(away.cos() * thrown, away.sin() * thrown, height * 0.16);
        let rise = lerp(height * 0.4 + 1.2, height * 0.08 + 1.0, heft) * (0.85 + r2 * 0.3);
        let lift = (2.0 * THROW_GRAVITY * rise).sqrt();
        let fall = (pivot.z + rise - land.z).max(0.0);
        let flight = lift / THROW_GRAVITY + (2.0 * fall / THROW_GRAVITY).sqrt();
        let along = (land.truncate() - pivot.truncate()).length() / flight;
        TurretThrow {
            heft,
            land,
            flight,
            skid: (along * lerp(0.05, 0.2, heft)).min(reach * 0.15),
        }
    }
}

impl Renderer {
    /// Dust and earth where a fresh wreck's thrown turret lands, in the tick it lands.
    pub(super) fn turret_landings(&mut self, units: &[UnitInstance], time: f32, camera: &Camera) {
        let reach = camera.distance * 2.5 + 300.0;
        let mut landed = Vec::new();
        for u in units {
            let landing = u.refit_modules & LANDING_MASK;
            // A fresh wreck only, once (not its inside), and not one that went down in
            // the sea, which keeps its turret.
            if u.owner_flags & KIND_WRECK == 0
                || u.packed != 0
                || u.gait[2] < 0.5
                || u.refit_modules & WRECK_POSED == 0
                || u.refit_modules & WRECK_INNER != 0
                || landing == LANDING_SANK
                || landing == LANDING_DITCHED
                || Vec3::from(u.pos).distance(camera.focus) > reach
            {
                continue;
            }
            let Some(site) = self.burn_sites.get(u.blueprint as usize) else {
                continue;
            };
            if !site.throws_turret {
                continue;
            }
            let throw = TurretThrow::of(
                u.unit_id,
                site.turret_pivot,
                site.height.max(1.0),
                site.bounds.max(1.0),
            );
            let [before, now, _] = u.gait;
            if !(before < throw.flight && throw.flight <= now) {
                continue;
            }
            // A hull broken in sections: the turret rides the one that keeps its ring.
            let count = (u.refit_modules >> WRECK_COUNT_SHIFT) & 15;
            let centre = if count > 1 {
                let (lo, hi) = (u.arm_pitch[2] * site.bounds, u.arm_pitch[3] * site.bounds);
                if site.turret_pivot.x < lo || site.turret_pivot.x >= hi {
                    continue;
                }
                0.5 * (lo.max(-site.bounds) + hi.min(site.bounds))
            } else {
                0.0
            };
            let frame = HullFrame::of(u, 1.0, true);
            let at = frame.at(throw.land - Vec3::X * centre);
            let skid = frame.at(throw.land - Vec3::X * centre
                + (throw.land - site.turret_pivot)
                    .with_z(0.0)
                    .normalize_or_zero()
                    * throw.skid)
                - at;
            let start =
                time + (throw.flight - before) / (now - before).max(1e-3) * self.tick_seconds;
            landed.push((
                at.truncate(),
                skid.truncate(),
                throw.heft,
                site.height,
                start,
            ));
        }
        for (xy, skid, heft, height, start) in landed {
            self.turret_impact(xy, skid, heft, height, start);
        }
    }

    /// A turret striking the ground at `xy`, skidding on by `skid`: `heft` zero to one
    /// for how heavy it is, `height` the hull's it came off. In the sea, a splash.
    fn turret_impact(&mut self, xy: Vec2, skid: Vec2, heft: f32, height: f32, start: f32) {
        let ground = xy.extend(self.ground_height(xy));
        // About the turret's own size.
        let size = (height * 0.35).clamp(0.6, 12.0);
        let previous = self.effect_origin.replace(ground);
        if let Some(water) = self.at_sea(ground, 1.0) {
            self.turret_splash(ground.with_z(water), heft, size, start);
        } else {
            self.turret_strike(ground, skid, heft, size, start);
        }
        self.effect_origin = previous;
    }

    /// Water thrown out all round where it goes in, and a column of it.
    fn turret_splash(&mut self, surface: Vec3, heft: f32, size: f32, start: f32) {
        let sheets = 10 + (10.0 * heft) as usize;
        for k in 0..sheets {
            let a = (k as f32 + self.scatter.unit()) * std::f32::consts::TAU / sheets as f32;
            let out = Vec3::new(a.cos(), a.sin(), 0.0);
            let vel = out * (5.0 + 6.0 * heft + 3.0 * self.scatter.unit())
                + Vec3::Z * (6.0 + 6.0 * heft + 4.0 * self.scatter.unit());
            let life = 1.2 + 0.8 * heft + 0.4 * self.scatter.unit();
            self.push_puff(
                PUFF_SPRAY,
                surface + out * size * 0.4 + Vec3::Z * 0.3,
                vel,
                start,
                life,
                (size * 0.4, size * 1.6),
            );
        }
        self.push_puff(
            PUFF_COLUMN,
            surface,
            Vec3::Z * (6.0 + 8.0 * heft),
            start,
            1.4 + 0.8 * heft,
            (size * 0.6, size * 1.4),
        );
    }

    /// On land: dust, earth and sparks, and for a heavy one the blow through the ground.
    fn turret_strike(&mut self, ground: Vec3, skid: Vec2, heft: f32, size: f32, start: f32) {
        let xy = ground.truncate();
        let ahead = skid.normalize_or_zero().extend(0.0);
        // Dust driven out from under it in billows that roll over the ground, most of
        // it the way it was going, and a plume of dirt thrown up where it struck.
        let billows = 8 + (10.0 * heft) as usize;
        for i in 0..billows {
            let a = (i as f32 + self.scatter.unit()) * std::f32::consts::TAU / billows as f32;
            let out = Vec3::new(a.cos(), a.sin(), 0.0);
            let speed = (6.0 + 12.0 * heft + size) * (1.0 + 0.6 * out.dot(ahead).max(0.0));
            let life = 1.8 + 1.6 * heft + self.scatter.unit() * 0.6;
            self.push_puff(
                PUFF_SHOCK_DUST,
                ground + out * size * 0.4 + Vec3::Z * 0.5,
                out * speed + Vec3::Z * (1.0 + 2.0 * heft),
                start,
                life,
                (size * 0.5, size * (1.6 + 1.2 * heft)),
            );
        }
        for _ in 0..3 + (5.0 * heft) as usize {
            let vel = (self.scatter.upward(0.55) + ahead * 0.3) * (size * 1.4 + 3.0);
            let lag = self.scatter.unit() * 0.08;
            let life = 2.0 + 1.2 * self.scatter.unit();
            self.push_puff(
                PUFF_SHOCK_DUST,
                ground + Vec3::Z * 0.5,
                vel,
                start + lag,
                life,
                (size * 0.4, size * (1.6 + 0.8 * heft)),
            );
        }
        // Earth and a few sparks off the metal, kicked up.
        for _ in 0..8 + (16.0 * heft) as usize {
            let vel = (self.scatter.upward(0.35) + ahead * 0.4) * (5.0 + 8.0 * heft);
            let life = 0.9 + self.scatter.unit() * 0.8;
            self.push_puff(
                PUFF_CLOD,
                ground,
                vel,
                start,
                life,
                (0.18 + size * 0.08, 0.12),
            );
        }
        for _ in 0..6 {
            let vel = (self.scatter.upward(0.2) + ahead * 0.6) * (8.0 + 6.0 * self.scatter.unit());
            self.push_puff(
                PUFF_SPARK,
                ground + Vec3::Z * 0.3,
                vel,
                start,
                0.5,
                (0.18, 0.04),
            );
        }
        if heft < 0.25 {
            return;
        }
        // A heavy one: the dent it leaves and dust kicked up along the furrow while it
        // ploughs on; the heaviest send a blow through the ground.
        if heft > 0.6 {
            self.push_shockwave(
                ground.to_array(),
                start,
                6.0 + size * 3.0,
                0.5,
                0.35 * heft,
                1.0,
                Vec3::ZERO,
            );
        }
        self.impact_craters.dig(xy + skid * 0.5, size * 0.9, start);
        let furrow = 3 + (skid.length() / size.max(1.0) * 4.0) as usize;
        for k in 0..furrow {
            let along = (k as f32 + 0.5) / furrow as f32;
            let at = xy + skid * along;
            let at = at.extend(self.ground_height(at) + 0.3);
            let side = Vec3::new(-ahead.y, ahead.x, 0.0) * self.scatter.signed();
            self.push_puff(
                PUFF_SHOCK_DUST,
                at,
                (side * 4.0 + ahead * 2.0 + Vec3::Z * 2.0) * (1.0 + heft),
                start + along * 0.25,
                2.0 + 1.5 * heft,
                (size * 0.4, size * 1.3),
            );
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heavy_turret_is_thrown_lower_and_shorter_than_a_light_one() {
        let light = TurretThrow::of(7, Vec3::new(0.0, 0.0, 2.0), 3.0, 4.0);
        let heavy = TurretThrow::of(7, Vec3::new(0.0, 0.0, 20.0), 28.0, 36.0);
        assert!(light.heft < 0.1 && heavy.heft > 0.99);
        // Out to beside the hull, not past twice its reach.
        assert!(heavy.land.truncate().length() < 36.0 * 1.05);
        assert!(light.land.truncate().length() > 4.0 * 0.9);
        // In the air no longer than its fall from that height takes, give or take the rise.
        assert!(light.flight > 0.5 && light.flight < 1.6, "{}", light.flight);
        assert!(heavy.flight > 1.2 && heavy.flight < 2.6, "{}", heavy.flight);
        assert!(heavy.skid > light.skid);
    }
}
