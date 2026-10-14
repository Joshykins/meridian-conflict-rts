//! Map roster controls share the document's atomic command and undo path.
use super::*;
#[derive(Default)]
pub(super) struct Controls {
    slot: usize,
    pub test_slot: u8,
    name: String,
    loaded: Option<(usize, String)>,
}
fn button(ui: &mut Ui, card: Rect, row: usize, col: usize, label: &str, enabled: bool) -> bool {
    ui.button(
        id("editor_map", row * 2 + col),
        Rect::new(
            card.x + 16.0 + col as f32 * 154.0,
            card.y + 48.0 + row as f32 * 42.0,
            146.0,
            34.0,
        ),
        label,
        ButtonKind::Secondary,
        enabled,
    )
}
impl Editor {
    pub(super) fn map_properties(
        &mut self,
        ui: &mut Ui,
        renderer: &mut mc_render::Renderer,
    ) -> Rect {
        let card = Rect::new(ui.size.x - 350.0, 100.0, 332.0, 390.0);
        ui.panel(card);
        ui.text(
            card.x + 16.0,
            card.y + 26.0,
            type_scale::CAPTION,
            rgb(palette::TEXT, 1.0),
            "MAP PLAYERS",
        );
        let mut command = self.slot_controls(ui, card);
        if button(ui, card, 6, 0, "No mission", true) {
            command = Some(Command::Mission { key: None });
        }
        if button(ui, card, 6, 1, "Vela: Contract", true) {
            command = Some(Command::Mission {
                key: Some("vela_m1".into()),
            });
        }
        ui.text_fit_left(
            card.x + 16.0,
            card.y + 350.0,
            300.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            &format!(
                "Mission: {}",
                self.doc.config.mission.as_deref().unwrap_or("none")
            ),
        );
        if let Some(command) = command {
            if let Err(error) = self.apply(command, renderer) {
                self.message = error;
            }
        }
        card
    }
    fn slot_controls(&mut self, ui: &mut Ui, card: Rect) -> Option<Command> {
        let mut slots = self.doc.config.players.clone();
        if slots.is_empty() {
            slots = config(&self.doc.placed, &self.doc.config)
                .players
                .into_iter()
                .map(|p| mc_data::map_settings::MapPlayer {
                    name: p.name,
                    faction: p.faction,
                    team: p.team,
                    start: p.start,
                    computer: p.controller == mc_sim::tables::Controller::Ai,
                })
                .collect();
        }
        self.map_controls.slot = self.map_controls.slot.min(slots.len() - 1);
        self.map_controls.test_slot = self.map_controls.test_slot.min((slots.len() - 1) as u8);
        let n = self.map_controls.slot;
        if button(
            ui,
            card,
            0,
            0,
            &format!("Slot {} / {}", n + 1, slots.len()),
            true,
        ) {
            self.map_controls.slot = (n + 1) % slots.len();
        }
        if button(
            ui,
            card,
            0,
            1,
            &format!("Test as: {}", self.map_controls.test_slot + 1),
            true,
        ) {
            self.map_controls.test_slot = (self.map_controls.test_slot + 1) % slots.len() as u8;
        }
        let loaded = (n, slots[n].name.clone());
        if self.map_controls.loaded.as_ref() != Some(&loaded) {
            self.map_controls.name = loaded.1.clone();
            self.map_controls.loaded = Some(loaded);
        }
        ui.text_field(
            id("editor_slot_name", 0),
            Rect::new(card.x + 16.0, card.y + 90.0, 208.0, 34.0),
            &mut self.map_controls.name,
            64,
        );
        let mut changed = ui.button(
            id("editor_slot_apply_name", 0),
            Rect::new(card.x + 232.0, card.y + 90.0, 84.0, 34.0),
            "Apply",
            ButtonKind::Secondary,
            true,
        );
        if changed {
            slots[n].name = self.map_controls.name.clone();
        }
        if button(ui, card, 2, 0, &slots[n].faction, true) {
            let factions = &self.blueprints.factions;
            let i = factions
                .iter()
                .position(|f| f.key == slots[n].faction)
                .unwrap_or(0);
            slots[n].faction = factions[(i + 1) % factions.len()].key.clone();
            changed = true;
        }
        if button(
            ui,
            card,
            2,
            1,
            if slots[n].computer {
                "Computer"
            } else {
                "Human / script"
            },
            true,
        ) {
            slots[n].computer = !slots[n].computer;
            changed = true;
        }
        if button(
            ui,
            card,
            3,
            0,
            &format!("Team: {}", slots[n].team + 1),
            true,
        ) {
            slots[n].team = (slots[n].team + 1) % mc_core::MAX_PLAYERS as u8;
            changed = true;
        }
        if button(
            ui,
            card,
            3,
            1,
            &format!("Start: {}", slots[n].start + 1),
            !self.doc.map.starts.is_empty(),
        ) {
            slots[n].start =
                (usize::from(slots[n].start) + 1).rem_euclid(self.doc.map.starts.len()) as u8;
            changed = true;
        }
        if button(
            ui,
            card,
            4,
            0,
            "Add slot",
            slots.len() < mc_core::MAX_PLAYERS,
        ) {
            slots.push(mc_data::map_settings::MapPlayer {
                name: format!("Player {}", slots.len() + 1),
                faction: "Aster".into(),
                team: slots.len() as u8,
                start: 0,
                computer: false,
            });
            self.map_controls.slot = slots.len() - 1;
            changed = true;
        }
        if button(ui, card, 4, 1, "Remove last slot", slots.len() > 1) {
            slots.pop();
            changed = true;
        }
        if button(ui, card, 5, 0, "Save roster", true) {
            changed = true;
        }
        changed.then_some(Command::Players { slots })
    }
}
