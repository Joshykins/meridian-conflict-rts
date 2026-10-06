//! The Carillon's orbs (`Weapon::orbs`, `mc_sim::orbs`): plasma held floating round the
//! spire, one at each of the weapon's `muzzles` the sim says is held (`status[2]`, a bit a
//! slot). Drawn as light, never as mesh: each tick lays every held orb as a ball of red
//! plasma round a hot pink heart in a faint halo, bobbing a little and drifting in a slow
//! circle about its place, each out of step with the next.
//!
//! A batch the sim has just made (bits that came on since last tick) forms: the four
//! balls swell out of nothing over a second, a white flash where each opens and motes of
//! plasma drawn in from round it. An orb let go simply is not drawn any more: its shot
//! leaves the same place (`regency_guns_fx`, `regency_trails`).
//!
//! Presentation only; the renderer's own clock. A spire under construction, a wreck, a
//! ghost or a radar blip holds none.

use std::collections::HashMap;

use glam::Vec3;
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_WRECK, STATE_RADAR};

use super::regency_guns_fx::{GLOW, HOT, MOTE, RED, WHITE};
use super::Renderer;
use crate::camera::Camera;
use crate::gpu_consts::puff;

const ORB: f32 = puff::PLASMA_ORB as f32;
/// Metres across a full-grown orb's ball (the quad holds its corona too).
const ACROSS: f32 = 7.5;
/// Seconds a new orb takes to swell to full size.
const GROW: f32 = 1.1;
/// Seconds a mote takes to reach the orb it is drawn into, and the share of its birth
/// speed it covers in that time (warp_puffs.wgsl: drag 1.8).
const MOTE_LIFE: f32 = 0.5;
const MOTE_REACH: f32 = 0.329;
/// Orbs drawn at most in one tick. A deliberate cosmetic cap, so a wall of full spires
/// costs no more than this: the nearest to the camera are kept.
const MAX_DRAWN: usize = 640;

/// What each orb of a spire was doing last tick.
struct Held {
    mask: u32,
    born: [f32; 32],
    seen: u64,
}

/// Each blueprint's orb places (`None`: it holds none), and the orbs each spire held last tick.
#[derive(Default)]
pub(super) struct OrbFx {
    places: Vec<Option<Vec<[f32; 3]>>>,
    held: HashMap<u32, Held>,
    tick: u64,
}

impl Renderer {
    /// Once a sim tick: lays every spire's orbs.
    pub(super) fn orb_tick(&mut self, units: &[UnitInstance], time: f32, camera: &Camera) {
        if self.orb_fx.places.is_empty() {
            self.orb_fx.places = self
                .blueprints
                .units
                .iter()
                .map(|bp| {
                    bp.weapons.iter().find(|w| w.orbs > 0).map(|w| {
                        w.muzzles
                            .iter()
                            .map(|m| m.to_f32())
                            .collect::<Vec<[f32; 3]>>()
                    })
                })
                .collect();
        }
        if self.orb_fx.places.iter().all(Option::is_none) {
            return;
        }
        self.orb_fx.tick += 1;
        let tick = self.orb_fx.tick;
        let hidden = KIND_WRECK
            | KIND_GHOST
            | STATE_RADAR
            | (mc_sim::tables::flag::UNDER_CONSTRUCTION as u32) << 8;
        let reach = camera.distance * 2.5 + 600.0;
        let focus = camera.focus.truncate();
        let life = self.tick_seconds.clamp(0.03, 0.25) * 2.0;
        let mut drawn = 0usize;
        for u in units {
            if u.owner_flags & hidden != 0 {
                continue;
            }
            let Some(Some(places)) = self.orb_fx.places.get(u.blueprint as usize) else {
                continue;
            };
            let at = Vec3::from(u.pos);
            if at.truncate().distance(focus) > reach {
                continue;
            }
            let places = places.clone();
            let mask = u.status[2];
            // A spire seen for the first time (it has just come into view) shows the orbs
            // it already holds as formed: only a batch made while it is watched flashes.
            let entry = self.orb_fx.held.entry(u.unit_id).or_insert(Held {
                mask,
                born: [f32::MIN / 2.0; 32],
                seen: 0,
            });
            entry.seen = tick;
            let fresh = mask & !entry.mask;
            entry.mask = mask;
            for k in (0..places.len().min(32)).filter(|k| fresh >> k & 1 == 1) {
                entry.born[k] = time;
            }
            let born = entry.born;
            let (s, c) = u.heading.sin_cos();
            for (k, p) in places.iter().enumerate().take(32) {
                if mask >> k & 1 == 0 {
                    continue;
                }
                if drawn >= MAX_DRAWN {
                    break;
                }
                drawn += 1;
                let grow = smooth((time - born[k]) / GROW);
                let phase = k as f32 * 1.713 + u.unit_id as f32 * 0.37;
                // Bobbing, and drifting in a slow circle about its place.
                let bob = (time * 1.1 + phase).sin() * 0.7;
                let drift = Vec3::new(
                    (time * 0.6 + phase).cos() * 0.5,
                    (time * 0.6 + phase).sin() * 0.5,
                    bob,
                );
                let centre = at + Vec3::new(p[0] * c - p[1] * s, p[0] * s + p[1] * c, p[2]) + drift;
                let size = ACROSS * grow;
                let breath = 1.0 + 0.12 * (time * 3.1 + phase * 2.0).sin();
                self.push_lit(
                    GLOW,
                    centre,
                    Vec3::ZERO,
                    time,
                    life,
                    (size * 1.9, size * 1.9),
                    RED * (0.12 * grow),
                    0.0,
                );
                self.push_lit(
                    ORB,
                    centre,
                    Vec3::ZERO,
                    time,
                    life,
                    (size * breath, size * breath * 1.04),
                    RED * (1.4 * breath),
                    0.0,
                );
                self.push_lit(
                    ORB,
                    centre,
                    Vec3::ZERO,
                    time,
                    life,
                    (size * 0.42, size * 0.42),
                    HOT * 0.9,
                    0.0,
                );
                if self.scatter.unit() < 0.12 * grow {
                    self.orb_mote(centre, size, time);
                }
                if fresh >> k & 1 == 1 {
                    self.orb_birth(centre, time);
                }
            }
        }
        self.orb_fx.held.retain(|_, h| h.seen == tick);
    }

    /// A mote of plasma drawn in to an orb from round it.
    fn orb_mote(&mut self, centre: Vec3, size: f32, time: f32) {
        let out = Vec3::new(
            self.scatter.signed(),
            self.scatter.signed(),
            self.scatter.signed(),
        )
        .normalize_or(Vec3::Z);
        let from = centre + out * size * (0.9 + self.scatter.unit() * 0.8);
        let rgb = RED.lerp(HOT, self.scatter.unit() * 0.6) * 4.0;
        let dot = size * 0.07;
        self.push_lit(
            MOTE,
            from,
            (centre - from) / MOTE_REACH,
            time,
            MOTE_LIFE,
            (dot, dot * 0.5),
            rgb,
            0.0,
        );
    }

    /// A new orb opening: a white flash and plasma drawn in from all round.
    fn orb_birth(&mut self, centre: Vec3, time: f32) {
        self.push_lit(
            GLOW,
            centre,
            Vec3::ZERO,
            time,
            0.6,
            (ACROSS * 1.6, ACROSS * 1.6),
            WHITE * 3.0,
            0.0,
        );
        for _ in 0..10 {
            let late = self.scatter.unit() * GROW * 0.7;
            self.orb_mote(centre, ACROSS * 2.0, time + late);
        }
    }
}

fn smooth(x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
