//! Nuclear blasts and strategic missiles on screen (docs/NUKES.md).
//!
//! The sim reports a launch, an interceptor, a kill and a detonation; the missiles in
//! flight come every tick in `RenderFrame::strategic`. What this draws:
//!
//! - The blast, as a volume in `nuke.wgsl` from `Globals::nukes`, marched at half size
//!   (`nuke_volume.rs`). The flash is the fireball itself, too bright to look at, and
//!   the light it throws over the country; nothing whites out the screen. The shock runs
//!   out along the ground fast (the sim's damage front, then on at the speed of sound)
//!   with a curtain of dust on it, trees bending as it passes, and a Wilson cloud blooms
//!   round the fireball for a few seconds behind it. The fireball hangs, then climbs
//!   slowly on its stem, its skin going to smoke while it glows through the cracks and
//!   underneath, and rolls over into the cap over about a minute, leaving rings of
//!   condensation round the stem; a ring of dust boils out along the ground. The clouds
//!   are thrown back once by the shock, and again where the cap climbs through them.
//! - The ground is left a crater, glassed and cooling over minutes (`craters.rs`).
//! - Lightning flickers in the cap while it forms, lighting it from inside.
//! - Trees the blast kills are thrown flat, away from it, charred; a few out at the edge
//!   of it are set alight instead (`tree_fate`).
//! - A warhead climbs out of its silo in a column of smoke and fire and leaves a thick
//!   white trail, the same all along, that hangs and drifts for most of a minute; an
//!   interceptor streaks up on a thinner one. The bodies and motor plumes are drawn in
//!   `nuke.wgsl` from `Globals::strategic`; their trails are puffs in slots of their own
//!   (`NUKE_PUFF_SLOTS`) so the battle's smoke cannot overwrite them.

use super::{Puff, Renderer, PUFF_FIREBALL, PUFF_SMOKE, PUFF_SPARK, PUFF_TREE_SMOKE};
use crate::camera::Camera;
use bytemuck::Zeroable;
use glam::{Vec2, Vec3};
use mc_sim::mirror::{RenderFrame, SimEvent, StrategicInstance, STRATEGIC_WARHEAD};
use std::mem::size_of;

/// Blasts drawn as volumes at once, the nearest (`Globals::nukes`, four vec4 each;
/// nuke.wgsl). A carpet of warheads over a base has dozens of clouds up at once, and
/// every one of them is drawn.
pub(super) const NUKE_SLOTS: usize = 64;
/// Missiles drawn at once, the nearest (`Globals::strategic`, two vec4 each).
pub(super) const MISSILE_SLOTS: usize = 64;
/// Blasts remembered at once; the nearest `NUKE_SLOTS` of them are drawn.
const MOST_BLASTS: usize = 96;
/// Puff slots kept for missile trails, after the ground fires' (renderer `MAX_PUFFS`).
pub(super) const NUKE_PUFF_SLOTS: usize = 6144;
/// A thick white ribbon along a strategic missile's path (puffs.wgsl).
pub(super) const PUFF_STRATEGIC_TRAIL: f32 = 34.0;
/// A warhead's damage radius: the size everything here is drawn for (`scale` 1).
const WARHEAD_RADIUS: f32 = 520.0;
/// A warhead's body at scale 1, nose to nozzle (shared with nuke.wgsl).
const WARHEAD_LENGTH: f32 = crate::gpu_consts::missile::WARHEAD_LENGTH;
/// Seconds a blast is drawn for (nuke.wgsl `fade_left`).
const BLAST_LIFE: f32 = 75.0;
/// How fast the shock runs on past the damage radius, m/s; the trees bend at it.
const SHOCK_SPEED: f32 = 330.0;
/// Seconds the sim's front takes to reach a warhead's damage radius (strategic.ron).
const FRONT_SECONDS: f32 = 1.0;
/// Metres of trail between two of its puffs, and how long they hang.
const TRAIL_STEP: f32 = 26.0;
const TRAIL_LIFE: f32 = 42.0;

// Salvos (docs/NUKES.md): every warhead keeps its own fireball, stem and cap. (Pouring
// one column into another was tried on 2026-09-25 and dropped: in a carpet some warheads
// never got a mushroom of their own, only a bigger neighbour.) Only a burst on the very
// spot of a fireball still on the ground, within `FOLD_REACH` of its damage radius and
// `FOLD_AGE` seconds, goes into it (the two could not be told apart): it flares and grows
// a little. Every burst stirs the columns already standing near it (within `STIR_REACH`
// of their heads, under `STIR_AGE` seconds old): their billows churn and their caps
// heave, and its flash lights them from below.
const FOLD_REACH: f32 = 0.25;
const FOLD_AGE: f32 = 1.5;
const STIR_REACH: f32 = 1.6;
const STIR_AGE: f32 = 60.0;
/// A fireball's size over one warhead's, at most, however many fold into it.
const MOST_GROWTH: f32 = 1.6;
/// Seconds a fireball takes to grow to its new size.
const GROW_SECONDS: f32 = 4.0;
/// Burst and feed records a blast keeps; older ones are banked (`Blast::churn_bank`).
const RECORDS: usize = 8;

#[derive(Clone, Copy)]
struct Blast {
    at: Vec3,
    start: f32,
    /// Its size over a warhead's as drawn now: `base` grown by the yield in it, and heaved
    /// for a moment by each burst under it; eased toward `grown` (`Blast::grow`).
    scale: f32,
    /// One burst's size over a warhead's: its damage radius over a warhead's.
    base: f32,
    /// Warheads in it: 1, more as bursts fold or pour into it.
    warheads: f32,
    /// The renderer's clock when `scale` was last eased.
    eased: f32,
    radius: f32,
    /// Fire fed into it (when, how much): hotter and thicker again for a while.
    feeds: [(f32, f32); RECORDS],
    /// Bursts under or into it (when, how hard): its billows churn on, a flare below.
    kicks: [(f32, f32); RECORDS],
    /// What kicks too old to keep added to the churn, so it never jumps back.
    churn_bank: f32,
    seed: f32,
    ground: f32,
    /// When the next lightning strikes in it, and the last one: when, how high, how bright.
    next_bolt: f32,
    bolt: (f32, f32, f32),
    /// The cap has climbed through the cloud layer and thrown it back.
    parted: bool,
    /// Trees it has thrown flat, and set alight (`tree_fate`).
    felled: u32,
    lit: u32,
}

impl Blast {
    fn age(&self, time: f32) -> f32 {
        (time - self.start).max(0.0)
    }

    /// The size its yield makes it: one warhead's, growing with the log of how many
    /// (two 1.24 times, eight 1.73, forty 2.3).
    fn grown(&self) -> f32 {
        self.base * (1.0 + 0.35 * self.warheads.max(1.0).ln()).min(MOST_GROWTH)
    }

