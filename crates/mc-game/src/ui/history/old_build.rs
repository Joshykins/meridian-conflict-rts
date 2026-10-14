//! A recording another build plays faithfully: watching it in that build,
//! which the launcher fetches first when it is not installed (docs/RELEASES.md).

use super::super::{id, palette, rgb, type_scale, ButtonKind, Rect, Ui};
use crate::audio::Sfx;
use crate::builds::Install;
use crate::replay::Summary;

/// Under the selected match's facts: where it plays faithfully, and the way
/// there. Returns the height it took and whether Watch in Its Build was pressed.
pub fn draw(
    ui: &mut Ui,
    x: f32,
    y: f32,
    w: f32,
    s: &Summary,
    install: Option<&Install>,
) -> (f32, bool) {
    let Some(build) = s.other_build() else {
        return (0.0, false);
    };
    let caption = |ui: &mut Ui, y: f32, text: &str| {
        ui.text_fit_left(x, y, w, type_scale::CAPTION, rgb(palette::DIM, 1.0), text);
    };
    let Some(install) = install else {
        caption(
            ui,
            y,
            &format!(
                "Plays as recorded in {build}: scripts/replay-build.sh {}",
                s.path.display()
            ),
        );
        return (24.0, false);
    };
    let pressed = ui.button(
        id("replay-old-watch", 0),
        Rect::new(x, y, 260.0, 44.0),
        "Watch in Its Build",
        ButtonKind::Secondary,
        true,
    );
    if pressed {
        ui.audio.play(Sfx::Select);
    }
    if install.has(build) {
        return (56.0, pressed);
    }
    caption(
        ui,
        y + 66.0,
        &format!("The launcher downloads {build} first"),
    );
    (88.0, pressed)
}
