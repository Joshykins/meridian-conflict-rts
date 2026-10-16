//! A gravitic shock shell as it is drawn (docs/STYLE.md "The Regency suite"): a `Gravitic`
//! gun lobbing a shell that shatters the ground where it lands (`Weapon::shatter`, the
//! Exarch's Gravitic Shock Mortar). Picked by the weapon's data, never by a unit. Violet
//! from the bore to the ground: no powder flash or smoke, no orange blast, no crater, no
//! red plasma.
//!
//! - **The charge** (`SimEvent::WeaponCharging`, `charge`): the crystal winding up and
//!   lighting, motes drawn in, light running up the tines, arcs across the gap, a ring of
//!   bent light closing on the mouth.
//! - **The shot** (`SimEvent::ShotFired`): the crystal lets go in a white-violet flash
//!   thrown up the bore, the knot pinched out between the points, a ring of bent light
//!   tightening on it at the mouth and the view bent out round it; violet pressure rings
//!   punched out one after another along the line of fire, lightning snapping off the
//!   points, motes and arcs thrown after it; lilac vapour blown out of the breech to either
//!   side as the gun slams back (the mirror's `run_out_pair`), and the air shoved down
//!   round the gun's feet.
//! - **In flight** (sprites.wgsl `plasma_look::GRAVITIC_SHELL`): a shard of violet crystal
//!   (`gravitic_shard`), a long point forward, turning about its length, lit from within
//!   and its points going white; a short twisting violet tail, and behind it a filament
//!   trail (`PLASMA_TRAIL_GRAVITIC`) that deepens to violet and goes out to indigo. It
//!   bends the view round it as it flies (a lens for the tone map) and lights what it
//!   passes violet (`lights.rs`). From afar it is a violet point, from orbit a marker.
//! - **The strike** (`SimEvent::Impact`): a pull first, a ring of bent light and motes
//!   drawn in to the point and the view bent in toward it; then the ground shatters
//!   (`rift::SHELL`, scars.wgsl): plates broken over `Weapon::shatter` metres, struck
//!   white-violet as the shatter runs out, a violet pressure ring and dust driven out over
//!   the ground, clods and motes thrown up, violet lightning run out over the breaking
//!   ground. The light in the cracks dies over a few seconds; the broken plates and dark
//!   cracks stay a while longer and then fade away. On a shield it is the shield's hit.
//!
//! Presentation only; the renderer's own clock.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};
use mc_data::{BlueprintId, Weapon};
use mc_sim::mirror::{
    ProjectileInstance, SimEvent, PROJECTILE_ENDS_SHIFT, PROJECTILE_FADE_BEAM,
    PROJECTILE_STARTS_SHIFT,
};

use super::gravitic_engine_fx::{LILAC, VIOLET, WHITE};
use super::heat_haze::GpuHeatPlume;
use super::lens_flare::Flare;
use super::regency_guns_fx::drawn_look;
use super::stun_fx::Flash;
use super::{Renderer, PUFF_CLOD, PUFF_DUST};
use crate::gpu_consts::{effect, fade_beam, haze, plasma_look, puff};

mod charge;

const MOTE: f32 = puff::WARP_MOTE as f32;
const ARC: f32 = puff::WARP_ARC as f32;
const LENS: f32 = puff::CRUSH_LENS as f32;
const GLOW: f32 = puff::WARP_GLOW as f32;
const STREAK: f32 = puff::WARP_STREAK as f32;
/// The pressure rings a shot punches out along its line of fire: metres out from the
/// mouth, metres across, each a little after the last.
const RINGS: [(f32, f32); 3] = [(3.0, 4.5), (6.5, 6.0), (10.5, 7.5)];
const GRAVITIC: f32 = effect::GRAVITIC as f32;
/// The motes' drag (warp_puffs.wgsl): one thrown at `v` goes `v * (1 - e^(-DRAG t)) / DRAG`.
const DRAG: f32 = 1.8;
/// Seconds the strike's pull draws in before the ground breaks.
const PULL: f32 = 0.14;
/// Metres a piece of a shell's trail runs, and the seconds its thread glows.
const TRAIL: (f32, f32) = (8.0, 1.6);
/// Trail pieces held at most. A deliberate cosmetic cap: the oldest goes first.
const MAX_TRAILS: usize = 1500;
/// Timed lights held at most. A deliberate cosmetic cap: the oldest goes first.
const MAX_GLOWS: usize = 64;
/// Lenses handed to the tone map at most (shells in flight and strikes' pulls together).
/// A deliberate cosmetic cap: the first laid are kept.
const MOST_LENSES: usize = 6;
/// Motes a shell's flight sheds a tick at most, over every shell. A deliberate cosmetic
/// cap: past it the trail is laid without them.
const MAX_MOTES: usize = 40;