    /// Eases the drawn size toward `grown`, over `GROW_SECONDS`.
    fn grow(&mut self, time: f32) {
        let dt = (time - self.eased).max(0.0);
        self.eased = time;
        let to = self.grown();
        self.scale += (to - self.scale) * (1.0 - (-dt / GROW_SECONDS).exp());
    }

    /// A burst under or into it: its billows churn and its cap heaves.
    fn kick(&mut self, time: f32, force: f32) {
        let oldest = (0..RECORDS)
            .min_by(|&a, &b| self.kicks[a].0.total_cmp(&self.kicks[b].0))
            .unwrap_or(0);
        let (t0, a0) = self.kicks[oldest];
        self.churn_bank += churned(time - t0, a0);
        self.kicks[oldest] = (time, force);
        // A heave, never more than a little past its size however many come at once.
        self.scale = (self.scale * (1.0 + 0.05 * force)).min(self.scale.max(self.grown() * 1.1));
    }

    /// Fire fed into it: it burns hotter and thickens again.
    fn feed(&mut self, time: f32, amount: f32) {
        let oldest = (0..RECORDS)
            .min_by(|&a, &b| self.feeds[a].0.total_cmp(&self.feeds[b].0))
            .unwrap_or(0);
        self.feeds[oldest] = (time, amount);
    }

    /// When fire last went into it (its own burst, else the latest feed).
    fn fed(&self) -> f32 {
        self.feeds.iter().map(|f| f.0).fold(self.start, f32::max)
    }

    /// How much of its billows' churn the bursts under it have added.
    fn churn(&self, time: f32) -> f32 {
        self.churn_bank
            + self
                .kicks
                .iter()
                .map(|&(t, a)| churned(time - t, a))
                .sum::<f32>()
    }

    /// Heat from fire fed into it and bursts under it, 0..1 (nuke.wgsl `heat_left`
    /// takes the greater of its own and this).
    fn fuel(&self, time: f32) -> f32 {
        let k = self.scale.max(0.3).sqrt();
        let fed: f32 = self
            .feeds
            .iter()
            .filter(|f| f.1 > 0.0 && time >= f.0)
            .map(|&(t, a)| {
                let s = time - t;
                a * ((-s / (2.5 * k)).exp() * 0.5 + (-s / (16.0 * k)).exp() * 0.5)
            })
            .sum();
        let flare: f32 = self
            .kicks
            .iter()
            .filter(|f| f.1 > 0.0 && time >= f.0)
            .map(|&(t, a)| a * 0.6 * (-(time - t) / 0.8).exp())
            .sum();
        (fed + flare).min(1.0)
    }

    /// How thick fire fed into it keeps it (nuke.wgsl `fade_left` takes the greater):
    /// each feed thickens it over a few seconds, then it thins as a fresh cloud would.
    fn thick(&self, time: f32) -> f32 {
        let k = self.scale.max(0.3).sqrt();
        self.feeds
            .iter()
            .filter(|f| f.1 > 0.0 && time >= f.0)
            .map(|&(t, a)| {
                let s = time - t;
                // Fed fire thickens it only partway: kept solid, a salvo's cloud was a
                // pale blob for as long as warheads kept landing.
                let thin = (-4.5 * smoothstep(2.0 * k, 22.0 * k, s)).exp().max(0.025);
                (a * 0.45).min(0.45)
                    * smoothstep(0.0, 3.0, s)
                    * thin
                    * (1.0 - smoothstep(45.0, 75.0, s))
            })
            .fold(0.0, f32::max)
    }

    /// Still drawn: its own cloud or fire folded into it has not thinned away.
    fn alive(&self, time: f32) -> bool {
        time - self.start < BLAST_LIFE || time - self.fed() < BLAST_LIFE
    }

    /// Mirrors nuke.wgsl `head_radius`.
    fn head_radius(&self, time: f32) -> f32 {
        let t = self.age(time);
        self.scale * (240.0 * (1.0 - (-t * 3.0).exp()).sqrt() + 430.0 * (1.0 - (-t / 26.0).exp()))
    }

    /// Mirrors nuke.wgsl `rise`: bigger blasts climb less than they spread.
    fn rise(&self) -> f32 {
        self.scale.min(self.scale.powf(0.6))
    }

    /// Mirrors nuke.wgsl `head_height`.
    fn head_height(&self, time: f32) -> f32 {
        let x = (self.age(time) / 30.0).powf(1.35);
        self.rise() * 1700.0 * (1.0 - (-x).exp())
    }

    /// Mirrors nuke.wgsl `heat_left`.
    fn heat(&self, time: f32) -> f32 {
        let t = self.age(time);
        // nuke.wgsl `slow`: a bigger ball burns longer.
        let k = self.scale.max(0.3).sqrt();
        (-t / (2.5 * k)).exp() * 0.5 + (-t / (16.0 * k)).exp() * 0.5
    }

    /// Seconds the front takes to reach the damage radius.
    fn first(&self) -> f32 {
        FRONT_SECONDS * self.scale.sqrt()
    }

    /// Where the shock's front has got to on the ground: the sim's damage front
    /// (`NuclearBlast::front_at`) out to the damage radius, then on at the speed of sound.
    fn front(&self, time: f32) -> f32 {
        let t = self.age(time);
        let first = self.first();
        let r = if t < first {
            self.radius * (1.0 / 12.0 + 11.0 / 12.0 * (t / first).sqrt())
        } else {
            self.radius + SHOCK_SPEED * (t - first)
        };
        r.min(4200.0 * self.scale)
    }
}

/// What becomes of a tree a blast killed.
pub(super) enum TreeFate {
    /// Blown flat, away from the middle, starting at this time.
    Flattened { away: Vec2, at: f32 },
    /// Set alight where it stands.
    Burning,
    /// Gone: burnt to nothing in the fireball, or torn away in the storm after it.
    Gone,
}

/// Trees one blast throws flat, and sets alight, at most. Each one left lying or burning
/// is drawn and shadowed on its own, and a warhead kills thousands: past these the rest
/// are simply gone, which a forest under a nuclear blast mostly is.
const MOST_FELLED: u32 = 320;
const MOST_LIT: u32 = 24;

#[derive(Default)]
pub(super) struct NukeFx {
    blasts: Vec<Blast>,
    /// Leaves the volumes out (`nuke_shots` times the frame both ways).
    volume_off: bool,
    /// Bursts heard in the last moment (where, when), so one handed over twice counts once.
    heard: Vec<(Vec3, f32)>,
    /// Trail puffs, oldest first, written into the reserved slots.
    trail: Vec<Puff>,
    /// Where each missile's trail was last laid (serial, point).
    laid: Vec<(u32, Vec3)>,
    /// Missiles as the last tick showed them.
    missiles: Vec<StrategicInstance>,
}

impl NukeFx {
    /// A trail puff of another long-range shot (a great gun's shell) laid in the reserved
    /// slots, so battle smoke cannot overwrite it before it has hung its time.
    pub(super) fn lay_trail(&mut self, puff: Puff) {
        self.trail.push(puff);
    }

