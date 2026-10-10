//! The ground a gravitic engine tears loose, as solid rock (mc-models `rift_slabs`): slabs
//! heaved and shaken round its foot as it gathers, slabs thrown up either side of its rift
//! as the front passes and lying where they land, the well's slabs lifted off the ground,
//! hanging and grinding slowly, and shards of rock hanging over it or circling it. Let go,
//! everything held up falls back. Drawn as props, posed each frame (the vertex shader
//! stands a prop on the ground, lifts it by `-arm_pitch.z`, pitches it along its heading
//! by `arm_pitch.x` and rolls it by `_pad2`).
//!
//! Presentation only; the renderer's clock.

use glam::{Vec2, Vec3};
use mc_models::rift_slabs::{SHARD_KEYS, SLAB_KEYS};
use mc_sim::mirror::{UnitInstance, KIND_PROP};

/// Pieces held at most, all engines together (`MAX_DYNAMIC` has room for them). A
/// deliberate cosmetic cap: past it the oldest fallen ones go first.
pub(crate) const MOST: usize = 1400;
/// Things thrown fall at this, m/s² (heavy rock; a touch over real gravity, as the
/// other thrown things in the game).
const GRAVITY: f32 = 13.0;
/// Seconds a fallen piece lies before it sinks away, and how long sinking takes.
const LIE: f32 = 50.0;
const SINK: f32 = 12.0;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Motion {
    /// Heaved up where it lay, tipped, shaken while its engine gathers.
    Heave,
    /// Thrown up off the ground at `vel` (m/s), falling back to lie where it lands.
    Thrown { vel: Vec3 },
    /// Lifted and hanging, grinding slowly round: the well's slabs.
    Hover,
    /// Circling `centre` at its distance, `rate` rad/s: shards held in the field.
    Orbit { centre: Vec2, rate: f32 },
}

#[derive(Clone, Copy)]
pub(super) struct Piece {
    pub(super) engine: u32,
    /// A slab (`SLAB_KEYS` index) or, past them, a shard.
    pub(super) model: u32,
    pub(super) at: Vec2,
    /// The ground there, roughly (the vertex shader stands it on the true ground; the
    /// cull wants it near).
    pub(super) ground: f32,
    pub(super) heading: f32,
    pub(super) scale: f32,
    pub(super) born: f32,
    /// Seconds it takes to come up to its pose.
    pub(super) rise: f32,
    /// Its pose once up: metres off the ground, pitch and roll (radians).
    pub(super) lift: f32,
    pub(super) pitch: f32,
    pub(super) roll: f32,
    pub(super) motion: Motion,
    pub(super) seed: f32,
}

#[derive(Clone, Copy)]
struct Held {
    piece: Piece,
    /// Let go at, and how it stood then: where, how high, pitch, roll, heading.
    fell: Option<(f32, Vec2, f32, f32, f32, f32)>,
}

/// Where a piece stands at a moment: ground point, lift, heading, pitch, roll.
#[derive(Clone, Copy)]
struct Pose {
    at: Vec2,
    lift: f32,
    heading: f32,
    pitch: f32,
    roll: f32,
}

#[derive(Default)]
pub(super) struct Slabs {
    held: Vec<Held>,
    /// Each engine's shake now (0 still, 1 violent), by unit id.
    shake: Vec<(u32, f32)>,
}

fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

impl Slabs {
    pub(super) fn clear(&mut self) {
        self.held.clear();
        self.shake.clear();
    }

    pub(super) fn add(&mut self, piece: Piece) {
        if self.held.len() >= MOST {
            let at = self.held.iter().position(|h| h.fell.is_some()).unwrap_or(0);
            self.held.remove(at);
        }
        self.held.push(Held { piece, fell: None });
    }

    /// How many pieces the engine holds up now of `motion`'s kind.
    pub(super) fn count(&self, engine: u32, hover: bool) -> usize {
        self.held
            .iter()
            .filter(|h| {
                h.piece.engine == engine
                    && h.fell.is_none()
                    && matches!(h.piece.motion, Motion::Hover | Motion::Orbit { .. }) == hover
                    && !matches!(h.piece.motion, Motion::Thrown { .. })
            })
            .count()
    }

    pub(super) fn set_shake(&mut self, engine: u32, shake: f32) {
        match self.shake.iter_mut().find(|s| s.0 == engine) {
            Some(s) => s.1 = shake,
            None => self.shake.push((engine, shake)),
        }
    }

    fn shake_of(&self, engine: u32) -> f32 {
        self.shake
            .iter()
            .find(|s| s.0 == engine)
            .map_or(0.0, |s| s.1)
    }

    /// Lets go of everything the engine holds up at `time`: it all falls.
    pub(super) fn release(&mut self, engine: u32, time: f32) {
        self.set_shake(engine, 0.0);
        for i in 0..self.held.len() {
            let h = self.held[i];
            if h.piece.engine != engine || h.fell.is_some() {
                continue;
            }
            if matches!(h.piece.motion, Motion::Thrown { .. }) {
                continue;
            }
            let p = pose(&h.piece, time, 0.0);
            self.held[i].fell = Some((time, p.at, p.lift, p.pitch, p.roll, p.heading));
        }
    }

