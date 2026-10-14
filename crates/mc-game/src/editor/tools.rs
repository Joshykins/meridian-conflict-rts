//! Tool families keep the authoring controls compact as layers are added.
use super::*;
pub(super) struct PaintSettings {
    pub value: u8,
    pub ground: u8,
    pub road: u8,
    pub width: f32,
}
impl Default for PaintSettings {
    fn default() -> Self {
        Self {
            value: 255,
            ground: 1,
            road: 1,
            width: 16.0,
        }
    }
}
const GROUPS: &[(&str, &[usize])] = &[
    ("Terrain", &[1, 2, 3, 4, 13, 14, 15, 16]),
    ("Paint", &[17, 18, 19, 20, 21]),
    ("Objects", &[0, 5, 27, 6, 7, 8, 9]),
    ("Environment", &[22, 23, 24, 25, 26]),
    ("Camera / Reload", &[12, 10, 11]),
];
const LABELS: &[&str] = &[
    "Select",
    "Raise",
    "Lower",
    "Flatten",
    "Smooth",
    "Unit",
    "Region",
    "Point",
    "Camera",
    "Move",
    "Reload data",
    "Reload shaders",
    "Free camera",
    "Cliff",
    "Noise",
    "Ramp",
    "Stamp",
    "Ground",
    "Snow",
    "Ice",
    "Trail",
    "Road",
    "Water",
    "Climate",
    "Weather",
    "Time",
    "Climate wall",
    "Doodad",
];
impl Editor {
    pub(super) fn choose_tool(&mut self, name: &str) -> Result<(), String> {
        let i = LABELS
            .iter()
            .position(|label| label.to_lowercase().replace(' ', "_") == name)
            .ok_or("Unknown editor tool")?;
        self.tool_group = GROUPS
            .iter()
            .position(|(_, tools)| tools.contains(&i))
            .ok_or("Tool has no group")?;
        self.select_tool(i);
        Ok(())
    }

