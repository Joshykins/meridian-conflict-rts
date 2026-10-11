//! A Regency site at work (`BEAM_NANITE_SITE`) catches the light: now and then a glint in
//! the camera's glass (`lens_flare`) where its build front runs round the hull on the side
//! facing the eye, so the front reads as glaring hot, not only bright.

use super::lens_flare::Flare;
use super::Renderer;
use glam::Vec3;
use mc_sim::mirror::RenderFrame;

/// A glint off the front: violet, near white at its core.
const FRONT_GLINT: Flare = Flare {
    color: Vec3::new(6.5, 2.6, 9.0),
    size: 18.0,
};
/// Glints a site throws a second, and how long each lasts, seconds.
const GLINTS_PER_SECOND: f32 = 3.0;
const GLINT_LIFE: f32 = 0.35;

impl Renderer {
    /// This tick's glints off every Regency site being fed.
    pub(super) fn nanite_site_glints(&mut self, frame: &RenderFrame, time: f32, eye: Vec3) {
        let chance = GLINTS_PER_SECOND * self.tick_seconds;
        for b in frame
            .beams
            .iter()
            .filter(|b| b.kind == mc_sim::reclaim::BEAM_NANITE_SITE)
        {
            let foot = Vec3::from(b.from);
            let front = b.to[2] - foot.z;
            // None once the front has reached the top and the site is settling.
            if front >= b.height * 0.95 || self.scatter.unit() > chance {
                continue;
            }
            // On the near side of the hull, where the front is seen.
            let toward = (eye - foot).truncate().normalize_or_zero();
            let turn = self.scatter.signed() * 1.2;
            let (s, c) = turn.sin_cos();
            let dir = Vec3::new(
                toward.x * c - toward.y * s,
                toward.x * s + toward.y * c,
                0.0,
            );
            let at = foot + dir * b.radius * (0.55 + 0.25 * self.scatter.unit()) + Vec3::Z * front;
            let start = time + self.scatter.unit() * self.tick_seconds;
            self.lens_flares.flash(at, FRONT_GLINT, start, GLINT_LIFE);
        }
    }
}
