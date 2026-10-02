//! Work beams (reclaim, repair, salvage relays, and the survival and nanite kinds that
//! share their buffer): the sim lists who is at work each tick; this keeps each beam's
//! own clock (when it came on, when it went off) and where its emitter was a tick ago,
//! so the shader can move both ends with the units they hang between.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use mc_sim::mirror::{RenderFrame, KIND_GHOST, KIND_PROP, KIND_WRECK};
use std::collections::HashMap;

/// Beams drawn at once, each `gpu_consts::beam::QUADS` quads.
pub(super) const MAX_BEAMS: usize = 1024;
/// A beam that has shut off is kept this long, so what was already on its way up it arrives.
const LINGER: f32 = 3.0;
/// The same for a Regency site's splashes: one born just before the work stopped rises
/// and thins out for up to `SPLASH_PERIOD_MAX` (beams.wgsl) more.
const NANITE_SITE_LINGER: f32 = 8.0;
/// A beam whose far end jumps farther than this (plus the target's size) between ticks is on something new.
const JUMP: f32 = 6.0;
/// How long a beam found already on is taken to have been on: long enough for its
/// stream to have filled it end to end (beams.wgsl: a bit's trip is at most ~4 s).
const ALREADY_ON: f32 = 5.0;
/// The beam kinds drawn as a work beam between two moving ends (beams.wgsl `vs_beam`);
/// the others are drawn from where they stand.
const MOVING_KINDS: [u32; 4] = [
    mc_sim::reclaim::BEAM_RECLAIM,
    mc_sim::repair::BEAM_REPAIR,
    mc_sim::reclaim::BEAM_RELAY,
    mc_sim::reclaim::BEAM_NANITE_RECLAIM,
];

/// Mirrors `Beam` in shaders/beams.wgsl: the sim's record, where its emitter was a tick
/// ago, and when the beam came on and went off.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct GpuBeam {
    beam: mc_sim::reclaim::BeamInstance,
    /// The emitter a tick ago: `beam.from` moved back with its unit, so the shader
    /// takes it from here to there over the tick as the unit is drawn.
    from_prev: [f32; 3],
    /// Fixed for the beam's life, so what is drawn along it does not jump as it moves.
    seed: f32,
    start: f32,
    /// Negative while the beam is on.
    end: f32,
    /// Its length when it came on, which sets how long a bit takes up it. Fixed for the
    /// beam's life: the shader's bit phase is `time / trip`, so a trip that followed the
    /// length as its unit moved would send every bit racing up and down the beam.
    trip_len: f32,
    pad: f32,
}

/// A beam that has shut off, emptying out.
struct Ended {
    beam: GpuBeam,
    /// The unit a moving kind's emitter rides on, and where on it the emitter sits (its
    /// offset from the unit, turned back to heading zero), so the stream draining home
    /// stays on the unit as it moves off. None for a beam drawn where it stands.
    rides: Option<(u32, Vec3)>,
}

/// The beams on and the beams emptying out.
#[derive(Default)]
pub(super) struct WorkBeams {
    /// On, by the unit they come from and which of its beams (one per head).
    live: HashMap<(u32, u32), GpuBeam>,
    /// Shut off, with what was already on its way still arriving.
    ended: Vec<Ended>,
    /// How many were written last.
    pub(super) count: u32,
    /// The sim tick last seen.
    tick: Option<u32>,
}

