//! An Edict Lance on its ship (`Weapon::edict_beam`, the Scourge's): the Regent's Edict
//! brought down to a held beam, laid from the core in the hull (`Model::beam_core`, its
//! lower lens). What drives it is how far the beam's projector has run up
//! (`UnitInstance::recoil`, last tick's in `prev_recoil`: 0 cold to 1 lit;
//! `mirror::beam_brace`: one only on a tick the beam fired), so it keeps time with the
//! sim's charge, with the charge sound (`regency_edict_lance_charge`) and with the release
//! heard as the beam lights (`regency_edict_lance_release`). Pinch fusion in the Regency's
//! red: red-hot, crimson round it; hard-edged, no mist.
//!
//! - **The gather** (regency.wgsl `reg_edict`, fed each ship's charge through
//!   `UnitInstance::mount` as `gpu_consts::edict` lays out): light runs in along the plate
//!   seams from the bow and the stern, two fronts closing on the core, red left burning in
//!   the seams behind them and pulses running on toward it. While the beam burns the charge
//!   is held there, a white-hot pool round the core; let go before it lit it gutters out;
//!   when the beam goes out the pool cools to red and the seams' last red dies.
//! - **The star** at the lens: from a spark to a blinding red-hot point as it charges, a
//!   crimson corona round it, lightning crawling in to it off the hull and red
//!   streamers licking off it; drawn in hard at the end, held beating while the beam
//!   burns, collapsing when it goes out.
//! - **Ignition** (the tick the beam first fires): a blinding flash at the lens, its light over the
//!   country, a lens flare, a pressure ring and a hard ring of pinched plasma thrown out
//!   round the lens (`plasma_fx.rs` draws the column itself and the filament it leaves).
//!
//! Presentation only; the renderer's own clock.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_WRECK, STATE_RADAR};

use super::lens_flare::Flare;
use super::star_core_fx::{ARC, STAR};
use super::stun_fx::Flash;
use super::Renderer;
use crate::camera::Camera;
use crate::gpu_consts::{edict, puff};

/// A streamer licking off the corona, and a knot of the ignition's ring.
const LICK: f32 = puff::WARP_STREAK as f32;
const KNOT: f32 = puff::PINCH_KNOT as f32;
/// The star's light: the Regency's red; and its lens flare at a glare of one, red going to
/// rose. Nothing of it white: it is all the ground under the ship sees.
const LIGHT: Vec3 = Vec3::new(1.0, 0.12, 0.07);
const FLARE: Vec3 = Vec3::new(1.0, 0.3, 0.26);
/// The star's face, against its glare: red-hot.
const FACE: Vec3 = Vec3::new(1.0, 0.36, 0.3);
/// The ignition's flash and its light thrown over the country.
const IGNITION: Vec3 = Vec3::new(1.0, 0.22, 0.14);
/// The star's face against the lens's radius as a spark and at its fullest.
const SPARK: f32 = 0.15;
const FULL: f32 = 1.8;
/// The share of the charge from which it draws in, and how far (its face's share left).
const DRAW_IN: f32 = 0.93;
const DRAWN: f32 = 0.45;
/// Its brightness (`appearance.rgb`) as a spark and at its fullest.
const GLARE_SPARK: f32 = 2.0;
const GLARE_FULL: f32 = 26.0;
/// Its light at its fullest, per metre of lens, and how far it reaches in lens radii: down
/// to the ground under a ship at its height, a red glow there and not a floodlight.
const POWER: f32 = 2.5e4;
const LIGHT_REACH: f32 = 70.0;
/// The ignition's ring: its knots, how long it runs out, how far it gets and how thick it
/// is, in lens radii.
const RING_KNOTS: usize = 48;
const RING_RUN: f32 = 0.4;
const RING_REACH: f32 = 22.0;
const RING_THICK: f32 = 0.18;

