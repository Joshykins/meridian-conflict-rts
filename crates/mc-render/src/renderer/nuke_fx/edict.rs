//! The Edict's strike on the ground (`SimEvent::EdictFired`; mc-sim `edict.rs`), where its
//! column of pinch-fusion plasma lands. The column, the firing point and the sky are
//! drawn elsewhere; this is what it does to the country, and it is neither a warhead's
//! fireball nor the Regency nova's star: nothing climbs, nothing hangs.
//!
//! - **The strike:** a blinding white flash at the mark and a lens of pinched plasma
//!   pressed flat over it (surge_fx.wgsl `EDICT_STRIKE`), white-hot with the prism's warm
//!   colours at its rim, cooling to rose and red and eaten through within seconds; hard
//!   globs of melt thrown out low; the country lit white, then a long red.
//! - **The shock** (`EDICT_SHOCK`): a hard line of light at the foot of a wall of dark
//!   driven dust, racing out with the sim's blast front (its square-root law, to the blast
//!   radius in `NuclearBlast::front_ticks`), the trees bending away as it passes and the
//!   clouds over it thrown back.
//! - **The molten wave** (`EDICT_WAVE`): a low crest of melt rolling out at the sim's
//!   pace (`EdictWave`), the ground behind it molten and burning as long as the sim's
//!   wave does, dimmer toward its rim.
//! - **The scar** (`craters.rs` `CraterStyle::Edict`): the blast radius glassed, red-hot
//!   and cooling over minutes to black glass with a crimson depth; it stays.
//! - **Trees** the blast kills: under the lens they are gone; further out a share are
//!   thrown flat, away from the mark, the rest gone; a few past the radius burn.

use super::super::regency_guns_fx::{GLOB, HOT, RED, WHITE};
use super::super::stun_fx::Flash;
use super::super::{Renderer, PUFF_SPARK};
use super::{NukeFx, TreeFate, MOST_FELLED, MOST_LIT};
use crate::gpu_consts::surge_fx::{EDICT_SHOCK, EDICT_STRIKE, EDICT_WAVE};
use glam::{Vec2, Vec3};
use mc_data::BlueprintId;
use std::f32::consts::TAU;

/// The lens's radius against the blast's core, and the seconds it takes to spread there
/// and then to cool away.
const LENS_REACH: f32 = 1.3;
const LENS_ROLL: f32 = 0.5;
const LENS_LINGER: f32 = 2.0;
/// Seconds the shock's wall takes to thin away once it has run past the radius.
const SHOCK_LINGER: f32 = 1.4;
/// Seconds after a strike its trees are still taken for its own.
const TREES_FOR: f32 = 20.0;

/// One strike, kept a while for the trees it kills.
#[derive(Clone, Copy)]
pub(super) struct Strike {
    at: Vec2,
    start: f32,
    core: f32,
    radius: f32,
    felled: u32,
    lit: u32,
}

impl NukeFx {
    /// What a tree at `at` that died this tick does, if an Edict's strike killed it.
    pub(super) fn edict_tree_fate(&mut self, at: Vec2, time: f32) -> Option<TreeFate> {
        let s = self
            .edicts
            .iter_mut()
            .filter(|s| time - s.start < TREES_FOR)
            .find(|s| at.distance(s.at) <= s.radius * 1.35)?;
        let d = at.distance(s.at);
        let pick = ((at.x * 12.9898 + at.y * 78.233).sin() * 43758.547)
            .fract()
            .abs();
        if d < s.core * LENS_REACH {
            Some(TreeFate::Gone)
        } else if d > s.radius * 1.1 && pick < 0.05 && s.lit < MOST_LIT {
            s.lit += 1;
            Some(TreeFate::Burning)
        } else if pick < 0.5 && s.felled < MOST_FELLED {
            s.felled += 1;
            let away = (at - s.at).normalize_or(Vec2::X);
            Some(TreeFate::Flattened { away, at: time })
        } else {
            Some(TreeFate::Gone)
        }
    }
}

