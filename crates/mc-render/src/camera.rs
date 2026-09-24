//! The strategic camera: one continuous zoom from a unit's tracks to the whole map.

use glam::{Mat4, Vec2, Vec3, Vec4};

pub const FOV_Y: f32 = 0.6;
pub const MIN_DISTANCE: f32 = 25.0;
/// Extra tilt the player can add: Page Down / mouse-down while Alt is held.
pub const MIN_TILT: f32 = -0.6;
/// Extra tilt the player can add: Page Up / mouse-up while Alt is held.
/// Enough to flatten the view to the horizon from a typical play height.
pub const MAX_TILT: f32 = 1.5;
/// Shallowest look: a couple of degrees below the horizon, so the skyline sits in frame.
pub const MIN_PITCH: f32 = 0.04;
const MAX_PITCH: f32 = 1.5607;

#[derive(Clone, Debug)]
pub struct Camera {
    /// Point on the ground the camera looks at.
    pub focus: Vec3,
    /// Eye distance from the focus. Zoom is exponential in this.
    pub distance: f32,
    /// Rotation around Z; zero looks along +Y.
    pub yaw: f32,
    /// Extra tilt the player added, radians; the base tilt follows the zoom.
    pub tilt: f32,
    pub viewport: Vec2,
    /// Map extent, for clamping and the maximum zoom.
    pub map_size: Vec2,
}

impl Camera {
    pub fn new(map_size: Vec2, viewport: Vec2) -> Camera {
        let mut c = Camera {
            focus: Vec3::new(map_size.x * 0.5, map_size.y * 0.5, 0.0),
            distance: 600.0,
            yaw: 0.0,
            tilt: 0.0,
            viewport,
            map_size,
        };
        c.distance = c.max_distance();
        c
    }

    /// Far enough to fit the whole map on screen, looking straight down.
    pub fn max_distance(&self) -> f32 {
        let aspect = (self.viewport.x / self.viewport.y.max(1.0)).max(0.1);
        let half_v = (FOV_Y * 0.5).tan();
        let fit_y = self.map_size.y * 0.5 / half_v;
        let fit_x = self.map_size.x * 0.5 / (half_v * aspect);
        fit_x.max(fit_y) * 1.06
    }

    fn zoom_t(&self) -> f32 {
        ((self.distance / MIN_DISTANCE).ln() / (self.max_distance() / MIN_DISTANCE).ln())
            .clamp(0.0, 1.0)
    }

    /// Angle below the horizon: shallow up close, top-down from orbit.
    pub fn pitch(&self) -> f32 {
        let t = self.zoom_t();
        let base = 0.75 + (MAX_PITCH - 0.75) * t * t;
        // Tilt keeps most of its bite when zoomed out, so Alt-orbit can still find the horizon.
        (base - self.tilt * (1.0 - 0.3 * t)).clamp(MIN_PITCH, MAX_PITCH)
    }

    /// Extra tilt that still changes the look. Past these the pitch clamp has already won.
    pub fn tilt_limits(&self) -> (f32, f32) {
        let t = self.zoom_t();
        let scale = (1.0 - 0.3 * t).max(1e-4);
        let base = 0.75 + (MAX_PITCH - 0.75) * t * t;
        (
            ((base - MAX_PITCH) / scale).max(MIN_TILT),
            ((base - MIN_PITCH) / scale).min(MAX_TILT),
        )
    }

    fn clamp_tilt(&mut self) {
        let (lo, hi) = self.tilt_limits();
        self.tilt = self.tilt.clamp(lo, hi);
    }

    pub fn eye(&self) -> Vec3 {
        let pitch = self.pitch();
        let back =
            Vec3::new(-self.yaw.sin(), -self.yaw.cos(), 0.0) * pitch.cos() + Vec3::Z * pitch.sin();
        self.focus + back * self.distance
    }

    pub fn view(&self) -> Mat4 {
        // Looking straight down makes Z a degenerate up vector; use the yaw heading instead.
        let up = if self.pitch() > 1.55 {
            Vec3::new(self.yaw.sin(), self.yaw.cos(), 0.0)
        } else {
            Vec3::Z
        };
        glam::camera::rh::view::look_at_mat4(self.eye(), self.focus, up)
    }