    pub(super) fn clear(&mut self) {
        *self = NukeFx::default();
    }

    /// What a tree at `at` that died this tick does, if a blast killed it.
    pub(super) fn tree_fate(&mut self, at: Vec2, time: f32) -> Option<TreeFate> {
        let b = self
            .blasts
            .iter_mut()
            .filter(|b| b.age(time) < 20.0)
            .find(|b| at.distance(b.at.truncate()) <= b.radius * 1.35)?;
        let d = at.distance(b.at.truncate());
        // Under the fireball nothing is left. Further out the air throws them flat, a
        // share of them lying where they fell; a few out at the edge, where the heat
        // reached but the air was weaker, are left standing to burn. A whole forest
        // alight is a carpet of flicker, and costs a light a tree.
        let pick = ((at.x * 12.9898 + at.y * 78.233).sin() * 43758.547)
            .fract()
            .abs();
        if d < b.radius * 0.45 * b.scale.sqrt().max(0.6) {
            Some(TreeFate::Gone)
        } else if d > b.radius * 0.95 && pick < 0.04 && b.lit < MOST_LIT {
            b.lit += 1;
            Some(TreeFate::Burning)
        } else if pick < 0.45 && b.felled < MOST_FELLED {
            b.felled += 1;
            let away = (at - b.at.truncate()).normalize_or(Vec2::X);
            Some(TreeFate::Flattened { away, at: time })
        } else {
            Some(TreeFate::Gone)
        }
    }
}

impl Renderer {
    /// A strategic event from the sim (`effects_of_inner`).
    pub(super) fn nuke_event(&mut self, event: &SimEvent, time: f32) {
        match event {
            SimEvent::NuclearDetonation {
                pos,
                radius,
                commander,
                ..
            } => {
                self.nuclear_detonation(Vec3::from(pos.to_f32()), radius.to_f32(), *commander, time)
            }
            SimEvent::NuclearLaunch { from, .. } => {
                self.silo_launch(Vec3::from(from.to_f32()), time)
            }
            SimEvent::InterceptorLaunch { from, .. } => {
                let at = Vec3::from(from.to_f32());
                self.cell_launch(at, Vec3::Z, 40.0, time);
                self.push_effect(at.to_array(), time, 9.0, 0.35, 1.0, 0.0);
            }
            SimEvent::WarheadIntercepted { pos, killed, .. } => {
                self.intercept_burst(Vec3::from(pos.to_f32()), *killed, time)
            }
            _ => {}
        }
    }

    fn nuclear_detonation(&mut self, at: Vec3, radius: f32, commander: bool, time: f32) {
        // The same tick's events can be handed over more than once (a burst folded into
        // another leaves no blast of its own to recognise it by).
        self.nuke_fx.heard.retain(|h| (h.1 - time).abs() < 0.5);
        if self.nuke_fx.heard.iter().any(|h| h.0.distance(at) < 1.0) {
            return;
        }
        self.nuke_fx.heard.push((at, time));
        let base = (radius / WARHEAD_RADIUS).max(0.2);
        // A reactor's pool is smaller and cools sooner than a warhead's; a pool already
        // glowing here is heated again (`craters.rs`).
        self.add_crater(
            at.truncate(),
            radius,
            if commander { 0.6 } else { 1.0 },
            time,
        );
        let flat = |b: &Blast| b.at.truncate().distance(at.truncate());
        // Into a fireball still on the ground right here: it flares and grows, and burns on.
        if let Some(i) = (0..self.nuke_fx.blasts.len())
            .filter(|&i| {
                let b = &self.nuke_fx.blasts[i];
                b.age(time) < FOLD_AGE && flat(b) < FOLD_REACH * radius.max(b.radius)
            })
            .min_by(|&a, &b| {
                flat(&self.nuke_fx.blasts[a]).total_cmp(&flat(&self.nuke_fx.blasts[b]))
            })
        {
            let b = &mut self.nuke_fx.blasts[i];
            b.warheads += base / b.base;
            b.kick(time, 1.0);
            b.feed(time, 0.9);
            return;
        }
        // Otherwise a fireball of its own; the columns standing round it are stirred.
        for b in self.nuke_fx.blasts.iter_mut() {
            let d = flat(b);
            let reach = STIR_REACH * b.head_radius(time).max(b.radius);
            if b.age(time) < STIR_AGE && d < reach {
                b.kick(time, 1.0 - d / reach);
            }
        }
        let ground = self
            .ground_height(at.truncate())
            .max(self.map_info.water_level.to_f32());
        let seed = self.scatter.unit();
        if self.nuke_fx.blasts.len() >= MOST_BLASTS {
            // The one fed last longest ago.
            let drop = (0..self.nuke_fx.blasts.len())
                .min_by(|&a, &b| {
                    self.nuke_fx.blasts[a]
                        .fed()
                        .total_cmp(&self.nuke_fx.blasts[b].fed())
                })
                .unwrap_or(0);
            self.nuke_fx.blasts.remove(drop);
        }
        let blast = Blast {
            at,
            start: time,
            scale: base,
            base,
            warheads: 1.0,
            eased: time,
            radius,
            feeds: [(-1.0e9, 0.0); RECORDS],
            kicks: [(-1.0e9, 0.0); RECORDS],
            churn_bank: 0.0,
            seed,
            ground,
            next_bolt: time + 3.0 + self.scatter.unit(),
            bolt: (-100.0, 0.0, 0.0),
            parted: false,
            felled: 0,
            lit: 0,
        };
        self.nuke_fx.blasts.push(blast);
        // The shock bends the trees as it passes, at the pace it is drawn going out.
        let reach = 3200.0 * base;
        let lag = blast.first() - radius / SHOCK_SPEED;
        self.tree_blasts
            .record_wide(at, time + lag, reach, 6.0 * base.sqrt(), SHOCK_SPEED);
        // And throws the clouds over it back.
        self.sky
            .blast(at + Vec3::Z * 400.0 * base, 1600.0 * base, 2.2, time);
    }

    /// Every blast eases to its size (`Blast::grow`); the thinned away go.
    fn settle_blasts(&mut self, time: f32) {
        for b in self.nuke_fx.blasts.iter_mut() {
            b.grow(time);
        }
        self.nuke_fx.blasts.retain(|b| b.alive(time));
    }

