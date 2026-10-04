//! The Regency's Gravitic Seekers and Counter-seekers as they are drawn (docs/STYLE.md "The
//! Regency suite"): a plasma charge held and steered in gravity containment, never a rocket.
//! Picked by data, never by a unit: a missile weapon with `plasma_grade: Gravitic`
//! (`Weapon::gravitic_seeker`), and a faction's `anti_missile_look: CounterSeeker`.
//!
//! - **The seeker** (the Pavise's battery and heavy seeker, the silos): it leaves its cell
//!   the moment it is fired, with a hard red flash and a snap of filaments, no motor flame,
//!   nothing gathered or held first. In flight every Regency missile is its charge (sprites.wgsl
//!   `gravitic_seeker`, `PLASMA_LOOK_GRAVITIC_SEEKER`, any plasma `missile`): a
//!   lavender-white heart in a violet body in a faint shimmering lens, and behind it a black
//!   smoke tube (`seeker_smoke`). Violet and black are how a Regency missile reads as one a
//!   missile defence can take, apart from ARC's white smoke. A cased seeker (the Sower's,
//!   `Weapon::cased_seeker`) is drawn as its faceted body instead (sprites.wgsl `vs_missile`),
//!   its charge glowing violet at the tail, with the same smoke, launch and strike. Where it
//!   strikes the lens lets go: it snaps in, then a hard
//!   red burst over a white heart, filaments torn out, globs and spatter thrown out low, the
//!   ground glassed under it; a heavy one's many times bigger, by its damage and `impact`.
//! - **The counter-seeker** (missile defence, `SimEvent::MissileLased` from a faction that
//!   throws them): a small red charge (`PLASMA_LOOK_COUNTER_SEEKER`) off the mount that runs the missile down along a
//!   cooling filament and bursts on it, hard and short, the tick the sim kills it. A burn the
//!   sim lets go of without a kill fizzles where the counter-seeker had got to. Only the
//!   look: what dies, and when, is the sim's.
//!
//! Nothing is wound round a middle (no spiral arms, no rings), and nothing hangs as a mist.
//! Presentation only; the renderer's own clock.

use super::nuke_fx::PUFF_STRATEGIC_TRAIL;
use super::regency_guns_fx::{cased_seeker, drawn_look};
use super::Renderer;
use crate::gpu_consts::{fade_beam, plasma_look, puff};
use glam::Vec3;
use mc_core::FxVec3;
use mc_data::{AntiMissileLook, BlueprintId, Weapon, WeaponColor};
use mc_sim::mirror::{
    ProjectileInstance, PROJECTILE_ENDS_SHIFT, PROJECTILE_FADE_BEAM, PROJECTILE_FRESH,
    PROJECTILE_STARTS_SHIFT,
};

const BURST: f32 = puff::PLASMA_BURST as f32;
const GLOB: f32 = puff::PLASMA_GLOB as f32;
const GLOW: f32 = puff::WARP_GLOW as f32;
const STREAK: f32 = puff::WARP_STREAK as f32;
const ARC: f32 = puff::WARP_ARC as f32;
const MOTE: f32 = puff::WARP_MOTE as f32;

/// Red plasma, its pink-white heart, and white: brightness in their size.
const RED: Vec3 = Vec3::new(1.0, 0.07, 0.04);
const HOT: Vec3 = Vec3::new(1.0, 0.55, 0.5);
const WHITE: Vec3 = Vec3::new(1.0, 0.96, 1.0);
/// Metres a piece of a counter-seeker's filament runs.
const TRAIL_STEP: f32 = 12.0;
/// Metres of a seeker's path between two puffs of its smoke tube.
const SMOKE_STEP: f32 = 9.0;
/// Trail pieces held at most. A deliberate cosmetic cap: the oldest goes first.
const MAX_TRAILS: usize = 1600;
/// Timed lights held at most. A deliberate cosmetic cap: the oldest goes first.
const MAX_GLOWS: usize = 64;
/// Counter-seekers in flight that are drawn. A deliberate cosmetic cap: past it a burn is
/// not drawn (the kill is still the sim's).
const MAX_COUNTERS: usize = 96;
/// A counter-seeker closes this share of the gap to its missile each tick it chases it,
/// and meets it this far through the tick the sim kills it.
const CHASE: f32 = 0.55;
const MEET: f32 = 0.6;
/// A counter-seeker's charge across, metres.
const COUNTER_SIZE: f32 = 0.55;