    /// Reversed-Z, infinite far plane: depth precision holds from 1 m to 100 km.
    pub fn projection(&self) -> Mat4 {
        let near = (self.distance * 0.02).clamp(0.5, 500.0);
        glam::camera::rh::proj::directx::perspective_infinite_reverse(
            FOV_Y,
            self.viewport.x / self.viewport.y.max(1.0),
            near,
        )
    }

    pub fn view_proj(&self) -> Mat4 {
        self.projection() * self.view()
    }

    /// Pixels covered by one metre at a distance of one metre; divide by distance for size on screen.
    pub fn projection_scale(&self) -> f32 {
        self.viewport.y * 0.5 / (FOV_Y * 0.5).tan()
    }

    /// Left, right, bottom, top and near planes, normalised, pointing inward.
    /// Entity cull uses the four sides only; near is left to the rasterizer.
    /// The sixth is unused.
    pub fn frustum(&self) -> [Vec4; 6] {
        let m = self.view_proj();
        let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
        let mut planes = [
            r3 + r0,
            r3 - r0,
            r3 + r1,
            r3 - r1,
            r3 - r2,
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        ];
        for p in planes.iter_mut().take(5) {
            let len = p.truncate().length();
            if len > 0.0 {
                *p /= len;
            }
        }
        planes
    }

    /// World-space ray through a pixel: origin and unit direction.
    pub fn ray(&self, pixel: Vec2) -> (Vec3, Vec3) {
        let ndc = Vec2::new(
            pixel.x / self.viewport.x * 2.0 - 1.0,
            1.0 - pixel.y / self.viewport.y * 2.0,
        );
        let inv = self.view_proj().inverse();
        let a = inv.project_point3(Vec3::new(ndc.x, ndc.y, 1.0));
        let b = inv.project_point3(Vec3::new(ndc.x, ndc.y, 0.01));
        (a, (b - a).normalize())
    }

    /// Pixel position of a world point, or `None` behind the camera.
    pub fn project(&self, world: Vec3) -> Option<Vec2> {
        let clip = self.view_proj() * world.extend(1.0);
        (clip.w > 0.0).then(|| {
            Vec2::new(
                (clip.x / clip.w * 0.5 + 0.5) * self.viewport.x,
                (0.5 - clip.y / clip.w * 0.5) * self.viewport.y,
            )
        })
    }

    /// Zooms by `factor`, keeping the ground point `anchor` under `pixel`,
    /// the way Supreme Commander does.
    pub fn zoom(&mut self, factor: f32, anchor: Option<(Vec3, Vec2)>) {
        let old = self.distance;
        self.distance = (self.distance * factor).clamp(MIN_DISTANCE, self.max_distance());
        self.clamp_tilt();
        if let Some((anchor, pixel)) = anchor {
            let k = self.distance / old;
            self.focus = anchor + (self.focus - anchor) * k;
            // The pitch follows the zoom, which scaling about the anchor does not
            // account for: slide the focus so the anchor is exactly under the pixel again.
            let (origin, dir) = self.ray(pixel);
            if dir.z < -0.05 {
                let hit = origin + dir * ((anchor.z - origin.z) / dir.z);
                let slide = (anchor - hit).truncate();
                if slide.is_finite() && slide.length() < self.distance {
                    self.focus += slide.extend(0.0);
                }
            }
        }
        self.clamp_focus();
    }

    /// Turns about the focus: `yaw` around Z, `tilt` the extra pitch the player added.
    pub fn orbit(&mut self, yaw: f32, tilt: f32) {
        self.yaw += yaw;
        self.tilt += tilt;
        self.clamp_tilt();
    }

    /// Pans by a screen-space delta in pixels.
    pub fn pan(&mut self, pixels: Vec2) {
        let metres_per_pixel = self.distance / self.projection_scale();
        let right = Vec3::new(self.yaw.cos(), -self.yaw.sin(), 0.0);
        let forward = Vec3::new(self.yaw.sin(), self.yaw.cos(), 0.0);
        self.focus += (right * -pixels.x + forward * pixels.y / self.pitch().sin().max(0.3))
            * metres_per_pixel;
        self.clamp_focus();
    }

