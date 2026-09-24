//! Small renders of every unit and structure, for the tiles that offer them.
//! Drawn once per match into the overlay image slots the minimap leaves free,
//! as a grid of cells; a blueprint without a model just has no picture.

use super::minimap::MINIMAP_SLOT;
use crate::ui::{Rect, Ui};
use mc_data::{BlueprintId, Blueprints};
use mc_render::overlay::{IMAGE_SLOT, IMAGE_SLOTS};
use mc_render::Overlay;
use std::collections::HashMap;

/// Edge of one picture, pixels.
pub const CELL: usize = 102;
const PER_ROW: usize = IMAGE_SLOT / CELL;
const PER_SLOT: usize = PER_ROW * PER_ROW;

/// Unit pictures drawn and not yet in the overlay.
pub struct Baked {
    at: HashMap<BlueprintId, (usize, [f32; 4])>,
    stage: Option<(usize, [f32; 4])>,
    slots: Vec<usize>,
    sheets: Vec<Vec<u8>>,
}

#[derive(Default)]
pub struct Thumbs {
    at: HashMap<BlueprintId, (usize, [f32; 4])>,
    /// The soft pool of light a picture stands in (`stage_light`).
    stage: Option<(usize, [f32; 4])>,
    baked: bool,
}

impl Thumbs {
    pub fn baked(&self) -> bool {
        self.baked
    }

    /// Renders every blueprint's model into the free image slots. The first
    /// blueprints in the catalogue win when there are more than fit.
    pub fn bake(&mut self, overlay: &mut Overlay, blueprints: &Blueprints, team: [f32; 3]) {
        self.install(overlay, Thumbs::render(blueprints, team));
    }

    /// Hands pictures drawn by `render` (on any thread) to the overlay.
    pub fn install(&mut self, overlay: &mut Overlay, baked: Baked) {
        self.baked = true;
        self.at = baked.at;
        self.stage = baked.stage;
        for (slot, sheet) in baked.slots.iter().zip(&baked.sheets) {
            overlay.set_image(*slot, IMAGE_SLOT, IMAGE_SLOT, sheet);
        }
    }

    /// The pictures alone, without the overlay: the loading screen draws them
    /// on its own thread while the renderer is being built.
    pub fn render(blueprints: &Blueprints, team: [f32; 3]) -> Baked {
        let mut at = HashMap::new();
        let slots: Vec<usize> = (0..IMAGE_SLOTS).filter(|s| *s != MINIMAP_SLOT).collect();
        let mut sheets: Vec<Vec<u8>> = slots.iter().map(|_| vec![0; IMAGE_SLOT * IMAGE_SLOT * 4]).collect();
        // The first cell is the light the pictures stand in.
        let stage = slots.first().map(|&slot| {
            let light = stage_light(CELL);
            for y in 0..CELL {
                sheets[0][y * IMAGE_SLOT * 4..(y * IMAGE_SLOT + CELL) * 4]
                    .copy_from_slice(&light[y * CELL * 4..(y + 1) * CELL * 4]);
            }
            (slot, [0.0, 0.0, CELL as f32, CELL as f32])
        });
        let mut n = 1;
        // A refit's loadouts and kits show their unit's picture. Each takes some tens
        // of milliseconds to light, so they are drawn across the cores.
        let listed: Vec<_> = blueprints.units.iter().filter(|bp| blueprints.is_listed(bp.id)).collect();
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 8);
        let drawn: Vec<Option<Vec<u8>>> = std::thread::scope(|scope| {
            let jobs: Vec<_> = listed
                .chunks(listed.len().div_ceil(threads).max(1))
                .map(|chunk| {
                    scope.spawn(move || {
                        chunk
                            .iter()
                            .map(|bp| {
                                // At its own tier's look, so an upgrade shows what it adds; the
                                // usual three-quarter view from the front.
                                let model = mc_render::models::build_model_scaled(
                                    &bp.visual.mesh,
                                    bp.radius.to_f32(),
                                    bp.height.to_f32(),
                                    bp.tech,
                                )?;
                                Some(mc_render::models::thumbnail_of(&model, CELL, -35.0, team))
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            jobs.into_iter().flat_map(|j| j.join().expect("thumbnail thread")).collect()
        });
        for (bp, rgba) in listed.iter().zip(drawn) {
            let Some(rgba) = rgba else {
                continue;
            };
            let (sheet, cell) = (n / PER_SLOT, n % PER_SLOT);
            if sheet >= slots.len() {
                break;
            }
            let (cx, cy) = ((cell % PER_ROW) * CELL, (cell / PER_ROW) * CELL);
            for y in 0..CELL {
                let from = y * CELL * 4;
                let to = ((cy + y) * IMAGE_SLOT + cx) * 4;
                sheets[sheet][to..to + CELL * 4].copy_from_slice(&rgba[from..from + CELL * 4]);
            }
            at.insert(
                bp.id,
                (slots[sheet], [cx as f32, cy as f32, CELL as f32, CELL as f32]),
            );
            n += 1;
        }
        for bp in &blueprints.units {
            let base = blueprints.base_of(bp.id);
            if base != bp.id {
                if let Some(&base_at) = at.get(&base) {
                    at.insert(bp.id, base_at);
                }
            }
        }
        Baked { at, stage, slots, sheets }
    }

    /// A soft pool of light over `r`, for a picture to stand in; `tint` colours it.
    pub fn stage(&self, ui: &mut Ui, r: Rect, tint: crate::ui::Color) {
        if let Some((slot, src)) = self.stage {
            ui.image(slot, src, r, tint);
        }
    }

    /// The picture of `blueprint` in `r`, if it has one. Returns whether it drew.
    pub fn draw(&self, ui: &mut Ui, blueprint: BlueprintId, r: Rect, alpha: f32) -> bool {
        let Some((slot, src)) = self.at.get(&blueprint) else {
            return false;
        };
        ui.image(*slot, *src, r, [1.0, 1.0, 1.0, alpha]);
        true
    }
}

/// White light falling off from a little below the middle, wider than it is
/// tall like a lamp's pool seen from above: straight alpha, `size` x `size`.
fn stage_light(size: usize) -> Vec<u8> {
    let mut out = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let u = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
            let d = (u * u / 1.1 + (v - 0.12) * (v - 0.12) / 0.8).sqrt();
            let a = (1.0 - d).clamp(0.0, 1.0);
            // Smooth to the edge, so the square it is drawn in never shows.
            let a = a * a * (3.0 - 2.0 * a);
            let at = (y * size + x) * 4;
            out[at..at + 4].copy_from_slice(&[255, 255, 255, (a * 255.0 + 0.5) as u8]);
        }
    }
    out
}
