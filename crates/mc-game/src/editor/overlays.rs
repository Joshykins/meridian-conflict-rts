//! World-space guides are drawn before panels so they cannot cross the controls.
use super::*;
impl Editor {
    pub(super) fn draw_regions(&self, ui: &mut Ui, renderer: &mc_render::Renderer) {
        if (22..=26).contains(&self.tool) {
            for wall in self.doc.config.walls().segments() {
                let a = Vec2::from(wall.a);
                let b = Vec2::from(wall.b);
                if let (Some(a), Some(b)) = (
                    self.camera.project(
                        a.extend(
                            renderer
                                .ground_height(a)
                                .max(self.doc.map.info.water_level.to_f32())
                                + 3.0,
                        ),
                    ),
                    self.camera.project(
                        b.extend(
                            renderer
                                .ground_height(b)
                                .max(self.doc.map.info.water_level.to_f32())
                                + 3.0,
                        ),
                    ),
                ) {
                    ui.o.line(a.to_array(), b.to_array(), 3.0, rgb(0xAA88FF, 1.0));
                }
            }
        }
        for region in &self.doc.placed.regions {
            let a = Vec2::from(region.min.to_f32());
            let b = Vec2::from(region.max.to_f32());
            let points = [a, Vec2::new(b.x, a.y), b, Vec2::new(a.x, b.y)];
            let projected: Vec<_> = points
                .iter()
                .filter_map(|p| {
                    self.camera
                        .project(p.extend(renderer.ground_height(*p) + 2.0))
                })
                .collect();
            let color = if self.selected == Some(region.id) {
                0xFFCD66
            } else {
                0x66DDBB
            };
            if projected.len() == 4 {
                for i in 0..4 {
                    ui.o.line(
                        projected[i].to_array(),
                        projected[(i + 1) % 4].to_array(),
                        2.0,
                        rgb(color, 1.0),
                    );
                }
            }
            self.marker(ui, renderer, (a + b) * 0.5, &region.name, color, false);
        }
        for point in &self.doc.placed.points {
            self.marker(
                ui,
                renderer,
                Vec2::from(point.at.to_f32()),
                &point.name,
                0xFFCD66,
                true,
            );
        }
        for camera in &self.doc.placed.cameras {
            self.marker(
                ui,
                renderer,
                Vec2::from(camera.at.to_f32()),
                &format!("Camera: {}", camera.name),
                0xA7BAFF,
                true,
            );
        }
        if let Some(unit) = self
            .selected
            .and_then(|id| self.doc.placed.units.iter().find(|u| u.id == id))
        {
            self.marker(
                ui,
                renderer,
                Vec2::from(unit.at.to_f32()),
                &unit.name,
                0xFFFFFF,
                true,
            );
        }
    }
    fn marker(
        &self,
        ui: &mut Ui,
        renderer: &mc_render::Renderer,
        at: Vec2,
        label: &str,
        color: u32,
        cross: bool,
    ) {
        let Some(p) = self
            .camera
            .project(at.extend(renderer.ground_height(at) + 3.0))
        else {
            return;
        };
        let p = p / ui.s;
        if p.x < 0.0 || p.y < 0.0 || p.x > ui.size.x || p.y > ui.size.y {
            return;
        }
        if cross {
            for direction in [Vec2::X, Vec2::Y] {
                ui.o.line(
                    ((p - direction * 7.0) * ui.s).to_array(),
                    ((p + direction * 7.0) * ui.s).to_array(),
                    2.0,
                    rgb(color, 1.0),
                );
            }
        }
        let width = ui.text_width(type_scale::CAPTION, label);
        ui.fill(
            Rect::new(p.x + 9.0, p.y - 12.0, width + 10.0, 24.0),
            crate::ui::ink(0.8),
        );
        ui.text(
            p.x + 14.0,
            p.y + 5.0,
            type_scale::CAPTION,
            rgb(color, 1.0),
            label,
        );
    }
}
