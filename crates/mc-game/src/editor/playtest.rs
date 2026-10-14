//! A playtest borrows the current GPU scene. Export sorts props by tile, so
//! install that same ordering before the simulation publishes prop indices.
use super::*;
impl Editor {
    pub(crate) fn poll_test(
        &mut self,
        game: &mut crate::game::Game,
        renderer: &mut mc_render::Renderer,
    ) -> bool {
        self.api.test_state = Some(game.editor_test_state());
        matches!(self.api_frame(renderer, Some(game)), Some(Event::StopTest))
    }

    pub(crate) fn prepare_test_renderer(
        &mut self,
        map: &MapFile,
        renderer: &mut mc_render::Renderer,
    ) -> Result<(), String> {
        let doc = mc_map::edit::EditMap::load(map).map_err(|e| e.to_string())?;
        renderer
            .edit_props(&doc, map.placed())
            .map_err(|e| e.to_string())?;
        let (w, h) = doc.info.size_cells();
        renderer
            .edit_terrain(&doc, [0, 0, w, h])
            .map_err(|e| e.to_string())?;
        self.held.clear();
        Ok(())
    }
}