/// A piece of a filament: from `start` for `life` seconds, cooling as it goes.
#[derive(Clone, Copy)]
struct Filament {
    from: Vec3,
    to: Vec3,
    start: f32,
    life: f32,
    width: f32,
}

/// A light: from `start` for `life` seconds, fading out.
#[derive(Clone, Copy)]
struct Glow {
    pos: Vec3,
    color: Vec3,
    range: f32,
    start: f32,
    life: f32,
}

/// A counter-seeker running a missile down.
struct Counter {
    /// The mount it left.
    mount: Vec3,
    /// Where it has got to.
    head: Vec3,
    /// Where the missile was on the tick before, and on the latest.
    prev_target: Vec3,
    target: Vec3,
    /// When the latest tick's burn came in.
    last: f32,
    killed: bool,
    fresh: bool,
}

#[derive(Default)]
pub(super) struct GraviticFx {
    trails: Vec<Filament>,
    glows: Vec<Glow>,
    counters: Vec<Counter>,
}

impl GraviticFx {
    fn light(&mut self, glow: Glow) {
        if self.glows.len() >= MAX_GLOWS {
            self.glows.remove(0);
        }
        self.glows.push(glow);
    }
}

/// A seeker's charge across in metres, as its strike scales: by its damage.
fn charge_size(w: &Weapon) -> f32 {
    charge_size_of(w, w.damage)
}

/// `charge_size` for a charge carrying `damage`: a cluster's piece (`Weapon::cluster`)
/// carries its share.
fn charge_size_of(w: &Weapon, damage: mc_core::Fx) -> f32 {
    (1.0 + damage.to_f32().max(1.0).sqrt() * 0.1) * w.flash.max(0.5)
}

/// A random direction, `lift` added upward before it is made unit length.
fn scatter_dir(r: &mut impl FnMut() -> f32, lift: f32) -> Vec3 {
    Vec3::new(r(), r(), r() + lift).normalize_or(Vec3::Z)
}

impl Renderer {
    /// A Gravitic Seeker left its cell (`ShotFired`): `at` the cell's mouth as drawn, `dir`
    /// the way it goes. A hard red flash and a snap of filaments; no flame, no smoke. True
    /// when this drew the launch.
    pub(super) fn seeker_fired(
        &mut self,
        blueprint: BlueprintId,
        weapon: u8,
        at: Vec3,
        dir: Vec3,
        time: f32,
    ) -> bool {
        let w = &self.blueprints.unit(blueprint).weapons[weapon as usize];
        if !w.gravitic_seeker() {
            return false;
        }
        let s = charge_size(w);
        // A heavy seeker leaves with more filaments snapping off it.
        let heavy = w.damage.to_f32() >= 1000.0;
        let mouth = at + dir * s * 0.3;
        self.push_lit(
            BURST,
            mouth,
            Vec3::ZERO,
            time,
            0.16,
            (s * 0.3, s * 1.1),
            RED * 3.5,
            0.0,
        );
        self.push_lit(
            GLOW,
            mouth,
            Vec3::ZERO,
            time,
            0.1,
            (s * 0.5, s * 0.8),
            HOT * 3.0,
            0.0,
        );
        // The containment closing on the charge as it leaves: filaments snapping in to it.
        let snaps = if heavy { 7 } else { 3 };
        for _ in 0..snaps {
            let mut r = || self.scatter.signed();
            let out = scatter_dir(&mut r, 0.0);
            let from = mouth + out * s * (1.2 + 0.8 * self.scatter.unit());
            self.push_lit(
                STREAK,
                from,
                mouth - from,
                time,
                0.12,
                (s * 0.05, s * 0.03),
                RED.lerp(HOT, 0.4) * 4.0,
                0.0,
            );
        }
        self.plasma_fx.seekers.light(Glow {
            pos: mouth,
            color: RED * 60.0 * s,
            range: 6.0 * s,
            start: time,
            life: 0.15,
        });
        true
    }