    fn silo_launch(&mut self, at: Vec3, time: f32) {
        let origin = self.effect_origin.replace(at);
        self.push_effect((at + Vec3::Z * 4.0).to_array(), time, 26.0, 1.2, 2.0, 0.0);
        self.push_shockwave(
            (at + Vec3::Z * 3.0).to_array(),
            time,
            90.0,
            1.4,
            0.7,
            1.0,
            Vec3::ZERO,
        );
        // Smoke boiling out of the tube and the flame trenches, rolling over the deck.
        for k in 0..40 {
            let a = (k as f32 + self.scatter.unit()) / 40.0 * std::f32::consts::TAU;
            let out = Vec3::new(a.cos(), a.sin(), 0.15) * (10.0 + self.scatter.unit() * 16.0);
            let start = time + self.scatter.unit() * 1.5;
            let r0 = self.scatter.unit();
            self.push_puff(
                PUFF_SMOKE,
                at + Vec3::Z * 3.0,
                out,
                start,
                7.0 + r0 * 5.0,
                (8.0, 30.0),
            );
        }
        for _ in 0..12 {
            let up = Vec3::new(
                self.scatter.signed() * 2.0,
                self.scatter.signed() * 2.0,
                18.0 + self.scatter.unit() * 16.0,
            );
            let r0 = self.scatter.unit();
            self.push_puff(
                PUFF_FIREBALL,
                at + Vec3::Z * 2.0,
                up,
                time + r0 * 0.6,
                1.4,
                (5.0, 14.0),
            );
        }
        self.effect_origin = origin;
    }

