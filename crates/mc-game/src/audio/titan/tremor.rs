//! The camera shake from ground shocks: a giant's footfall, a great gun's shell (the
//! Culverin's), a bore's strike and its storm. Presentation only; nothing here reaches
//! the sim.
//!
//! - Felt only near the blow. Each shock has a `reach` over the ground, and the camera
//!   feels it by how far the point it looks at is from the shock, not by how much of the
//!   map it can see. A shell landing across the map, or at the edge of a zoomed-out view,
//!   is heard (`far`) but not felt, and a camera high over the field feels less.
//! - Many at once do not add up to a rattle. Blows landing together in one place fold
//!   into one shock; the camera tires, so each blow after a recent one lands more
//!   lightly; and what is felt passes a soft limit. A barrage settles into a rumble a
//!   little stronger than one hit, not a violent shudder.
//! - The wobble runs on one clock, so shocks coming and going never jerk its phase.

use glam::Vec3;
use std::time::Instant;

/// Most shocks remembered at once. A deliberate cap: past it the soft limit has long
/// flattened what one more would add, so the faintest is let go for the newest.
const MAX_TREMORS: usize = 48;
/// A camera this far from the ground (metres) feels half of what it would up close.
const FELT_HEIGHT: f32 = 2500.0;
/// Seconds for the camera's tiredness from recent blows to fall by a factor of e.
const FATIGUE_TIME: f32 = 2.0;
/// Blows this close in time (seconds) and ground (share of reach) fold into one shock.
const FOLD_SECONDS: f32 = 0.2;
const FOLD_REACH: f32 = 0.3;
/// The widest sway, as a share of the camera's distance (so it looks the same at every
/// zoom), which the soft limit only approaches.
const MAX_SWAY: f32 = 0.0085;

struct Tremor {
    at: Vec3,
    strength: f32,
    when: Instant,
    /// Seconds it rings. Over 2 s it is a storm: it holds its level, then stops.
    ring: f32,
    /// Ground distance (metres) past which it is not felt.
    reach: f32,
}

impl Tremor {
    fn storm(&self) -> bool {
        self.ring > 2.0
    }

    fn fade(&self, now: Instant) -> f32 {
        let age = now.saturating_duration_since(self.when).as_secs_f32();
        if self.storm() {
            (1.0 - age / self.ring).max(0.0)
        } else {
            (-age * 3.0 / self.ring).exp()
        }
    }

    fn alive(&self, now: Instant) -> bool {
        let age = now.saturating_duration_since(self.when).as_secs_f32();
        age < self.ring * if self.storm() { 1.0 } else { 2.0 }
    }
}

/// Full at the shock, gone at `reach`, smoothly between.
fn near(d: f32, reach: f32) -> f32 {
    let x = d / reach.max(1.0);
    if x >= 1.0 {
        0.0
    } else {
        (1.0 - x * x).powi(2)
    }
}

pub(super) struct Tremors {
    list: Vec<Tremor>,
    /// How tired the camera is of being shaken, and when that was last brought up to date.
    fatigue: f32,
    fatigue_at: Instant,
    /// The wobble's clock.
    epoch: Instant,
}

impl Default for Tremors {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            list: Vec::new(),
            fatigue: 0.0,
            fatigue_at: now,
            epoch: now,
        }
    }
}

impl Tremors {
    /// A shock at `at`. `focus` is the ground point the camera looks at now: only blows
    /// the camera feels tire it.
    pub(super) fn jolt(&mut self, at: Vec3, strength: f32, ring: f32, reach: f32, focus: Vec3) {
        self.jolt_at(Instant::now(), at, strength, ring, reach, focus);
    }

    fn jolt_at(
        &mut self,
        now: Instant,
        at: Vec3,
        strength: f32,
        ring: f32,
        reach: f32,
        focus: Vec3,
    ) {
        self.list.retain(|t| t.alive(now));
        let dt = now.saturating_duration_since(self.fatigue_at).as_secs_f32();
        self.fatigue *= (-dt / FATIGUE_TIME).exp();
        self.fatigue_at = now;
        let strength = strength / (1.0 + self.fatigue);
        self.fatigue += strength * near((at - focus).truncate().length(), reach);

        let fold = self.list.iter_mut().find(|t| {
            !t.storm()
                && ring <= 2.0
                && now.saturating_duration_since(t.when).as_secs_f32() < FOLD_SECONDS
                && (t.at - at).truncate().length() < t.reach * FOLD_REACH
        });
        if let Some(t) = fold {
            t.strength += strength;
            t.ring = t.ring.max(ring);
            t.reach = t.reach.max(reach);
            return;
        }
        if self.list.len() >= MAX_TREMORS {
            let faintest = self
                .list
                .iter()
                .enumerate()
                .min_by(|a, b| {
                    let felt = |t: &Tremor| t.strength * t.fade(now);
                    felt(a.1).total_cmp(&felt(b.1))
                })
                .map(|(i, _)| i);
            if let Some(i) = faintest {
                self.list.swap_remove(i);
            }
        }
        self.list.push(Tremor {
            at,
            strength,
            when: now,
            ring,
            reach,
        });
    }