impl WorkBeams {
    /// A new tick's beams, as the GPU draws them.
    pub(super) fn tick(&mut self, frame: &RenderFrame, time: f32) -> Vec<GpuBeam> {
        // Not the tick after the last one (the first frame, a seek, a new headless shot):
        // what is on now was on before we looked, so it is drawn at full strength with
        // its stream already running, not switched on in front of the camera.
        let joined = self.tick.is_none_or(|t| frame.tick != t.wrapping_add(1));
        self.tick = Some(frame.tick);
        if joined {
            self.live.clear();
            self.ended.clear();
        }
        let on_since = if joined { time - ALREADY_ON } else { time };
        let mut was = std::mem::take(&mut self.live);
        // Each source unit's pose this tick and last, to move its emitters back a tick.
        // Only live units: wrecks, props and ghosts share their ids' numbers with them.
        let not_units = KIND_WRECK | KIND_PROP | KIND_GHOST;
        let poses: HashMap<u32, Pose> = frame
            .units
            .iter()
            .filter(|u| u.owner_flags & not_units == 0)
            .map(|u| {
                (
                    u.unit_id,
                    Pose {
                        prev: Vec3::from(u.prev_pos),
                        now: Vec3::from(u.pos),
                        prev_heading: u.prev_heading,
                        heading: u.heading,
                    },
                )
            })
            .collect();
        let shut_off = |(source, _): (u32, u32), mut old: GpuBeam, ended: &mut Vec<Ended>| {
            (old.end, old.beam.to_prev, old.from_prev) = (time, old.beam.to, old.beam.from);
            let rides = poses
                .get(&source)
                .filter(|_| rides_its_unit(old.beam.kind, source))
                .map(|pose| (source, pose.local_then(Vec3::from(old.beam.from))));
            ended.push(Ended { beam: old, rides });
        };
        // A reclaimer with several heads lists a beam per head under the same source, in
        // the same order every tick: the n-th of them carries on the n-th of last tick's.
        let mut heads: HashMap<u32, u32> = HashMap::new();
        for (&source, beam) in frame.beam_sources.iter().zip(&frame.beams) {
            let head = heads.entry(source).or_insert(0);
            let key = (source, *head);
            *head += 1;
            let mut start = on_since;
            let mut from_prev = beam.from;
            let mut trip_len = length_of(beam);
            if let Some(old) = was.remove(&key) {
                let jump = Vec3::from(old.beam.to).distance(Vec3::from(beam.to_prev));
                if jump <= JUMP + beam.radius {
                    start = old.start;
                    from_prev = old.beam.from;
                    trip_len = old.trip_len;
                } else {
                    shut_off(key, old, &mut self.ended);
                }
            }
            // A relay's emitter is its carrier's belly, not on the drone that is its source:
            // it keeps last tick's emitter. Every other emitter rides its own unit.
            if rides_its_unit(beam.kind, source) {
                if let Some(pose) = poses.get(&source) {
                    from_prev = pose.back(Vec3::from(beam.from)).to_array();
                }
            }
            self.live.insert(
                key,
                GpuBeam {
                    beam: *beam,
                    from_prev,
                    seed: seed_of(key),
                    start,
                    end: -1.0,
                    trip_len,
                    pad: 0.0,
                },
            );
        }
        for (key, old) in was {
            shut_off(key, old, &mut self.ended);
        }
        self.ended.retain(|e| {
            let linger = if e.beam.beam.kind == mc_sim::reclaim::BEAM_NANITE_SITE {
                NANITE_SITE_LINGER
            } else {
                LINGER
            };
            time - e.beam.end < linger
        });
        // What is still draining home keeps to its emitter while the unit lives.
        for e in &mut self.ended {
            let Some((pose, local)) = e
                .rides
                .and_then(|(source, local)| Some((poses.get(&source)?, local)))
            else {
                continue;
            };
            e.beam.beam.from = pose.world(local).to_array();
            e.beam.from_prev = pose.back(Vec3::from(e.beam.beam.from)).to_array();
        }
        // Deliberate cap: the GPU buffer holds `MAX_BEAMS`; past it, the rest go undrawn
        // this tick (they are cosmetic, and a thousand beams already fill any view).
        let all: Vec<GpuBeam> = self
            .live
            .values()
            .chain(self.ended.iter().map(|e| &e.beam))
            .copied()
            .take(MAX_BEAMS)
            .collect();
        self.count = all.len() as u32;
        all
    }
}

/// A unit's place and heading at the start and the end of the tick.
struct Pose {
    prev: Vec3,
    now: Vec3,
    prev_heading: f32,
    heading: f32,
}