    fn intercept_burst(&mut self, at: Vec3, killed: bool, time: f32) {
        let origin = self.effect_origin.replace(at);
        if !killed {
            // A spent interceptor destroying itself.
            self.push_effect(at.to_array(), time, 14.0, 0.5, 2.0, 0.0);
            for _ in 0..10 {
                let dir = Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.signed(),
                )
                .normalize_or(Vec3::Z);
                self.push_puff(PUFF_SPARK, at, dir * 40.0, time, 1.2, (0.8, 0.2));
            }
            self.effect_origin = origin;
            return;
        }
        // A warhead broken open high up: a hard white flash, a ring of fire, and the
        // pieces falling burning a long way down. No nuclear yield: it never went off.
        self.push_effect(at.to_array(), time, 160.0, 0.6, 4.0, 0.0);
        self.push_effect(at.to_array(), time + 0.04, 70.0, 2.2, 2.0, 1.0);
        self.push_shockwave(at.to_array(), time, 520.0, 2.5, 1.0, 1.0, Vec3::ZERO);
        for k in 0..24 {
            let a = (k as f32 + self.scatter.unit()) / 24.0 * std::f32::consts::TAU;
            let out = Vec3::new(a.cos(), a.sin(), self.scatter.signed() * 0.4) * 34.0;
            let r0 = self.scatter.unit();
            self.push_puff(PUFF_FIREBALL, at, out, time, 2.4 + r0, (10.0, 40.0));
        }
        for _ in 0..30 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed() * 0.4,
            )
            .normalize_or(Vec3::X)
                * 26.0;
            let r0 = self.scatter.unit();
            self.push_puff(
                PUFF_SMOKE,
                at,
                out,
                time + 0.3,
                14.0 + r0 * 8.0,
                (18.0, 90.0),
            );
        }
        for _ in 0..60 {
            let dir = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed() * 0.6,
            )
            .normalize_or(Vec3::Z);
            let speed = 40.0 + self.scatter.unit() * 90.0;
            let r0 = self.scatter.unit();
            self.push_puff(
                PUFF_SPARK,
                at,
                dir * speed,
                time,
                4.0 + r0 * 3.0,
                (2.2, 0.6),
            );
        }
        self.sky.blast(at, 700.0, 2.0, time);
        self.effect_origin = origin;
    }

    /// Every tick: trails, launch smoke, what a blast keeps doing (smoke, the cap
    /// through the clouds, lightning).
    pub(super) fn nuke_tick(&mut self, frame: &RenderFrame, time: f32, camera: &Camera) {
        self.settle_blasts(time);
        self.nuke_fx.missiles.clone_from(&frame.strategic);
        // Trails: a ribbon puff every stretch of the path since the last one, all alike
        // from the silo to the end, so the trail reads as one line.
        let missiles = frame.strategic.clone();
        self.nuke_fx
            .laid
            .retain(|(s, _)| missiles.iter().any(|m| m.serial == *s));
        // A salvo lays its trails in longer steps (each puff's tent spans its step, so the
        // line stays whole): the reserved slots hold every trail, and far fewer puffs are
        // drawn over one another where the trails converge.
        let spread = (missiles.len() as f32 / 6.0).sqrt().clamp(1.0, 3.0);
        for m in &missiles {
            let warhead = m.kind == STRATEGIC_WARHEAD;
            let now = Vec3::from(m.pos);
            let last = self
                .nuke_fx
                .laid
                .iter()
                .find(|(s, _)| *s == m.serial)
                .map(|(_, p)| *p)
                .unwrap_or(Vec3::from(m.prev_pos));
            // A smaller missile (a boat's) lays a thinner trail, in shorter steps.
            let scale = m.scale;
            let (step, life, size) = if warhead {
                (
                    TRAIL_STEP * spread * scale.max(0.5),
                    TRAIL_LIFE,
                    (12.0 * scale, 120.0 * scale),
                )
            } else {
                (TRAIL_STEP * spread, TRAIL_LIFE * 0.5, (6.0, 60.0))
            };
            let span = now - last;
            let len = span.length();
            let count = (len / step).floor() as usize;
            let dir = span / len.max(0.001);
            let tick = self.tick_seconds.max(0.02);
            // The path is the nose's; the smoke hangs a step behind the nozzle, so the
            // newest segment's forward half ends at the nozzle instead of over the body
            // (the plume covers the gap while the motor burns). Under the ground, in the
            // tube, there is none.
            let axis = (now - Vec3::from(m.prev_pos)).normalize_or(dir);
            let behind = axis * (if warhead { WARHEAD_LENGTH * scale } else { 9.0 } + step);
            for k in 1..=count.min(96) {
                let p = last + dir * step * k as f32 - behind;
                if p.z < self.ground_height(p.truncate()) + 1.0 {
                    continue;
                }
                // Laid when the missile passed it, so the oldest end fades first.
                let born = time - tick * (1.0 - k as f32 / count.max(1) as f32);
                let puff = Puff {
                    origin: p.to_array(),
                    opacity: 1.0,
                    pos: p.to_array(),
                    start: born,
                    // A tent a step either side (puffs.wgsl `strategic_trail`): each
                    // point of the path shared by two, adding up to one, so no seams.
                    vel: (dir * step).to_array(),
                    life,
                    params: [size.0, size.1, PUFF_STRATEGIC_TRAIL, self.scatter.unit()],
                    appearance: [-1.0, -1.0, -1.0, if warhead { 1.0 } else { 0.7 }],
                };
                self.nuke_fx.trail.push(puff);
            }
            // Remembered from the first tick seen, even before a puff is laid: in the slow
            // first seconds of the boost a tick covers less than a step, and forgetting the
            // start each tick left the climb bare until it was fast, at the pitch-over.
            let laid = last + dir * step * count as f32;
            match self.nuke_fx.laid.iter_mut().find(|(s, _)| *s == m.serial) {
                Some(l) => l.1 = laid,
                None => self.nuke_fx.laid.push((m.serial, laid)),
            }
            // A warhead on its boost: the tube pours smoke and fire round it.
            if warhead && m.boost > 0.5 {
                let low = Vec3::new(
                    m.pos[0],
                    m.pos[1],
                    self.ground_height(Vec2::new(m.pos[0], m.pos[1])) + 2.0,
                );
                if now.z - low.z < 400.0 {
                    for _ in 0..3 {
                        let a = self.scatter.unit() * std::f32::consts::TAU;
                        let out = Vec3::new(a.cos(), a.sin(), 0.2)
                            * (8.0 + self.scatter.unit() * 14.0)
                            * scale;
                        let r0 = self.scatter.unit();
                        let s = (10.0 * scale, 38.0 * scale);
                        self.push_puff(PUFF_SMOKE, low, out, time, 9.0 + r0 * 5.0, s);
                    }
                }
            }
        }
        self.nuke_fx.trail.retain(|p| time - p.start < p.life);
        // A deliberate cap, cosmetic: past the reserved slots the oldest puffs go. The
        // trails lay longer steps when many are up (salvos here, `great_gun_fx` for
        // shells) so that this is not reached in play.
        let excess = self.nuke_fx.trail.len().saturating_sub(NUKE_PUFF_SLOTS);
        self.nuke_fx.trail.drain(..excess);
        let mut slots = vec![Puff::zeroed(); NUKE_PUFF_SLOTS];
        let n = self.nuke_fx.trail.len();
        slots[..n].copy_from_slice(&self.nuke_fx.trail);
        self.puffs.write(
            ((super::PUFF_RING + super::GROUND_FIRE_SLOTS) * size_of::<Puff>()) as u64,
            bytemuck::cast_slice(&slots),
        );

        // What a blast keeps doing.
        let wind = self.sky.wind_heading();
        for i in 0..self.nuke_fx.blasts.len() {
            let b = self.nuke_fx.blasts[i];
            let age = b.age(time);
            // The cap climbing through the cloud layer throws it back round itself, once.
            let hc = b.head_height(time);
            if !b.parted && b.at.z + hc > self.sky.cloud_base_at(b.at.truncate()) {
                self.nuke_fx.blasts[i].parted = true;
                self.sky
                    .blast(b.at + Vec3::Z * hc, b.head_radius(time) * 2.2, 1.6, time);
            }
            let near = b.at.distance(camera.focus) < camera.distance * 3.0 + 3000.0 * b.scale;
            if !near {
                continue;
            }
            let origin = self.effect_origin.replace(b.at);
            // Smoke standing up off the burnt country for a couple of minutes.
            let smoke = smoothstep(4.0, 12.0, age) * (1.0 - age / 110.0).clamp(0.0, 1.0);
            if self.scatter.unit() < 0.3 * smoke {
                let a = self.scatter.unit() * std::f32::consts::TAU;
                let r = b.radius * (0.3 + 0.8 * self.scatter.unit().sqrt());
                let xy = b.at.truncate() + Vec2::from_angle(a) * r;
                let p = xy.extend(self.ground_height(xy) + 3.0);
                self.push_puff(
                    PUFF_TREE_SMOKE,
                    p,
                    (wind * 2.2).extend(9.0),
                    time,
                    22.0,
                    (20.0, 110.0),
                );
            }
            // Lightning in the cap while it forms: through the cloud, and down to the ground;
            // fire folded into it stirs it up again.
            let stormy = age.min(time - b.fed() + 3.0);
            if (3.0..45.0).contains(&stormy) && age >= 3.0 && time >= b.next_bolt {
                let rc = b.head_radius(time);
                let quiet = ((stormy - 3.0) / 42.0).clamp(0.0, 1.0);
                self.nuke_fx.blasts[i].next_bolt =
                    time + 0.6 + self.scatter.unit() * (1.2 + quiet * 3.5);
                let a = self.scatter.unit() * std::f32::consts::TAU;
                let cap = b.at + Vec3::new(a.cos() * rc * 0.5, a.sin() * rc * 0.5, hc - rc * 0.35);
                let grounded = self.scatter.unit() < 0.35;
                let end = if grounded {
                    let xy = b.at.truncate()
                        + Vec2::from_angle(a + self.scatter.signed() * 0.6)
                            * b.radius
                            * (0.4 + self.scatter.unit() * 0.9);
                    xy.extend(self.ground_height(xy))
                } else {
                    let a2 = a + std::f32::consts::PI * (0.5 + self.scatter.unit());
                    b.at + Vec3::new(
                        a2.cos() * rc * 0.7,
                        a2.sin() * rc * 0.7,
                        hc + self.scatter.signed() * rc * 0.3,
                    )
                };
                self.nuke_bolt(cap, end, time, b.scale);
                let brightness = 0.6 + self.scatter.unit() * 0.6;
                self.nuke_fx.blasts[i].bolt = (time, (cap.z - b.at.z).max(0.0), brightness);
                // A third of the light on the clouds of a storm's stroke, so the cloud round the cap
                // reads lit by the fire, not washed blue-white.
                self.sky
                    .strike(cap, time, brightness * 1.4 * b.scale.sqrt(), 0.3, grounded);
            }
            self.effect_origin = origin;
        }
    }

    /// One lightning stroke from `from` to `to`, jagged, with forks off it.
    fn nuke_bolt(&mut self, from: Vec3, to: Vec3, time: f32, scale: f32) {
        let length = from.distance(to);
        if length < 1.0 {
            return;
        }
        let along = (to - from) / length;
        let side = along.cross(Vec3::Z).normalize_or(Vec3::X);
        let up = side.cross(along).normalize_or(Vec3::Z);
        let wander = (length * 0.06).clamp(6.0, 60.0);
        let width = (3.5 * scale.sqrt()).max(1.6);
        for (delay, life, thick) in [(0.0, 0.18, 1.3), (0.07, 0.3, 1.0), (0.2, 0.26, 0.7)] {
            let kinks = ((length / 40.0) as usize).clamp(6, 22);
            let mut last = from;
            for k in 1..=kinks {
                let t = k as f32 / kinks as f32;
                let taper = (t * std::f32::consts::PI).sin().sqrt();
                let jitter = if k == kinks {
                    Vec3::ZERO
                } else {
                    (side * self.scatter.signed() + up * self.scatter.signed() * 0.6)
                        * wander
                        * taper
                };
                let next = from + (to - from) * t + jitter;
                self.bore_fx
                    .lightning(last, next, time + delay, life, width * thick);
                // Now and then a fork off it.
                if self.scatter.unit() < 0.18 {
                    let fork = next
                        + (side * self.scatter.signed() + up * self.scatter.signed() - along * 0.3)
                            * wander
                            * 2.0;
                    self.bore_fx.lightning(
                        next,
                        fork,
                        time + delay,
                        life * 0.7,
                        width * thick * 0.5,
                    );
                }
                last = next;
            }
        }
    }

    /// Fills this frame's blast volumes, missiles and flash (`Globals`), and lights them.
    pub(super) fn nuke_frame(
        &mut self,
        time: f32,
        alpha: f32,
        camera: &Camera,
        _view_proj: glam::Mat4,
    ) -> (
        [[f32; 4]; NUKE_SLOTS * 4],
        [[f32; 4]; MISSILE_SLOTS * 2],
        [f32; 4],
    ) {
        let mut nukes = [[0.0; 4]; NUKE_SLOTS * 4];
        let wind = self.sky.wind_heading();
        // The nearest to the eye get the slots; a blast past them is too far to matter.
        let eye = camera.eye();
        let mut blasts: Vec<Blast> = self.nuke_fx.blasts.clone();
        blasts.sort_by(|a, b| {
            a.at.distance_squared(eye)
                .total_cmp(&b.at.distance_squared(eye))
        });
        let mut shown = 0;
        for b in &blasts {
            let age = b.age(time);
            let hc = b.head_height(time);
            let fire = b.at + Vec3::Z * hc;
            // The fireball lights the country for kilometres (this is the flash), then its
            // glow sinks to embers; fire fed into it lights it up again.
            let fuel = b.fuel(time);
            let heat = b.heat(time).max(fuel);
            if heat > 0.02 {
                let flash = (-age / (2.5 * b.scale.max(0.3).sqrt())).exp();
                let color = Vec3::new(1.0, 0.55 + 0.35 * heat, 0.22 + 0.5 * heat * heat);
                let power = (9.0e6 * heat * heat + 4.0e7 * flash) * b.scale * b.scale;
                self.lights.lamp(
                    fire,
                    Vec3::NEG_Z,
                    color * power,
                    3400.0 * b.scale,
                    180.0,
                    1.0,
                );
            }
            let (bolt_t, bolt_z, bolt_b) = b.bolt;
            let bolt = bolt_b * (-(time - bolt_t).max(0.0) / 0.12).exp();
            if shown < NUKE_SLOTS && b.alive(time) {
                let drift = wind * ((age - 8.0).max(0.0) * 0.9).min(700.0 * b.scale);
                nukes[shown * 4] = [b.at.x, b.at.y, b.at.z, b.start];
                nukes[shown * 4 + 1] = [b.scale, b.seed, bolt, b.ground];
                nukes[shown * 4 + 2] = [drift.x, drift.y, bolt_z, b.front(time)];
                nukes[shown * 4 + 3] = [fuel, 0.0, b.churn(time), b.thick(time)];
                shown += 1;
            }
        }
        // Missiles: the body at its place between ticks, its plume, its heat.
        let mut missiles = [[0.0; 4]; MISSILE_SLOTS * 2];
        let mut drawn = 0;
        let mut near: Vec<&StrategicInstance> = self.nuke_fx.missiles.iter().collect();
        if near.len() > MISSILE_SLOTS {
            near.sort_by(|a, b| {
                Vec3::from(a.pos)
                    .distance_squared(eye)
                    .total_cmp(&Vec3::from(b.pos).distance_squared(eye))
            });
        }
        for m in near {
            if drawn >= MISSILE_SLOTS {
                break;
            }
            let a = Vec3::from(m.prev_pos);
            let b = Vec3::from(m.pos);
            let at = a.lerp(b, alpha);
            let axis = (b - a).normalize_or(Vec3::Z);
            let warhead = m.kind == STRATEGIC_WARHEAD;
            let climbing = axis.z > -0.05;
            // The motor burns out at the top of a warhead's arc; an interceptor's burns all the way.
            let plume = if warhead {
                if m.boost > 0.5 {
                    105.0
                } else if climbing {
                    80.0
                } else {
                    0.0
                }
            } else {
                34.0
            };
            let heat = if warhead && !climbing {
                (1.0 - m.eta / 14.0).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let scale = if warhead { m.scale } else { 1.0 };
            let plume = plume * scale;
            let kind = {
                use crate::gpu_consts::missile::*;
                let size = ((scale * SCALE_STEPS).round() as u32).clamp(1, SCALE_MASK);
                (m.kind & KIND_MASK
                    | (m.owner & crate::gpu_consts::owner::MASK) << OWNER_SHIFT
                    | (plume as u32).min(PLUME_MASK) << PLUME_SHIFT
                    | size << SCALE_SHIFT) as f32
            };
            missiles[drawn * 2] = [at.x, at.y, at.z, kind];
            missiles[drawn * 2 + 1] = [axis.x, axis.y, axis.z, heat];
            drawn += 1;
            // The motor's light, and the re-entry glow's.
            if plume > 0.0 {
                let nozzle = at - axis * if warhead { WARHEAD_LENGTH * scale } else { 9.0 };
                let power = if warhead {
                    1.4e5 * scale * scale
                } else {
                    3.0e4
                };
                self.lights.lamp(
                    nozzle,
                    -axis,
                    Vec3::new(1.0, 0.62, 0.3) * power,
                    if warhead { 420.0 * scale } else { 160.0 },
                    180.0,
                    1.0,
                );
            } else if heat > 0.1 {
                self.lights.lamp(
                    at,
                    Vec3::NEG_Z,
                    Vec3::new(1.0, 0.5, 0.2) * 6.0e4 * heat,
                    260.0,
                    180.0,
                    1.0,
                );
            }
        }
        // Nothing whites the screen out any more: the flash is the fireball and its light.
        // `nuke_shots` times a frame with and without the volume.
        let shown = if self.nuke_fx.volume_off { 0 } else { shown };
        let view = [0.0, 0.0, shown as f32, drawn as f32];
        (nukes, missiles, view)
    }
}

