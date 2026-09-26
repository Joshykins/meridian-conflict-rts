//! The free camera's flight (Ctrl+Alt), for pictures and recordings.
//!
//! The camera is a pose: an eye, a heading, a pitch that can look up past the
//! horizon, and a lens. The keys and the mouse move a goal pose; what is shown
//! eases after it at the chosen smoothing, and movement has momentum, so every
//! move is a camera move rather than a jump.
//!
//! On top of that: a lock-on that keeps a unit or a point in the middle of the
//! frame while the camera flies, a follow that carries the camera along with a
//! unit, an orbit about whatever is aimed at, nine saved shots to glide between
//! (or play one after another), and a lock that stops every input from moving it.
//!
//! Leaving hands the view to the strategic camera nearest where the free one
//! is, and that camera is the player's again at once: what is shown only eases
//! out of the free pose into it over a moment.

use glam::{Vec2, Vec3};
use mc_render::camera::{FOV_Y, MIN_DISTANCE};
use mc_render::Camera;

/// Looking up, about 34 degrees above the horizon.
pub const PITCH_MIN: f32 = -0.6;
/// Straight down, less a hair so the heading still means something.
pub const PITCH_MAX: f32 = 1.55;
/// Long lens, about 200 mm on a full-frame body.
pub const FOV_MIN: f32 = 0.12;
/// Wide lens, about 14 mm.
pub const FOV_MAX: f32 = 1.4;
/// How close the eye may come to the ground, metres.
const CLEARANCE: f32 = 3.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Pose {
    pub eye: Vec3,
    /// Around Z; zero looks along +Y.
    pub yaw: f32,
    /// Below the horizon; negative looks up.
    pub pitch: f32,
    pub fov: f32,
}

impl Pose {
    pub fn of(camera: &Camera) -> Pose {
        Pose {
            eye: camera.eye(),
            yaw: camera.yaw,
            pitch: camera.pitch(),
            fov: camera.fov,
        }
    }

    pub fn forward(&self) -> Vec3 {
        forward(self.yaw, self.pitch)
    }

    /// The same pose with its heading moved by whole turns to lie nearest `yaw`,
    /// so easing to it goes the short way round.
    fn near_yaw(mut self, yaw: f32) -> Pose {
        self.yaw = nearest(self.yaw, yaw);
        self
    }

    fn lerp(&self, to: &Pose, k: f32) -> Pose {
        Pose {
            eye: self.eye.lerp(to.eye, k),
            yaw: self.yaw + (to.yaw - self.yaw) * k,
            pitch: self.pitch + (to.pitch - self.pitch) * k,
            // The lens eases in its log, so a zoom reads even at either end.
            fov: (self.fov.ln() + (to.fov.ln() - self.fov.ln()) * k).exp(),
        }
    }
}

pub fn forward(yaw: f32, pitch: f32) -> Vec3 {
    Vec3::new(
        yaw.sin() * pitch.cos(),
        yaw.cos() * pitch.cos(),
        -pitch.sin(),
    )
}

/// `angle` moved by whole turns to lie within half a turn of `near`.
fn nearest(angle: f32, near: f32) -> f32 {
    let turn = std::f32::consts::TAU;
    near + (angle - near + turn * 0.5).rem_euclid(turn) - turn * 0.5
}

/// Heading and pitch that look from `eye` at `at`.
fn look_at(eye: Vec3, at: Vec3) -> Option<(f32, f32)> {
    let d = at - eye;
    let len = d.length();
    (len > 0.5).then(|| (d.x.atan2(d.y), (-d.z / len).asin()))
}

