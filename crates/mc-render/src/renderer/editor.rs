//! Authoring updates use the regular terrain upload queue and GPU fence.
use super::Renderer;
impl Renderer {
    pub fn edit_terrain(
        &mut self,
        document: &mc_map::edit::EditMap,
        rect: [u32; 4],
    ) -> Result<(), mc_map::MapError> {
        self.tile_cache
            .edit_terrain(document, rect, &mut self.upload_scratch)
    }
}

#[derive(Default)]
pub(super) struct EditorState {
    cover: Option<crate::ground_cover::GroundCover>,
}
impl Renderer {
    /// Update an inclusive rectangle in a source layer's sample grid.
    #[track_caller]
    pub fn edit_cover(
        &mut self,
        document: &mc_map::edit::EditMap,
        layer: mc_map::edit::Layer,
        rect: [u32; 4],
    ) -> Result<(), crate::gpu::GpuError> {
        let cover = self
            .editor
            .cover
            .get_or_insert_with(|| crate::ground_cover::ground_cover(self.tile_cache.source()));
        let mut bounds = cover.paint(document, layer, rect);
        let (index, data) = match layer {
            mc_map::edit::Layer::Snow => (0, &cover.texels),
            mc_map::edit::Layer::Ways => (1, &cover.ways),
            mc_map::edit::Layer::Streets => (2, &cover.streets),
        };
        upload(
            &self.gpu,
            &self.ground_cover,
            index,
            0,
            cover.width,
            bounds,
            data,
        )?;
        if layer == mc_map::edit::Layer::Snow {
            for (mip, (w, h, data)) in cover.mips.iter().enumerate() {
                bounds = [
                    (bounds[0] / 2).min(w - 1),
                    (bounds[1] / 2).min(h - 1),
                    (bounds[2] / 2).min(w - 1),
                    (bounds[3] / 2).min(h - 1),
                ];
                upload(
                    &self.gpu,
                    &self.ground_cover,
                    0,
                    mip as u32 + 1,
                    *w,
                    bounds,
                    data,
                )?;
            }
        }
        Ok(())
    }
}
#[track_caller]
fn upload(
    gpu: &crate::gpu::Gpu,
    image: &crate::gpu::Image,
    layer: u32,
    mip: u32,
    width: u32,
    rect: [u32; 4],
    data: &[u8],
) -> Result<(), crate::gpu::GpuError> {
    let w = rect[2] - rect[0] + 1;
    let h = rect[3] - rect[1] + 1;
    let mut bytes = Vec::with_capacity((w * h * 4) as usize);
    for y in rect[1]..=rect[3] {
        let start = ((y * width + rect[0]) * 4) as usize;
        bytes.extend_from_slice(&data[start..start + (w * 4) as usize]);
    }
    gpu.upload_image(
        image,
        layer,
        mip,
        Some(ash::vk::Rect2D {
            offset: ash::vk::Offset2D {
                x: rect[0] as i32,
                y: rect[1] as i32,
            },
            extent: ash::vk::Extent2D {
                width: w,
                height: h,
            },
        }),
        &bytes,
        false,
    )
}
