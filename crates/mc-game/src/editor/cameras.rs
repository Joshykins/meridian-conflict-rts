use super::*;
impl Editor {
    pub(super) fn capture_view(&self, name: String) -> Command {
        Command::Camera {
            name,
            at: self.camera.focus.truncate().to_array(),
            distance: self.camera.distance.round().clamp(25.0, 300_000.0) as u32,
            yaw: self.camera.yaw.to_degrees(),
            height: Some(self.camera.focus.z),
            pitch: Some(self.camera.pitch().to_degrees()),
            free: self.camera.pitch_free.is_some(),
        }
    }
    pub(super) fn recall_view(
        &mut self,
        name: &str,
        renderer: &mc_render::Renderer,
    ) -> Result<(), String> {
        let view = self
            .doc
            .placed
            .cameras
            .iter()
            .find(|c| c.name == name)
            .cloned()
            .ok_or("Unknown saved camera")?;
        let at = Vec2::from(view.at.to_f32());
        self.navigation
            .recall(&mut self.camera, &view, renderer.ground_height(at));
        self.selected = Some(view.id);
        self.name = view.name;
        Ok(())
    }
}
impl navigation::Navigation {
    pub(super) fn recall(
        &mut self,
        camera: &mut Camera,
        view: &mc_map::placed::Camera,
        ground: f32,
    ) {
        if view.pose.as_ref().is_some_and(|p| p.free) {
            self.enter(camera);
        } else {
            self.exit(camera);
        }
        camera.focus = Vec2::from(view.at.to_f32())
            .extend(view.pose.as_ref().map_or(ground, |p| p.height.to_f32()));
        camera.distance = view.distance as f32;
        camera.yaw = view.yaw.to_radians_f32();
        camera.pitch_free = None;
        if let Some(pose) = &view.pose {
            let pitch = (pose.pitch_centideg as f32 * 0.01).to_radians();
            if pose.free {
                camera.pitch_free = Some(pitch);
            } else {
                camera.tilt_to(pitch);
            }
        } else {
            camera.tilt = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_save_reload_and_recall_preserve_free_and_orbit_views() {
        let (folder, mut doc, bp) = super::super::document::tests::fixture();
        let mut camera = Camera::new(Vec2::splat(4096.0), Vec2::new(1280.0, 720.0));
        let mut navigation = navigation::Navigation::default();
        for free in [false, true] {
            camera.focus = glam::Vec3::new(512.0, 768.0, 120.0);
            camera.distance = if free { 25.0 } else { 1200.0 };
            camera.yaw = 0.4;
            camera.pitch_free = free.then_some(0.15);
            if !free {
                camera.tilt_to(0.6);
            }
            let before = camera.clone();
            doc.apply(
                &[Command::Camera {
                    name: "view".into(),
                    at: [512.0, 768.0],
                    distance: camera.distance as u32,
                    yaw: camera.yaw.to_degrees(),
                    height: Some(camera.focus.z),
                    pitch: Some(camera.pitch().to_degrees()),
                    free,
                }],
                &bp,
            )
            .unwrap();
            let saved = folder.join(if free { "free" } else { "orbit" });
            doc.save(&saved).unwrap();
            let loaded = Document::open(&saved.join("terrain.mcmap")).unwrap();
            let view = &loaded.placed.cameras[0];
            camera.focus = glam::Vec3::ZERO;
            camera.distance = 3000.0;
            camera.pitch_free = None;
            navigation.recall(&mut camera, view, 0.0);
            assert_eq!(camera.focus, before.focus);
            assert!(camera.eye().distance(before.eye()) < 0.2);
            assert_eq!(camera.pitch_free.is_some(), free);
            let id = view.id;
            doc.apply(
                &[Command::Camera {
                    name: "view".into(),
                    at: [500.0, 500.0],
                    distance: 500,
                    yaw: 0.0,
                    height: None,
                    pitch: None,
                    free: false,
                }],
                &bp,
            )
            .unwrap();
            assert_eq!(doc.placed.cameras[0].id, id);
            doc.undo().unwrap();
            assert_eq!(&doc.placed.cameras[0], view);
        }
        std::fs::remove_dir_all(folder).unwrap();
    }
}
