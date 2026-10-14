//! Live environmental controls; every button submits the shared document commands.
use super::*;
use mc_data::weather::{Climate, TimeOfDay, WeatherPreset};
#[derive(Default)]
pub(super) struct Controls {
    pub region: usize,
    pub right: usize,
}
fn button(ui: &mut Ui, card: Rect, row: usize, column: usize, label: &str, enabled: bool) -> bool {
    ui.button(
        id("editor_environment", row * 2 + column),
        Rect::new(
            card.x + 16.0 + column as f32 * 154.0,
            card.y + 104.0 + row as f32 * 44.0,
            146.0,
            36.0,
        ),
        label,
        ButtonKind::Secondary,
        enabled,
    )
}
impl Editor {
    pub(super) fn apply_environment(&self, renderer: &mut mc_render::Renderer) {
        renderer.edit_environment(&self.doc.config);
    }
    pub(super) fn environment_properties(
        &mut self,
        ui: &mut Ui,
        renderer: &mut mc_render::Renderer,
    ) -> Rect {
        let card = Rect::new(ui.size.x - 350.0, 100.0, 332.0, 388.0);
        ui.panel(card);
        ui.text(
            card.x + 16.0,
            card.y + 26.0,
            type_scale::CAPTION,
            rgb(palette::TEXT, 1.0),
            "ENVIRONMENT",
        );
        let n = self.doc.config.regions().len();
        self.environment.region = self.environment.region.min(n - 1);
        self.environment.right = self.environment.right.min(n - 1);
        if n > 1 && self.environment.right == self.environment.region {
            self.environment.right = (self.environment.region + 1) % n;
        }
        let region = &self.doc.config.regions()[self.environment.region];
        let label = if region.name.is_empty() {
            "Whole map".into()
        } else {
            format!("{}: {}", self.environment.region + 1, region.name)
        };
        if ui.button(
            id("editor_environment_region", 0),
            Rect::new(card.x + 16.0, card.y + 48.0, 300.0, 36.0),
            &label,
            ButtonKind::Secondary,
            n > 1,
        ) {
            self.environment.region = (self.environment.region + 1) % n;
        }
        let command = match self.tool {
            22 => self.water_controls(ui, card),
            23 => self.climate_controls(ui, card),
            24 => self.weather_controls(ui, card),
            25 => self.time_controls(ui, card),
            _ => self.wall_controls(ui, card),
        };
        if let Some(command) = command {
            if let Err(error) = self.apply(command, renderer) {
                self.message = error;
            }
        }
        card
    }
    fn water_controls(&self, ui: &mut Ui, card: Rect) -> Option<Command> {
        let level = self.doc.map.info.water_level.to_f32();
        ui.text(
            card.x + 16.0,
            card.y + 126.0,
            type_scale::BODY,
            rgb(palette::TEXT, 1.0),
            &format!("Sea level: {level:.1} m"),
        );
        for (row, step) in [1.0, 10.0, 100.0].into_iter().enumerate() {
            for (column, delta) in [-step, step].into_iter().enumerate() {
                if button(
                    ui,
                    card,
                    row + 1,
                    column,
                    &format!("{delta:+.0} metres"),
                    true,
                ) {
                    return Some(Command::Water {
                        level: level + delta,
                    });
                }
            }
        }
        None
    }
    fn time_controls(&self, ui: &mut Ui, card: Rect) -> Option<Command> {
        let hour = self.doc.config.hour(None);
        ui.text(
            card.x + 16.0,
            card.y + 126.0,
            type_scale::BODY,
            rgb(palette::TEXT, 1.0),
            &format!(
                "Time: {:02}:{:02}",
                hour as u32,
                ((hour.fract() * 60.0).round() as u32).min(59)
            ),
        );
        for (i, time) in TimeOfDay::ALL.into_iter().enumerate() {
            if button(ui, card, 1 + i / 2, i % 2, time.label(), true) {
                return Some(Command::Time { hour: time.hour() });
            }
        }
        for (column, delta) in [-0.25, 0.25].into_iter().enumerate() {
            if button(
                ui,
                card,
                4,
                column,
                if delta < 0.0 {
                    "-15 minutes"
                } else {
                    "+15 minutes"
                },
                true,
            ) {
                return Some(Command::Time {
                    hour: (hour + delta).rem_euclid(24.0),
                });
            }
        }
        None
    }
    fn climate_controls(&mut self, ui: &mut Ui, card: Rect) -> Option<Command> {
        for (i, climate) in [
            Climate::Temperate,
            Climate::Tropical,
            Climate::Desert,
            Climate::Polar,
            Climate::Autumn,
        ]
        .into_iter()
        .enumerate()
        {
            if button(ui, card, i / 2, i % 2, &format!("{climate:?}"), true) {
                return Some(Command::Climate {
                    region: self.environment.region,
                    value: climate,
                });
            }
        }
        if button(
            ui,
            card,
            3,
            0,
            "Add region",
            self.doc.config.regions().len() < mc_data::regions::MAX_REGIONS,
        ) {
            let mut regions = self.doc.config.regions().to_vec();
            if regions[0].name.is_empty() {
                regions[0].name = "Region 1".into();
            }
            regions.push(mc_data::regions::Region {
                name: format!("Region {}", regions.len() + 1),
                ..Default::default()
            });
            self.environment.right = regions.len() - 1;
            return Some(Command::ClimateRegions {
                regions,
                walls: self.doc.config.walls().walls().to_vec(),
                blend: self.doc.config.blend,
            });
        }
        if button(
            ui,
            card,
            3,
            1,
            "Remove region",
            self.doc.config.has_regions(),
        ) {
            let removed = self.environment.region;
            let mut regions = self.doc.config.regions().to_vec();
            regions.remove(removed);
            let walls = self
                .doc
                .config
                .walls()
                .walls()
                .iter()
                .filter(|w| w.left != removed && w.right != removed)
                .cloned()
                .map(|mut w| {
                    w.left -= usize::from(w.left > removed);
                    w.right -= usize::from(w.right > removed);
                    w
                })
                .collect();
            return Some(Command::ClimateRegions {
                regions,
                walls,
                blend: self.doc.config.blend,
            });
        }
        ui.text_fit_left(
            card.x + 16.0,
            card.y + 324.0,
            300.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            "Use Climate Wall to divide the regions.",
        );
        None
    }
    fn weather_controls(&self, ui: &mut Ui, card: Rect) -> Option<Command> {
        let region = &self.doc.config.regions()[self.environment.region];
        ui.text(
            card.x + 16.0,
            card.y + 126.0,
            type_scale::BODY,
            rgb(palette::TEXT, 1.0),
            &format!("Weather: {}", region.weather.label()),
        );
        for (i, preset) in WeatherPreset::ALL.into_iter().enumerate() {
            if button(ui, card, 1 + i / 2, i % 2, preset.label(), true) {
                return Some(Command::Weather {
                    region: self.environment.region,
                    preset,
                });
            }
        }
        None
    }
    fn wall_controls(&mut self, ui: &mut Ui, card: Rect) -> Option<Command> {
        let n = self.doc.config.regions().len();
        if button(
            ui,
            card,
            0,
            0,
            &format!("Left: {}", self.environment.region + 1),
            n > 1,
        ) {
            self.environment.region = (self.environment.region + 1) % n;
        }
        if button(
            ui,
            card,
            0,
            1,
            &format!("Right: {}", self.environment.right + 1),
            n > 1,
        ) {
            self.environment.right = (self.environment.right + 1) % n;
        }
        ui.text_fit_left(
            card.x + 16.0,
            card.y + 174.0,
            300.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            "Click two endpoints, from south to north.",
        );
        ui.text_fit_left(
            card.x + 16.0,
            card.y + 200.0,
            300.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            "Walls should reach the map edge or another wall.",
        );
        for (i, blend) in [0.0, 100.0, 500.0, 1000.0].into_iter().enumerate() {
            if button(
                ui,
                card,
                3 + i / 2,
                i % 2,
                &format!("Blend: {blend:.0} m"),
                true,
            ) {
                return Some(Command::ClimateRegions {
                    regions: self.doc.config.regions().to_vec(),
                    walls: self.doc.config.walls().walls().to_vec(),
                    blend,
                });
            }
        }
        if button(
            ui,
            card,
            5,
            0,
            "Remove last wall",
            !self.doc.config.walls().is_empty(),
        ) {
            let mut walls = self.doc.config.walls().walls().to_vec();
            walls.pop();
            return Some(Command::ClimateRegions {
                regions: self.doc.config.regions().to_vec(),
                walls,
                blend: self.doc.config.blend,
            });
        }
        None
    }
    pub(super) fn climate_wall(&mut self, at: [f32; 2]) -> Option<Command> {
        if self.environment.region == self.environment.right {
            self.message = "Choose different regions on the wall's left and right.".into();
            return None;
        }
        let Some(from) = self.region_start.take() else {
            self.region_start = Some(at);
            self.message = "Click the other climate wall endpoint.".into();
            return None;
        };
        let mut walls = self.doc.config.walls().walls().to_vec();
        walls.push(mc_data::regions::Wall {
            line: vec![(from[0], from[1]), (at[0], at[1])],
            left: self.environment.region,
            right: self.environment.right,
        });
        Some(Command::ClimateRegions {
            regions: self.doc.config.regions().to_vec(),
            walls,
            blend: self.doc.config.blend,
        })
    }
}
