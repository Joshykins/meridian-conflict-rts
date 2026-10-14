//! Localized captions make authored propaganda readable at tactical distance.
use super::*;
impl Presentation {
    pub(crate) fn signs(
        &self,
        ui: &mut Ui,
        map: &mc_map::MapFile,
        camera: &mc_render::Camera,
        renderer: &mc_render::Renderer,
        frame: &mc_sim::RenderFrame,
    ) {
        if camera.distance > 2200.0 {
            return;
        }
        let mut occupied: Vec<Rect> = Vec::new();
        for d in &map.placed().doodads {
            let Some(text) = self.strings.get(&format!("prop.{}", d.name)) else {
                continue;
            };
            let index = d.prop_index as usize;
            if frame
                .props_dead
                .get(index / 32)
                .is_some_and(|bits| bits & (1 << (index % 32)) != 0)
            {
                continue;
            }
            let Some(prop) = map.props().get(index) else {
                continue;
            };
            let at = glam::Vec2::from(prop.pos.to_f32());
            if !crate::titan_marks::seen(frame, at) {
                continue;
            }
            let Some(p) = camera.project(at.extend(renderer.ground_height(at) + 25.0)) else {
                continue;
            };
            let p = p / ui.s;
            if p.x < 380.0 || p.x > ui.size.x - 350.0 || p.y < 110.0 || p.y > ui.size.y - 300.0 {
                continue;
            }
            let width = ui.text_width(type_scale::MICRO, &text.text).min(360.0) + 20.0;
            let rect = Rect::new(p.x - width * 0.5, p.y - 28.0, width, 28.0);
            if occupied.iter().any(|r| {
                rect.x < r.x + r.w
                    && rect.x + rect.w > r.x
                    && rect.y < r.y + r.h
                    && rect.y + rect.h > r.y
            }) {
                continue;
            }
            occupied.push(rect);
            ui.fill(rect, crate::ui::ink(0.82));
            ui.text_fit_left(
                rect.x + 10.0,
                rect.y + 18.0,
                width - 20.0,
                type_scale::MICRO,
                rgb(0xF1D49A, 1.0),
                &text.text,
            );
        }
    }
}