/// Full-frame focal length for a vertical field of view, for the readout.
pub fn focal_mm(fov: f32) -> f32 {
    12.0 / (fov * 0.5).tan()
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Easing {
    /// Barely eased: for looking around and setting up.
    Snappy,
    #[default]
    Smooth,
    /// Heavy, slow easing: for recording.
    Cinematic,
}

impl Easing {
    pub fn label(self) -> &'static str {
        match self {
            Easing::Snappy => "Snappy",
            Easing::Smooth => "Smooth",
            Easing::Cinematic => "Cinematic",
        }
    }

    pub fn next(self) -> Easing {
        match self {
            Easing::Snappy => Easing::Smooth,
            Easing::Smooth => Easing::Cinematic,
            Easing::Cinematic => Easing::Snappy,
        }
    }

    /// Per second: how fast flight speeds up and slows, and how fast what is
    /// shown closes on the goal.
    fn rates(self) -> (f32, f32) {
        match self {
            Easing::Snappy => (16.0, 26.0),
            Easing::Smooth => (6.0, 9.0),
            Easing::Cinematic => (1.8, 2.8),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Aim {
    Unit(u32),
    Point(Vec3),
}

/// The free pose, as what it adds to the strategic camera it is handing back
/// to: along the focus, the heading, and the pitch and lens it had.
struct HandBack {
    shift: Vec3,
    yaw: f32,
    pitch: f32,
    fov: f32,
    t: f32,
    seconds: f32,
    /// The shift and heading last put on the camera, and the focus they left,
    /// so they can be taken off again.
    applied: Option<(Vec3, f32, Vec3)>,
}

struct Glide {
    from: Pose,
    to: Pose,
    t: f32,
    seconds: f32,
}

/// What the player asked of the camera this frame.
#[derive(Default, Clone, Copy)]
pub struct Controls {
    /// Right, forward, up; each -1..1.
    pub fly: Vec3,
    pub fast: bool,
    /// Mouse-look: heading and pitch, radians.
    pub look: Vec2,
    /// Orbit about the aim (Alt): heading and elevation, radians.
    pub orbit: Vec2,
    /// Alt is held: the orbit keeps one pivot for the whole hold.
    pub orbiting: bool,
    /// Wheel notches: forward along the view.
    pub dolly: f32,
    /// Wheel notches with the look button held: flight speed.
    pub throttle: f32,
    /// Lens, per second: positive is longer (narrower).
    pub lens: f32,
    /// Middle drag, pixels: the ground slides with the pointer.
    pub drag: Vec2,
}

impl Controls {
    fn moves(&self) -> bool {
        self.fly != Vec3::ZERO
            || self.look != Vec2::ZERO
            || self.orbit != Vec2::ZERO
            || self.dolly != 0.0
            || self.drag != Vec2::ZERO
    }
}

/// What the camera needs to know of the world.
pub trait World {
    fn ground(&self, xy: Vec2) -> f32;
    /// Where a unit is now, and its radius.
    fn unit(&self, id: u32) -> Option<(Vec3, f32)>;
}

pub struct Cine {
    pub goal: Pose,
    pub shown: Pose,
    vel: Vec3,
    pub smooth: Easing,
    /// Flight speed multiplier.
    pub speed: f32,
    pub aim: Option<Aim>,
    /// The unit the camera rides along with, and the eye's offset from it.
    pub follow: Option<(u32, Vec3)>,
    /// Nothing the player does moves the camera.
    pub locked: bool,
    pub shots: [Option<Pose>; 9],
    glide: Option<Glide>,
    /// Playing the saved shots in order: the one being flown to.
    pub playing: Option<usize>,
    /// The shot the camera last went to or was saved in, for the readout.
    pub at_shot: Option<usize>,
    /// Thirds lines over the picture.
    pub grid: bool,
    orbit_pivot: Option<Vec3>,
    /// Where the followed unit was last frame, so the shown eye moves with it.
    follow_prev: Option<Vec3>,
    hand_back: Option<HandBack>,
}

impl Default for Cine {
    fn default() -> Cine {
        let pose = Pose {
            eye: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.8,
            fov: FOV_Y,
        };
        Cine {
            goal: pose,
            shown: pose,
            vel: Vec3::ZERO,
            smooth: Easing::Smooth,
            speed: 1.0,
            aim: None,
            follow: None,
            locked: false,
            shots: [None; 9],
            glide: None,
            playing: None,
            at_shot: None,
            grid: false,
            orbit_pivot: None,
            follow_prev: None,
            hand_back: None,
        }
    }
}

impl Cine {
    /// Takes over from the strategic camera exactly where it is: no jump.
    pub fn enter(&mut self, camera: &Camera) {
        let pose = Pose::of(camera);
        self.goal = pose;
        self.shown = pose;
        self.vel = Vec3::ZERO;
        self.glide = None;
        self.playing = None;
        self.orbit_pivot = None;
        self.locked = false;
        self.hand_back = None;
    }

    /// Gives the view back to the strategic camera nearest the free one: looking
    /// at the same place from about the same eye (at what was locked on, if
    /// anything was), the lens back to normal. The strategic camera is left on
    /// `camera`, for `hand_back_apply` to ease into. Returns the unit it rode
    /// with, for the strategic camera to track.
    pub fn leave(&mut self, camera: &mut Camera, world: &dyn World) -> Option<u32> {
        let from = Pose::of(camera);
        let at = |id: u32| world.unit(id).map(|(p, _)| p);
        let track = self.follow.map(|(id, _)| id).filter(|&id| at(id).is_some());
        let pin = match (track, self.aim) {
            (Some(id), _) | (None, Some(Aim::Unit(id))) => at(id),
            (None, Some(Aim::Point(p))) => Some(p),
            (None, None) => None,
        };
        self.release();
        self.playing = None;
        self.locked = false;
        self.glide = None;

        camera.pitch_free = None;
        camera.fov = FOV_Y;
        camera.focus = pin.unwrap_or_else(|| ground_ahead(&from, world, camera.max_distance()));
        camera.clamp_focus();
        let (yaw, pitch) = look_at(from.eye, camera.focus).unwrap_or((from.yaw, from.pitch));
        camera.yaw = nearest(yaw, from.yaw);
        camera.distance = from
            .eye
            .distance(camera.focus)
            .clamp(MIN_DISTANCE, camera.max_distance());
        camera.tilt_to(pitch);

        // Shown at the start: the free view exactly, the same eye looking the same way.
        let moved = camera.eye().distance(from.eye);
        let turn = (from.yaw - camera.yaw).abs()
            + (from.pitch - camera.pitch()).abs()
            + (from.fov / FOV_Y).ln().abs();
        self.hand_back = Some(HandBack {
            shift: from.eye + from.forward() * camera.distance - camera.focus,
            yaw: from.yaw - camera.yaw,
            pitch: from.pitch,
            fov: from.fov,
            t: 0.0,
            seconds: (0.2 + moved.sqrt() * 0.012 + turn * 0.25).clamp(0.2, 0.6),
            applied: None,
        });
        track
    }

    /// Takes the hand-back's easing off the camera, so the strategic camera's
    /// frame runs on the player's own view. A jump since it was put on (the
    /// minimap, a unit) ends it.
    pub fn hand_back_lift(&mut self, camera: &mut Camera) {
        let Some((shift, yaw, left)) = self.hand_back.as_mut().and_then(|h| h.applied.take())
        else {
            return;
        };
        camera.pitch_free = None;
        camera.fov = FOV_Y;
        if camera.focus.distance(left) > camera.distance {
            self.hand_back = None;
            return;
        }
        camera.focus -= shift;
        camera.yaw -= yaw;
    }

    /// Puts the hand-back's easing on the camera, `dt` further on. What is
    /// left of the free pose falls away fast at first, so the hand-back reads
    /// as a response, not a camera move.
    pub fn hand_back_apply(&mut self, camera: &mut Camera, dt: f32) {
        self.hand_back_lift(camera);
        let Some(h) = &mut self.hand_back else {
            return;
        };
        h.t = (h.t + dt / h.seconds).min(1.0);
        if h.t >= 1.0 {
            self.hand_back = None;
            return;
        }
        let left = (1.0 - h.t).powi(3);
        let pitch = camera.pitch();
        let shift = h.shift * left;
        let yaw = h.yaw * left;
        camera.focus += shift;
        camera.yaw += yaw;
        camera.pitch_free = Some(pitch + (h.pitch - pitch) * left);
        camera.fov = (FOV_Y.ln() + (h.fov / FOV_Y).ln() * left).exp();
        h.applied = Some((shift, yaw, camera.focus));
    }

    pub fn release(&mut self) -> bool {
        let had = self.aim.is_some() || self.follow.is_some();
        self.aim = None;
        self.follow = None;
        had
    }

    /// Eases to `to` over a time that suits how far it is.
    pub fn glide_to(&mut self, to: Pose) {
        let from = self.shown;
        let to = to.near_yaw(from.yaw);
        let far = from.eye.distance(to.eye);
        let turn = (to.yaw - from.yaw).abs() + (to.pitch - from.pitch).abs();
        let seconds = (0.9 + far.sqrt() * 0.045 + turn * 0.5).clamp(0.9, 6.0);
        self.glide = Some(Glide {
            from,
            to,
            t: 0.0,
            seconds: if self.playing.is_some() {
                seconds * 1.6
            } else {
                seconds
            },
        });
        self.goal = from;
        self.vel = Vec3::ZERO;
        self.orbit_pivot = None;
    }

    /// Cuts straight to `to`.
    pub fn cut_to(&mut self, to: Pose) {
        self.glide = None;
        self.goal = to;
        self.shown = to;
        self.vel = Vec3::ZERO;
    }

    pub fn gliding(&self) -> bool {
        self.glide.is_some()
    }

    /// No glide under way and what is shown has caught up with it.
    #[cfg(test)]
    fn settled(&self) -> bool {
        let (a, b) = (&self.shown, &self.goal);
        self.glide.is_none()
            && a.eye.distance(b.eye) < 0.05
            && (a.yaw - b.yaw).abs() < 5e-4
            && (a.pitch - b.pitch).abs() < 5e-4
            && (a.fov - b.fov).abs() < 5e-4
    }

    pub fn save(&mut self, slot: usize) {
        self.shots[slot] = Some(self.goal);
        self.at_shot = Some(slot);
    }

    /// Goes to a saved shot: a glide, or a cut when `cut` or when it is asked
    /// for again while already gliding there. Aim and follow let go, since the
    /// shot is the framing.
    pub fn recall(&mut self, slot: usize, cut: bool) -> bool {
        let Some(pose) = self.shots[slot] else {
            return false;
        };
        self.release();
        self.playing = None;
        if cut || (self.gliding() && self.at_shot == Some(slot)) {
            self.cut_to(pose);
        } else {
            self.glide_to(pose);
        }
        self.at_shot = Some(slot);
        true
    }

    /// Plays the saved shots in order from the first, or stops. `false` with fewer than two.
    pub fn play(&mut self) -> bool {
        if self.playing.is_some() {
            self.playing = None;
            self.glide = None;
            return true;
        }
        let saved: Vec<usize> = (0..9).filter(|&i| self.shots[i].is_some()).collect();
        if saved.len() < 2 {
            return false;
        }
        self.release();
        self.playing = Some(saved[0]);
        self.at_shot = Some(saved[0]);
        self.glide_to(self.shots[saved[0]].unwrap());
        true
    }

    /// F: flies to a good look at `at` (a unit's radius, or 0 for ground).
    pub fn frame(&mut self, at: Vec3, radius: f32) {
        let dist = if radius > 0.0 {
            (radius * 7.0 + 30.0).clamp(35.0, 900.0)
        } else {
            (self.shown.eye.distance(at) * 0.45).clamp(60.0, 700.0)
        };
        let flat = (at - self.shown.eye).truncate();
        let yaw = if flat.length() > 1.0 {
            flat.x.atan2(flat.y)
        } else {
            self.shown.yaw
        };
        let pitch = 0.42;
        let eye = at - forward(yaw, pitch) * dist;
        self.release();
        self.playing = None;
        self.glide_to(Pose {
            eye,
            yaw,
            pitch,
            fov: self.goal.fov,
        });
    }

    pub fn update(&mut self, dt: f32, c: &Controls, world: &dyn World, map: Vec2) {
        let dt = dt.min(0.1);
        let (accel, follow_rate) = self.smooth.rates();
        let c = if self.locked { Controls::default() } else { *c };
        if !c.orbiting {
            self.orbit_pivot = None;
        }

        if let Some(g) = &mut self.glide {
            if c.moves() {
                // Taking hold of the camera ends the glide where it is.
                self.goal = self.shown;
                self.glide = None;
                self.playing = None;
            } else {
                g.t = (g.t + dt / g.seconds).min(1.0);
                let k = ease_in_out(g.t);
                self.goal = g.from.lerp(&g.to, k);
                if g.t >= 1.0 {
                    self.glide = None;
                    if let Some(at) = self.playing {
                        let next = (at + 1..9).chain(0..at).find(|&i| self.shots[i].is_some());
                        match next.filter(|&n| n > at) {
                            Some(n) => {
                                self.playing = Some(n);
                                self.at_shot = Some(n);
                                self.glide_to(self.shots[n].unwrap());
                            }
                            None => self.playing = None,
                        }
                    }
                }
                // A glide is already eased: shown follows it closely.
                let share = 1.0 - (-dt * follow_rate.max(9.0)).exp();
                self.shown = self.shown.lerp(&self.goal, share);
                return;
            }
        }

        let g = &mut self.goal;
        let ground = |p: Vec2| world.ground(p);

        // Riding along: the eye keeps its offset from the unit.
        let followed = self
            .follow
            .and_then(|(id, off)| world.unit(id).map(|(p, _)| (id, p, off)));
        if self.follow.is_some() && followed.is_none() {
            self.follow = None;
        }
        if let Some((_, p, off)) = followed {
            g.eye = p + off;
        }

        // The lens, in its log.
        if c.lens != 0.0 {
            g.fov = (g.fov * (-c.lens * dt * 1.1).exp()).clamp(FOV_MIN, FOV_MAX);
        }
        // A long lens turns and flies slower, so it stays steady.
        let steady = (g.fov / FOV_Y).clamp(0.25, 1.5);

        // Mouse-look takes the aim back from a lock-on.
        if c.look != Vec2::ZERO {
            self.aim = None;
            g.yaw += c.look.x * steady;
            g.pitch = (g.pitch + c.look.y * steady).clamp(PITCH_MIN, PITCH_MAX);
        }

        let height = (g.eye.z - ground(g.eye.truncate())).max(0.0);
        if c.throttle != 0.0 {
            self.speed = (self.speed * 1.25f32.powf(c.throttle)).clamp(0.05, 20.0);
        }
        // Speed follows height: a crawl among the tanks, a sweep from the clouds.
        let base = (height * 1.1).clamp(10.0, 4000.0) * self.speed * if c.fast { 4.0 } else { 1.0 };
        let fwd = forward(g.yaw, g.pitch);
        let right = Vec3::new(g.yaw.cos(), -g.yaw.sin(), 0.0);
        let wish = right * c.fly.x + fwd * c.fly.y + Vec3::Z * c.fly.z;
        let wish = if wish.length_squared() > 1.0 {
            wish.normalize()
        } else {
            wish
        } * base
            * steady.sqrt();
        self.vel += (wish - self.vel) * (1.0 - (-dt * accel).exp());
        g.eye += self.vel * dt;
        // The wheel moves along the view, further the higher it is.
        if c.dolly != 0.0 {
            g.eye += fwd * c.dolly * (height * 0.22).clamp(4.0, 900.0) * self.speed.sqrt();
        }
        // Middle drag: the ground under the pointer stays under it.
        if c.drag != Vec2::ZERO {
            let per_px = (height.max(10.0) / fwd.z.abs().max(0.35)) * (g.fov * 0.5).tan() / 540.0;
            let flat_fwd = Vec3::new(g.yaw.sin(), g.yaw.cos(), 0.0);
            g.eye += (-right * c.drag.x + flat_fwd * c.drag.y) * per_px;
        }

        // The point the lock-on or the orbit turns about.
        let aim_at = match self.aim {
            Some(Aim::Unit(id)) => match world.unit(id) {
                // The unit's own point, which the strategic camera also swings round:
                // taking over from an Alt-orbit keeps the frame exactly.
                Some((p, _)) => Some(p),
                None => {
                    self.aim = None;
                    None
                }
            },
            Some(Aim::Point(p)) => Some(p),
            None => None,
        };

        // Alt: swing about the aim, or about what is in the middle of the frame.
        if c.orbiting {
            let pivot = *self.orbit_pivot.get_or_insert_with(|| {
                aim_at.unwrap_or_else(|| centre_hit(g, world).unwrap_or(g.eye + fwd * 300.0))
            });
            if c.orbit != Vec2::ZERO {
                let off = g.eye - pivot;
                let r = off.length().max(1.0);
                let heading = off.x.atan2(off.y) + c.orbit.x;
                let elev = ((off.z / r).asin() + c.orbit.y).clamp(-0.1, 1.5);
                g.eye = pivot
                    + Vec3::new(
                        heading.sin() * elev.cos(),
                        heading.cos() * elev.cos(),
                        elev.sin(),
                    ) * r;
                if aim_at.is_none() {
                    if let Some((yaw, pitch)) = look_at(g.eye, pivot) {
                        g.yaw = nearest(yaw, g.yaw);
                        g.pitch = pitch.clamp(PITCH_MIN, PITCH_MAX);
                    }
                }
            }
        }

        // Stay over the map and off the ground.
        g.eye.x = g.eye.x.clamp(-2000.0, map.x + 2000.0);
        g.eye.y = g.eye.y.clamp(-2000.0, map.y + 2000.0);
        let floor = ground(g.eye.truncate()) + CLEARANCE;
        if g.eye.z < floor {
            g.eye.z = floor;
            self.vel.z = self.vel.z.max(0.0);
        }
        g.eye.z = g.eye.z.min(30_000.0);

        if let Some(at) = aim_at {
            if let Some((yaw, pitch)) = look_at(g.eye, at) {
                g.yaw = nearest(yaw, g.yaw);
                g.pitch = pitch.clamp(PITCH_MIN, PITCH_MAX);
            }
        }
        if let Some((id, p, _)) = followed {
            self.follow = Some((id, g.eye - p));
        }

        // What is shown eases after the goal. Following a unit, the eye keeps up
        // with it however heavy the smoothing, or the unit would drive out of frame.
        let share = 1.0 - (-dt * follow_rate).exp();
        let mut shown = self.shown.lerp(&self.goal, share);
        if let Some((_, p, _)) = followed {
            let carried = self.shown.eye + (p - self.last_follow_pos(p));
            shown.eye = carried.lerp(self.goal.eye, share);
        }
        self.shown = shown;
        self.follow_prev = followed.map(|(_, p, _)| p);
    }

    fn last_follow_pos(&self, now: Vec3) -> Vec3 {
        self.follow_prev.unwrap_or(now)
    }

    /// Puts the shown pose on the renderer's camera.
    pub fn apply(&self, camera: &mut Camera, world: &dyn World) {
        let p = &self.shown;
        let fwd = p.forward();
        let height = (p.eye.z - world.ground(p.eye.truncate())).max(1.0);
        // The focus sits along the view: on the ground when that is near, else a
        // little ahead, so the near plane (a share of the distance) stays close.
        let near_ground = ray_ground(p.eye, fwd, world, 40_000.0);
        let reach = (height * 3.0).max(40.0);
        let distance = near_ground
            .map_or(reach, |d| d.min(reach))
            .clamp(8.0, 40_000.0);
        camera.focus = p.eye + fwd * distance;
        camera.distance = distance;
        camera.yaw = p.yaw;
        camera.pitch_free = Some(p.pitch);
        camera.fov = p.fov;
    }
}

/// How far along a ray the ground is, marched coarse then refined.
fn ray_ground(eye: Vec3, dir: Vec3, world: &dyn World, max: f32) -> Option<f32> {
    if dir.z > 0.3 {
        return None;
    }
    let mut t = 0.0;
    let mut step = 8.0;
    let mut last = 0.0;
    while t < max {
        let p = eye + dir * t;
        if p.z <= world.ground(p.truncate()) {
            // Halve back to the crossing.
            let (mut lo, mut hi) = (last, t);
            for _ in 0..12 {
                let mid = (lo + hi) * 0.5;
                let q = eye + dir * mid;
                if q.z <= world.ground(q.truncate()) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            return Some(hi);
        }
        last = t;
        t += step;
        step = (step * 1.15).min(400.0);
    }
    None
}

/// Where the strategic camera looks from a free pose with nothing locked on:
/// the ground along the view, or, from the sky or the far horizon, tipped down
/// onto it.
fn ground_ahead(p: &Pose, world: &dyn World, reach: f32) -> Vec3 {
    [p.pitch, p.pitch.max(0.35), p.pitch.max(0.8)]
        .into_iter()
        .find_map(|pitch| {
            let dir = forward(p.yaw, pitch);
            ray_ground(p.eye, dir, world, reach).map(|d| p.eye + dir * d)
        })
        .unwrap_or_else(|| {
            let xy = p.eye.truncate();
            xy.extend(world.ground(xy))
        })
}

fn centre_hit(p: &Pose, world: &dyn World) -> Option<Vec3> {
    let fwd = p.forward();
    ray_ground(p.eye, fwd, world, 20_000.0).map(|d| p.eye + fwd * d)
}

fn ease_in_out(t: f32) -> f32 {
    // Smootherstep: no jolt as the glide starts or lands.
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[cfg(test)]
mod tests;
