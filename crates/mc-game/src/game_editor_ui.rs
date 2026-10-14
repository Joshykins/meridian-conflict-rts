use super::*;
use crate::ui::{id, rgb, type_scale, ButtonKind, Rect};
impl Game {
    pub(in crate::game) fn editor_panel(&mut self, ui: &mut Ui) -> bool {
        let Some(mut tools) = self.editor_tools.take() else {
            return false;
        };
        if tools.target.is_some_and(|to| self.tick() >= to) {
            tools.target = None;
        }
        let card = Rect::new(18.0, 210.0, 310.0, if tools.open { 470.0 } else { 42.0 });
        tools.rect = Some(Rect::new(
            card.x * ui.s,
            card.y * ui.s,
            card.w * ui.s,
            card.h * ui.s,
        ));
        ui.panel(card);
        if ui.button(
            id("test_tools_toggle", 0),
            Rect::new(card.x + 8.0, card.y + 6.0, 142.0, 30.0),
            if tools.open {
                "Playtest / Hide"
            } else {
                "Playtest / Show"
            },
            ButtonKind::Secondary,
            true,
        ) {
            tools.open = !tools.open;
        }
        let stop = ui.button(
            id("test_tools_stop", 0),
            Rect::new(card.x + 158.0, card.y + 6.0, 142.0, 30.0),
            "Stop test",
            ButtonKind::Primary,
            true,
        );
        let action = if tools.open {
            self.editor_tool_controls(ui, card, &mut tools)
        } else {
            None
        };
        let editing = ui.mem.editing.is_some_and(|key| {
            key == id("test_spawn_search", 0) || key == id("test_jump_seconds", 0)
        });
        if !tools.open && editing {
            ui.mem.editing = None;
        }
        tools.typing = tools.open && editing;
        self.editor_tools = Some(tools);
        if let Some(action) = action {
            let result = self.editor_debug(action);
            if let Some(tools) = &mut self.editor_tools {
                tools.message = result
                    .err()
                    .unwrap_or_else(|| "Test command sent. The authored map is unchanged.".into());
            }
        }
        stop
    }
    fn editor_tool_controls(&self, ui: &mut Ui, card: Rect, t: &mut Tools) -> Option<DebugAction> {
        let button = |ui: &mut Ui, row: usize, col: usize, label: &str| {
            ui.button(
                id("test_tools", row * 2 + col),
                Rect::new(
                    card.x + 10.0 + col as f32 * 148.0,
                    card.y + 48.0 + row as f32 * 38.0,
                    142.0,
                    30.0,
                ),
                label,
                ButtonKind::Secondary,
                true,
            )
        };
        let mut action = None;
        if button(
            ui,
            0,
            0,
            if t.reveal {
                "Reveal: on"
            } else {
                "Reveal: off"
            },
        ) {
            action = Some(DebugAction::Reveal { on: !t.reveal });
        }
        if button(ui, 0, 1, if self.view.paused { "Resume" } else { "Pause" }) {
            action = Some(DebugAction::Pause {
                on: !self.view.paused,
            });
        }
        if button(ui, 1, 0, &format!("Speed: {}%", self.view.speed)) {
            let speeds = [25, 50, 100, 200, 400, 800, 1600];
            let at = speeds
                .iter()
                .position(|&s| s == self.view.speed)
                .unwrap_or(2);
            action = Some(DebugAction::Speed {
                percent: speeds[(at + 1) % speeds.len()],
            });
        }
        if button(ui, 1, 1, "Advance 60 sec") {
            action = Some(DebugAction::Jump {
                tick: self.tick().saturating_add(600),
            });
        }
        ui.text_field(
            id("test_jump_seconds", 0),
            Rect::new(card.x + 10.0, card.y + 124.0, 142.0, 30.0),
            &mut t.seconds,
            8,
        );
        if button(ui, 2, 1, "Jump to seconds") {
            match t
                .seconds
                .parse::<u32>()
                .ok()
                .and_then(|v| v.checked_mul(10))
            {
                Some(tick) => action = Some(DebugAction::Jump { tick }),
                None => t.message = "Enter a whole number of seconds.".into(),
            }
        }
        self.editor_spawn_picker(ui, card, t);
        if button(ui, 8, 0, &format!("Player: {}", t.owner + 1)) {
            t.owner =
                (usize::from(t.owner) + 1).rem_euclid(self.view.status.players.len().max(1)) as u8;
        }
        if button(ui, 8, 1, &format!("Count: {}", t.count)) {
            t.count = match t.count {
                1 => 5,
                5 => 10,
                10 => 25,
                _ => 1,
            };
        }
        if ui.button(
            id("test_spawn", 0),
            Rect::new(card.x + 10.0, card.y + 390.0, 290.0, 30.0),
            "Spawn at camera",
            ButtonKind::Primary,
            !t.blueprint.is_empty(),
        ) {
            action = Some(DebugAction::Spawn {
                blueprint: t.blueprint.clone(),
                owner: t.owner,
                at: self.camera.focus.truncate().to_array(),
                heading: 0.0,
                count: t.count,
            });
        }
        let label = t.target.map_or_else(
            || t.message.clone(),
            |tick| format!("Advancing: {} / {} seconds", self.tick() / 10, tick / 10),
        );
        ui.text_fit_left(
            card.x + 10.0,
            card.y + 446.0,
            290.0,
            type_scale::MICRO,
            rgb(palette::TEXT, 1.0),
            &label,
        );
        action
    }
    fn editor_spawn_picker(&self, ui: &mut Ui, card: Rect, t: &mut Tools) {
        if ui.text_field(
            id("test_spawn_search", 0),
            Rect::new(card.x + 10.0, card.y + 166.0, 290.0, 30.0),
            &mut t.query,
            64,
        ) {
            t.scroll = 0;
        }
        let query = t.query.to_lowercase();
        let units: Vec<_> = self
            .blueprints
            .units
            .iter()
            .filter(|b| {
                format!("{} {}", b.name, b.key)
                    .to_lowercase()
                    .contains(&query)
            })
            .collect();
        let area = Rect::new(card.x + 10.0, card.y + 204.0, 290.0, 132.0);
        if area.contains(ui.cursor) && ui.input.scroll != 0.0 {
            t.scroll = (t.scroll as i32 - ui.input.scroll.signum() as i32)
                .clamp(0, units.len().saturating_sub(4) as i32) as usize;
        }
        t.scroll = t.scroll.min(units.len().saturating_sub(4));
        // Four visible rows, with scrolling and a count; the full catalogue stays available.
        for (i, bp) in units.iter().skip(t.scroll).take(4).enumerate() {
            if ui.button(
                id("test_spawn_unit", usize::from(bp.id.0)),
                Rect::new(area.x, area.y + i as f32 * 30.0, area.w, 27.0),
                &bp.name,
                if t.blueprint == bp.key {
                    ButtonKind::Primary
                } else {
                    ButtonKind::Secondary
                },
                true,
            ) {
                t.blueprint = bp.key.clone();
            }
        }
        ui.text_fit_left(
            card.x + 10.0,
            card.y + 338.0,
            290.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            &format!("{} units; search or scroll to choose", units.len()),
        );
    }
}
