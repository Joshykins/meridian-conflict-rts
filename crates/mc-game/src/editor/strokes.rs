//! Continuous spatial brush strokes. Scenery regeneration waits for release.
use super::*;
#[derive(Default)]
pub(super) struct Stroke {
    last: Option<Vec2>,
}
impl Editor {
    pub(super) fn finish_stroke(
        &mut self,
        renderer: &mut mc_render::Renderer,
    ) -> Result<(), String> {
        self.stroke.last = None;
        if self.doc.end_stroke() {
            self.fresh = true;
            renderer
                .edit_water(self.doc.map.info.water_level)
                .map_err(|e| e.to_string())?;
            renderer
                .edit_props(&self.doc.map, &self.doc.placed)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub(super) fn world_input(
        &mut self,
        ui: &mut Ui,
        renderer: &mut mc_render::Renderer,
        bottom: f32,
        card: Rect,
        outliner: Rect,
    ) {
        let input = ui.input;
        if !input.down && self.doc.stroking() {
            if let Err(e) = self.finish_stroke(renderer) {
                self.message = e;
            }
        }
        let viewport = ui.cursor.y > 90.0
            && ui.cursor.y < bottom
            && !card.contains(ui.cursor)
            && !outliner.contains(ui.cursor);
        if self.gizmo_input(ui, renderer, viewport && !self.confirm_exit) {
            return;
        }
        if !viewport || self.confirm_exit {
            self.stroke.last = None;
            return;
        }
        if input.scroll != 0.0 {
            self.navigation.wheel(&mut self.camera, input.scroll);
        }
        let (origin, dir) = self.camera.ray(input.cursor);
        let Some(hit) = renderer.pick_ground(origin, dir) else {
            return;
        };
        let brush = matches!(self.tool,1..=4|13|14|16..=20);
        if brush && (input.pressed || (input.down && self.doc.stroking())) {
            if input.pressed {
                self.doc.begin_stroke();
            }
            let at = hit.truncate();
            let spacing = (self.radius * 0.25).max(4.0);
            let mut commands = Vec::new();
            if let Some(previous) = self.stroke.last {
                let delta = at - previous;
                let distance = delta.length();
                let steps = (distance / spacing).floor() as usize;
                // A viewport pointer cannot normally span this many stamps in
                // one frame; keep any remaining distance for the next frame.
                for i in 1..=steps.min(512) {
                    let p = previous + delta / distance * (i as f32 * spacing);
                    if let Some(c) = self.world_command(p.to_array(), input.cursor, renderer) {
                        commands.push(c);
                    }
                    self.stroke.last = Some(p);
                }
            } else {
                if let Some(c) = self.world_command(at.to_array(), input.cursor, renderer) {
                    commands.push(c);
                }
                self.stroke.last = Some(at);
            }
            if !commands.is_empty() {
                if let Err(e) = self.apply_batch(&commands, renderer) {
                    self.message = e;
                }
            }
        } else if input.pressed {
            if let Some(command) =
                self.world_command(self.gizmo.snap([hit.x, hit.y]), input.cursor, renderer)
            {
                let placing = matches!(command, Command::Unit { .. } | Command::Doodad { .. });
                match self.apply(command, renderer) {
                    Err(e) => self.message = e,
                    Ok(()) if placing => {
                        self.name = format!(
                            "{}_{}",
                            if self.tool == 27 {
                                &self.doodad.kind
                            } else {
                                &self.blueprint
                            },
                            self.doc.placed.next_id
                        )
                    }
                    Ok(()) => {}
                }
            }
        }
    }
}