/// The star's face (in lens radii) and its brightness at `share` of the charge.
fn size(share: f32) -> (f32, f32) {
    let p = share.clamp(0.0, 1.0);
    let grown = SPARK + (FULL - SPARK) * p.powf(1.5);
    let drawn = ((p - DRAW_IN) / (1.0 - DRAW_IN)).clamp(0.0, 1.0);
    let r = grown * (1.0 - (1.0 - DRAWN) * drawn * drawn);
    let glare = GLARE_SPARK + (GLARE_FULL - GLARE_SPARK) * p.powi(3) * (1.0 + 0.8 * drawn);
    (r, glare)
}

/// Where a ship's lance stands this tick.
#[derive(Clone, Copy, PartialEq)]
enum Stage {
    /// Charging, or held lit at the full.
    Charging,
    Burning,
    /// Gone out after burning, at this time.
    Out(f32),
    /// Let go before it lit, at this time and this share.
    LetGo(f32, f32),
}

/// One ship's lance as the renderer follows it.
#[derive(Clone, Copy)]
struct Ship {
    id: u32,
    stage: Stage,
    level: f32,
    /// Its core in the world this tick: the lens, the lens's radius, down the hull.
    lens: Vec3,
    r: f32,
    fwd: Vec3,
    /// The core's place along model x, and the charge's rate a second.
    along: f32,
    rate: f32,
    seed: f32,
}

impl Ship {
    /// Its record for the shader (`gpu_consts::edict`), or none once its light is out.
    fn record(&self, time: f32) -> Option<[f32; 4]> {
        match self.stage {
            Stage::Charging => Some([
                self.level,
                time,
                self.along,
                edict::GATHERING - self.rate.min(0.99),
            ]),
            Stage::Burning => Some([1.0, time, self.along, edict::GATHERING]),
            Stage::Out(when) => (time < when + edict::AFTERGLOW + 1.0).then_some([
                when,
                0.0,
                self.along,
                edict::FIRED,
            ]),
            Stage::LetGo(when, share) => {
                (time < when + edict::FADE).then_some([share, when, self.along, edict::CANCELLED])
            }
        }
    }

    /// Whether anything of it is still to be drawn.
    fn alive(&self, time: f32) -> bool {
        self.record(time).is_some()
    }
}

/// An ignition's ring running out round the lens.
#[derive(Clone, Copy)]
struct Ring {
    at: Vec3,
    r: f32,
    start: f32,
}

/// Each blueprint's core (`None`: it has none), and the lances lit or cooling.
#[derive(Default)]
pub(super) struct EdictLances {
    cores: Vec<Option<[f32; 4]>>,
    ships: Vec<Ship>,
    rings: Vec<Ring>,
    /// Light renewed each tick: where, colour times brightness, reach.
    lamps: Vec<(Vec3, Vec3, f32)>,
}

impl EdictLances {
    /// `cores`: per blueprint, in id order.
    pub(super) fn new(cores: Vec<Option<[f32; 4]>>) -> Self {
        Self {
            cores,
            ..Self::default()
        }
    }

    /// Forgets everything lit, keeping the blueprints' cores.
    pub(super) fn clear(&mut self) {
        self.ships.clear();
        self.rings.clear();
        self.lamps.clear();
    }

    /// Before the instances go to the GPU: hands each ship's hull its charge in `mount`
    /// (`gpu_consts::edict`). A ship with an Edict Lance carries its guns on houses of
    /// their own, so its `mount` is free for this.
    pub(super) fn patch(&self, units: &mut [UnitInstance], time: f32) {
        if self.ships.is_empty() {
            return;
        }
        for u in units.iter_mut() {
            let Some(s) = self.ships.iter().find(|s| s.id == u.unit_id) else {
                continue;
            };
            if let Some(record) = s.record(time) {
                u.mount = record;
            }
        }
    }
}