impl Pose {
    /// Where a point the unit carried a tick ago sits on it: its offset from where the unit
    /// was then, turned back to heading zero.
    fn local_then(&self, at: Vec3) -> Vec3 {
        let off = at - self.prev;
        let (s, c) = (-self.prev_heading).sin_cos();
        Vec3::new(off.x * c - off.y * s, off.x * s + off.y * c, off.z)
    }

    /// Where a point on the unit (as `local_then` gives it) is now.
    fn world(&self, local: Vec3) -> Vec3 {
        let (s, c) = self.heading.sin_cos();
        self.now
            + Vec3::new(
                local.x * c - local.y * s,
                local.x * s + local.y * c,
                local.z,
            )
    }

    /// Where a point carried on the unit was a tick ago: turned back and moved back with it.
    fn back(&self, at: Vec3) -> Vec3 {
        let turn = self.prev_heading - self.heading;
        let local = at - self.now;
        let (s, c) = turn.sin_cos();
        let turned = Vec3::new(
            local.x * c - local.y * s,
            local.x * s + local.y * c,
            local.z,
        );
        self.prev + turned
    }
}

/// Whether a beam's emitter rides on its source unit. A relay's emitter is its carrier's
/// belly, not on the drone that is its source; the kinds drawn where they stand ride on nothing.
fn rides_its_unit(kind: u32, source: u32) -> bool {
    MOVING_KINDS.contains(&kind) && source & (1 << 31) == 0
}

/// Emitter to where the beam grips its target (beams.wgsl `vs_beam`: `grip`).
fn length_of(beam: &mc_sim::reclaim::BeamInstance) -> f32 {
    let grip = Vec3::from(beam.to) + Vec3::Z * beam.height * 0.55;
    Vec3::from(beam.from).distance(grip)
}

/// A number in 0..1000 for this beam, the same every tick it is on.
fn seed_of((source, head): (u32, u32)) -> f32 {
    let mut h = source.wrapping_mul(0x9E37_79B1) ^ head.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h % 1_000_000) as f32 / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_emitter_rides_back_with_its_unit() {
        // Moved 10 m east and turned a quarter left over the tick; the emitter sits 2 m
        // ahead of the unit now (north), so a tick ago it was 2 m ahead of it then (east).
        let pose = Pose {
            prev: Vec3::new(0.0, 0.0, 5.0),
            now: Vec3::new(10.0, 0.0, 5.0),
            prev_heading: 0.0,
            heading: std::f32::consts::FRAC_PI_2,
        };
        let back = pose.back(Vec3::new(10.0, 2.0, 6.0));
        assert!(back.distance(Vec3::new(2.0, 0.0, 6.0)) < 1e-4, "{back}");
    }

    #[test]
    fn a_point_on_a_unit_moves_with_it() {
        // The point was 2 m ahead (east) of the unit a tick ago, heading 0.
        let then = Pose {
            prev: Vec3::new(4.0, -3.0, 1.0),
            now: Vec3::new(9.0, -3.0, 1.0),
            prev_heading: 0.0,
            heading: 0.0,
        };
        let at = Vec3::new(6.0, -3.0, 3.0);
        let local = then.local_then(at);
        assert!(then.world(local).distance(Vec3::new(11.0, -3.0, 3.0)) < 1e-4);
        // Later, turned a quarter left: the point is 2 m north of the unit.
        let later = Pose {
            prev: then.now,
            now: Vec3::new(14.0, -3.0, 1.0),
            prev_heading: 0.0,
            heading: std::f32::consts::FRAC_PI_2,
        };
        let moved = later.world(local);
        assert!(moved.distance(Vec3::new(14.0, -1.0, 3.0)) < 1e-4, "{moved}");
    }

    #[test]
    fn a_beam_keeps_its_seed_and_heads_differ() {
        assert_eq!(seed_of((7, 0)), seed_of((7, 0)));
        assert_ne!(seed_of((7, 0)), seed_of((7, 1)));
        assert_ne!(seed_of((7, 0)), seed_of((8, 0)));
    }
}
