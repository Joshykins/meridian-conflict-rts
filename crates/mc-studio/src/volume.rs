//! The studio's volume: one level for everything it plays (songs, moments,
//! recordings, game sounds), set with a small speaker and slider kept in the
//! bottom-right corner of every screen and remembered between runs. It turns
//! the studio down, not the song: nothing in the song file changes.

use crate::audio::{Audio, RefCmd};
use crate::theme::{self, ACCENT, BG0, BG3, DIM, FAINT, TEXT};
use crate::widgets::{self, Icon};
use eframe::egui::{self, pos2, vec2, Align2, CornerRadius, Rect, Sense, Stroke, StrokeKind};

pub struct Volume {
    /// Where the slider is, 0..1. The gain is its square, which sounds even.
    pub level: f32,
    pub muted: bool,
    sent: Option<f32>,
}

impl Volume {
    /// The level last used, or three quarters.
    pub fn load() -> Volume {
        let saved = crate::files::read_config("volume")
            .and_then(|t| t.trim().parse::<f32>().ok())
            .filter(|v| v.is_finite());
        Volume {
            level: saved.unwrap_or(0.75).clamp(0.0, 1.0),
            muted: false,
            sent: None,
        }
    }

    pub fn gain(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.level * self.level
        }
    }

    /// Tells the audio thread when the level changed.
    fn sync(&mut self, audio: &Audio) {
        let g = self.gain();
        if self.sent != Some(g) {
            audio.send_ref(RefCmd::Volume(g));
            self.sent = Some(g);
        }
    }

    /// Draws the control and applies it. `in_status_bar`: the Advanced
    /// view's bottom strip is full at the corners, so it sits small in its middle.
    pub fn show(&mut self, ctx: &egui::Context, audio: &Audio, in_status_bar: bool) {
        let before = (self.level, self.muted);
        let (anchor, offset, height) = if in_status_bar {
            (Align2::CENTER_BOTTOM, vec2(0.0, -1.0), 24.0)
        } else {
            (Align2::RIGHT_BOTTOM, vec2(-14.0, -12.0), 34.0)
        };
        egui::Area::new(egui::Id::new("volume"))
            .anchor(anchor, offset)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| self.control(ui, height));
        if (self.level, self.muted) != before && !self.muted {
            crate::files::write_config("volume", &format!("{:.3}", self.level));
        }
        self.sync(audio);
    }

    fn control(&mut self, ui: &mut egui::Ui, height: f32) {
        let (rect, _) = ui.allocate_exact_size(vec2(176.0, height), Sense::hover());
        let p = ui.painter();
        p.rect(
            rect,
            CornerRadius::same((height * 0.5) as u8),
            theme::with_alpha(BG0, 235),
            Stroke::new(1.0, theme::line(16)),
            StrokeKind::Inside,
        );
        // The speaker mutes and unmutes.
        let icon =
            Rect::from_center_size(pos2(rect.left() + 20.0, rect.center().y), vec2(24.0, 24.0));
        let speaker = ui.interact(icon, ui.id().with("mute"), Sense::click());
        let quiet = self.muted || self.level == 0.0;
        let c = if speaker.hovered() { TEXT } else { DIM };
        widgets::draw_icon(
            ui.painter(),
            icon,
            if quiet {
                Icon::SpeakerOff
            } else {
                Icon::Speaker
            },
            c,
        );
        if speaker
            .on_hover_text(if self.muted { "Sound on" } else { "Mute" })
            .clicked()
        {
            self.muted = !self.muted;
        }
        // The slider: drag or click along it, or scroll over the whole control.
        let track = Rect::from_min_max(
            pos2(rect.left() + 40.0, rect.center().y - 9.0),
            pos2(rect.right() - 46.0, rect.center().y + 9.0),
        );
        let slide = ui.interact(track, ui.id().with("level"), Sense::click_and_drag());
        if let Some(pos) = slide.interact_pointer_pos() {
            if slide.dragged() || slide.clicked() {
                self.level = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0);
                self.muted = false;
            }
        }
        if ui.rect_contains_pointer(rect) {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.level = (self.level + scroll * 0.002).clamp(0.0, 1.0);
                self.muted = false;
                // The page under the control does not scroll with it.
                ui.input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
            }
        }
        let p = ui.painter();
        let y = track.center().y;
        p.line_segment(
            [pos2(track.left(), y), pos2(track.right(), y)],
            Stroke::new(4.0, BG3),
        );
        let x = track.left() + track.width() * self.level;
        let fill = if self.muted { FAINT } else { ACCENT };
        p.line_segment([pos2(track.left(), y), pos2(x, y)], Stroke::new(4.0, fill));
        let grab = if slide.hovered() || slide.dragged() {
            7.0
        } else {
            6.0
        };
        p.circle_filled(pos2(x, y), grab, if self.muted { DIM } else { TEXT });
        p.text(
            pos2(rect.right() - 14.0, rect.center().y),
            Align2::RIGHT_CENTER,
            if self.muted {
                "off".to_owned()
            } else {
                format!("{:.0}", self.level * 100.0)
            },
            theme::font_semi(13.0),
            if self.muted { FAINT } else { DIM },
        );
    }
}