    pub(super) fn tool_rows(width: f32) -> usize {
        1 + 8usize.div_ceil(((width - 68.0) / 112.0).floor().max(1.0) as usize)
    }
    pub(super) fn tools_panel(&mut self, ui: &mut Ui, bottom: f32) {
        let w = ui.size.x;
        let rows = Self::tool_rows(w);
        ui.panel(Rect::new(18.0, bottom, w - 36.0, ui.size.y - bottom - 18.0));
        ui.text(
            34.0,
            bottom + 23.0,
            type_scale::CAPTION,
            rgb(palette::DIM, 1.0),
            "AUTHORING TOOLS",
        );
        for (i, (label, _)) in GROUPS.iter().enumerate() {
            if ui.button(
                id("editor_tool_group", i),
                Rect::new(
                    34.0 + i as f32 * (w - 68.0) / GROUPS.len() as f32,
                    bottom + 36.0,
                    (w - 68.0) / GROUPS.len() as f32 - 8.0,
                    30.0,
                ),
                label,
                if i == self.tool_group {
                    ButtonKind::Primary
                } else {
                    ButtonKind::Secondary
                },
                true,
            ) {
                self.tool_group = i;
            }
        }
        let columns = ((w - 68.0) / 112.0).floor().max(1.0) as usize;
        let step = (w - 68.0) / columns as f32;
        for (n, &i) in GROUPS[self.tool_group].1.iter().enumerate() {
            if ui.button(
                id("editor_tool", i),
                Rect::new(
                    34.0 + (n % columns) as f32 * step,
                    bottom + 78.0 + (n / columns) as f32 * 44.0,
                    step - 8.0,
                    36.0,
                ),
                LABELS[i],
                if self.tool == i || (i == 12 && self.camera.pitch_free.is_some()) {
                    ButtonKind::Primary
                } else {
                    ButtonKind::Secondary
                },
                true,
            ) {
                self.select_tool(i);
            }
        }
        let y = bottom + 54.0 + rows as f32 * 44.0;
        let width = (w - 102.0) * 0.5;
        let mut radius = (self.radius - 8.0) / 504.0;
        if (22..=26).contains(&self.tool) {
            ui.text(
                34.0,
                y + 20.0,
                type_scale::BODY,
                rgb(palette::TEXT, 1.0),
                "Adjust the environment in the properties panel. Changes apply immediately.",
            );
        } else if self.tool == 21 {
            ui.text(
                34.0,
                y + 20.0,
                type_scale::BODY,
                rgb(palette::TEXT, 1.0),
                "Road: click two endpoints; set width in Paint Properties.",
            );
        } else if ui.slider(
            id("editor_radius", 0),
            Rect::new(34.0, y, width, 28.0),
            &format!("Brush radius: {:.0} m", self.radius),
            &mut radius,
        ) {
            self.radius = 8.0 + radius * 504.0;
        }
        if (18..=20).contains(&self.tool) {
            let mut value = self.paint.value as f32 / 255.0;
            if ui.slider(
                id("editor_paint_value", 0),
                Rect::new(68.0 + width, y, width, 28.0),
                &format!("Paint coverage: {:.0}%", value * 100.0),
                &mut value,
            ) {
                self.paint.value = (value * 255.0).round() as u8;
            }
        } else if self.tool == 17 {
            ui.text(
                68.0 + width,
                y + 20.0,
                type_scale::BODY,
                rgb(palette::TEXT, 1.0),
                "Choose the surface in Paint Properties.",
            );
        } else if self.tool < 21 {
            let mut strength = self.strength / 64.0;
            if ui.slider(
                id("editor_strength", 0),
                Rect::new(68.0 + width, y, width, 28.0),
                &format!("Strength: {:.0} m", self.strength),
                &mut strength,
            ) {
                self.strength = strength * 64.0;
            }
        }
        ui.text_fit_left(
            34.0,
            y + 50.0,
            w - 68.0,
            type_scale::BODY,
            rgb(palette::TEXT, 1.0),
            &self.message,
        );
    }
    fn select_tool(&mut self, i: usize) {
        if !(10..13).contains(&i) {
            self.tool = i;
            self.region_start = None;
            if i == 5 && self.doc.placed.units.iter().any(|u| u.name == self.name) {
                self.name = format!("{}_{}", self.blueprint, self.doc.placed.next_id);
                self.selected = None;
            }
        } else if i == 12 {
            self.navigation.toggle(&mut self.camera);
            self.message = if self.camera.pitch_free.is_some() {
                "Free camera: WASD fly; Q/E height; arrows look; Shift faster; wheel speed."
            } else {
                "Orbit camera: WASD pan; Q/E turn; Page Up/Down tilt; wheel zoom."
            }
            .into();
        } else if let Err(e) = self.request_reload(ReloadRequest {
            data: i == 10,
            shaders: i == 11,
            ..Default::default()
        }) {
            self.message = e;
        }
    }
    pub(super) fn paint_properties(&mut self, ui: &mut Ui) -> Rect {
        let card = Rect::new(ui.size.x - 350.0, 100.0, 332.0, 236.0);
        ui.panel(card);
        ui.text(
            card.x + 16.0,
            card.y + 26.0,
            type_scale::CAPTION,
            rgb(palette::TEXT, 1.0),
            "PAINT PROPERTIES",
        );
        let grounds = [
            "Natural",
            "Paving",
            "Lawn",
            "Yard",
            "Ballast",
            "Field",
            "Rubble",
            "Earth",
            "Spaceport apron",
        ];
        let roads = [
            "Erase road",
            "Street",
            "Avenue",
            "Highway",
            "Country lane",
            "Railway",
        ];
        let label = if self.tool == 21 {
            format!("Road: {}", roads[self.paint.road as usize])
        } else {
            format!("Ground: {}", grounds[self.paint.ground as usize])
        };
        if ui.button(
            id("editor_paint_kind", 0),
            Rect::new(card.x + 16.0, card.y + 48.0, 300.0, 36.0),
            &label,
            ButtonKind::Secondary,
            self.tool == 17 || self.tool == 21,
        ) {
            if self.tool == 21 {
                self.paint.road = (self.paint.road + 1) % 6;
            } else {
                self.paint.ground = (self.paint.ground + 1) % 9;
            }
        }
        if self.tool == 21 {
            let mut width = (self.paint.width - 2.0) / 118.0;
            if ui.slider(
                id("editor_road_width", 0),
                Rect::new(card.x + 16.0, card.y + 101.0, 300.0, 28.0),
                &format!("Road width: {:.0} m", self.paint.width),
                &mut width,
            ) {
                self.paint.width = 2.0 + 118.0 * width;
            }
        } else if ui.button(
            id("editor_paint_erase", 0),
            Rect::new(card.x + 16.0, card.y + 98.0, 300.0, 36.0),
            if self.paint.value == 0 {
                "Erase coverage"
            } else {
                "Paint coverage"
            },
            ButtonKind::Secondary,
            self.tool != 17,
        ) {
            self.paint.value = if self.paint.value == 0 { 255 } else { 0 };
        }
        if ui.button(
            id("editor_trail_heading", 0),
            Rect::new(card.x + 16.0, card.y + 146.0, 300.0, 32.0),
            &format!("Trail heading: {:.0}° (+15°)", self.heading),
            ButtonKind::Secondary,
            self.tool == 20,
        ) {
            self.heading = (self.heading + 15.0) % 360.0;
        }
        ui.text_fit_left(
            card.x + 16.0,
            card.y + 210.0,
            300.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            if self.tool == 21 {
                "Click two endpoints to lay a road."
            } else {
                "Click terrain to paint. Undo restores the stroke."
            },
        );
        card
    }
}
