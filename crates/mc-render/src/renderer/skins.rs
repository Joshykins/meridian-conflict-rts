//! The game's skins on the GPU (`mc_data::skins`, docs/STANDARD.md): one `GpuSkin` per
//! skin in scene set binding 34, which entity.wgsl `material_of` and regency.wgsl read
//! by the model's `ModelInfo::skin`. A model paints slots; what a slot looks like is
//! the skin's.

use crate::gpu::{Buffer, Gpu, GpuError};
use crate::gpu_consts::skin as gs;
use ash::vk;
use bytemuck::{Pod, Zeroable};
use mc_data::skins::{Skin, SkinId};
use mc_data::Blueprints;

/// One skin as the shaders read it (common.wgsl `Skin`).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct GpuSkin {
    /// Per slot (`gpu_consts::skin`): albedo and metallic, then emissive and roughness.
    pub(crate) slots: [[f32; 4]; gs::SLOT_WORDS as usize],
    /// The Regency finish's colours (`gpu_consts::skin::FILM_*`), rgb.
    pub(crate) film: [[f32; 4]; gs::FILM_COLORS as usize],
    /// The skin's light: its emitters' hue, rgb.
    pub(crate) highlight: [f32; 4],
}

impl GpuSkin {
    fn of(skin: &Skin) -> GpuSkin {
        let mut g = GpuSkin::zeroed();
        for (i, (_, s)) in skin.slot_list().iter().enumerate() {
            let [r, gr, b] = skin.paint(s.color);
            let [er, eg, eb] = skin.paint(s.glow);
            g.slots[2 * i] = [r, gr, b, s.metal];
            g.slots[2 * i + 1] = [er, eg, eb, s.rough];
        }
        if let Some(f) = &skin.film {
            let film = [
                (gs::FILM_BLACK, f.black),
                (gs::FILM_RED, f.red),
                (gs::FILM_DEEP, f.deep),
                (gs::FILM_COPPER, f.copper),
                (gs::FILM_PALE, f.pale),
                (gs::FILM_STEEL_LIT, f.steel_lit),
                (gs::FILM_WORKS, f.works),
                (gs::FILM_WORKS_DEEP, f.works_deep),
                (gs::FILM_WORKS_POLISH, f.works_polish),
            ];
            for (at, [r, gr, b]) in film {
                g.film[at as usize] = [r, gr, b, 0.0];
            }
        }
        let [r, gr, b] = skin.highlight;
        g.highlight = [r, gr, b, 0.0];
        g
    }
}

/// The skins' buffer.
pub(crate) struct Skins {
    buffer: Buffer,
    /// Each skin as it was loaded, in `Blueprints::skins` order.
    table: Vec<GpuSkin>,
}

impl Skins {
    pub(crate) fn new(gpu: &Gpu, bps: &Blueprints) -> Result<Skins, GpuError> {
        let mut table: Vec<GpuSkin> = bps.skins.iter().map(GpuSkin::of).collect();
        // A scene with no skins (blueprints made in a test) still binds one.
        if table.is_empty() {
            table.push(GpuSkin::zeroed());
        }
        let bytes: &[u8] = bytemuck::cast_slice(&table);
        let buffer = gpu.host_buffer(bytes.len() as u64, vk::BufferUsageFlags::STORAGE_BUFFER)?;
        buffer.write(0, bytes);
        Ok(Skins { buffer, table })
    }

    /// Dresses every model in skin `worn` (an index into `Blueprints::skins`), or each in
    /// its own again for `None`: a shot of one unit in another skin (`--skin`). Skins
    /// are presentation only, so nothing else changes. False if there is no such skin.
    pub(crate) fn wear(&self, worn: Option<usize>) -> bool {
        let table: Vec<GpuSkin> = match worn {
            None => self.table.clone(),
            Some(i) => match self.table.get(i) {
                Some(skin) => vec![*skin; self.table.len()],
                None => return false,
            },
        };
        self.buffer.write(0, bytemuck::cast_slice(&table));
        true
    }

    /// What the scene set's binding 34 reads.
    pub(crate) fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Frees the buffer. The renderer calls this from its `Drop`, after the device has
    /// gone idle.
    pub(crate) fn destroy(&mut self, gpu: &Gpu) {
        gpu.destroy_buffer(std::mem::replace(&mut self.buffer, Buffer::null()));
    }
}

impl super::Renderer {
    /// Draws every model in `skin` (`Blueprints::skins`), or each in its own faction's
    /// again for `None`: `--skin` shows one unit in another skin. False if there is
    /// no such skin.
    pub fn wear_skin(&mut self, skin: Option<SkinId>) -> bool {
        self.skins.wear(skin.map(|s| usize::from(s.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn a_skin_packs_each_slot_where_the_shader_reads_it() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let bps = Blueprints::load(&dir).unwrap();
        let arc = bps.skins.iter().find(|s| s.key == "arc").unwrap();
        let g = GpuSkin::of(arc);
        let plate = gs::PLATE as usize;
        assert_eq!(g.slots[2 * plate][..3], arc.paint(arc.slots.plate.color));
        let emitter = gs::EMITTER as usize;
        assert_eq!(
            g.slots[2 * emitter + 1][..3],
            arc.highlight.map(|v| v * 5.0)
        );
        assert_eq!(g.slots[2 * emitter + 1][3], arc.slots.emitter.rough);
        let reg = bps.skins.iter().find(|s| s.key == "regency").unwrap();
        let film = reg.film.unwrap();
        assert_eq!(
            GpuSkin::of(reg).film[gs::FILM_COPPER as usize][..3],
            film.copper
        );
    }
}