/// Churn a burst `s` seconds ago of force `a` has added to a cloud's billows: quick at
/// first, settling (nuke.wgsl adds it to the boiling's phase).
fn churned(s: f32, a: f32) -> f32 {
    if a <= 0.0 || s <= 0.0 {
        return 0.0;
    }
    a * 0.3 * (1.0 - (-s / 1.5).exp())
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod shots {
    use crate::camera::Camera;
    use crate::overlay::Overlay;
    use crate::renderer::{FrameInput, Renderer, SceneDesc, Target};
    use glam::{Vec2, Vec3};
    use mc_core::{Fx, FxVec3};
    use mc_sim::mirror::{
        RenderFrame, SimEvent, StrategicInstance, STRATEGIC_INTERCEPTOR, STRATEGIC_WARHEAD,
    };
    use std::sync::Arc;

    /// Sets off a warhead headless and writes frames of it. `NUKE_AT` = `x,y` (dev16 woods
    /// by default), `NUKE_CAM` = `dist,yaw,tilt` (radians), `NUKE_TIMES` = seconds after the
    /// burst to write (`0.05,1,3,...`), `NUKE_OUT` the folder, `NUKE_SIZE` = `w,h`,
    /// `NUKE_RADIUS` the damage radius (520 a warhead, 300 a commander). `NUKE_MISSILE=1`
    /// flies a warhead and an interceptor across the view first instead.
    #[test]
    #[ignore = "requires Vulkan and maps/dev16.mcmap"]
    fn nuke_shots() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let map = Arc::new(mc_map::MapFile::open(root.join("maps/dev16.mcmap")).unwrap());
        let blueprints = Arc::new(mc_data::Blueprints::load(&root.join("data")).unwrap());
        let nums = |key: &str, def: &str| -> Vec<f32> {
            std::env::var(key)
                .unwrap_or_else(|_| def.into())
                .split(',')
                .map(|v| v.trim().parse().unwrap())
                .collect()
        };
        let size = nums("NUKE_SIZE", "1280,720");
        let (w, h) = (size[0] as u32, size[1] as u32);
        let mut renderer = Renderer::new(
            Target::Headless {
                width: w,
                height: h,
            },
            SceneDesc {
                map: map.clone(),
                blueprints,
                pool: Arc::new(mc_jobs::Pool::new(2)),
                team_colors: [[0.1, 0.6, 0.9]; mc_core::MAX_PLAYERS],
            },
        )
        .unwrap();
        // More pairs (`x,y,x2,y2,..`) set off more warheads at once, beside the first.
        let ats: Vec<Vec2> = nums("NUKE_AT", "3825,5925")
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| Vec2::new(p[0], p[1]))
            .collect();
        let at = ats[0];
        // `NUKE_SALVO=n,seconds,metres`: n more warheads on the first mark after it, spread
        // evenly over that many seconds, each up to that far off the mark.
        let salvo = nums("NUKE_SALVO", "0,0,0");
        let mut bursts: Vec<(f32, Vec2)> = ats.iter().map(|&p| (0.0, p)).collect();
        for k in 0..salvo[0] as usize {
            let h = |s: f32| {
                ((k as f32 * 12.9898 + s * 78.233).sin() * 43758.547)
                    .fract()
                    .abs()
            };
            let off = Vec2::from_angle(h(1.0) * std::f32::consts::TAU) * salvo[2] * h(2.0).sqrt();
            bursts.push(((k + 1) as f32 * salvo[1] / salvo[0].max(1.0), at + off));
        }
        let cam = nums("NUKE_CAM", "5200,0.6,0.35");
        let times = nums("NUKE_TIMES", "0.05,0.6,2,5,10,20,40,90");
        let radius = nums("NUKE_RADIUS", "520")[0];
        let out = std::path::PathBuf::from(
            std::env::var("NUKE_OUT")
                .unwrap_or_else(|_| root.join("artifacts/nuke").display().to_string()),
        );
        std::fs::create_dir_all(&out).unwrap();
        let mut camera = Camera::new(
            Vec2::from(map.info().size_metres().to_f32()),
            Vec2::new(w as f32, h as f32),
        );
        let ground = renderer.ground_height(at);
        camera.focus = at.extend(ground + cam[0] * 0.06);
        camera.distance = cam[0];
        camera.yaw = cam[1];
        camera.tilt = cam[2];
        let mut frame = RenderFrame {
            props_dead: vec![0; map.props().len().div_ceil(32)],
            ..Default::default()
        };
        frame.stains.push(mc_sim::mirror::StainInstance {
            pos: [0.0, 0.0],
            radius: 1.0,
            strength_seed: 0,
        });
        let trees: Vec<(usize, Vec2)> = map
            .props()
            .iter()
            .enumerate()
            .filter(|(_, p)| p.kind.is_tree())
            .map(|(i, p)| (i, Vec2::from(p.pos.to_f32())))
            .filter(|(_, p)| p.distance(at) < radius * 1.3)
            .filter(|_| std::env::var("NUKE_NO_TREES").is_err())
            .collect();
        let overlay = Overlay::default();
        // `NUKE_MISSILES=n`: n warheads come down on the mark from all round (a salvo in flight).
        let flights = nums("NUKE_MISSILES", "1")[0].max(1.0) as usize;
        let missile = std::env::var("NUKE_MISSILE").is_ok() || flights > 1;
        let start = 100.0;
        let burst = if missile { start + 12.0 } else { start };
        let last = times.iter().copied().fold(0.0, f32::max);
        let mut t = start - 1.0;
        let mut tick = -1i64;
        let mut written = 0;
        while t <= burst + last + 0.06 {
            let now_tick = ((t - start) / 0.1).floor() as i64;
            let fresh = now_tick != tick;
            if fresh {
                tick = now_tick;
                frame.tick = (tick + 10) as u32;
                frame.events.clear();
                frame.strategic.clear();
                let age = t - burst;
                for &(when, p) in &bursts {
                    if (when..when + 0.1).contains(&age) {
                        let g = renderer.ground_height(p);
                        frame.events.push(SimEvent::NuclearDetonation {
                            pos: FxVec3::new(
                                Fx::from_f32(p.x),
                                Fx::from_f32(p.y),
                                Fx::from_f32(g + 55.0),
                            ),
                            radius: Fx::from_f32(radius),
                            owner: 0,
                            commander: false,
                        });
                    }
                }
                // Trees die as the front reaches them (the sim's pace, roughly).
                let front = radius * 1.3 * (age / 4.5).clamp(0.0, 1.0).sqrt();
                for &(i, p) in &trees {
                    if age >= 0.0 && p.distance(at) <= front {
                        frame.props_dead[i / 32] |= 1 << (i % 32);
                    }
                }
                if missile && t < burst {
                    // A warhead coming down on the mark, an interceptor going up past it.
                    for k in 0..flights {
                        let s = (t - start) / 12.0;
                        let dir = Vec2::from_angle(
                            std::f32::consts::PI
                                + k as f32 / flights as f32 * std::f32::consts::TAU,
                        );
                        let from = at + dir * (9000.0 - 3000.0 * (k % 3) as f32);
                        let xy = from.lerp(at, s);
                        let z = ground + 55.0 + 3500.0 * (1.0 - s) * (0.6 + s);
                        let s0 = ((t - 0.1 - start) / 12.0).max(0.0);
                        let xy0 = from.lerp(at, s0);
                        let z0 = ground + 55.0 + 3500.0 * (1.0 - s0) * (0.6 + s0);
                        frame.strategic.push(StrategicInstance {
                            prev_pos: [xy0.x, xy0.y, z0],
                            kind: STRATEGIC_WARHEAD,
                            pos: [xy.x, xy.y, z],
                            owner: 1,
                            mark: [at.x, at.y, ground],
                            age: t - start,
                            serial: 1 + k as u32 * 2,
                            eta: (1.0 - s) * 12.0,
                            boost: 0.0,
                            quarry: 0,
                            scale: 1.0,
                        });
                    }
                    let up = at + Vec2::new(900.0, 400.0);
                    let zi = ground + 20.0 + 600.0 * (t - start);
                    frame.strategic.push(StrategicInstance {
                        prev_pos: [up.x, up.y, zi - 60.0],
                        kind: STRATEGIC_INTERCEPTOR,
                        pos: [up.x, up.y, zi],
                        owner: 0,
                        mark: [at.x, at.y, ground + 1500.0],
                        age: t - start,
                        serial: 2,
                        eta: 0.0,
                        boost: 0.0,
                        quarry: 1,
                        scale: 1.0,
                    });
                }
            }
            let clock = std::time::Instant::now();
            renderer
                .render(&FrameInput {
                    camera: &camera,
                    time: t,
                    alpha: ((t - start) / 0.1).fract(),
                    sim: fresh.then_some(&frame),
                    ghosts: &[],
                    marks: &[],
                    ranges: &[],
                    ranges_drawn: 0,
                    overlay: &overlay,
                    build_grid: false,
                })
                .unwrap();
            let cpu_ms = clock.elapsed().as_secs_f32() * 1000.0;
            let age = t - burst;
            if written < times.len() && age >= times[written] - 1e-4 {
                let pixels = renderer.read_pixels().unwrap();
                let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
                for p in pixels.as_chunks::<4>().0 {
                    ppm.extend_from_slice(&p[..3]);
                }
                std::fs::write(out.join(format!("nuke_{:05.1}.ppm", times[written])), ppm).unwrap();
                // The same frame again a few times: the least each pass took, as another
                // program on the GPU makes single frames swing.
                let mut best: Vec<(&str, f32)> = Vec::new();
                let mut best_cpu = cpu_ms;
                for _ in 0..8 {
                    let clock = std::time::Instant::now();
                    renderer
                        .render(&FrameInput {
                            camera: &camera,
                            time: t,
                            alpha: ((t - start) / 0.1).fract(),
                            sim: None,
                            ghosts: &[],
                            marks: &[],
                            ranges: &[],
                            ranges_drawn: 0,
                            overlay: &overlay,
                            build_grid: false,
                        })
                        .unwrap();
                    renderer.read_pixels();
                    best_cpu = best_cpu.min(clock.elapsed().as_secs_f32() * 1000.0);
                    for &(n, ms) in &renderer.stats.gpu_passes {
                        match best.iter_mut().find(|b| b.0 == n) {
                            Some(b) => b.1 = b.1.min(ms),
                            None => best.push((n, ms)),
                        }
                    }
                }
                // And again without the volumes: what they cost.
                renderer.nuke_fx.volume_off = true;
                let mut bare = f32::MAX;
                for _ in 0..6 {
                    renderer
                        .render(&FrameInput {
                            camera: &camera,
                            time: t,
                            alpha: ((t - start) / 0.1).fract(),
                            sim: None,
                            ghosts: &[],
                            marks: &[],
                            ranges: &[],
                            ranges_drawn: 0,
                            overlay: &overlay,
                            build_grid: false,
                        })
                        .unwrap();
                    renderer.read_pixels();
                    if let Some(&(_, ms)) =
                        renderer.stats.gpu_passes.iter().find(|p| p.0 == "scene")
                    {
                        bare = bare.min(ms);
                    }
                }
                renderer.nuke_fx.volume_off = false;
                let with = best.iter().find(|p| p.0 == "scene").map_or(0.0, |p| p.1);
                println!(
                    "volume costs {:.2} ms of the scene ({:.2} with, {:.2} without)",
                    with - bare,
                    with,
                    bare
                );
                let passes: Vec<String> =
                    best.iter().map(|(n, ms)| format!("{n} {ms:.2}")).collect();
                let blasts = &renderer.nuke_fx.blasts;
                println!(
                    "wrote t={} cpu {:.2} ms (least {:.2}), gpu least {}; blasts {}, sizes {:?}",
                    times[written],
                    cpu_ms,
                    best_cpu,
                    passes.join(", "),
                    blasts.len(),
                    blasts
                        .iter()
                        .map(|b| (b.scale * 100.0).round() / 100.0)
                        .collect::<Vec<_>>(),
                );
                written += 1;
            }
            t += 0.05;
        }
        let _ = Vec3::ZERO;
    }
}

