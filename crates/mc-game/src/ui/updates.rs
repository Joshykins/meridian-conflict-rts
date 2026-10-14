//! The install's builds as the front end shows them: this channel's update,
//! downloaded in the background from the moment the front end opens, and a
//! card at the menu's foot that offers the restart into it (docs/RELEASES.md).

use std::sync::Arc;

use super::{id, palette, rgb, type_scale, ButtonKind, Rect, Ui};
use crate::audio::Sfx;
use crate::builds::{Download, Install, Status};

pub struct Builds {
    /// `None` outside an install (a dev build, or run without the launcher).
    pub install: Option<Arc<Install>>,
    update: Option<Download>,
}

impl Builds {
    pub fn new() -> Builds {
        let install = Install::find().map(Arc::new);
        let update = install.as_deref().map(Download::update);
        Builds { install, update }
    }
}

const MARGIN: f32 = 20.0;
const W: f32 = 404.0;
const H: f32 = 76.0;

/// The update card, when there is something to say; true when Restart was pressed.
pub fn card(ui: &mut Ui, builds: &Builds, enter: f32) -> bool {
    let Some(update) = &builds.update else {
        return false;
    };
    let (line, ready) = match update.status() {
        Status::Checking | Status::UpToDate => return false,
        Status::Downloading { progress } => (
            format!(
                "Downloading an update\u{2026} {:.0}%",
                100.0 * progress.done as f32 / progress.total.max(1) as f32
            ),
            false,
        ),
        Status::Ready { number, .. } => (format!("Build {number} is ready"), true),
        Status::Failed(e) => (format!("The update did not download: {e}"), false),
    };
    ui.fade = enter;
    let r = Rect::new(MARGIN, ui.size.y - MARGIN - H, W, H);
    ui.panel(r);
    ui.text(
        r.x + 18.0,
        r.y + 26.0,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        "Update",
    );
    ui.text_fit_left(
        r.x + 18.0,
        r.y + 52.0,
        if ready { W - 170.0 } else { W - 36.0 },
        type_scale::VALUE,
        rgb(palette::TEXT, 1.0),
        &line,
    );
    let pressed = ready
        && ui.button(
            id("update-restart", 0),
            Rect::new(r.right() - 140.0, r.y + 16.0, 124.0, 44.0),
            "Restart",
            ButtonKind::Primary,
            true,
        );
    if pressed {
        ui.audio.play(Sfx::Select);
    }
    ui.fade = 1.0;
    pressed
}