impl Renderer {
    /// Once a sim tick: every lance charging, burning, gone out or let go.
    pub(super) fn edict_lance_tick(&mut self, units: &[UnitInstance], time: f32, camera: &Camera) {
        let fx = &mut self.plasma_fx.cores;
        fx.lamps.clear();
        fx.ships.retain(|s| s.alive(time));
        fx.rings.retain(|r| time < r.start + RING_RUN);
        if fx.cores.iter().all(Option::is_none) {
            return;
        }
        let hidden = KIND_WRECK | KIND_GHOST | STATE_RADAR;
        let rows = self
            .unit_kinds
            .rows_where(|b| matches!(self.plasma_fx.cores.cores.get(b as usize), Some(Some(_))));
        for u in rows.iter().map(|&i| &units[i]) {
            if u.owner_flags & hidden != 0 {
                continue;
            }
            let Some(Some(core)) = self.plasma_fx.cores.cores.get(u.blueprint as usize) else {
                continue;
            };
            let core = *core;
            let (level, before) = (u.recoil.clamp(0.0, 1.0), u.prev_recoil.clamp(0.0, 1.0));
            let known = self
                .plasma_fx
                .cores
                .ships
                .iter()
                .position(|s| s.id == u.unit_id);
            if known.is_none() && level.max(before) < 0.01 {
                continue;
            }
            let spin = self
                .blueprints
                .units
                .get(u.blueprint as usize)
                .and_then(|bp| bp.weapons.iter().find(|w| w.edict_beam))
                .map_or(1, |w| w.spin_ticks.max(1));
            let rate = mc_core::TICKS_PER_SECOND as f32 / spin as f32;
            let (s, c) = u.heading.sin_cos();
            let fwd = Vec3::new(c, s, 0.0);
            let lens = Vec3::from(u.pos)
                + Vec3::new(
                    core[0] * c - core[1] * s,
                    core[0] * s + core[1] * c,
                    core[2],
                );
            let was = known.map(|k| self.plasma_fx.cores.ships[k]);
            let stage = match was.map(|s| s.stage) {
                _ if level >= 1.0 => Stage::Burning,
                Some(Stage::Burning) if level < before => Stage::Out(time),
                Some(Stage::Charging) if level < before => Stage::LetGo(time, before),
                Some(stage @ (Stage::Out(_) | Stage::LetGo(..))) if level <= before => stage,
                _ => Stage::Charging,
            };
            let ship = Ship {
                id: u.unit_id,
                stage,
                level,
                lens,
                r: core[3],
                fwd,
                along: core[0],
                rate,
                seed: (u.unit_id % 97) as f32 * 0.613,
            };
            let fx = &mut self.plasma_fx.cores;
            match known {
                Some(k) => fx.ships[k] = ship,
                None => fx.ships.push(ship),
            }
            // The charge reads one only on a tick the beam fired (`mirror::beam_brace`), so
            // it lights with the column even when the hull was still swinging onto its mark.
            if stage == Stage::Burning && was.is_none_or(|s| s.stage != Stage::Burning) {
                self.edict_lance_ignites(&ship, time);
            }
        }
        let focus = camera.focus.truncate();
        let reach = camera.distance * 2.5 + 2000.0;
        let ships = self.plasma_fx.cores.ships.clone();
        for s in &ships {
            let near = s.lens.truncate().distance(focus) < reach && camera.distance < 30_000.0;
            self.edict_lance_star(s, time, near);
        }
        let rings = self.plasma_fx.cores.rings.clone();
        for r in &rings {
            self.edict_lance_ring(r, time);
        }
    }

