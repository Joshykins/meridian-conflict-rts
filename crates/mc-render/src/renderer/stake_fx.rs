//! A ground stake striking home (`mc_models::stakes`): as a siege unit plants, each of
//! its stakes fires into the ground in turn (`entity.wgsl` `stake_pose`), and where each
//! one strikes the ground is punched: a hard shock, earth thrown up and a ring of dust
//! blown out flat. The sim reports nothing of it; the strikes are read off the unit's
//! deploy tick by tick, the way `capital_fx.rs` reads a landing.
//!
//! Presentation only; the renderer's own clock.

use super::{Renderer, PUFF_CLOD, PUFF_DUST, PUFF_SHOCK_DUST};
use glam::Vec3;
use mc_models::stakes::STAKES;
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_PROP, KIND_WRECK};

/// Which blueprints plant stakes, and how much each model is scaled from the authored deck.
pub(super) struct StakeFx {
    scales: Vec<Option<f32>>,
}

impl StakeFx {
    pub(super) fn new(scales: Vec<Option<f32>>) -> StakeFx {
        StakeFx { scales }
    }
}

impl Renderer {
    /// Once a tick: the stakes that struck the ground during it, each at its moment.
    pub(super) fn stake_strikes(&mut self, units: &[UnitInstance], time: f32) {
        for u in units {
            if u.owner_flags & (KIND_WRECK | KIND_GHOST | KIND_PROP) != 0
                || u.deploy <= u.prev_deploy
            {
                continue;
            }
            let Some(&Some(scale)) = self.stake_fx.scales.get(u.blueprint as usize) else {
                continue;
            };
            let (before, now) = (u.prev_deploy, u.deploy);
            for stake in STAKES {
                let strikes = stake.strikes();
                if strikes <= before || strikes > now {
                    continue;
                }
                let start = time + (strikes - before) / (now - before) * self.tick_seconds;
                let (s, c) = u.heading.sin_cos();
                let local = stake.strike_point() * scale;
                let at = Vec3::from(u.pos)
                    + Vec3::new(local.x * c - local.y * s, local.x * s + local.y * c, 0.0);
                let down = stake.down();
                let along = Vec3::new(down.x * c - down.y * s, down.x * s + down.y * c, 0.0);
                self.stake_strike(at, along.normalize_or_zero(), scale, start);
            }
        }
    }

    /// One stake punching into the ground at `at`, driven outward along `along`.
    fn stake_strike(&mut self, at: Vec3, along: Vec3, scale: f32, start: f32) {
        let k = scale;
        self.push_shockwave(at.to_array(), start, 6.0 * k, 0.35, 0.7, 0.0, Vec3::ZERO);
        // Earth thrown up and back from the point, the way the spike went in.
        for _ in 0..10 {
            let spray = (self.scatter.upward(0.35) - along * 0.5).normalize_or_zero();
            let speed = (7.0 + self.scatter.unit() * 9.0) * k;
            let life = 0.9 + self.scatter.unit() * 0.6;
            self.push_puff(
                PUFF_CLOD,
                at,
                spray * speed,
                start,
                life,
                (0.16 * k, 0.1 * k),
            );
        }
        // A ring of dust blown out flat.
        for i in 0..10 {
            let a = (i as f32 + self.scatter.unit()) * std::f32::consts::TAU / 10.0;
            let out = Vec3::new(a.cos(), a.sin(), 0.05);
            let life = 1.0 + self.scatter.unit() * 0.6;
            let speed = (3.0 + self.scatter.unit() * 2.0) * k;
            self.push_puff(
                PUFF_DUST,
                at + out * 0.6 * k + Vec3::Z * 0.2,
                out * speed,
                start + 0.02,
                life,
                (0.4 * k, 1.5 * k),
            );
        }
        // A burst of shock dust over the hole.
        for _ in 0..3 {
            let vel = self.scatter.upward(0.6) * 3.0 * k;
            let life = 1.8 + self.scatter.unit();
            self.push_puff(
                PUFF_SHOCK_DUST,
                at + Vec3::Z * 0.4 * k,
                vel,
                start,
                life,
                (0.9 * k, 2.4 * k),
            );
        }
    }
}
