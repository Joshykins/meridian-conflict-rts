//! Cinematic interpolation is presentation-only; authored poses use integers.
use mc_render::Camera;
pub(super) struct Pan {
    start: Camera,
    elapsed: f32,
}
impl Pan {
    pub fn new(camera: &Camera) -> Self {
        Self {
            start: camera.clone(),
            elapsed: 0.0,
        }
    }
    pub fn advance(
        &mut self,
        camera: &mut Camera,
        target: &mc_map::placed::Camera,
        ground: f32,
        dt: f32,
    ) -> bool {
        self.elapsed += dt;
        let t = (self.elapsed / 1.4).clamp(0.0, 1.0);
        let ease = t * t * (3.0 - 2.0 * t);
        let mut goal = self.start.clone();
        goal.focus = glam::Vec2::from(target.at.to_f32())
            .extend(target.pose.as_ref().map_or(ground, |p| p.height.to_f32()));
        goal.distance = target.distance as f32;
        goal.yaw = target.yaw.to_radians_f32();
        goal.pitch_free = None;
        goal.tilt = 0.0;
        if let Some(p) = &target.pose {
            let pitch = (p.pitch_centideg as f32 * 0.01).to_radians();
            if p.free {
                goal.pitch_free = Some(pitch);
            } else {
                goal.tilt_to(pitch);
            }
        }
        camera.focus = self.start.focus.lerp(goal.focus, ease);
        camera.distance = self.start.distance + (goal.distance - self.start.distance) * ease;
        let delta = (goal.yaw - self.start.yaw + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        camera.yaw = self.start.yaw + delta * ease;
        camera.pitch_free = Some(self.start.pitch() + (goal.pitch() - self.start.pitch()) * ease);
        if t >= 1.0 {
            camera.pitch_free = goal.pitch_free;
            camera.tilt = goal.tilt;
        }
        t >= 1.0
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cinematic_reaches_authored_altitude_pitch_and_shortest_yaw() {
        let mut camera = Camera::new(glam::Vec2::splat(16000.0), glam::Vec2::new(1280.0, 720.0));
        camera.yaw = 350.0f32.to_radians();
        let mut pan = Pan::new(&camera);
        let mut target = mc_map::placed::Camera {
            id: 1,
            name: "street".into(),
            at: mc_core::FxVec2::from_ints(4000, 5000),
            distance: 75,
            yaw: mc_core::Angle::from_radians_f32(10.0f32.to_radians()),
            pose: Some(mc_map::placed::CameraPose {
                height: mc_core::Fx::from_int(120),
                pitch_centideg: 1500,
                free: true,
            }),
        };
        assert!(!pan.advance(&mut camera, &target, 0.0, 0.7));
        assert!(
            camera.yaw.to_degrees() > 349.0,
            "pan must cross north rather than circle the map"
        );
        assert!(pan.advance(&mut camera, &target, 0.0, 0.7));
        assert_eq!(camera.focus, glam::Vec3::new(4000.0, 5000.0, 120.0));
        assert!((camera.pitch().to_degrees() - 15.0).abs() < 0.001);
        target.pose = None;
        let mut pan = Pan::new(&camera);
        assert!(pan.advance(&mut camera, &target, 25.0, 1.4));
        assert_eq!(camera.focus.z, 25.0);
        assert!(camera.pitch_free.is_none());
    }
}
