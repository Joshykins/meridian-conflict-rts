//! Tile-aligned bounds edits are staged before a replacement renderer is published.
use super::*;
#[derive(Default)]
pub(super) struct Controls {
    source: [u32; 2],
    tiles: [u32; 2],
    origin: [i32; 2],
    fill: f32,
}
impl Editor {
    pub(super) fn bounds_properties(
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
            "MAP BOUNDS",
        );
        let source = [self.doc.map.info.tiles_w, self.doc.map.info.tiles_h];
        let c = &mut self.map_controls.bounds;
        if c.source != source {
            c.source = source;
            c.tiles = source;
            c.origin = [0, 0];
            c.fill = self.doc.map.info.water_level.to_f32() - 20.0;
        }
        let tile = mc_map::TILE_SIZE_M as f32;
        ui.text_fit_left(
            card.x + 16.0,
            card.y + 54.0,
            300.0,
            type_scale::BODY,
            rgb(palette::TEXT, 1.0),
            &format!(
                "{:.2} × {:.2} km",
                c.tiles[0] as f32 * tile / 1000.0,
                c.tiles[1] as f32 * tile / 1000.0
            ),
        );
        for (row, label) in ["Width", "Height", "Origin east", "Origin north"]
            .iter()
            .enumerate()
        {
            for (col, delta) in [-1, 1].into_iter().enumerate() {
                let value = if row < 2 {
                    c.tiles[row] as i32
                } else {
                    c.origin[row - 2]
                };
                let limit = mc_map::MAX_MAP_TILES as i32;
                let enabled = if row < 2 {
                    (1..=limit).contains(&(value + delta))
                } else {
                    (-limit..=limit).contains(&(value + delta))
                };
                if ui.button(
                    id("editor_bounds", row * 2 + col),
                    Rect::new(
                        card.x + 16.0 + col as f32 * 154.0,
                        card.y + 68.0 + row as f32 * 42.0,
                        146.0,
                        34.0,
                    ),
                    &format!("{label} {value} {}", if delta < 0 { "−" } else { "+" }),
                    ButtonKind::Secondary,
                    enabled,
                ) {
                    if row < 2 {
                        c.tiles[row] = (value + delta) as u32;
                    } else {
                        c.origin[row - 2] = value + delta;
                    }
                }
            }
        }
        let mut fill = (c.fill + 2000.0) / 6000.0;
        if ui.slider_compact(
            id("editor_bounds_fill", 0),
            Rect::new(card.x + 16.0, card.y + 242.0, 300.0, 28.0),
            &format!("New ground: {:.0} m", c.fill),
            &mut fill,
        ) {
            c.fill = fill * 6000.0 - 2000.0;
        }
        let command = Command::Bounds {
            origin: c.origin,
            tiles: c.tiles,
            fill: c.fill,
        };
        let changed = c.origin != [0, 0] || c.tiles != source;
        ui.text_fit_left(
            card.x + 16.0,
            card.y + 294.0,
            300.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            "1 tile = 2048 m. Negative origin extends west/south.",
        );
        ui.text_fit_left(
            card.x + 16.0,
            card.y + 316.0,
            300.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            "Crop removes objects outside the new map. Undo restores.",
        );
        if ui.button(
            id("editor_bounds_apply", 0),
            Rect::new(card.x + 16.0, card.y + 338.0, 300.0, 34.0),
            "Apply bounds",
            ButtonKind::Primary,
            changed,
        ) {
            if let Err(error) = self.apply(command, renderer) {
                self.message = error;
            }
        }
        card
    }
}
