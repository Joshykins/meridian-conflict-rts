//! The pause strip. A paused match is still played: the camera moves, orders and
//! spawns are carried out at once (the sim applies them without the clock moving),
//! so the strip sits at the top of the screen, clear of the battlefield.

use super::{free_camera, Hud, HudAction, Scene, ECONOMY_W, EDGE, GAP, STALL_CHIP_W, TOP_BAR_W};
use crate::audio::Sfx;
use crate::ui::{id, palette, rgb, type_scale, ButtonKind, Rect, Ui};
use glam::Vec2;

impl Hud {
    /// The battlefield held still: a strip at the top of the screen, and a way back.
    pub(super) fn pause_card(&mut self, ui: &mut Ui, s: &Scene) {
        let fold = self.fold_begin(ui, free_camera::Part::Top);
        let w = ui.size.x;
        let k = ui.ease(id("pause-card", 0), 1.0, 9.0);
        // A hairline along the top edge, brightest over the strip: the whole view is held.
        for (x, from, to) in [(0.0, 0.0, 0.6 * k), (w * 0.5, 0.6 * k, 0.0)] {
            ui.gradient_h(
                Rect::new(x, 0.0, w * 0.5, 2.0),
                rgb(palette::LINE, from),
                rgb(palette::LINE, to),
            );
        }
        // A network match is held for everyone, by the relay: orders wait for the clock too,
        // and anyone playing may let it go.
        let (note, can_resume) = match s.net.and_then(|l| l.paused_by) {
            Some(by) => {
                let who = if by == Some(s.view.local) && !s.view.observing {
                    "You".to_owned()
                } else {
                    by.and_then(|p| s.view.status.players.get(p as usize))
                        .map_or_else(|| "An observer".to_owned(), |p| p.name.clone())
                };
                (
                    format!("{who} paused for everyone  \u{b7}  Any player may resume"),
                    !s.view.observing,
                )
            }
            None => (
                "Orders go through now  \u{b7}  The clock waits".to_owned(),
                true,
            ),
        };
        let note = note.as_str();
        let text_w = ui.text_width(type_scale::MICRO, note).max(96.0);
        // Centred, but clear of the economy (and its stall chip) and the clock bar.
        let wide = text_w + 190.0;
        let left = EDGE + ECONOMY_W + GAP + STALL_CHIP_W + GAP;
        let right = w - EDGE - TOP_BAR_W - GAP - wide;
        let r = Rect::new(
            ((w - wide) * 0.5).max(left).min(right.max(left)),
            EDGE - 6.0 * (1.0 - k),
            wide,
            44.0,
        );
        self.glass(ui, r);
        let mid = r.mid_y();
        // Instrument mark: broken rings turning against each other.
        let rc = Vec2::new(r.x + 26.0, mid);
        for i in 0..3 {
            let a = ui.time * 0.5 + i as f32 * std::f32::consts::TAU / 3.0;
            ui.arc(rc, 9.0, a, a + 1.5, 1.4, rgb(palette::TEXT, 0.9 * k));
            ui.arc(rc, 13.0, -a, -a + 0.7, 1.0, rgb(palette::LINE, 0.5 * k));
        }
        ui.disc(rc, 1.5, rgb(palette::TEXT, k));
        ui.text(
            r.x + 50.0,
            mid - 8.0,
            type_scale::OVERLINE,
            rgb(0xFFFFFF, k),
            "Paused",
        );
        ui.text(
            r.x + 50.0,
            mid + 9.0,
            type_scale::MICRO,
            rgb(palette::DIM, k),
            note,
        );
        if ui.button(
            id("pause-card-resume", 0),
            Rect::new(r.right() - 106.0, r.y + 7.0, 96.0, 30.0),
            "Resume",
            ButtonKind::Primary,
            can_resume,
        ) {
            ui.audio.play(Sfx::Back);
            self.actions.push(HudAction::Pause);
        }
        self.fold_end(ui, fold);
    }
}
