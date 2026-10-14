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
        let mut x = w - 810.0;
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
                    1 | 2 => {
                        let result = if i == 1 {
                            self.doc.undo()
                        } else {
                            self.doc.redo()
                        };
                        result.and_then(|rect| {
                            if let Some(rect) = rect {
                                renderer
                                    .edit_terrain(&self.doc.map, rect)
                                    .map_err(|e| e.to_string())?;
                            }
                            self.refresh_units()
                        })
                    }
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
    pub(super) fn tools_panel(&mut self, ui: &mut Ui, bottom: f32) {
        let w = ui.size.x;
        ui.panel(Rect::new(18.0, bottom, w - 36.0, 162.0));
        ui.text(
            34.0,
            bottom + 23.0,
            type_scale::CAPTION,
            rgb(palette::DIM, 1.0),
            "AUTHORING TOOLS",
        );
        for (i, label) in [
            "Select", "Raise", "Lower", "Flatten", "Smooth", "Unit", "Region", "Point", "Camera",
            "Move",
        ]
        .iter()
        .enumerate()
        {
            let kind = if self.tool == i {
                ButtonKind::Primary
            } else {
                ButtonKind::Secondary
            };
            if ui.button(
                id("editor_tool", i),
                Rect::new(34.0 + i as f32 * 112.0, bottom + 42.0, 102.0, 40.0),
                label,
                kind,
                true,
            ) {
                self.tool = i;
                self.region_start = None;
            }
        }
        let mut radius = (self.radius - 8.0) / 504.0;
        if ui.slider(
            id("editor_radius", 0),
            Rect::new(34.0, bottom + 96.0, 450.0, 28.0),
            "Brush radius",
            &mut radius,
        ) {
            self.radius = 8.0 + radius * 504.0;
        }
        let mut strength = self.strength / 64.0;
        if ui.slider(
            id("editor_strength", 0),
            Rect::new(510.0, bottom + 96.0, 420.0, 28.0),
            "Strength",
            &mut strength,
        ) {
            self.strength = strength * 64.0;
        }
        ui.text(
            970.0,
            bottom + 114.0,
            type_scale::BODY,
            rgb(palette::DIM, 1.0),
            "WASD pan  |  Q/E orbit  |  Wheel zoom",
        );
        ui.text_fit_left(
            34.0,
            bottom + 131.0,
            w - 68.0,
            type_scale::BODY,
            rgb(palette::TEXT, 1.0),
            &self.message,
        );
    }
    pub(super) fn properties(&mut self, ui: &mut Ui, renderer: &mut mc_render::Renderer) -> Rect {
        let w = ui.size.x;
        let card = Rect::new(w - 350.0, 100.0, 332.0, 370.0);
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
            ("Blueprint", &mut self.blueprint),
            ("Group", &mut self.group),
            ("Save name", &mut self.save_name),
        ]
        .into_iter()
        .enumerate()
        {
            let y = card.y + 47.0 + i as f32 * 62.0;
            ui.text(
                card.x + 16.0,
                y,
                type_scale::MICRO,
                rgb(palette::DIM, 1.0),
                label,
            );
            ui.text_field(
                id("editor_field", i),
                Rect::new(card.x + 16.0, y + 9.0, 300.0, 34.0),
                value,
                128,
            );
        }
        if ui.button(
            id("editor_owner", 0),
            Rect::new(card.x + 16.0, card.y + 300.0, 190.0, 34.0),
            &format!("Player slot: {}", self.owner + 1),
            ButtonKind::Secondary,
            true,
        ) {
            self.owner = (self.owner + 1) % 6;
        }
        if ui.button(
            id("editor_delete", 0),
            Rect::new(card.x + 218.0, card.y + 300.0, 96.0, 34.0),
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
            Rect::new(card.x + 16.0, card.y + 338.0, 298.0, 27.0),
            &format!("Heading: {:.0}°  (+15°)", self.heading),
            ButtonKind::Secondary,
            true,
        ) {
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

        card
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
    pub(super) fn world_input(
        &mut self,
        ui: &mut Ui,
        renderer: &mut mc_render::Renderer,
        bottom: f32,
        card: Rect,
        outliner: Rect,
    ) {
        let input = ui.input;
        let viewport = ui.cursor.y > 90.0
            && ui.cursor.y < bottom
            && !card.contains(ui.cursor)
            && !outliner.contains(ui.cursor);
        if viewport && !self.confirm_exit {
            if input.scroll != 0.0 {
                self.camera.zoom((-input.scroll * 0.13).exp(), None);
            }
            let (origin, dir) = self.camera.ray(input.cursor);
            if let Some(hit) = renderer.pick_ground(origin, dir) {
                if input.pressed {
                    let at = [hit.x, hit.y];
                    let command = match self.tool {
                        1..=4 => Some(Command::Terrain {
                            at,
                            radius: self.radius,
                            strength: self.strength,
                            brush: [Brush::Raise, Brush::Lower, Brush::Flatten, Brush::Smooth]
                                [self.tool - 1],
                        }),
                        5 => Some(Command::Unit {
                            name: self.name.clone(),
                            group: self.group.clone(),
                            blueprint: self.blueprint.clone(),
                            owner: self.owner,
                            at,
                            heading: self.heading,
                        }),
                        6 => match self.region_start.take() {
                            None => {
                                self.region_start = Some(at);
                                self.message = "Click the opposite corner of the region.".into();
                                None
                            }
                            Some(a) => Some(Command::Region {
                                name: self.name.clone(),
                                min: [a[0].min(at[0]), a[1].min(at[1])],
                                max: [a[0].max(at[0]), a[1].max(at[1])],
                            }),
                        },
                        7 => Some(Command::Point {
                            name: self.name.clone(),
                            at,
                        }),
                        8 => Some(Command::Camera {
                            name: self.name.clone(),
                            at,
                            distance: self.camera.distance.round().clamp(25.0, 300_000.0) as u32,
                            yaw: self.camera.yaw.to_degrees(),
                        }),
                        9 => self.selected.map(|id| Command::Transform {
                            id,
                            at,
                            heading: self.heading,
                        }),
                        _ => {
                            self.pick(input.cursor, renderer);
                            None
                        }
                    };
                    if let Some(command) = command {
                        if let Err(e) = self.apply(command, renderer) {
                            self.message = e;
                        }
                    }
                }
            }
        }
    }
    pub(super) fn draw_regions(&self, ui: &mut Ui, renderer: &mc_render::Renderer) {
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
            if projected.len() == 4 {
                for i in 0..4 {
                    let a = projected[i] / ui.s;
                    let b = projected[(i + 1) % 4] / ui.s;
                    ui.o.line(
                        (a * ui.s).to_array(),
                        (b * ui.s).to_array(),
                        2.0,
                        rgb(0x66DDBB, 1.0),
                    );
                }
            }
        }
    }
}
