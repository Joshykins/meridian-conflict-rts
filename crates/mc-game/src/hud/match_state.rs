//! What stands over the whole battlefield: loading, and a match that cannot go on.
//! A paused match is shown by the frame round the screen (`pause.rs`).

use super::{Hud, Scene};
use crate::ui::{palette, rgb, type_scale, Rect, Ui};

impl Hud {
    /// Loading and a fatal error. The result of the match is the menu's to show.
    pub(super) fn match_state(&mut self, ui: &mut Ui, s: &Scene) {
        let view = s.view;
        let (w, h) = (ui.size.x, ui.size.y);
        // A network match's own moments come first (`net_cards.rs`).
        if let Some(link) = s.net {
            if let Some(report) = &link.desync {
                return self.desync_card(ui, s, report);
            }
            if let Some(r) = &link.rejoining {
                return self.rejoin_band(ui, r);
            }
            if view.status.tick == 0 && view.status.error.is_none() && link.loading.is_some() {
                return self.waiting_card(ui, s, link);
            }
        }
        if let Some(e) = &view.status.error {
            let tw = ui.text_width(type_scale::BODY, e) + 80.0;
            let r = Rect::new((w - tw) * 0.5, h * 0.38, tw, 84.0);
            ui.panel(r);
            ui.text_centred(
                w * 0.5,
                r.y + 28.0,
                type_scale::CAPTION,
                rgb(palette::BAD, 1.0),
                "The Match Cannot Continue",
            );
            ui.text_centred(
                w * 0.5,
                r.y + 56.0,
                type_scale::BODY,
                rgb(palette::TEXT, 1.0),
                e,
            );
        } else if view.status.tick == 0 && view.status.winner.is_none() {
            ui.text_centred(
                w * 0.5,
                h * 0.46,
                type_scale::TITLE,
                rgb(palette::TEXT, 0.9),
                "Establishing Uplink",
            );
            let k = (ui.time * 0.8).fract();
            ui.fill(
                Rect::new(w * 0.5 - 120.0 + 200.0 * k, h * 0.46 + 34.0, 40.0, 2.0),
                rgb(palette::TEXT, 1.0),
            );
            ui.fill(
                Rect::new(w * 0.5 - 120.0, h * 0.46 + 34.0, 240.0, 1.0),
                rgb(palette::LINE, 0.25),
            );
        }
    }
}
