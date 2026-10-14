use super::*;
impl Editor {
    pub(super) fn save_as(&mut self, name: String) -> Result<(), String> {
        let previous = self.doc.destination.take();
        let previous_name = std::mem::replace(&mut self.save_name, name);
        if let Err(error) = self.save() {
            self.doc.destination = previous;
            self.save_name = previous_name;
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn pick(&mut self, cursor: Vec2, renderer: &mc_render::Renderer) {
        let mut nearest: Option<(f32, u64, String)> = None;
        let objects = self
            .doc
            .placed
            .units
            .iter()
            .map(|u| (u.id, &u.name, u.at))
            .chain(self.doc.placed.points.iter().map(|p| (p.id, &p.name, p.at)))
            .chain(
                self.doc
                    .placed
                    .cameras
                    .iter()
                    .map(|c| (c.id, &c.name, c.at)),
            )
            .chain(
                self.doc
                    .placed
                    .regions
                    .iter()
                    .map(|r| (r.id, &r.name, (r.min + r.max) * mc_core::Fx::HALF)),
            );
        for (id, name, at) in objects {
            let at = Vec2::from(at.to_f32());
            if let Some(screen) = self.camera.project(at.extend(renderer.ground_height(at))) {
                let distance = screen.distance(cursor);
                if distance < 35.0 && nearest.as_ref().is_none_or(|(best, _, _)| distance < *best) {
                    nearest = Some((distance, id, name.clone()));
                }
            }
        }
        self.selected = nearest.as_ref().map(|(_, id, _)| *id);
        if let Some((_, id, name)) = nearest {
            self.name = name;
            if let Some(u) = self.doc.placed.units.iter().find(|u| u.id == id) {
                self.blueprint = u.blueprint.clone();
                self.group = u.group.clone();
                self.owner = u.owner;
                self.heading = u.heading.to_radians_f32().to_degrees();
            }
            self.message = "Selected. Move places this object; heading rotates it.".into();
        }
    }

    pub(super) fn exit_dialog(&mut self, ui: &mut Ui, event: &mut Option<Event>) {
        if !self.confirm_exit {
            return;
        }
        let r = Rect::new(
            (ui.size.x - 500.0) * 0.5,
            (ui.size.y - 160.0) * 0.5,
            500.0,
            160.0,
        );
        ui.panel(r);
        ui.text(
            r.x + 20.0,
            r.y + 30.0,
            type_scale::TITLE,
            rgb(palette::TEXT, 1.0),
            "Unsaved map changes",
        );
        ui.text(
            r.x + 20.0,
            r.y + 62.0,
            type_scale::BODY,
            rgb(palette::DIM, 1.0),
            "Save this document before leaving?",
        );
        for (i, label) in ["Save & exit", "Discard", "Keep editing"]
            .iter()
            .enumerate()
        {
            if ui.button(
                id("editor_exit", i),
                Rect::new(r.x + 20.0 + i as f32 * 155.0, r.y + 95.0, 145.0, 40.0),
                label,
                ButtonKind::Secondary,
                true,
            ) {
                match i {
                    0 => match self.save() {
                        Ok(()) => *event = Some(Event::Leave),
                        Err(e) => {
                            self.message = e;
                            self.confirm_exit = false;
                        }
                    },
                    1 => *event = Some(Event::Leave),
                    _ => self.confirm_exit = false,
                }
            }
        }
    }
}