impl Renderer {
    /// The Edict of `blueprint` struck `mark` (on the ground, or the sea's surface).
    pub(super) fn edict_strike(&mut self, blueprint: BlueprintId, mark: Vec3, time: f32) {
        let Some(e) = self.blueprints.unit(blueprint).edict.clone() else {
            return;
        };
        let tick = self.tick_seconds.max(0.02);
        let (radius, core) = (e.blast.radius.to_f32(), e.blast.core.to_f32());
        let front = e.blast.front_ticks as f32 * tick;
        let wave = e.wave.radius.to_f32();
        let roll = e.wave.roll_ticks as f32 * tick;
        let linger = e.wave.linger_ticks as f32 * tick;
        self.nuke_fx.edicts.retain(|s| time - s.start < TREES_FOR);
        self.nuke_fx.edicts.push(Strike {
            at: mark.truncate(),
            start: time,
            core,
            radius,
            felled: 0,
            lit: 0,
        });
        let lens = core * LENS_REACH;
        self.add_surge(
            mark,
            time,
            lens,
            lens / LENS_ROLL,
            LENS_LINGER,
            1.0,
            EDICT_STRIKE,
        );
        self.add_surge(
            mark,
            time,
            radius,
            radius / front.max(0.1),
            SHOCK_LINGER,
            1.0,
            EDICT_SHOCK,
        );
        self.add_surge(
            mark,
            time,
            wave,
            wave / roll.max(0.1),
            linger,
            1.0,
            EDICT_WAVE,
        );
        self.add_edict_scar(mark.truncate(), radius, time, roll);
        // The strike goes through domes (they take it for what stands under them), so no
        // shield clips what it throws.
        let origin = self.effect_origin.replace(mark);
        let outbound = std::mem::replace(&mut self.effect_outbound, true);
        self.edict_flash(mark, lens, radius, roll + linger, time);
        self.edict_spatter(mark, lens, time);
        self.effect_outbound = outbound;
        self.effect_origin = origin;
        // The shock bends the trees as it passes (on at its mean pace), and throws the
        // clouds over it back.
        self.tree_blasts
            .record_wide(mark, time, radius * 2.6, 9.0, radius / front.max(0.1));
        self.sky
            .blast(mark + Vec3::Z * 300.0, radius * 2.0, 2.0, time);
    }

    /// The strike's light: a white flash past what the screen shows, over the country for
    /// kilometres, then a long red one while the ground burns.
    fn edict_flash(&mut self, at: Vec3, lens: f32, radius: f32, burn: f32, time: f32) {
        self.push_effect(at.to_array(), time, lens * 1.4, 0.6, 9.0, 0.0);
        self.push_effect(at.to_array(), time + 0.05, lens * 1.8, 1.6, 8.0, 0.0);
        self.emp_fx.flashes.push(Flash {
            pos: at + Vec3::Z * lens * 0.3,
            start: time,
            life: 1.1,
            color: WHITE * 1.2e8,
            range: radius * 5.0,
            flicker: 0.05,
        });
        self.emp_fx.flashes.push(Flash {
            pos: at + Vec3::Z * lens,
            start: time + 0.4,
            life: burn,
            color: RED * 1.5e6,
            range: radius * 3.0,
            flicker: 0.2,
        });
    }

    /// Hard globs of melt thrown out low from under the lens, falling and cooling, and
    /// sparks sprayed with them.
    fn edict_spatter(&mut self, at: Vec3, lens: f32, time: f32) {
        for _ in 0..90 {
            let a = self.scatter.unit() * TAU;
            let lift = 0.1 + 0.35 * self.scatter.unit();
            let dir = Vec3::new(a.cos(), a.sin(), lift).normalize();
            let speed = 90.0 + 220.0 * self.scatter.unit();
            let size = 8.0 + 12.0 * self.scatter.unit();
            let r0 = self.scatter.unit();
            self.push_lingering(
                GLOB,
                at + dir * lens * 0.3 + Vec3::Z * 10.0,
                dir * speed,
                time + 0.15 * r0,
                3.0 + 2.5 * r0,
                (size, size * 0.6),
                RED.lerp(HOT, r0) * 7.0,
                0.0,
                1,
            );
        }
        for _ in 0..140 {
            let vel = self.scatter.upward(0.05) * (80.0 + 260.0 * self.scatter.unit());
            let life = 1.5 + 2.0 * self.scatter.unit();
            let size = 2.0 + 2.0 * self.scatter.unit();
            self.push_puff(PUFF_SPARK, at + Vec3::Z * 8.0, vel, time, life, (size, 0.4));
        }
    }
}
