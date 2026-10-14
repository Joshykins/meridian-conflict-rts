//! A recording another build plays faithfully: getting that build and
//! watching the replay in it (docs/RELEASES.md).

use super::super::{id, palette, rgb, type_scale, ButtonKind, Rect, Ui};
use crate::audio::Sfx;
use crate::builds::{Download, Install, Status};
use crate::replay::Summary;

/// The build being fetched for a recording, by name.
#[derive(Default)]
pub struct Fetching(Option<(String, Download)>);

/// Under the selected match's facts: where it plays faithfully, and the way
/// there. Returns the height it took and whether Watch in Its Build was pressed.
pub fn draw(
    ui: &mut Ui,
    x: f32,
    y: f32,
    w: f32,
    s: &Summary,
    install: Option<&Install>,
    fetching: &mut Fetching,
) -> (f32, bool) {
    let Some(build) = s.other_build() else {
        return (0.0, false);
    };
    let caption = |ui: &mut Ui, y: f32, colour: u32, text: &str| {
        ui.text_fit_left(x, y, w, type_scale::CAPTION, rgb(colour, 1.0), text);
    };
    let Some(install) = install else {
        caption(
            ui,
            y,
            palette::DIM,
            &format!(
                "Plays as recorded in {build}: scripts/replay-build.sh {}",
                s.path.display()
            ),
        );
        return (24.0, false);
    };
    let button = Rect::new(x, y, 260.0, 44.0);
    if install.has(build) {
        let pressed = ui.button(
            id("replay-old-watch", 0),
            button,
            "Watch in Its Build",
            ButtonKind::Secondary,
            true,
        );
        if pressed {
            ui.audio.play(Sfx::Select);
        }
        return (56.0, pressed);
    }
    let status = match &fetching.0 {
        Some((b, d)) if b == build => Some(d.status()),
        _ => None,
    };
    let note = |ui: &mut Ui, colour: u32, text: &str| {
        caption(ui, y + 14.0, colour, text);
        (40.0, false)
    };
    match status {
        Some(Status::Checking) => note(ui, palette::DIM, &format!("Looking for {build}\u{2026}")),
        Some(Status::Downloading { progress }) => note(
            ui,
            palette::TEXT,
            &format!(
                "Downloading {build}\u{2026} {:.0}%",
                100.0 * progress.done as f32 / progress.total.max(1) as f32
            ),
        ),
        // Installed meanwhile: `has` says so next frame.
        Some(Status::Ready { .. }) | Some(Status::UpToDate) => (40.0, false),
        failed => {
            if let Some(Status::Failed(e)) = failed {
                caption(
                    ui,
                    y + 56.0,
                    palette::WARN,
                    &format!("It did not download: {e}"),
                );
            }
            if ui.button(
                id("replay-old-get", 0),
                button,
                "Download Its Build",
                ButtonKind::Secondary,
                true,
            ) {
                ui.audio.play(Sfx::Select);
                fetching.0 = Some((build.to_owned(), Download::build(install, build.to_owned())));
            }
            (80.0, false)
        }
    }
}