/// A piece of a shell's trail: from `start` for `life` seconds.
#[derive(Clone, Copy)]
struct Piece {
    from: Vec3,
    to: Vec3,
    start: f32,
    life: f32,
    width: f32,
}

/// A light a shell throws: from `start` for `life` seconds, fading out.
#[derive(Clone, Copy)]
struct Glow {
    pos: Vec3,
    color: Vec3,
    range: f32,
    start: f32,
    life: f32,
}

/// The view bent toward a point (a strike's pull): from `start` for `life` seconds,
/// strongest at `peak` of the way through.
#[derive(Clone, Copy)]
struct Pull {
    at: Vec3,
    radius: f32,
    start: f32,
    life: f32,
    peak: f32,
}

#[derive(Default)]
pub(super) struct GraviticShellFx {
    trails: Vec<Piece>,
    glows: Vec<Glow>,
    /// Shells in flight this tick: where each was and is, and its size.
    flying: Vec<(Vec3, Vec3, f32)>,
    pulls: Vec<Pull>,
    charges: Vec<charge::Charge>,
    /// This frame's lenses for the tone map (heat_haze.rs).
    pub(super) lenses: Vec<GpuHeatPlume>,
}

impl GraviticShellFx {
    fn light(&mut self, glow: Glow) {
        if self.glows.len() >= MAX_GLOWS {
            self.glows.remove(0);
        }
        self.glows.push(glow);
    }
}

/// A gravitic shock shell's gun: a `Gravitic` gun lobbing a shell that shatters the ground.
fn shell_gun(w: &Weapon) -> bool {
    w.shatter > 0.0 && !w.missile && !w.beam
}

/// `v` for a mote to cover `reach` metres in `life` seconds against the air's drag.
fn mote_speed(reach: f32, life: f32) -> f32 {
    reach * DRAG / (1.0 - (-DRAG * life).exp()).max(0.05)
}

impl Renderer {
    fn shell_weapon(&self, blueprint: BlueprintId, weapon: u8) -> Option<&Weapon> {
        let w = self
            .blueprints
            .unit(blueprint)
            .weapons
            .get(weapon as usize)?;
        shell_gun(w).then_some(w)
    }

    /// Takes `event` when it is a gravitic shock shell's firing or strike: true when that is
    /// all of it (instead of a gun's flash and smoke, or a shell's blast). A strike on a
    /// shield is the shield's hit, as any shot's.
    pub(super) fn gravitic_shell_event(&mut self, event: &SimEvent, time: f32) -> bool {
        match event {
            SimEvent::ShotFired {
                pos,
                vel,
                travel,
                blueprint,
                weapon,
                ..
            } => {
                let Some(w) = self.shell_weapon(*blueprint, *weapon) else {
                    return false;
                };
                let (flash, shock) = (w.flash.clamp(0.5, 3.0), w.shockwave);
                let pivot = w.pivot.map_or(Vec3::ZERO, |p| Vec3::from(p.to_f32()));
                let bore = Vec3::from(w.muzzle.to_f32()).distance(pivot);
                let at = Vec3::from(pos.to_f32()) - Vec3::from(travel.to_f32());
                let dir = Vec3::from(vel.to_f32()).normalize_or(Vec3::X);
                let outbound = std::mem::replace(&mut self.effect_outbound, true);
                self.gravitic_shell_fired(at, dir, (flash, shock, bore), time);
                self.effect_outbound = outbound;
                true
            }
            SimEvent::Impact {
                pos,
                after,
                on_unit,
                on_shield,
                blueprint,
                weapon,
                ..
            } => {
                if *on_shield {
                    return false;
                }
                let Some(w) = self.shell_weapon(*blueprint, *weapon) else {
                    return false;
                };
                let (radius, flash) = (w.shatter, w.flash.clamp(0.5, 3.0));
                let start = time + after.to_f32() * self.tick_seconds;
                let at = Vec3::from(pos.to_f32());
                self.gravitic_shell_struck(at, radius, flash, *on_unit, start);
                true
            }
            _ => false,
        }
    }