    /// Every piece as a prop at `time`; `first` is the first slab model slot (the shards'
    /// follow the slabs').
    pub(super) fn instances(&mut self, time: f32, first: u32, out: &mut Vec<UnitInstance>) {
        self.held.retain(|h| match h.fell {
            Some((at, ..)) => time - at < LIE + SINK,
            None => match h.piece.motion {
                Motion::Thrown { .. } => time - h.piece.born < LIE + SINK,
                _ => true,
            },
        });
        for h in &self.held {
            let shake = self.shake_of(h.piece.engine);
            let (p, sink) = match h.fell {
                None => {
                    let p = pose(&h.piece, time, shake);
                    let sink = match h.piece.motion {
                        Motion::Thrown { .. } => sunk(time - h.piece.born),
                        _ => 0.0,
                    };
                    (p, sink)
                }
                Some((at, ground, lift, pitch, roll, heading)) => {
                    let t = (time - at).max(0.0);
                    let drop = lift - 0.5 * GRAVITY * t * t;
                    let down = (drop <= 0.0) as u8 as f32;
                    // It comes down flatter than it hung, and lies a little askew.
                    let settle = ease(t / 0.8) * down;
                    (
                        Pose {
                            at: ground,
                            lift: drop.max(0.0),
                            heading,
                            pitch: pitch * (1.0 - 0.6 * settle),
                            roll: roll * (1.0 - 0.6 * settle),
                        },
                        sunk(t),
                    )
                }
            };
            let pitch = if p.pitch.abs() < 1e-3 { 1e-3 } else { p.pitch };
            let pos = [p.at.x, p.at.y, h.piece.ground + p.lift];
            let size = if h.piece.model < SLAB_KEYS.len() as u32 {
                mc_models::rift_slabs::SLAB_HALF
            } else {
                mc_models::rift_slabs::SHARD_HEIGHT
            };
            out.push(UnitInstance {
                prev_pos: pos,
                prev_heading: p.heading,
                pos,
                heading: p.heading,
                blueprint: first + h.piece.model,
                owner_flags: KIND_PROP,
                health: 1.0,
                build: 1.0,
                radius: size * h.piece.scale,
                packed: (h.piece.scale * 1000.0).round().max(1.0) as u32,
                arm_pitch: [pitch, pitch, sink - p.lift, 0.0],
                _pad2: [p.roll, p.roll],
                ..bytemuck::Zeroable::zeroed()
            });
        }
    }
}

/// How far a piece lying `t` seconds has sunk into the ground, metres.
fn sunk(t: f32) -> f32 {
    ((t - LIE) / SINK).clamp(0.0, 1.0) * 6.0
}

/// Where `piece` stands at `time`, shaken by `shake`.
fn pose(piece: &Piece, time: f32, shake: f32) -> Pose {
    let t = (time - piece.born).max(0.0);
    let up = ease(t / piece.rise.max(0.05));
    let s = piece.seed * 37.0;
    match piece.motion {
        Motion::Heave => {
            // Shaken: it jumps and rocks on the ground while the engine gathers.
            let jolt = shake * shake;
            let lift = piece.lift * up
                + jolt * 0.6 * piece.scale * (0.5 + 0.5 * (time * 13.0 + s).sin()).powi(3);
            Pose {
                at: piece.at,
                lift,
                heading: piece.heading,
                pitch: piece.pitch * up + jolt * 0.05 * (time * 9.0 + s).sin(),
                roll: piece.roll * up + jolt * 0.05 * (time * 11.0 + s * 1.3).cos(),
            }
        }
        Motion::Thrown { vel } => {
            // Up and out, falling back; it turns over toward how it lands as it flies.
            let land = 2.0 * vel.z / GRAVITY;
            let tf = t.min(land);
            let lift = (vel.z * tf - 0.5 * GRAVITY * tf * tf).max(0.0);
            let along = Vec2::new(vel.x, vel.y) * tf;
            let k = ease(tf / land.max(0.05));
            Pose {
                at: piece.at + along,
                lift,
                heading: piece.heading,
                pitch: piece.pitch * k,
                roll: piece.roll * k,
            }
        }
        Motion::Hover => {
            // Lifted, bobbing slowly, grinding round a little either way.
            let bob = 0.18 * piece.lift * (time * 0.7 + s).sin();
            Pose {
                at: piece.at,
                lift: piece.lift * up + bob * up,
                heading: piece.heading + 0.06 * (time * 0.11 + s).sin() * up,
                pitch: piece.pitch * up + 0.04 * (time * 0.5 + s).sin() * up,
                roll: piece.roll * up + 0.04 * (time * 0.43 + s * 0.7).cos() * up,
            }
        }
        Motion::Orbit { centre, rate } => {
            let off = piece.at - centre;
            let turn = rate * t;
            let (sn, cs) = turn.sin_cos();
            let at = centre + Vec2::new(off.x * cs - off.y * sn, off.x * sn + off.y * cs);
            let bob = 0.1 * piece.lift * (time * 0.6 + s).sin();
            Pose {
                at,
                lift: piece.lift * up + bob,
                heading: piece.heading + turn * 1.7 + 0.3 * t * piece.seed,
                pitch: piece.pitch + 0.2 * (time * 0.3 + s).sin(),
                roll: piece.roll + 0.2 * (time * 0.37 + s).cos(),
            }
        }
    }
}

/// A slab or shard model index for a roll of 0..1.
pub(super) fn slab_model(roll: f32) -> u32 {
    ((roll * SLAB_KEYS.len() as f32) as u32).min(SLAB_KEYS.len() as u32 - 1)
}

pub(super) fn shard_model(roll: f32) -> u32 {
    SLAB_KEYS.len() as u32
        + ((roll * SHARD_KEYS.len() as f32) as u32).min(SHARD_KEYS.len() as u32 - 1)
}