    /// One ship's star this tick.
    fn edict_lance_star(&mut self, s: &Ship, time: f32, near: bool) {
        let (share, beat) = match s.stage {
            Stage::Charging => (s.level, 1.0),
            Stage::Burning => (
                1.0,
                1.0 + 0.12 * (time * 9.0 + s.seed).sin() + 0.06 * (time * 23.0 + s.seed).sin(),
            ),
            // Gone out or let go: it collapses, its light dying within `FADE`.
            Stage::Out(when) | Stage::LetGo(when, _) => {
                let from = if let Stage::LetGo(_, was) = s.stage {
                    was
                } else {
                    1.0
                };
                let k = 1.0 - (time - when) / edict::FADE;
                if k <= 0.0 {
                    return;
                }
                (from * k * k, 1.0)
            }
        };
        let (face, glare) = size(share);
        let r = face * s.r * beat;
        // Its face hangs clear under the lens: its light is drawn at its face's front, so a
        // star tucked into the hull would show through it from above.
        let at = s.lens - Vec3::Z * (r * 0.9 + s.r * 0.2);
        let life = self.tick_seconds.clamp(0.03, 0.25) * 2.0;
        // The quad holds the corona too: the face is the middle 0.42 of it.
        let across = r / 0.42;
        let red = 3.0 + 34.0 * share * share;
        self.push_lit(
            STAR,
            at,
            Vec3::ZERO,
            time,
            life,
            (across * 2.2, across * 2.2),
            Vec3::new(red, 0.0, 0.0),
            s.seed + 3.1,
        );
        self.push_lit(
            STAR,
            at,
            Vec3::ZERO,
            time,
            life,
            (across, across),
            FACE * glare * beat,
            s.seed,
        );
        let power = POWER * s.r * s.r * share.powi(3) * beat;
        let reach = s.r * (8.0 + LIGHT_REACH * share.sqrt());
        self.plasma_fx.cores.lamps.push((at, LIGHT * power, reach));
        if share > 0.2 {
            self.lens_flares.flash(
                at,
                Flare {
                    color: FLARE * (0.5 + 20.0 * share.powi(3)),
                    size: s.r * (4.0 + 50.0 * share * share),
                },
                time,
                life,
            );
        }
        if !near || matches!(s.stage, Stage::Out(_) | Stage::LetGo(..)) {
            return;
        }
        self.edict_lance_arcs(s, at, r, share, time);
        self.edict_lance_licks(at, r, share, time);
    }

    /// Lightning crawling in to the star off the hull round the core, more and longer as it
    /// fills, white-pink at the heart of each stroke.
    fn edict_lance_arcs(&mut self, s: &Ship, at: Vec3, r: f32, share: f32, time: f32) {
        if share < 0.2 {
            return;
        }
        let side = Vec3::new(-s.fwd.y, s.fwd.x, 0.0);
        let strokes = (share * 4.0) as usize + usize::from(self.scatter.unit() < share);
        for _ in 0..strokes {
            // A point on the hull's underside near the lens, the reach of the crawl
            // shrinking in as the star fills.
            let spread = s.r * (4.0 + 16.0 * (1.0 - share));
            let off = s.fwd * self.scatter.signed() * spread
                + side * self.scatter.signed() * s.r * (1.5 + 4.0 * (1.0 - share));
            let from = s.lens + off + Vec3::Z * s.r * (0.3 + 0.8 * self.scatter.unit());
            let to = at + (from - at).normalize_or(Vec3::Z) * r * 0.7;
            let tint = Vec3::new(1.0, 0.55 + 0.4 * share, 0.62 + 0.3 * share);
            let lasts = 0.06 + 0.08 * self.scatter.unit();
            let thick = s.r * (0.12 + 0.3 * share);
            let when = time + self.scatter.unit() * self.tick_seconds;
            self.push_lit(
                ARC,
                from,
                to - from,
                when,
                lasts,
                (thick, thick),
                tint * (3.0 + 5.0 * share),
                0.0,
            );
        }
    }