    /// A Gravitic Seeker struck (`Impact`, not on a shield). True when this drew the strike
    /// (and a shell's blast should not be): the lens snaps in on the charge and lets it go.
    pub(super) fn seeker_struck(
        &mut self,
        blueprint: BlueprintId,
        weapon: u8,
        at: Vec3,
        splash: mc_core::Fx,
        on_unit: bool,
        start: f32,
    ) -> bool {
        let w = &self.blueprints.unit(blueprint).weapons[weapon as usize];
        if !w.gravitic_seeker() {
            return false;
        }
        // A cluster's piece bursts at its share's size (`Weapon::landed_damage`).
        let s = charge_size_of(w, w.landed_damage(splash)) * 1.4 * w.impact.max(0.5);
        let height = at.z - self.ground_height(at.truncate());
        let ground = height < s * 0.5 && !on_unit;
        self.gravitic_burst(at, s, ground, start);
        true
    }

    /// The containment letting go of a charge `s` metres across at `at`: the lens snaps in on
    /// it, then a hard red burst over a white heart, filaments torn out, globs and spatter
    /// thrown out low, and the ground glassed under it when it is near.
    fn gravitic_burst(&mut self, at: Vec3, s: f32, ground: bool, start: f32) {
        // The lens collapsing onto the charge.
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            start,
            0.05,
            (s * 1.6, s * 0.3),
            HOT * 1.5,
            0.0,
        );
        let go = start + 0.04;
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            go,
            0.16,
            (s * 0.7, s * 1.1),
            WHITE * 5.0,
            0.0,
        );
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            go,
            0.45,
            (s * 0.6, s * 1.9),
            RED * 5.0,
            0.0,
        );
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            go + 0.04,
            0.5,
            (s * 0.45, s * 1.3),
            RED.lerp(HOT, 0.35) * 3.5,
            0.0,
        );
        // Filaments torn out of it.
        let tears = (3.0 + s * 0.4).min(12.0) as usize;
        for _ in 0..tears {
            let mut r = || self.scatter.signed();
            let out = scatter_dir(&mut r, 0.1);
            let reach = s * (0.6 + 0.5 * self.scatter.unit());
            self.push_lit(
                STREAK,
                at,
                out * reach,
                go,
                0.22,
                (s * 0.035, s * 0.02),
                RED.lerp(HOT, 0.5) * 4.0,
                0.0,
            );
        }
        // Globs of it thrown out as the bind breaks, in place of a shock ring.
        let globs = (5.0 + s * 0.5).min(14.0) as usize;
        for _ in 0..globs {
            let mut r = || self.scatter.signed();
            let out = scatter_dir(&mut r, 0.3);
            let speed = s * (2.2 + 2.8 * self.scatter.unit());
            let blob = s * (0.08 + 0.06 * self.scatter.unit());
            let roll = self.scatter.unit();
            self.push_lit(
                GLOB,
                at,
                out * speed,
                go,
                0.75,
                (blob, blob * 0.35),
                RED.lerp(HOT, roll * 0.4) * 3.5,
                0.0,
            );
        }
        let sparks = (8.0 + s * 1.2).min(28.0) as usize;
        for _ in 0..sparks {
            let mut r = || self.scatter.signed();
            let out = scatter_dir(&mut r, 0.2);
            let speed = 10.0 + s * 3.0 * self.scatter.unit();
            let life = 0.4 + self.scatter.unit() * 0.5;
            let dot = (0.12 + s * 0.02).min(0.5);
            self.push_lit(
                MOTE,
                at,
                out * speed,
                go,
                life,
                (dot, dot * 0.35),
                RED.lerp(HOT, 0.3) * 4.0,
                0.0,
            );
        }
        // A big one throws red lightning into the ground round it.
        if ground && s > 8.0 {
            for _ in 0..5 {
                let a = self.scatter.unit() * std::f32::consts::TAU;
                let reach = s * (0.8 + 0.7 * self.scatter.unit());
                let to = Vec3::new(a.cos() * reach, a.sin() * reach, -s * 0.3);
                let late = self.scatter.unit() * 0.15;
                self.push_lit(
                    ARC,
                    at,
                    to,
                    go + late,
                    0.12,
                    (s * 0.05, s * 0.05),
                    RED.lerp(HOT, 0.5) * 5.0,
                    0.0,
                );
            }
        }
        if ground {
            self.ground_melt.melt(at.truncate(), s * 0.45, go, 3.0);
        }
        self.plasma_fx.seekers.light(Glow {
            pos: at + Vec3::Z,
            color: RED * 90.0 * s,
            range: s * 5.0,
            start: go,
            life: 0.35,
        });
    }

    /// A tick of missile defence (`MissileLased`) from a defender whose faction throws
    /// counter-seekers (`AntiMissileLook::CounterSeeker`). True when this draws it (and the
    /// laser should not).
    pub(super) fn counter_seeker(
        &mut self,
        defender: BlueprintId,
        from: &FxVec3,
        to: &FxVec3,
        killed: bool,
        time: f32,
    ) -> bool {
        let faction = self.blueprints.unit(defender).faction;
        let look = self
            .blueprints
            .factions
            .get(faction.0 as usize)
            .map_or(AntiMissileLook::Laser, |f| f.anti_missile_look);
        if look != AntiMissileLook::CounterSeeker {
            return false;
        }
        let mount = Vec3::from(from.to_f32());
        let at = Vec3::from(to.to_f32());
        let tick = self.tick_seconds.max(0.02);
        let fx = &mut self.plasma_fx.seekers;
        // The one already running this missile down from this mount: the missile near where
        // its last step says it would be.
        let held = fx.counters.iter().position(|c| {
            !c.killed
                && c.mount.distance(mount) < 0.5
                && (c.target
                    + (c.target - c.prev_target) * ((time - c.last) / tick).clamp(0.0, 2.0))
                .distance(at)
                    < 60.0
        });
        match held {
            Some(i) => {
                let c = &mut fx.counters[i];
                c.prev_target = c.target;
                c.target = at;
                c.last = time;
                c.killed = killed;
            }
            None if fx.counters.len() < MAX_COUNTERS => fx.counters.push(Counter {
                mount,
                head: mount,
                prev_target: at,
                target: at,
                last: time,
                killed,
                fresh: true,
            }),
            None => {}
        }
        true
    }

    /// The next tick of every seeker and counter-seeker (from `upload_sim`, after this tick's
    /// events and shots): the filaments seekers leave, the counter-seekers moved on, burst
    /// or fizzled.
    pub(super) fn gravitic_tick(&mut self, projectiles: &[ProjectileInstance], time: f32) {
        self.seeker_trails(projectiles, time);
        self.counter_seekers(time);
        self.write_gravitic(time);
    }

    /// The black smoke each seeker in flight lays down the stretch it flies this tick, each
    /// piece lit as the charge passes it. A cased seeker's leaves from its tail.
    fn seeker_trails(&mut self, projectiles: &[ProjectileInstance], time: f32) {
        for p in projectiles {
            let cased = cased_seeker(p);
            if p.color & PROJECTILE_FADE_BEAM != 0
                || !(cased || drawn_look(p) == plasma_look::GRAVITIC_SEEKER)
            {
                continue;
            }
            let behind = if cased {
                super::missile_half_length(p.size, p.aim[3])
            } else {
                p.size * 0.5
            };
            let ends = ((p.color >> PROJECTILE_ENDS_SHIFT) & 0xFF) as f32 / 255.0;
            let span = if ends > 0.0 { ends } else { 1.0 };
            let starts = (p.color >> PROJECTILE_STARTS_SHIFT) as f32 / 255.0;
            let to = Vec3::from(p.pos);
            let from = Vec3::from(p.prev_pos).lerp(to, (starts / span).min(1.0));
            self.seeker_smoke(from, to, time, starts, span, p.size, behind);
        }
    }

    /// A seeker's smoke down `from` to `to`, flown from `starts` to `span` of the tick, laid
    /// `behind` metres back of where the charge is drawn: the missile's smoke tube
    /// (puffs.wgsl `strategic_trail`) in black, a little violet glow where the charge has
    /// just passed, spreading and going grey as it hangs. Puffs a step apart, each a tent a
    /// step either side, add up to an unbroken column.
    fn seeker_smoke(
        &mut self,
        from: Vec3,
        to: Vec3,
        time: f32,
        starts: f32,
        span: f32,
        size: f32,
        behind: f32,
    ) {
        let tick = self.tick_seconds.max(0.02);
        let dir = (to - from).normalize_or_zero();
        let width = (0.8 + size * 0.35).min(2.6);
        let life = (2.2 + size * 0.6).min(5.0);
        // The tube's radius is 0.28 of a puff's size; a negative end size cools the glow
        // fast, over a few tens of metres.
        let tube = (width * 1.8, -width * 4.5 * 1.8);
        // A deliberate cosmetic cap on one stretch: a tick's flight is far under this many.
        let n = (from.distance(to) / SMOKE_STEP).ceil().clamp(1.0, 8.0) as u32;
        let step = from.distance(to) / n as f32;
        for k in 0..n {
            let at = from.lerp(to, k as f32 / n as f32) - dir * behind;
            let f1 = (k + 1) as f32 / n as f32;
            let start = time + tick * (starts + (span - starts).max(0.0) * f1);
            // Strength one, below zero for black smoke (`push_puff_with_motion`).
            self.push_puff_with_motion(
                PUFF_STRATEGIC_TRAIL,
                at,
                dir * step,
                start,
                life,
                tube,
                Vec3::new(-1.0, 0.0, 0.0),
            );
        }
    }

    /// A filament down `from` to `to`, flown from `starts` to `span` of the tick: a thin
    /// hot thread and the sheath round it that goes out sooner.
    fn lay_filament(
        &mut self,
        from: Vec3,
        to: Vec3,
        time: f32,
        starts: f32,
        span: f32,
        size: f32,
        life: f32,
    ) {
        let tick = self.tick_seconds.max(0.02);
        // A deliberate cosmetic cap on one stretch: a tick's flight is far under this many.
        let n = ((from.distance(to) / TRAIL_STEP).ceil() as usize).clamp(1, 16);
        let fx = &mut self.plasma_fx.seekers;
        for k in 0..n {
            let (f0, f1) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
            let (a, b) = (from.lerp(to, f0), from.lerp(to, f1));
            let start = time + tick * (starts + (span - starts).max(0.0) * f1);
            fx.trails.push(Filament {
                from: a,
                to: b,
                start,
                life,
                width: size * 0.1,
            });
            fx.trails.push(Filament {
                from: a,
                to: b,
                start,
                life: life * 0.25,
                width: size * 0.25,
            });
        }
    }

    /// Every counter-seeker moved on a tick: closing on its missile while the burn goes on,
    /// meeting it and bursting on the tick it is killed, fizzling where it got to when the
    /// burn stops without one.
    fn counter_seekers(&mut self, time: f32) {
        let tick = self.tick_seconds.max(0.02);
        let mut shots = Vec::new();
        let mut bursts = Vec::new();
        let mut fizzles = Vec::new();
        let mut filaments = Vec::new();
        self.plasma_fx.seekers.counters.retain_mut(|c| {
            if c.last < time - tick * 0.5 {
                fizzles.push(c.head);
                return false;
            }
            let lead = c.target + (c.target - c.prev_target) * 0.5;
            let next = if c.killed {
                c.target
            } else {
                c.head + (lead - c.head) * CHASE
            };
            let ends = if c.killed { MEET } else { 0.0 };
            shots.push((c.head, next, ends, c.fresh));
            filaments.push((c.head, next, if c.killed { MEET } else { 1.0 }));
            c.head = next;
            c.fresh = false;
            if c.killed {
                bursts.push(c.target);
            }
            !c.killed
        });
        let hot = 1.0 + 1.0 + 2.0 * plasma_look::COUNTER_SEEKER as f32;
        let out: Vec<ProjectileInstance> = shots
            .into_iter()
            .map(|(from, to, ends, fresh)| ProjectileInstance {
                prev_pos: from.to_array(),
                color: WeaponColor::Orange as u32
                    | ((ends * 255.0) as u32) << PROJECTILE_ENDS_SHIFT
                    | if fresh { PROJECTILE_FRESH } else { 0 },
                pos: to.to_array(),
                size: COUNTER_SIZE,
                wake: 0.0,
                plasma: 0.0,
                _pad: [hot, 0.0],
                aim: [0.0; 4],
                prev_aim: [0.0; 4],
            })
            .collect();
        self.push_projectiles(&out);
        for (from, to, span) in filaments {
            self.lay_filament(from, to, time, 0.0, span, COUNTER_SIZE * 1.4, 0.3);
        }
        for at in bursts {
            self.counter_burst(at, time + tick * MEET);
        }
        for at in fizzles {
            self.push_lit(
                BURST,
                at,
                Vec3::ZERO,
                time,
                0.15,
                (0.3, 1.2),
                RED * 2.4,
                0.0,
            );
        }
    }

    /// A counter-seeker meeting its missile: a small hard burst, white at the heart, red
    /// filaments and sparks thrown out, gone fast.
    fn counter_burst(&mut self, at: Vec3, start: f32) {
        let s = 3.2;
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            start,
            0.08,
            (s * 0.5, s * 0.9),
            WHITE * 3.0,
            0.0,
        );
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            start,
            0.3,
            (s * 0.3, s * 1.3),
            RED * 3.2,
            0.0,
        );
        for _ in 0..6 {
            let mut r = || self.scatter.signed();
            let out = scatter_dir(&mut r, 0.0);
            let reach = s * (0.8 + 0.6 * self.scatter.unit());
            self.push_lit(
                STREAK,
                at,
                out * reach,
                start,
                0.18,
                (0.1, 0.06),
                RED.lerp(HOT, 0.4) * 4.0,
                0.0,
            );
        }
        for _ in 0..10 {
            let mut r = || self.scatter.signed();
            let out = scatter_dir(&mut r, 0.1);
            let speed = 14.0 + 20.0 * self.scatter.unit();
            self.push_lit(
                MOTE,
                at,
                out * speed,
                start,
                0.45,
                (0.22, 0.08),
                RED.lerp(HOT, 0.3) * 4.0,
                0.0,
            );
        }
        self.plasma_fx.seekers.light(Glow {
            pos: at,
            color: RED * 260.0,
            range: 18.0,
            start,
            life: 0.25,
        });
    }

    /// This tick's filaments among the fading beams.
    fn write_gravitic(&mut self, time: f32) {
        let fx = &mut self.plasma_fx.seekers;
        fx.trails.retain(|t| time < t.start + t.life);
        if fx.trails.len() > MAX_TRAILS {
            let extra = fx.trails.len() - MAX_TRAILS;
            fx.trails.drain(..extra);
        }
        let out: Vec<ProjectileInstance> = fx
            .trails
            .iter()
            .map(|t| ProjectileInstance {
                prev_pos: t.from.to_array(),
                color: PROJECTILE_FADE_BEAM | fade_beam::PLASMA_TRAIL,
                pos: t.to.to_array(),
                size: t.width,
                wake: t.start,
                plasma: t.life,
                _pad: [0.0; 2],
                // A counter-seeker's filament: pink-hot to red, never white (sprites.wgsl).
                aim: [0.0, 0.0, 0.0, fade_beam::PLASMA_TRAIL_PINK],
                prev_aim: [0.0; 4],
            })
            .collect();
        self.push_projectiles(&out);
    }

    /// Every seeker's and counter-seeker's light this frame (from `upload_lights`).
    pub(super) fn gravitic_lights(&mut self, time: f32) {
        let fx = &mut self.plasma_fx.seekers;
        fx.glows.retain(|g| time < g.start + g.life);
        for g in &fx.glows {
            let age = (time - g.start) / g.life.max(0.01);
            if age < 0.0 {
                continue;
            }
            let fade = (1.0 - age).clamp(0.0, 1.0);
            self.lights
                .lamp(g.pos, Vec3::NEG_Z, g.color * fade, g.range, 180.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_seekers_look_is_the_one_the_mirror_writes() {
        assert_eq!(
            crate::gpu_consts::plasma_look::GRAVITIC_SEEKER,
            mc_sim::mirror::PLASMA_LOOK_GRAVITIC_SEEKER
        );
    }
}
