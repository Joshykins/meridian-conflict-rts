use super::*;
impl Editor {
    pub(super) fn top_bar(
        &mut self,
        ui: &mut Ui,
        renderer: &mut mc_render::Renderer,
        event: &mut Option<Event>,
    ) {
        let w = ui.size.x;
        ui.panel(Rect::new(18.0, 18.0, w - 36.0, 64.0));
        ui.text(
            34.0,
            42.0,
            type_scale::TITLE,
            rgb(palette::TEXT, 1.0),
            "MAP EDITOR",
        );
        if w > 1450.0 {
            ui.text(
                250.0,
                43.0,
                type_scale::BODY,
                rgb(palette::DIM, 1.0),
                &format!(
                    "{}{}",
                    self.doc.map.info.name,
                    if self.doc.dirty { " *" } else { "" }
                ),
            );
        }
        let mut x = (w - 810.0).max(230.0);
        for (i, label) in ["Save", "Undo", "Redo", "Test", "Save As", "Open", "Exit"]
            .iter()
            .enumerate()
        {
            let enabled = match i {
                1 => self.doc.can_undo(),
                2 => self.doc.can_redo(),
                _ => true,
            };
            if ui.button(
                id("editor_top", i),
                Rect::new(x, 31.0, 98.0, 36.0),
                label,
                ButtonKind::Secondary,
                enabled,
            ) {
                let result = match i {
                    0 => self.save(),
                    1 | 2 => self.history(i == 2, renderer),
                    3 => self
                        .test()
                        .map(|start| *event = Some(Event::Test(Box::new(start)))),
                    4 => self.save_as(self.save_name.clone()),
                    5 => self.open_browser(),
                    _ => {
                        if self.doc.dirty {
                            self.confirm_exit = true;
                        } else {
                            *event = Some(Event::Leave);
                        }
                        Ok(())
                    }
                };
                if let Err(e) = result {
                    self.message = e;
                }
            }
            x += 108.0;
        }
    }
    pub(super) fn properties(&mut self, ui: &mut Ui, renderer: &mut mc_render::Renderer) -> Rect {
        if self.tool == 28 {
            return self.map_properties(ui, renderer);
        }
        if self.tool == 27
            || (matches!(self.tool, 0 | 9)
                && self
                    .selected
                    .is_some_and(|id| self.doc.placed.doodads.iter().any(|d| d.id == id)))
        {
            return self.doodad_properties(ui, renderer);
        }
        if (22..=26).contains(&self.tool) {
            return self.environment_properties(ui, renderer);
        }
        if (17..=21).contains(&self.tool) {
            return self.paint_properties(ui);
        }
        let w = ui.size.x;
        let card = Rect::new(w - 350.0, 100.0, 332.0, 388.0);
        ui.panel(card);
        ui.text(
            card.x + 16.0,
            card.y + 24.0,
            type_scale::CAPTION,
            rgb(palette::TEXT, 1.0),
            "OBJECT PROPERTIES",
        );
        for (i, (label, value)) in [
            ("Name", &mut self.name),
            if self.tool == 16 {
                ("Heightmap file", &mut self.stamp_asset)
            } else {
                ("Blueprint", &mut self.blueprint)
            },
            ("Group", &mut self.group),
            ("Save name", &mut self.save_name),
        ]
        .into_iter()
        .enumerate()
        {
            let y = card.y + 47.0 + i as f32 * 56.0;
            ui.text(
                card.x + 16.0,
                y,
                type_scale::MICRO,
                rgb(palette::DIM, 1.0),
                label,
            );
            ui.text_field(
                id("editor_field", i),
                Rect::new(
                    card.x + 16.0,
                    y + 9.0,
                    if i == 1 { 210.0 } else { 300.0 },
                    34.0,
                ),
                value,
                128,
            );
        }
        if ui.button(
            id("editor_browse_units", 0),
            Rect::new(card.x + 232.0, card.y + 118.0, 82.0, 34.0),
            "Browse",
            ButtonKind::Secondary,
            self.tool != 16,
        ) {
            self.catalogue = Some(Default::default());
        }
        self.property_actions(ui, card, renderer);
        card
    }
    fn property_actions(&mut self, ui: &mut Ui, card: Rect, renderer: &mut mc_render::Renderer) {
        if ui.button(
            id("editor_owner", 0),
            Rect::new(card.x + 16.0, card.y + 280.0, 190.0, 34.0),
            &if self.tool == 8 {
                "Capture current view".into()
            } else {
                format!("Player slot: {}", self.owner + 1)
            },
            ButtonKind::Secondary,
            true,
        ) {
            if self.tool == 8 {
                if let Err(error) = self.apply(self.capture_view(self.name.clone()), renderer) {
                    self.message = error;
                }
            } else {
                self.owner = (self.owner + 1) % mc_core::MAX_PLAYERS as u8;
            }
        }
        if ui.button(
            id("editor_delete", 0),
            Rect::new(card.x + 218.0, card.y + 280.0, 96.0, 34.0),
            "Delete",
            ButtonKind::Secondary,
            self.selected.is_some(),
        ) {
            if let Some(id) = self.selected.take() {
                if let Err(e) = self.apply(Command::Delete { id }, renderer) {
                    self.message = e;
                }
            }
        }
        if ui.button(
            id("editor_rotate", 0),
            Rect::new(card.x + 16.0, card.y + 318.0, 298.0, 27.0),
            &if self
                .doc
                .placed
                .cameras
                .iter()
                .any(|c| Some(c.id) == self.selected)
            {
                "View saved camera".into()
            } else {
                format!("Heading: {:.0}°  (+15°)", self.heading)
            },
            ButtonKind::Secondary,
            true,
        ) {
            if let Some(name) = self
                .doc
                .placed
                .cameras
                .iter()
                .find(|c| Some(c.id) == self.selected)
                .map(|c| c.name.clone())
            {
                if let Err(error) = self.recall_view(&name, renderer) {
                    self.message = error;
                }
            } else {
                self.heading = (self.heading + 15.0) % 360.0;
                if let Some(unit) = self
                    .selected
                    .and_then(|id| self.doc.placed.units.iter().find(|u| u.id == id))
                    .cloned()
                {
                    if let Err(e) = self.apply(
                        Command::Transform {
                            id: unit.id,
                            at: unit.at.to_f32(),
                            heading: self.heading,
                        },
                        renderer,
                    ) {
                        self.message = e;
                    }
                }
            }
        }

        if ui.button(
            id("editor_apply_properties", 0),
            Rect::new(card.x + 16.0, card.y + 350.0, 298.0, 30.0),
            "Apply to selection",
            ButtonKind::Secondary,
            self.selected.is_some(),
        ) {
            if let Some(id) = self.selected {
                let unit = self.doc.placed.units.iter().any(|u| u.id == id);
                if let Err(e) = self.apply(
                    Command::Properties {
                        id,
                        name: self.name.clone(),
                        group: unit.then(|| self.group.clone()),
                        owner: unit.then_some(self.owner),
                        blueprint: unit.then(|| self.blueprint.clone()),
                    },
                    renderer,
                ) {
                    self.message = e;
                }
            }
        }
    }
    pub(super) fn outliner(&mut self, ui: &mut Ui, bottom: f32) -> Rect {
        let input = ui.input;
        let outliner = Rect::new(18.0, 100.0, 260.0, (bottom - 120.0).min(550.0));
        ui.panel(outliner);
        ui.text(
            34.0,
            125.0,
            type_scale::CAPTION,
            rgb(palette::TEXT, 1.0),
            "PLACED OBJECTS",
        );
        let entries: Vec<_> = self
            .doc
            .placed
            .units
            .iter()
            .map(|u| (u.id, u.name.clone(), u.at))
            .chain(
                self.doc
                    .placed
                    .regions
                    .iter()
                    .map(|r| (r.id, r.name.clone(), (r.min + r.max) * mc_core::Fx::HALF)),
            )
            .chain(
                self.doc
                    .placed
                    .points
                    .iter()
                    .map(|p| (p.id, p.name.clone(), p.at)),
            )
            .chain(
                self.doc
                    .placed
                    .cameras
                    .iter()
                    .map(|c| (c.id, c.name.clone(), c.at)),
            )
            .chain(self.doc.placed.doodads.iter().map(|d| {
                (
                    d.id,
                    d.name.clone(),
                    self.doc.map.props[d.prop_index as usize].pos,
                )
            }))
            .collect();
        let rows = ((outliner.h - 48.0) / 34.0).max(0.0) as usize;
        if outliner.contains(ui.cursor) && input.scroll != 0.0 {
            self.object_scroll = (self.object_scroll as i32 - input.scroll.signum() as i32)
                .clamp(0, entries.len().saturating_sub(rows) as i32)
                as usize;
        }
        self.object_scroll = self.object_scroll.min(entries.len().saturating_sub(rows));
        for (row, (object, name, at)) in entries
            .iter()
            .skip(self.object_scroll)
            .take(rows)
            .enumerate()
        {
            if ui.button(
                id("editor_object", *object as usize),
                Rect::new(28.0, 140.0 + row as f32 * 34.0, 240.0, 30.0),
                name,
                ButtonKind::Secondary,
                true,
            ) {
                self.selected = Some(*object);
                self.load_doodad(*object);
                self.name = name.clone();
                self.camera.focus = Vec2::from(at.to_f32()).extend(0.0);
                if let Some(u) = self.doc.placed.units.iter().find(|u| u.id == *object) {
                    self.blueprint = u.blueprint.clone();
                    self.group = u.group.clone();
                    self.owner = u.owner;
                    self.heading = u.heading.to_radians_f32().to_degrees();
                }
            }
        }

        outliner
    }
}