    /// Crimson streamers licking off the star's corona, more and longer as it fills: the
    /// Regency's red round the red-hot face.
    fn edict_lance_licks(&mut self, at: Vec3, r: f32, share: f32, time: f32) {
        if share < 0.3 {
            return;
        }
        for _ in 0..(share * 3.0) as usize {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed() * 0.6,
            )
            .normalize_or(Vec3::X);
            let reach = r * (0.8 + 1.2 * self.scatter.unit()) * (0.5 + share);
            let (when, lasts) = (self.scatter.unit(), self.scatter.unit());
            self.push_warp(
                LICK,
                at + out * r * 0.8,
                out * reach,
                time + when * self.tick_seconds,
                0.25 + 0.2 * lasts,
                (r * 0.3, r * 0.1),
                Vec3::new(1.0, 0.06, 0.05) * (3.0 + 4.0 * share),
                0.4,
            );
        }
    }

    /// The tick the lance lights (its first shot, not the tick its charge fills: the hull
    /// may still be swinging onto its mark): a blinding flash at the lens, its light thrown over the
    /// country, a lens flare, a pressure ring and the hard ring of pinched plasma.
    fn edict_lance_ignites(&mut self, s: &Ship, time: f32) {
        let at = s.lens - Vec3::Z * s.r;
        self.push_lit(
            STAR,
            at,
            Vec3::ZERO,
            time,
            0.3,
            (s.r * 9.0, s.r * 9.0),
            IGNITION * 40.0,
            1.7,
        );
        self.emp_fx.flashes.push(Flash {
            pos: at - Vec3::Z * s.r * 2.0,
            start: time,
            life: 0.3,
            color: IGNITION * 1.2e6 * s.r,
            range: s.r * 100.0,
            flicker: 0.0,
        });
        self.lens_flares.flash(
            at,
            Flare {
                color: FLARE * 30.0,
                size: s.r * 40.0,
            },
            time,
            0.6,
        );
        self.push_shockwave(at.to_array(), time, s.r * 25.0, 0.8, 1.0, 1.0, Vec3::NEG_Z);
        self.plasma_fx.cores.rings.push(Ring {
            at,
            r: s.r,
            start: time,
        });
    }

    /// The hard ring thrown out round the lens as it lights: a band of pinched plasma knots
    /// edge-on round the beam's line (down), scarlet going to crimson as it runs out and
    /// thins.
    fn edict_lance_ring(&mut self, ring: &Ring, time: f32) {
        let age = time - ring.start;
        let k = age / RING_RUN;
        if !(0.0..1.0).contains(&k) {
            return;
        }
        let reach = ring.r * (1.0 + RING_REACH * (1.0 - (-age * 3.2).exp()));
        let half = reach * TAU / RING_KNOTS as f32 * 0.75;
        let glow = 2.2 * (1.0 - k).powf(2.5);
        let rgb = Vec3::new(1.0, 0.3, 0.2).lerp(Vec3::new(0.8, 0.03, 0.04), k) * glow;
        let life = self.tick_seconds.clamp(0.03, 0.25) * 2.0;
        for i in 0..RING_KNOTS {
            // Torn as it runs out: knots drop out of it, more the further it has gone.
            if self.scatter.unit() < 0.15 + 0.5 * k {
                continue;
            }
            let out = Vec2::from_angle(i as f32 * TAU / RING_KNOTS as f32).extend(0.0);
            self.push_lit(
                KNOT,
                ring.at + out * reach,
                out,
                time,
                life,
                (half, ring.r * RING_THICK * (1.0 + 2.0 * k) / half),
                rgb,
                0.0,
            );
        }
    }

    /// Every frame: the stars light the hull and the country under them.
    pub(super) fn edict_lance_lights(&mut self) {
        for &(pos, color, range) in &self.plasma_fx.cores.lamps {
            self.lights.lamp(pos, Vec3::NEG_Z, color, range, 180.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_star_grows_then_draws_in_at_the_end() {
        let (spark, dim) = size(0.0);
        let (full, _) = size(DRAW_IN);
        let (drawn, blaze) = size(1.0);
        assert!(spark < 0.3 && dim < 3.0);
        assert!(full > 1.5);
        assert!(drawn < full * 0.6, "drawn in to {drawn} from {full}");
        assert!(blaze > GLARE_FULL);
    }
}
