//! Where the battle is heard from: the camera's ear.
//!
//! - The strategic camera hears around the ground point it looks at, over about
//!   the ground it shows (`distance`), and quieter the higher it hangs.
//! - The free camera (Ctrl+Alt) hears from its eye. It can sit at a unit's height
//!   and look kilometres over the field, so a sound is placed by how far it is from
//!   the eye: a gun 1 km off in view is heard as if the strategic camera hung 1 km
//!   over it. Its focus is only a few tens of metres ahead, and measuring from
//!   there with the strategic reach (0.9 x distance + 80 m) silenced everything
//!   past a hundred metres or so, while the far-heard hits (`titan.rs`) carried in
//!   from across the map: guns firing in view, and only distant booms heard.

use glam::{Vec2, Vec3};
use mc_render::camera::Camera;

/// Free camera: the eye distance at which a battle sound has fallen to half of what
/// its distance alone gives, metres. Past this the far field thins out quickly, so
/// the whole map does not rattle at once.
const FREE_REACH: f32 = 3000.0;
/// Free camera: the same for the small sounds of work (construction beams).
const FREE_WORK_REACH: f32 = 300.0;
/// How loud a sound is by how far it is heard from: at the strategic camera's
/// focus this is its height; for the free camera, the eye's distance to the sound.
const HEIGHT_EAR: f32 = 260.0;
/// The distance at which something meant to be heard beside it (a gun's wind-up,
/// a loud gun's extra weight) is at half its closeness.
const CLOSE: f32 = 400.0;

/// The camera's ear for one frame: `free` when the free camera is flying.
#[derive(Clone, Copy)]
pub(crate) struct Ear<'a> {
    pub camera: &'a Camera,
    pub free: bool,
}

impl Ear<'_> {
    /// Gain (0..1) and pan (-0.85..0.85) of a battle sound at `pos`.
    pub(crate) fn hear(self, pos: Vec3) -> (f32, f32) {
        if self.free {
            return free(self.camera.eye(), self.camera.yaw, pos, FREE_REACH);
        }
        let (near, height, pan) = self.strategic(pos);
        (near * height, pan)
    }

    /// `hear`, for a sound that fills a line from `from` to `to` (an electric bore's
    /// bolt): heard from the point of it nearest the ear, so a bolt that crosses the view
    /// is loud even when both its ends are off in the distance. Returns the gain, the pan
    /// and that point.
    pub(crate) fn hear_line(self, from: Vec3, to: Vec3) -> (f32, f32, Vec3) {
        const STEPS: usize = 32;
        (0..=STEPS)
            .map(|k| {
                let at = from.lerp(to, k as f32 / STEPS as f32);
                let (gain, pan) = self.hear(at);
                (gain, pan, at)
            })
            .fold(
                (0.0, 0.0, from),
                |best, h| if h.0 > best.0 { h } else { best },
            )
    }

    /// `hear`, for the small sounds of work (construction beams): they die away
    /// just past the edge of the view instead of carrying across the map, so a base
    /// full of factories is heard when you look at it and not from everywhere.
    pub(crate) fn hear_work(self, pos: Vec3) -> (f32, f32) {
        if self.free {
            return free(self.camera.eye(), self.camera.yaw, pos, FREE_WORK_REACH);
        }
        let (near, height, pan) = self.strategic(pos);
        (near * near * height, pan)
    }

    /// Beside the sound, 1; far from it (strategic zoom, or a free eye far away), near 0.
    pub(crate) fn closeness(self, pos: Vec3) -> f32 {
        let d = if self.free {
            self.camera.eye().distance(pos)
        } else {
            self.camera.distance
        };
        CLOSE / (CLOSE + d)
    }

    /// (near, height, pan) around the strategic camera's focus.
    fn strategic(self, pos: Vec3) -> (f32, f32, f32) {
        let camera = self.camera;
        let offset = (pos - camera.focus).truncate();
        let view = camera.distance * 0.9 + 80.0;
        let near = 1.0 / (1.0 + (offset.length() / view).powi(2) * 1.5);
        let height = (HEIGHT_EAR / (HEIGHT_EAR + camera.distance)).sqrt();
        let pan = (offset.dot(right(camera.yaw)) / view).clamp(-0.85, 0.85);
        (near, height, pan)
    }
}

/// The screen's right, on the ground, for a camera turned `yaw`.
fn right(yaw: f32) -> Vec2 {
    Vec2::new(yaw.cos(), -yaw.sin())
}