#[cfg(test)]
mod salvo {
    use super::*;

    fn blast(start: f32) -> Blast {
        Blast {
            at: Vec3::ZERO,
            start,
            scale: 1.0,
            base: 1.0,
            warheads: 1.0,
            eased: start,
            radius: WARHEAD_RADIUS,
            feeds: [(-1.0e9, 0.0); RECORDS],
            kicks: [(-1.0e9, 0.0); RECORDS],
            churn_bank: 0.0,
            seed: 0.3,
            ground: 0.0,
            next_bolt: 0.0,
            bolt: (-100.0, 0.0, 0.0),
            parted: false,
            felled: 0,
            lit: 0,
        }
    }

    #[test]
    fn a_fireball_grows_with_what_folds_into_it_but_never_past_its_bound() {
        let mut b = blast(0.0);
        let mut last = b.grown();
        for n in 2..200 {
            b.warheads = n as f32;
            assert!(b.grown() >= last);
            last = b.grown();
        }
        assert!((last - MOST_GROWTH).abs() < 1e-4, "{last}");
        // Easing gets there over some seconds, not at once.
        b.warheads = 40.0;
        b.grow(1.0);
        assert!(
            b.scale > 1.0 && b.scale < b.grown() * 0.5 + 0.5,
            "{}",
            b.scale
        );
        for k in 2..60 {
            b.grow(k as f32);
        }
        assert!((b.scale - b.grown()).abs() < 0.01);
    }

