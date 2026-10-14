//! Authoring updates use the regular terrain upload queue and GPU fence.
mod props;
use super::Renderer;
impl Renderer {
    pub fn restore_editor_terrain(
        &mut self,
        doc: &mc_map::edit::EditMap,
    ) -> Result<(), mc_map::MapError> {
        self.tile_cache
            .restore_editor(doc, &mut self.upload_scratch)
    }
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
    cliff_base: u32,
    cover: Option<crate::ground_cover::GroundCover>,
    environment: Option<(
        mc_data::weather::MapLook,
        Vec<mc_data::weather::Weather>,
        f32,
    )>,
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

impl Renderer {
    /// Refresh water and the terrain-following atmosphere after authoring edits.
    #[track_caller]
    pub fn edit_water(&mut self, level: mc_core::Fx) -> Result<(), crate::gpu::GpuError> {
        let tiles = &self.tile_cache;
        self.sky
            .edit_floor(&self.gpu, level.to_f32(), |xy| tiles.overview_height(xy))?;
        self.map_info.water_level = level;
        Ok(())
    }
}

impl Renderer {
    /// Authoring changes only restart weather when its parameters change.
    pub fn edit_environment(&mut self, config: &mc_data::weather::MapConfig) {
        let look = config.look();
        let weather = config.weathers(&mc_data::weather::SkyChoice::default());
        let hour = config.hour(None);
        if self
            .editor
            .environment
            .as_ref()
            .is_none_or(|(old, _, _)| old != &look)
        {
            self.set_map_look(&look);
        }
        if self
            .editor
            .environment
            .as_ref()
            .is_none_or(|(_, old, _)| old != &weather)
        {
            self.set_weathers(&weather);
        }
        if self
            .editor
            .environment
            .as_ref()
            .is_none_or(|(_, _, old)| *old != hour)
        {
            self.set_hour(hour);
        }
        self.editor.environment = Some((look, weather, hour));
    }
}

impl EditorState {
    pub(super) fn new(cliff_base: u32) -> Self {
        Self {
            cliff_base,
            ..Default::default()
        }
    }
}

impl Renderer {
    #[track_caller]
    fn edit_scenery_cover(
        &mut self,
        map: &mc_map::edit::EditMap,
    ) -> Result<(), crate::gpu::GpuError> {
        let cover = crate::ground_cover::edited(map);
        let bounds = [0, 0, cover.width - 1, cover.height - 1];
        for (layer, data) in [&cover.texels, &cover.ways, &cover.streets]
            .iter()
            .enumerate()
        {
            upload(
                &self.gpu,
                &self.ground_cover,
                layer as u32,
                0,
                cover.width,
                bounds,
                data,
            )?;
        }
        for (mip, (w, h, data)) in cover.mips.iter().enumerate() {
            upload(
                &self.gpu,
                &self.ground_cover,
                0,
                mip as u32 + 1,
                *w,
                [0, 0, w - 1, h - 1],
                data,
            )?;
        }
        let mut cliffs = crate::cliff_blocks::CliffBlocks::new(
            &cover,
            glam::Vec2::from(map.info.size_metres().to_f32()),
        );
        if let Some((look, _, _)) = &self.editor.environment {
            cliffs.set_look(look);
        }
        self.tile_cache.cliffs = cliffs;
        self.editor.cover = Some(cover);
        Ok(())
    }
}
