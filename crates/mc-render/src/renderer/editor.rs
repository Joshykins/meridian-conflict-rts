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