    #[test]
    fn many_bursts_at_once_heave_the_cap_only_a_little() {
        let mut b = blast(0.0);
        for k in 0..50 {
            b.kick(1.0 + k as f32 * 0.01, 1.0);
        }
        assert!(b.scale <= b.grown() * 1.1 + 1e-4, "{}", b.scale);
    }

    #[test]
    fn the_churn_never_jumps_back_when_old_kicks_are_forgotten() {
        let mut b = blast(0.0);
        let mut last = 0.0;
        let mut t = 0.0;
        for k in 0..40 {
            // Sample just before and after each kick: no drop.
            t = k as f32 * 0.7;
            let before = b.churn(t);
            assert!(before + 1e-4 >= last, "{before} after {last}");
            b.kick(t, 1.0);
            let after = b.churn(t);
            assert!(
                (after - before).abs() < 1e-4,
                "a kick moves the churn at once: {before} -> {after}"
            );
            last = after;
        }
        assert!(b.churn(t + 10.0) > last);
    }

    #[test]
    fn folded_fire_thickens_partway_and_keeps_it_drawn() {
        let mut b = blast(0.0);
        b.feed(30.0, 0.9);
        let most = (0..400)
            .map(|k| b.thick(30.0 + k as f32 * 0.1))
            .fold(0.0, f32::max);
        assert!(most > 0.2 && most <= 0.45 + 1e-4, "{most}");
        assert!(b.fuel(30.2) > b.fuel(50.0));
        // Fed late, it stays drawn past its own life.
        assert!(b.alive(90.0));
        assert!(!blast(0.0).alive(90.0));
    }
}
