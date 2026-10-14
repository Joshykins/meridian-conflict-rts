//! Searchable authoring catalogue, including mission-only units.
use super::*;
#[derive(Default)]
pub(super) struct Catalogue {
    pub query: String,
    scroll: usize,
}
impl Catalogue {
    pub fn with_query(query: String) -> Self {
        Self { query, scroll: 0 }
    }
}
impl Editor {
    pub(super) fn catalogue_entries(&self, query: &str) -> Vec<(String, String)> {
        let query = query.to_lowercase();
        self.blueprints
            .units
            .iter()
            .filter(|bp| self.blueprints.is_listed(bp.id) || bp.mission_only)
            .filter(|bp| {
                let text = format!("{} {} {}", bp.key, bp.name, bp.role).to_lowercase();
                query.split_whitespace().all(|q| text.contains(q))
            })
            .map(|bp| {
                (
                    bp.key.clone(),
                    format!("{} · T{} · {}", bp.name, bp.tech, bp.role),
                )
            })
            .collect()
    }
    pub(super) fn choose_unit(&mut self, key: &str) -> Result<(), String> {
        let unit = self
            .blueprints
            .unit_by_key(key)
            .ok_or_else(|| format!("Unknown unit: {key}"))?;
        self.blueprint = unit.key.clone();
        self.name = format!("{}_{}", unit.key, self.doc.placed.next_id);
        self.selected = None;
        self.tool = 5;
        self.catalogue = None;
        self.message = format!("{} selected. Click the terrain to place it.", unit.name);
        Ok(())
    }
    pub(super) fn catalogue_frame(&mut self, ui: &mut Ui) {
        let Some(mut catalogue) = self.catalogue.take() else {
            return;
        };
        ui.fill(
            Rect::new(0.0, 0.0, ui.size.x, ui.size.y),
            crate::ui::ink(0.8),
        );
        let w = (ui.size.x - 80.0).min(1050.0);
        let h = ui.size.y - 100.0;
        let card = Rect::new((ui.size.x - w) * 0.5, 50.0, w, h);
        ui.panel(card);
        ui.text(
            card.x + 24.0,
            card.y + 35.0,
            type_scale::TITLE,
            rgb(palette::TEXT, 1.0),
            "UNIT CATALOGUE",
        );
        if ui.button(
            id("editor_catalogue_close", 0),
            Rect::new(card.x + w - 124.0, card.y + 18.0, 100.0, 36.0),
            "Close",
            ButtonKind::Secondary,
            true,
        ) {
            ui.mem.editing = None;
            return;
        }
        let old = catalogue.query.clone();
        ui.text_field(
            id("editor_catalogue_search", 0),
            Rect::new(card.x + 24.0, card.y + 64.0, w - 48.0, 40.0),
            &mut catalogue.query,
            128,
        );
        if old != catalogue.query {
            catalogue.scroll = 0;
        }
        let entries = self.catalogue_entries(&catalogue.query);
        let rows = ((h - 150.0) / 52.0).floor().max(0.0) as usize;
        if card.contains(ui.cursor) && ui.input.scroll != 0.0 {
            catalogue.scroll = (catalogue.scroll as i32 - ui.input.scroll.signum() as i32 * 3)
                .clamp(0, entries.len().saturating_sub(rows) as i32)
                as usize;
        }
        catalogue.scroll = catalogue.scroll.min(entries.len().saturating_sub(rows));
        ui.text(
            card.x + 24.0,
            card.y + 127.0,
            type_scale::CAPTION,
            rgb(palette::DIM, 1.0),
            &format!(
                "{} units · search names, roles or keys · scroll for more",
                entries.len()
            ),
        );
        for (i, (key, label)) in entries.iter().skip(catalogue.scroll).take(rows).enumerate() {
            if ui.button(
                id("editor_catalogue_unit", i),
                Rect::new(
                    card.x + 24.0,
                    card.y + 146.0 + i as f32 * 52.0,
                    w - 48.0,
                    44.0,
                ),
                label,
                ButtonKind::Secondary,
                true,
            ) {
                ui.mem.editing = None;
                if let Err(e) = self.choose_unit(key) {
                    self.message = e;
                }
                return;
            }
        }
        self.catalogue = Some(catalogue);
    }
}
