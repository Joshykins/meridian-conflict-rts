//! A ring of effect slots the GPU draws every frame (puffs, sprites): where the next one
//! goes, and when each slot's occupant fades. A ring too small for a battle shows in the
//! perf report (`fx.puffs_cut`, `fx.effects_cut`: occupants overwritten while still on
//! screen) instead of only as smoke and flashes vanishing mid-life.
//!
//! What can never be seen is not put in at all (`FxRing::seen`): a battle of tens of
//! thousands lays hundreds of thousands of puffs a second, most of them far off the
//! screen or under a pixel, and in a ring of fixed size each of those pushed out one the
//! player was watching. And when what can be seen is still more than the ring holds,
//! the fine stuff (dust, sparks, droplets) is laid sparser and bigger (`FxRing::thin`)
//! before anything already in the air is cut short.

use crate::camera::Camera;
use glam::{Vec3, Vec4};

/// The least share of fine stuff laid however crowded the ring: a deliberate floor, as
/// below it a dust cloud is a few big blobs. Past it, the oldest is cut short.
const MIN_KEEP: f32 = 0.08;
/// Most a thinned occupant grows to cover for the ones left out. Short of covering
/// them all: a fight crowded enough to thin hard would otherwise stand in a wall of
/// dust that hides it.
const MOST_GROWTH: f32 = 1.6;

/// What the camera takes in this frame.
#[derive(Clone, Copy)]
struct View {
    /// Left, right, bottom and top planes, pointing inward.
    planes: [Vec4; 4],
    eye: Vec3,
    /// Pixels a metre covers a metre away.
    scale: f32,
}

pub(super) struct FxRing {
    cursor: usize,
    /// Per slot: when its occupant is gone.
    ends: Vec<f32>,
    view: Option<View>,
    /// The share of thinnable occupants put in (`thin`), and what is owed of it.
    keep: f32,
    credit: f32,
    /// This frame's occupants, and how many found the ring three quarters full.
    writes: u32,
    crowded: u32,
}

impl FxRing {
    pub(super) fn new(len: usize) -> FxRing {
        FxRing {
            cursor: 0,
            ends: vec![f32::NEG_INFINITY; len],
            view: None,
            keep: 1.0,
            credit: 0.0,
            writes: 0,
            crowded: 0,
        }
    }

    /// Where the next occupant goes.
    #[cfg(test)]
    pub(super) fn cursor(&self) -> usize {
        self.cursor
    }

    /// The view the next occupants are judged by (`seen`). Once a frame: the share
    /// `thin` keeps follows how full the ring ran over the last frame, falling while
    /// more than three quarters of it was still showing and coming back once not.
    pub(super) fn set_view(&mut self, camera: &Camera) {
        if self.crowded > 0 {
            let crowded = self.crowded as f32 / self.writes.max(1) as f32;
            self.keep = (self.keep * (1.0 - crowded).max(0.6)).max(MIN_KEEP);
        } else {
            self.keep = (self.keep * 1.05).min(1.0);
        }
        mc_core::perf_count!("fx.puffs_keep_pct", (self.keep * 100.0) as u64);
        (self.writes, self.crowded) = (0, 0);
        let f = camera.frustum();
        self.view = Some(View {
            planes: [f[0], f[1], f[2], f[3]],
            eye: camera.eye(),
            scale: camera.projection_scale(),
        });
    }

    /// Whether an occupant at `pos`, at most `size` metres across, drifting `vel` for
    /// `life` seconds, could be seen: somewhere it gets to lies in the view (with a
    /// quarter of its distance spare, for the camera moving meanwhile), and it is at
    /// least a quarter of a pixel across there.
    pub(super) fn seen(&self, pos: Vec3, vel: Vec3, life: f32, size: f32) -> bool {
        let Some(view) = self.view else {
            return true;
        };
        let far = pos.distance(view.eye).max(1.0);
        if size * view.scale < 0.25 * far {
            return false;
        }
        let reach = size + vel.length() * life + 0.25 * far;
        view.planes
            .iter()
            .all(|p| p.truncate().dot(pos) + p.w >= -reach)
    }

    /// Whether a thinnable occupant (fine stuff, one of many) goes in, and if so how
    /// many times bigger, to cover what the ones left out would have: the share kept
    /// is spread evenly over them.
    pub(super) fn thin(&mut self) -> Option<f32> {
        self.credit += self.keep;
        if self.credit < 1.0 {
            return None;
        }
        self.credit -= 1.0;
        Some((1.0 / self.keep).sqrt().min(MOST_GROWTH))
    }

    /// The slot for an occupant shown from `start` until `end`, and whether it cut the
    /// one there short.
    pub(super) fn take(&mut self, start: f32, end: f32) -> (usize, bool) {
        let at = self.cursor;
        let len = self.ends.len();
        self.writes += 1;
        if self.ends[(at + len / 4) % len] > start {
            self.crowded += 1;
        }
        let cut = self.ends[at] > start;
        self.ends[at] = end;
        self.cursor = (at + 1) % self.ends.len();
        (at, cut)
    }
}
