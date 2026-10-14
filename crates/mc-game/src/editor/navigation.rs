//! Editor orbit and free camera. Navigation never changes the authored document.
use super::*;
use glam::Vec3;

pub(super) struct Navigation {
    orbit: Option<Camera>,
    speed: f32,
}
impl Default for Navigation {
    fn default() -> Self {
        Self {
            orbit: None,
            speed: 240.0,
        }
    }
}
impl Navigation {
    pub fn enter(&mut self, camera: &mut Camera) {
        if self.orbit.is_some() {
            return;
        }
        self.orbit = Some(camera.clone());
        let eye = camera.eye();
        let forward = (camera.focus - eye).normalize();
        camera.pitch_free = Some(camera.pitch());
        camera.distance = 25.0;
        camera.focus = eye + forward * camera.distance;
    }
    pub fn exit(&mut self, camera: &mut Camera) {
        if let Some(mut orbit) = self.orbit.take() {
            orbit.viewport = camera.viewport;
            *camera = orbit;
        }
    }
    pub fn toggle(&mut self, camera: &mut Camera) {
        if self.orbit.is_some() {
            self.exit(camera);
        } else {
            self.enter(camera);
        }
    }
    pub fn wheel(&mut self, camera: &mut Camera, scroll: f32) {
        if self.orbit.is_some() {
            self.speed = (self.speed * (scroll * 0.15).exp()).clamp(4.0, 4000.0);
        } else {
            camera.zoom((-scroll * 0.13).exp(), None);
        }
    }
    pub fn update(
        &mut self,
        camera: &mut Camera,
        held: &std::collections::BTreeSet<KeyCode>,
        dt: f32,
    ) {
        let axis = |a, b| (held.contains(&a) as i32 - held.contains(&b) as i32) as f32;
        let dt = dt.min(0.1);
        if let Some(pitch) = camera.pitch_free {
            let eye = camera.eye();
            camera.yaw += axis(KeyCode::ArrowRight, KeyCode::ArrowLeft) * dt;
            camera.pitch_free =
                Some((pitch + axis(KeyCode::ArrowDown, KeyCode::ArrowUp) * dt).clamp(-1.5, 1.5));
            let pitch = camera.pitch();
            let forward = Vec3::new(
                camera.yaw.sin() * pitch.cos(),
                camera.yaw.cos() * pitch.cos(),
                -pitch.sin(),
            );
            let right = Vec3::new(camera.yaw.cos(), -camera.yaw.sin(), 0.0);
            let movement = forward * axis(KeyCode::KeyW, KeyCode::KeyS)
                + right * axis(KeyCode::KeyD, KeyCode::KeyA)
                + Vec3::Z * axis(KeyCode::KeyE, KeyCode::KeyQ);
            let boost = if held.contains(&KeyCode::ShiftLeft) || held.contains(&KeyCode::ShiftRight)
            {
                4.0
            } else {
                1.0
            };
            camera.focus = eye
                + movement.clamp_length_max(1.0) * self.speed * boost * dt
                + forward * camera.distance;
        } else {
            camera.pan(
                Vec2::new(
                    axis(KeyCode::KeyA, KeyCode::KeyD),
                    axis(KeyCode::KeyW, KeyCode::KeyS),
                ) * dt
                    * 600.0,
            );
            camera.orbit(
                axis(KeyCode::KeyE, KeyCode::KeyQ) * dt,
                axis(KeyCode::PageUp, KeyCode::PageDown) * dt,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn free_camera_enters_without_jumping_and_returns_to_orbit() {
        let mut camera = Camera::new(Vec2::splat(12000.0), Vec2::new(1280.0, 720.0));
        camera.distance = 900.0;
        camera.yaw = 0.3;
        let original = camera.clone();
        let mut nav = Navigation::default();
        nav.enter(&mut camera);
        assert!(camera.eye().distance(original.eye()) < 0.001);
        let eye = camera.eye();
        nav.update(
            &mut camera,
            &[KeyCode::ArrowRight].into_iter().collect(),
            0.1,
        );
        assert!(
            camera.eye().distance(eye) < 0.001,
            "looking must not orbit the eye"
        );
        nav.update(&mut camera, &[KeyCode::KeyW].into_iter().collect(), 0.1);
        assert!(camera.eye().distance(eye) > 20.0);
        nav.exit(&mut camera);
        assert_eq!(camera.focus, original.focus);
        assert_eq!(camera.distance, original.distance);
        assert!(camera.pitch_free.is_none());
    }
}