    /// The knot leaves the bore at `at` along `dir`; `s` the weapon's flash, `shock` its
    /// shockwave and `bore` metres from its trunnions out to its mouth.
    fn gravitic_shell_fired(
        &mut self,
        at: Vec3,
        dir: Vec3,
        (s, shock, bore): (f32, f32, f32),
        time: f32,
    ) {
        let mouth = at + dir * 0.6 * s;
        self.gravitic_shell_let_go(at, dir, s, bore, time);
        self.gravitic_shell_kick(at, dir, s, bore, time);
        // The knot pinched out: a ring of bent light tightening on it at the mouth.
        self.push_lit(
            LENS,
            mouth,
            Vec3::ZERO,
            time,
            0.22,
            (3.2 * s, 0.6 * s),
            VIOLET * 6.0,
            0.0,
        );
        self.push_effect(mouth.to_array(), time, 2.6 * s, 0.28, GRAVITIC, 0.6);
        self.push_effect(
            (mouth + dir * 1.2 * s).to_array(),
            time + 0.03,
            1.4 * s,
            0.16,
            GRAVITIC,
            0.0,
        );
        if shock > 0.0 {
            // A violet pressure ring punched out along the bore, not a powder blast's.
            let tint = self
                .effect_settings
                .shockwave_color
                .replace(VIOLET.to_array());
            self.push_shockwave(
                (mouth + dir * 2.0).to_array(),
                time,
                (14.0 + 6.0 * s) * shock,
                0.5,
                (0.7 * shock).min(1.0),
                0.0,
                dir,
            );
            // Tight rings punched out one after another down the line of fire.
            for (k, &(out, across)) in RINGS.iter().enumerate() {
                self.push_shockwave(
                    (mouth + dir * out * s).to_array(),
                    time + 0.025 * k as f32,
                    across * s,
                    0.3 + 0.05 * k as f32,
                    (0.8 * shock).min(1.0),
                    0.0,
                    dir,
                );
            }
            self.effect_settings.shockwave_color = tint;
        }
        // The view bent out round the mouth for a moment as the knot leaves.
        self.plasma_fx.shells.pulls.push(Pull {
            at: mouth + dir * 2.0,
            radius: 5.0 * s,
            start: time,
            life: 0.4,
            peak: 0.12,
        });
        // Motes of violet thrown out after it in a loose cone, and arcs snapping forward.
        let side = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
        let up = side.cross(dir);
        for k in 0..14 {
            let a = TAU * (k as f32 + self.scatter.unit()) / 14.0;
            let spread = (side * a.cos() + up * a.sin()) * (0.25 + 0.35 * self.scatter.unit());
            let v = (dir + spread).normalize_or(dir) * mote_speed(6.0 * s, 0.5);
            let roll = self.scatter.unit();
            self.push_lit(
                MOTE,
                mouth,
                v * (0.5 + 0.7 * roll),
                time + roll * 0.05,
                0.4 + 0.3 * roll,
                (0.35 * s, 0.12 * s),
                VIOLET.lerp(LILAC, roll) * 5.0,
                0.0,
            );
        }
        for _ in 0..4 {
            let spread = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            ) * 0.5;
            let reach = s * (2.5 + 3.5 * self.scatter.unit());
            let late = self.scatter.unit() * 0.08;
            self.push_lit(
                ARC,
                mouth,
                (dir + spread).normalize_or(dir) * reach,
                time + late,
                0.12,
                (0.18 * s, 0.18 * s),
                LILAC * 5.0,
                0.0,
            );
        }
        self.plasma_fx.shells.light(Glow {
            pos: mouth,
            color: VIOLET.lerp(LILAC, 0.3) * 260.0 * s,
            range: 14.0 * s,
            start: time,
            life: 0.35,
        });
    }

    /// The crystal letting the knot go, the mouth at `at`: a white-violet flash where it
    /// hangs between the tines (`charge::CRYSTAL` of the `bore` back), its light thrown up
    /// the bore, and lightning snapping off the tines' points after the knot.
    fn gravitic_shell_let_go(&mut self, at: Vec3, dir: Vec3, s: f32, bore: f32, time: f32) {
        let side = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
        let up = side.cross(dir);
        let crystal = at - dir * bore * (1.0 - charge::CRYSTAL);
        self.push_lit(
            GLOW,
            crystal,
            Vec3::ZERO,
            time,
            0.22,
            (2.2 * s, 2.2 * s),
            WHITE.lerp(LILAC, 0.4) * 5.0,
            0.0,
        );
        self.push_lit(
            STREAK,
            crystal,
            dir * (at.distance(crystal) + 4.0 * s),
            time,
            0.16,
            (0.5 * s, 0.2 * s),
            LILAC * 6.0,
            0.0,
        );
        for tip in [up * 0.3, -up * 0.3] {
            let ahead = at + dir * s * (3.5 + 2.5 * self.scatter.unit());
            self.emp_kinks(
                at + tip,
                ahead,
                0.35 * s,
                side,
                up * 0.1,
                time,
                0.12,
                0.35,
                5,
                fade_beam::RIFT_BOLT,
            );
        }
    }

    /// The gun slamming back in its yoke, the mouth at `at` and the breech `bore` metres
    /// back: lilac vapour blown out of the breech to either side and back, and the air
    /// shoved down round the gun's feet.
    fn gravitic_shell_kick(&mut self, at: Vec3, dir: Vec3, s: f32, bore: f32, time: f32) {
        let side = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
        let up = side.cross(dir);
        let breech = at - dir * (bore + 1.2);
        for sign in [1.0f32, -1.0] {
            let vent = breech + side * sign * 0.8;
            for k in 0..3 {
                let out = (side * sign - dir * 0.5 + up * 0.25 * self.scatter.signed())
                    .normalize_or(side);
                let reach = s * (3.0 + 2.0 * self.scatter.unit());
                self.push_lit(
                    STREAK,
                    vent,
                    out * reach,
                    time + 0.012 * k as f32,
                    0.25,
                    (0.25 * s, 0.1 * s),
                    LILAC * 3.0,
                    0.0,
                );
            }
            for _ in 0..6 {
                let out = (side * sign - dir * 0.4
                    + Vec3::new(
                        self.scatter.signed(),
                        self.scatter.signed(),
                        self.scatter.signed(),
                    ) * 0.35)
                    .normalize_or(side);
                let roll = self.scatter.unit();
                self.push_lit(
                    MOTE,
                    vent,
                    out * mote_speed(4.0 * s * (0.6 + 0.6 * roll), 0.6),
                    time + roll * 0.04,
                    0.5 + 0.3 * roll,
                    (0.25 * s, 0.1 * s),
                    VIOLET.lerp(LILAC, 0.5 + 0.5 * roll) * 3.0,
                    0.0,
                );
            }
        }
        // The air shoved down round the feet: a low skirt of dust rolling out.
        let foot = breech.truncate();
        let floor = self.ground_height(foot);
        if at.z - floor > 60.0 || self.under_sea(foot.extend(floor + 0.1)) {
            return;
        }
        for i in 0..12 {
            let a = TAU * (i as f32 + self.scatter.unit() * 0.6) / 12.0;
            let way = Vec2::new(a.cos(), a.sin());
            let p = foot + way * (3.0 + 5.0 * self.scatter.unit());
            let speed = 9.0 + 7.0 * self.scatter.unit();
            let life = 1.4 + 0.6 * self.scatter.unit();
            self.push_puff(
                PUFF_DUST,
                p.extend(self.ground_height(p)),
                way.extend(0.1) * speed,
                time + 0.05,
                life,
                (1.0, 3.2),
            );
        }
    }

    /// The shell lands at `at` at `start`: a pull, then the ground shattered `radius` round.
    fn gravitic_shell_struck(&mut self, at: Vec3, radius: f32, s: f32, on_unit: bool, start: f32) {
        let r = radius.max(4.0);
        let centre = at.truncate();
        let floor = self.ground_height(centre);
        // On a hull the knot breaks against it; the ground under it shatters all the same
        // (the gun shells only what stands on the ground or the sea), as it does under a
        // shell that lands near enough to it.
        let ground = centre.extend(floor);
        let near = on_unit || at.z - floor < r * 0.6;
        let wet = self.under_sea(ground + Vec3::Z * 0.1);
        let heart = if on_unit || !near {
            at
        } else {
            ground + Vec3::Z * r * 0.1
        };
        // The pull: a ring of bent light tightening on the point, motes drawn in to it.
        self.push_lit(
            LENS,
            heart,
            Vec3::ZERO,
            start,
            PULL * 1.3,
            (r * 0.9, r * 0.12),
            VIOLET * 7.0,
            0.0,
        );
        for k in 0..18 {
            let a = TAU * (k as f32 + self.scatter.unit()) / 18.0;
            let d = r * (0.6 + 0.5 * self.scatter.unit());
            let from = heart + Vec3::new(a.cos() * d, a.sin() * d, r * 0.3 * self.scatter.signed());
            let v = (heart - from).normalize_or_zero() * mote_speed(d, PULL * 1.2);
            let roll = self.scatter.unit();
            self.push_lit(
                MOTE,
                from,
                v,
                start,
                PULL * 1.2,
                (0.3 * s, 0.1 * s),
                VIOLET.lerp(LILAC, roll) * 4.0,
                0.0,
            );
        }
        let bang = start + PULL;
        self.plasma_fx.shells.pulls.push(Pull {
            at: heart,
            radius: r * 0.9,
            start,
            life: PULL + 0.5,
            peak: PULL / (PULL + 0.5),
        });
        // The ground breaks into plates; on the sea, nothing to break.
        if near && !wet {
            self.gravitic_engine_fx.ground.shell(centre, r, bang);
        }
        // A hard white-violet flash, gone in a breath.
        self.push_effect(heart.to_array(), bang, r * 0.42, 0.4, GRAVITIC, 0.0);
        self.push_effect(heart.to_array(), bang + 0.03, r * 0.15, 0.5, GRAVITIC, 0.0);
        self.lens_flares.flash(
            heart,
            Flare {
                color: WHITE.lerp(VIOLET, 0.6) * 8.0,
                size: r * 0.7,
            },
            bang,
            0.45,
        );
        self.emp_fx.flashes.push(Flash {
            pos: heart + Vec3::Z * 2.0,
            start: bang,
            life: 0.6,
            color: VIOLET.lerp(LILAC, 0.3) * 8.0e3,
            range: r * 4.0,
            flicker: 0.2,
        });
        // A violet pressure ring and dust driven out over the ground.
        let tint = self
            .effect_settings
            .shockwave_color
            .replace(VIOLET.to_array());
        self.push_shockwave(
            (ground + Vec3::Z * 1.0).to_array(),
            bang,
            r * 2.6,
            0.75,
            0.85,
            0.0,
            Vec3::ZERO,
        );
        self.effect_settings.shockwave_color = tint;
        if !wet {
            // Earth and rock thrown up off the plates as they break.
            for _ in 0..26 {
                let off = Vec2::new(self.scatter.signed(), self.scatter.signed()) * r * 0.6;
                let from = (centre + off).extend(self.ground_height(centre + off));
                let out = off.normalize_or_zero() * (3.0 + 6.0 * self.scatter.unit());
                let vel = out.extend(14.0 + 22.0 * self.scatter.unit());
                let size = 0.5 + 0.9 * self.scatter.unit();
                let born = bang + off.length() / (r * 10.0);
                self.push_puff(PUFF_CLOD, from, vel, born, 2.6, (size, 0.8));
            }
            for i in 0..20 {
                let a = TAU * (i as f32 + self.scatter.unit() * 0.6) / 20.0;
                let dir = Vec2::new(a.cos(), a.sin());
                let p = centre + dir * r * (0.5 + 0.3 * self.scatter.unit());
                let from = p.extend(self.ground_height(p));
                let speed = r * (0.5 + 0.4 * self.scatter.unit());
                let life = 2.5 + 1.5 * self.scatter.unit();
                self.push_puff(
                    PUFF_DUST,
                    from,
                    dir.extend(0.15) * speed,
                    bang + 0.05,
                    life,
                    (r * 0.08, r * 0.22),
                );
            }
        }
        // Motes of violet thrown up out of the breaking ground.
        for _ in 0..22 {
            let a = TAU * self.scatter.unit();
            let out = Vec3::new(a.cos(), a.sin(), 0.8 + 1.4 * self.scatter.unit()).normalize();
            let roll = self.scatter.unit();
            self.push_lit(
                MOTE,
                heart,
                out * mote_speed(r * (0.5 + 0.7 * roll), 0.9),
                bang,
                0.7 + 0.5 * roll,
                (0.4 * s, 0.14 * s),
                VIOLET.lerp(LILAC, roll * 0.7) * 5.0,
                0.0,
            );
        }
        // Violet lightning run out over the breaking ground.
        for _ in 0..9 {
            let a = TAU * self.scatter.unit();
            let to = centre + Vec2::new(a.cos(), a.sin()) * r * (0.5 + 0.6 * self.scatter.unit());
            let to = to.extend(self.ground_height(to) + 0.4);
            let side = (to - heart).cross(Vec3::Z).normalize_or(Vec3::X);
            let when = bang + self.scatter.unit() * 0.2;
            self.emp_kinks(
                heart,
                to,
                r * 0.06,
                side,
                Vec3::Z * 0.2,
                when,
                0.25,
                0.9,
                8,
                fade_beam::RIFT_BOLT,
            );
        }
        let fx = &mut self.plasma_fx.shells;
        fx.light(Glow {
            pos: heart + Vec3::Z * r * 0.3,
            color: VIOLET.lerp(LILAC, 0.3) * 500.0 * s,
            range: r * 3.0,
            start: bang,
            life: 0.4,
        });
        // The cracks' violet light on what stands round them, dying with it.
        fx.light(Glow {
            pos: ground + Vec3::Z * r * 0.25,
            color: VIOLET * 220.0 * s,
            range: r * 2.2,
            start: bang,
            life: 3.0,
        });
    }

    /// The shells in flight this tick (from `regency_trails`, once a tick, after this
    /// tick's shots are written): each lays its filament trail down the stretch it flies
    /// this tick, each piece lit as the shell passes it, and sheds a few violet motes.
    pub(super) fn gravitic_shells_flying(&mut self, projectiles: &[ProjectileInstance], time: f32) {
        let tick = self.tick_seconds.max(0.02);
        self.plasma_fx.shells.flying.clear();
        let mut motes = 0;
        for p in projectiles {
            if drawn_look(p) != plasma_look::GRAVITIC_SHELL {
                continue;
            }
            let ends = ((p.color >> PROJECTILE_ENDS_SHIFT) & 0xFF) as f32 / 255.0;
            let span = if ends > 0.0 { ends } else { 1.0 };
            let starts = (p.color >> PROJECTILE_STARTS_SHIFT) as f32 / 255.0;
            let to = Vec3::from(p.pos);
            let from = Vec3::from(p.prev_pos).lerp(to, (starts / span).min(1.0));
            self.plasma_fx.shells.flying.push((from, to, p.size));
            // A deliberate cosmetic cap on one shell's stretch: a tick's flight is far
            // under this many pieces.
            let n = ((from.distance(to) / TRAIL.0).ceil() as usize).clamp(1, 16);
            for k in 0..n {
                let (f0, f1) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
                let (a, b) = (from.lerp(to, f0), from.lerp(to, f1));
                let when = time + tick * (starts + (span - starts).max(0.0) * f1);
                let fx = &mut self.plasma_fx.shells;
                // The thread where it passed, and the sheath round it that goes out sooner.
                for (life, width) in [(TRAIL.1, 0.3), (TRAIL.1 * 0.3, 0.9)] {
                    fx.trails.push(Piece {
                        from: a,
                        to: b,
                        start: when,
                        life,
                        width: p.size * width,
                    });
                }
                if motes < MAX_MOTES && self.scatter.unit() < 0.5 {
                    motes += 1;
                    let at = a.lerp(b, self.scatter.unit());
                    let drift = Vec3::new(
                        self.scatter.signed(),
                        self.scatter.signed(),
                        self.scatter.signed(),
                    );
                    let roll = self.scatter.unit();
                    self.push_lit(
                        MOTE,
                        at,
                        drift * 4.0,
                        when,
                        0.6 + 0.6 * roll,
                        (0.3 * p.size, 0.12 * p.size),
                        VIOLET.lerp(LILAC, roll * 0.6) * 4.0,
                        0.0,
                    );
                }
            }
        }
    }

    /// This tick's trail pieces among the fading beams (from `write_regency_trails`).
    pub(super) fn write_gravitic_shell_trails(&mut self, time: f32) {
        let fx = &mut self.plasma_fx.shells;
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
                aim: [0.0, 0.0, 0.0, fade_beam::PLASMA_TRAIL_GRAVITIC],
                prev_aim: [0.0; 4],
            })
            .collect();
        self.push_projectiles(&out);
    }

    /// Every frame: the view bent round each shell in flight (`alpha` of the way through
    /// the tick) and toward each strike's pull, for the tone map.
    pub(super) fn gravitic_shell_frame(&mut self, time: f32, alpha: f32) {
        let fx = &mut self.plasma_fx.shells;
        fx.pulls.retain(|p| time < p.start + p.life);
        fx.lenses.clear();
        for (i, &(from, to, size)) in fx.flying.iter().enumerate() {
            fx.lenses.push(GpuHeatPlume {
                port: from.lerp(to, alpha).to_array(),
                radius: size * 2.2,
                axis: [0.0, 0.0, 1.0],
                height: 0.0,
                strength: 0.55,
                seed: i as f32 * 0.37,
                kind: haze::KIND_LENS,
                _pad: 0.0,
            });
        }
        for (i, p) in fx.pulls.iter().enumerate() {
            let f = ((time - p.start) / p.life).clamp(0.0, 1.0);
            let k = if f < p.peak {
                f / p.peak
            } else {
                1.0 - (f - p.peak) / (1.0 - p.peak)
            };
            if k <= 0.0 {
                continue;
            }
            fx.lenses.push(GpuHeatPlume {
                port: p.at.to_array(),
                radius: p.radius,
                axis: [0.0, 0.0, 1.0],
                height: 0.0,
                strength: k * k,
                seed: 3.1 + i as f32 * 0.71,
                kind: haze::KIND_LENS,
                _pad: 0.0,
            });
        }
        fx.lenses.truncate(MOST_LENSES);
    }

    /// Every frame: the shells' light (from `upload_lights`).
    pub(super) fn gravitic_shell_lights(&mut self, time: f32) {
        self.plasma_fx
            .shells
            .glows
            .retain(|g| time < g.start + g.life);
        for g in &self.plasma_fx.shells.glows {
            let age = (time - g.start) / g.life.max(0.01);
            if age < 0.0 {
                continue;
            }
            let fade = (1.0 - age).clamp(0.0, 1.0);
            self.lights.lamp(
                g.pos,
                Vec3::NEG_Z,
                g.color * fade * fade,
                g.range,
                180.0,
                1.0,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gravitic_shells_look_is_the_one_the_mirror_writes() {
        assert_eq!(
            plasma_look::GRAVITIC_SHELL,
            mc_sim::mirror::PLASMA_LOOK_GRAVITIC_SHELL
        );
    }

    #[test]
    fn a_shatter_gun_is_drawn_as_a_gravitic_shell() {
        let bp = mc_data::Blueprints::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )
        .unwrap();
        let guns: Vec<&Weapon> = bp
            .units
            .iter()
            .flat_map(|u| u.weapons.iter())
            .filter(|w| w.shatter > 0.0)
            .collect();
        assert!(!guns.is_empty(), "a gravitic shock shell is in the data");
        for w in guns {
            assert!(shell_gun(w), "{}", w.name);
            assert_eq!(
                mc_sim::mirror::plasma_look(w),
                plasma_look::GRAVITIC_SHELL,
                "{}",
                w.name
            );
        }
    }
}