/// Heard from an eye at `eye` facing `yaw`: louder the nearer, thinning past `reach`,
/// panned by the direction it comes from.
fn free(eye: Vec3, yaw: f32, pos: Vec3, reach: f32) -> (f32, f32) {
    let r = eye.distance(pos);
    let by_distance = (HEIGHT_EAR / (HEIGHT_EAR + r)).sqrt();
    let thinning = 1.0 / (1.0 + (r / reach).powi(2));
    let flat = (pos - eye).truncate();
    let pan = if flat.length() > 1.0 {
        (flat.normalize().dot(right(yaw)) * 0.85).clamp(-0.85, 0.85)
    } else {
        0.0
    };
    (by_distance * thinning, pan)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The marked moment (match 20260927-170903, mark 3): a free eye near the ground,
    /// looking over a field of Culverins 0.8-6 km off, whose shells land on domes
    /// 11.6 km away.
    fn mark_camera() -> Camera {
        let mut camera = Camera::new(Vec2::splat(16_384.0), Vec2::new(1928.0, 1351.0));
        camera.focus = Vec3::new(7167.0, 11535.0, 137.0);
        camera.distance = 40.0;
        camera.yaw = (-109.4f32).to_radians();
        camera.pitch_free = Some(0.04);
        camera
    }

    #[test]
    fn a_free_eye_hears_the_guns_it_looks_at() {
        let camera = mark_camera();
        let free = Ear {
            camera: &camera,
            free: true,
        };
        let pinned = Ear {
            camera: &camera,
            free: false,
        };
        // Culverins firing 0.8 km and 2 km off.
        let gun_near = Vec3::new(6373.0, 11811.0, 95.0);
        let gun_far = Vec3::new(5259.0, 11885.0, 94.0);
        // Measured from the focus with the strategic reach they were all but silent,
        // below the mixer's floor (0.004) past a couple of kilometres.
        assert!(pinned.hear(gun_near).0 < 0.02);
        assert!(pinned.hear(gun_far).0 < 0.004);
        let (near, _) = free.hear(gun_near);
        let (far, _) = free.hear(gun_far);
        assert!(near > 0.4, "a gun 0.8 km off is heard: {near}");
        assert!(
            far > 0.2 && far < near,
            "one 2 km off, a little quieter: {far}"
        );
        // A shell on a dome 11.6 km away stays faint beside them.
        let (dome, _) = free.hear(Vec3::new(13656.0, 1924.0, 91.0));
        assert!(dome < far * 0.1, "{dome}");
    }

    #[test]
    fn a_free_eye_pans_by_direction() {
        let mut camera = mark_camera();
        camera.yaw = 0.0;
        camera.focus = Vec3::new(1000.0, 1000.0, 10.0);
        let ear = Ear {
            camera: &camera,
            free: true,
        };
        let eye = camera.eye();
        let (_, ahead) = ear.hear(eye + Vec3::new(0.0, 900.0, 0.0));
        let (_, right) = ear.hear(eye + Vec3::new(900.0, 0.0, 0.0));
        let (_, left) = ear.hear(eye + Vec3::new(-900.0, 0.0, 0.0));
        assert!(ahead.abs() < 0.05, "{ahead}");
        assert!(right > 0.8 && left < -0.8, "{right} {left}");
    }

    #[test]
    fn a_bolt_across_the_view_is_heard_where_it_passes() {
        let camera = mark_camera();
        let ear = Ear {
            camera: &camera,
            free: false,
        };
        // Both ends 3 km either side of what the camera looks at; the bolt runs under it.
        let (from, to) = (
            camera.focus + Vec3::new(-3000.0, 0.0, 0.0),
            camera.focus + Vec3::new(3000.0, 0.0, 0.0),
        );
        let (line, _, at) = ear.hear_line(from, to);
        assert!(line > ear.hear(to).0 * 20.0, "{line} vs {}", ear.hear(to).0);
        assert!(at.distance(camera.focus) < 200.0, "{at}");
    }

    #[test]
    fn work_dies_away_sooner_than_battle() {
        let camera = mark_camera();
        let ear = Ear {
            camera: &camera,
            free: true,
        };
        let at = camera.eye() + Vec3::new(700.0, 0.0, 0.0);
        assert!(ear.hear_work(at).0 < ear.hear(at).0 * 0.3);
        assert!(ear.closeness(at) < 0.4);
        assert!(ear.closeness(camera.eye() + Vec3::X * 20.0) > 0.9);
    }
}