    /// How hard the camera shakes, 0 to 1, looking at `focus` from `distance` away.
    fn level(&self, now: Instant, focus: Vec3, distance: f32) -> f32 {
        let height = 1.0 / (1.0 + (distance / FELT_HEIGHT).powi(2));
        let felt: f32 = self
            .list
            .iter()
            .map(|t| t.strength * t.fade(now) * near((t.at - focus).truncate().length(), t.reach))
            .sum::<f32>()
            * height;
        felt / (1.0 + felt)
    }

    /// `camera` as the shocks near it shake it this frame.
    pub(super) fn shaken(&self, camera: &mc_render::camera::Camera) -> mc_render::camera::Camera {
        let now = Instant::now();
        let level = self.level(now, camera.focus, camera.distance);
        let mut out = camera.clone();
        if level < 0.005 {
            return out;
        }
        // Wrapped so the phase keeps its precision in a long session.
        let t = (now.saturating_duration_since(self.epoch).as_secs_f64() % 600.0) as f32;
        let wobble = Vec3::new(
            (t * 23.0).sin() + 0.5 * (t * 37.0).sin(),
            (t * 29.0).cos() + 0.5 * (t * 41.0).sin(),
            0.6 * (t * 31.0).sin(),
        );
        out.focus += wobble * level * MAX_SWAY * camera.distance;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HIT: f32 = 1.4;
    const REACH: f32 = 540.0;

    #[test]
    fn felt_only_near_the_blow() {
        let now = Instant::now();
        let mut tremors = Tremors::default();
        let blast = Vec3::new(5000.0, 5000.0, 0.0);
        tremors.jolt_at(now, blast, HIT, 0.9, REACH, Vec3::ZERO);
        assert!(tremors.level(now, blast, 800.0) > 0.4);
        // Across the map, or just past its reach, even from high up: nothing.
        assert_eq!(tremors.level(now, Vec3::ZERO, 800.0), 0.0);
        let beside = blast + Vec3::new(REACH + 10.0, 0.0, 0.0);
        assert_eq!(tremors.level(now, beside, 6000.0), 0.0);
        // Zoomed far out over it: much less.
        assert!(tremors.level(now, blast, 6000.0) < tremors.level(now, blast, 800.0) * 0.4);
    }

    #[test]
    fn a_salvo_is_not_much_worse_than_one_shell() {
        let now = Instant::now();
        let spot = Vec3::new(100.0, 100.0, 0.0);
        let mut one = Tremors::default();
        one.jolt_at(now, spot, HIT, 0.9, REACH, spot);
        let single = one.level(now, spot, 800.0);

        let mut salvo = Tremors::default();
        for i in 0..20 {
            let at = spot + Vec3::new((i % 5) as f32 * 40.0, (i / 5) as f32 * 40.0, 0.0);
            salvo.jolt_at(now, at, HIT, 0.9, REACH, spot);
        }
        let many = salvo.level(now, spot, 800.0);
        assert!(many > single);
        assert!(many < single * 1.6, "20 shells {many} vs one {single}");
        // Folded, not twenty shocks.
        assert!(salvo.list.len() < 4);
    }

    #[test]
    fn a_barrage_tires_the_camera() {
        let start = Instant::now();
        let spot = Vec3::ZERO;
        let mut tremors = Tremors::default();
        let mut peaks = Vec::new();
        for i in 0..20 {
            let now = start + std::time::Duration::from_millis(400 * i);
            tremors.jolt_at(now, spot, HIT, 0.9, REACH, spot);
            peaks.push(tremors.level(now, spot, 800.0));
        }
        // The first shell kicks; later ones settle below it instead of stacking.
        assert!(peaks[19] < peaks[0], "{peaks:?}");
        assert!(peaks.iter().all(|&p| p <= peaks[0] * 1.2), "{peaks:?}");
    }
}
