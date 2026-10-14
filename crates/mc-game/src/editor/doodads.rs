//! Doodad catalogue and editable component card.
use super::*;
pub(super) struct Controls {
    pub kind: String,
    scale: f32,
    tint: [u8; 3],
    blocking: bool,
    health: String,
    reclaim: String,
    footprint: [String; 3],
    custom: bool,
    components: bool,
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            kind: "CityBillboard".into(),
            scale: 1.0,
            tint: [255; 3],
            blocking: false,
            health: "250".into(),
            reclaim: "0".into(),
            footprint: ["5".into(), "2".into(), "12".into()],
            custom: true,
            components: false,
        }
    }
}
pub(super) fn catalogue(query: &str) -> Vec<(String, String)> {
    mc_map::PropKind::ALL
        .iter()
        .map(|k| format!("{k:?}"))
        .filter(|s| {
            query
                .split_whitespace()
                .all(|q| s.to_lowercase().contains(q))
        })
        .map(|s| (s.clone(), s))
        .collect()
}
impl Editor {
    pub(super) fn choose_doodad(&mut self, key: &str) -> Result<(), String> {
        let kind = *mc_map::PropKind::ALL
            .iter()
            .find(|k| format!("{k:?}").eq_ignore_ascii_case(key))
            .ok_or("Unknown doodad")?;
        self.doodad = Controls {
            kind: format!("{kind:?}"),
            ..Default::default()
        };
        if let Some(native) = mc_map::city::structure(kind).filter(|s| !s.plan.is_empty()) {
            self.doodad.custom = false;
            self.doodad.health = native.health.to_string();
            self.doodad.blocking = native.health > 0;
        }
        self.name = format!("{}_{}", self.doodad.kind, self.doc.placed.next_id);
        self.selected = None;
        self.tool = 27;
        self.tool_group = 2;
        self.catalogue = None;
        self.message = format!("{} selected. Click terrain to place it.", self.doodad.kind);
        Ok(())
    }
    pub(super) fn load_doodad(&mut self, id: u64) {
        let Some(d) = self.doc.placed.doodads.iter().find(|d| d.id == id) else {
            return;
        };
        let p = self.doc.map.props[d.prop_index as usize];
        self.doodad = Controls {
            kind: format!("{:?}", p.kind),
            scale: p.scale_milli as f32 / 1000.0,
            tint: d.tint,
            blocking: d.blocking,
            health: d.health.to_string(),
            reclaim: d.reclaim.to_string(),
            custom: d.footprint.is_some(),
            footprint: d
                .footprint
                .as_ref()
                .map_or(["5".into(), "2".into(), "12".into()], |f| {
                    [
                        f.half.x.to_f32().to_string(),
                        f.half.y.to_f32().to_string(),
                        f.height.to_f32().to_string(),
                    ]
                }),
            components: self.doodad.components,
        };
        self.heading = p.heading.to_radians_f32().to_degrees();
    }
    pub(super) fn doodad_command(&self, at: [f32; 2]) -> Result<Command, String> {
        let d = &self.doodad;
        let integer = |s: &str| {
            s.parse::<u32>()
                .map_err(|_| "Health and reclaim must be whole numbers".to_owned())
        };
        let footprint = if d.custom {
            let mut f = [0.0; 3];
            for (v, s) in f.iter_mut().zip(&d.footprint) {
                *v = s.parse().map_err(|_| "Footprint must contain metres")?;
            }
            Some(f)
        } else {
            None
        };
        Ok(Command::Doodad {
            name: self.name.clone(),
            kind: d.kind.clone(),
            at,
            heading: self.heading,
            scale: d.scale,
            tint: d.tint,
            blocking: d.blocking,
            health: integer(&d.health)?,
            reclaim: integer(&d.reclaim)?,
            footprint,
        })
    }
    pub(super) fn doodad_properties(
        &mut self,
        ui: &mut Ui,
        renderer: &mut mc_render::Renderer,
    ) -> Rect {
        let card = Rect::new(ui.size.x - 350.0, 100.0, 332.0, 388.0);
        ui.panel(card);
        ui.text(
            card.x + 16.0,
            card.y + 24.0,
            type_scale::CAPTION,
            rgb(palette::TEXT, 1.0),
            "DOODAD PROPERTIES",
        );
        ui.text_field(
            id("doodad_name", 0),
            Rect::new(card.x + 16.0, card.y + 38.0, 300.0, 30.0),
            &mut self.name,
            128,
        );
        if ui.button(
            id("doodad_browse", 0),
            Rect::new(card.x + 16.0, card.y + 76.0, 300.0, 30.0),
            &self.doodad.kind,
            ButtonKind::Secondary,
            true,
        ) {
            self.tool = 27;
            self.catalogue = Some(Default::default());
        }
        for (i, label) in ["Appearance", "Components"].iter().enumerate() {
            if ui.button(
                id("doodad_tab", i),
                Rect::new(
                    card.x + 16.0 + i as f32 * 154.0,
                    card.y + 114.0,
                    146.0,
                    28.0,
                ),
                label,
                if self.doodad.components == (i == 1) {
                    ButtonKind::Primary
                } else {
                    ButtonKind::Secondary
                },
                true,
            ) {
                self.doodad.components = i == 1;
            }
        }
        if self.doodad.components {
            self.doodad_components(ui, card);
        } else {
            let mut scale = (self.doodad.scale - 0.05) / 15.95;
            if ui.slider_compact(
                id("doodad_scale", 0),
                Rect::new(card.x + 16.0, card.y + 155.0, 300.0, 25.0),
                &format!("Scale: {:.2}", self.doodad.scale),
                &mut scale,
            ) {
                self.doodad.scale = 0.05 + 15.95 * scale;
            }
            let mut heading = self.heading / 360.0;
            if ui.slider_compact(
                id("doodad_heading", 0),
                Rect::new(card.x + 16.0, card.y + 192.0, 300.0, 25.0),
                &format!("Rotation: {:.0} degrees", self.heading),
                &mut heading,
            ) {
                self.heading = heading * 360.0;
            }
            for (i, label) in ["Red", "Green", "Blue"].iter().enumerate() {
                let mut value = self.doodad.tint[i] as f32 / 255.0;
                if ui.slider_compact(
                    id("doodad_tint", i),
                    Rect::new(card.x + 16.0, card.y + 229.0 + i as f32 * 33.0, 300.0, 24.0),
                    &format!("{}: {}", label, self.doodad.tint[i]),
                    &mut value,
                ) {
                    self.doodad.tint[i] = (value * 255.0).round() as u8;
                }
            }
        }
        let selected = self
            .selected
            .and_then(|id| self.doc.placed.doodads.iter().find(|d| d.id == id))
            .cloned();
        if ui.button(
            id("doodad_apply", 0),
            Rect::new(card.x + 16.0, card.y + 346.0, 195.0, 30.0),
            "Apply to selection",
            ButtonKind::Secondary,
            selected.is_some(),
        ) {
            if let Some(d) = &selected {
                let at = self.doc.map.props[d.prop_index as usize].pos.to_f32();
                let result = self.doodad_command(at).and_then(|c| {
                    self.apply_batch(
                        &[
                            Command::Properties {
                                id: d.id,
                                name: self.name.clone(),
                                group: None,
                                owner: None,
                                blueprint: None,
                            },
                            c,
                        ],
                        renderer,
                    )
                });
                if let Err(e) = result {
                    self.message = e;
                }
            }
        }
        if ui.button(
            id("doodad_delete", 0),
            Rect::new(card.x + 221.0, card.y + 346.0, 95.0, 30.0),
            "Delete",
            ButtonKind::Secondary,
            selected.is_some(),
        ) {
            if let Some(d) = selected {
                match self.apply(Command::Delete { id: d.id }, renderer) {
                    Ok(()) => self.selected = None,
                    Err(e) => self.message = e,
                }
            }
        }
        card
    }
    fn doodad_components(&mut self, ui: &mut Ui, card: Rect) {
        if ui.button(
            id("doodad_block", 0),
            Rect::new(card.x + 16.0, card.y + 150.0, 300.0, 28.0),
            if self.doodad.blocking {
                "Blocks movement: on"
            } else {
                "Blocks movement: off"
            },
            ButtonKind::Secondary,
            true,
        ) {
            self.doodad.blocking = !self.doodad.blocking;
        }
        for (i, (label, value)) in [
            ("Health (0 = invulnerable)", &mut self.doodad.health),
            ("Reclaim materials", &mut self.doodad.reclaim),
        ]
        .into_iter()
        .enumerate()
        {
            ui.text(
                card.x + 16.0 + i as f32 * 154.0,
                card.y + 196.0,
                type_scale::MICRO,
                rgb(palette::DIM, 1.0),
                label,
            );
            ui.text_field(
                id("doodad_value", i),
                Rect::new(
                    card.x + 16.0 + i as f32 * 154.0,
                    card.y + 204.0,
                    146.0,
                    28.0,
                ),
                value,
                7,
            );
        }
        if ui.button(
            id("doodad_custom", 0),
            Rect::new(card.x + 16.0, card.y + 240.0, 300.0, 28.0),
            if self.doodad.custom {
                "Custom footprint"
            } else {
                "Model footprint"
            },
            ButtonKind::Secondary,
            true,
        ) {
            self.doodad.custom = !self.doodad.custom;
        }
        if self.doodad.custom {
            for (i, label) in ["Half width", "Half depth", "Height"].iter().enumerate() {
                ui.text(
                    card.x + 16.0 + i as f32 * 102.0,
                    card.y + 288.0,
                    type_scale::MICRO,
                    rgb(palette::DIM, 1.0),
                    label,
                );
                ui.text_field(
                    id("doodad_box", i),
                    Rect::new(card.x + 16.0 + i as f32 * 102.0, card.y + 296.0, 94.0, 28.0),
                    &mut self.doodad.footprint[i],
                    8,
                );
            }
        }
    }
}