    pub fn clamp_focus(&mut self) {
        self.focus.x = self.focus.x.clamp(0.0, self.map_size.x);
        self.focus.y = self.focus.y.clamp(0.0, self.map_size.y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_map_fits_at_max_zoom() {
        let mut cam = Camera::new(Vec2::splat(81_920.0), Vec2::new(2560.0, 1440.0));
        cam.distance = cam.max_distance();
        for corner in [
            Vec3::ZERO,
            Vec3::new(81_920.0, 81_920.0, 0.0),
            Vec3::new(0.0, 81_920.0, 0.0),
        ] {
            let p = cam.project(corner).expect("in front of the camera");
            assert!(
                p.x >= 0.0 && p.x <= 2560.0 && p.y >= 0.0 && p.y <= 1440.0,
                "{corner:?} -> {p:?}"
            );
        }
    }

    #[test]
    fn ray_passes_through_the_focus() {
        let mut cam = Camera::new(Vec2::splat(16_384.0), Vec2::new(1280.0, 720.0));
        cam.distance = 400.0;
        cam.yaw = 0.7;
        let (origin, dir) = cam.ray(Vec2::new(640.0, 360.0));
        let t = -origin.z / dir.z;
        let hit = origin + dir * t;
        assert!(
            (hit - cam.focus).length() < 0.5,
            "{hit:?} vs {:?}",
            cam.focus
        );
    }

    #[test]
    fn zoom_keeps_the_anchor_under_the_cursor() {
        let mut cam = Camera::new(Vec2::splat(16_384.0), Vec2::new(1280.0, 720.0));
        cam.distance = 3000.0;
        cam.yaw = 0.4;
        let pixel = Vec2::new(1000.0, 200.0);
        let (origin, dir) = cam.ray(pixel);
        let anchor = origin + dir * (-origin.z / dir.z);
        for _ in 0..30 {
            cam.zoom(0.9, Some((anchor, pixel)));
        }
        let p = cam.project(anchor).expect("in front of the camera");
        assert!(p.distance(pixel) < 1.5, "{p:?} vs {pixel:?}");
    }

    #[test]
    fn orbit_turns_around_the_focus() {
        let mut cam = Camera::new(Vec2::splat(16_384.0), Vec2::new(1280.0, 720.0));
        cam.distance = 400.0;
        cam.yaw = 0.4;
        let focus = cam.focus;
        let eye = cam.eye();
        cam.orbit(0.7, 0.2);
        assert!((cam.focus - focus).length() < 1e-5);
        assert!((cam.eye() - eye).length() > 10.0);
        let (origin, dir) = cam.ray(Vec2::new(640.0, 360.0));
        let hit = origin + dir * ((focus.z - origin.z) / dir.z);
        assert!((hit - focus).length() < 0.5, "{hit:?} vs {focus:?}");
        cam.orbit(0.0, 10.0);
        assert!(
            (cam.pitch() - MIN_PITCH).abs() < 1e-4,
            "pitch {} should sit on the horizon",
            cam.pitch()
        );
        cam.orbit(0.0, -10.0);
        assert!((cam.tilt - MIN_TILT).abs() < 1e-5);
    }

    #[test]
    fn max_tilt_reaches_the_horizon_from_play_height() {
        let mut cam = Camera::new(Vec2::splat(16_384.0), Vec2::new(1280.0, 720.0));
        cam.distance = 700.0;
        cam.orbit(0.0, MAX_TILT);
        assert!(
            (cam.pitch() - MIN_PITCH).abs() < 1e-4,
            "pitch {} should sit on the horizon clamp",
            cam.pitch()
        );
    }

    #[test]
    fn frustum_contains_the_focus() {
        let cam = Camera::new(Vec2::splat(16_384.0), Vec2::new(1280.0, 720.0));
        for p in cam.frustum().iter().take(5) {
            assert!(p.truncate().dot(cam.focus) + p.w > 0.0);
        }
    }

    #[test]
    fn frustum_sides_hold_what_the_camera_can_see() {
        let mut cam = Camera::new(Vec2::splat(16_384.0), Vec2::new(1280.0, 720.0));
        cam.distance = 400.0;
        cam.yaw = 0.4;
        let planes = cam.frustum();
        for pixel in [
            Vec2::new(40.0, 40.0),
            Vec2::new(1240.0, 40.0),
            Vec2::new(40.0, 680.0),
            Vec2::new(640.0, 360.0),
            Vec2::new(1240.0, 680.0),
        ] {
            let (origin, dir) = cam.ray(pixel);
            if dir.z >= -0.05 {
                continue;
            }
            let hit = origin + dir * (-origin.z / dir.z);
            for p in planes.iter().take(4) {
                assert!(
                    p.truncate().dot(hit) + p.w > -1.0,
                    "screen {pixel:?} ground {hit:?} is outside {p:?}"
                );
            }
        }
    }
}
