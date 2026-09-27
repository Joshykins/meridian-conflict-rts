//! Watching a replay: the timeline along the bottom. The match clock, the
//! marks made on it (`crate::issues`), a track to click or drag to any
//! moment, and buttons to step about and to go from mark to mark.

use super::{HudAction, Scene};
use crate::audio::Sfx;
use crate::issues::{self, Mark};
use crate::ui::{id, ink, palette, rgb, type_scale, ButtonKind, Rect, Ui};
use mc_core::TICKS_PER_SECOND;

/// A step of the -/+ buttons, in ticks: 30 s.
const STEP: u32 = 30 * TICKS_PER_SECOND;
/// A jump to a mark lands this far in front of it, so the moment plays: 5 s.
const LEAD_IN: u32 = 5 * TICKS_PER_SECOND;
pub const HEIGHT: f32 = 96.0;

#[derive(Default)]
pub struct ReplayBar {
    /// The replay being watched, by match id; `None` in a live match.
    id: Option<String>,
    marks: Vec<Mark>,
    /// Where a drag along the track is, 0..1.
    drag: Option<f32>,
}

/// `m:ss`, or `h:mm:ss` past the hour.
pub fn clock(tick: u32) -> String {
    let s = tick / TICKS_PER_SECOND;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

impl ReplayBar {
    pub fn watching(id: String) -> ReplayBar {
        ReplayBar {
            marks: issues::marks_of(&id),
            id: Some(id),
            drag: None,
        }
    }

    /// A mark made while watching shows on the track at once.
    pub fn marked(&mut self, mark: Mark) {
        if self.id.is_some() {
            self.marks.push(mark);
        }
    }

    /// Draws the bar with its bottom edge at `bottom`, centred in `[left, right]`.
    pub fn draw(
        &mut self,
        ui: &mut Ui,
        s: &Scene,
        left: f32,
        right: f32,
        bottom: f32,
        actions: &mut Vec<HudAction>,
    ) -> Option<Rect> {
        let replay = s.view.status.replay?;
        let length = replay.length.max(1);
        let now = s.view.status.tick.min(length);
        let w = (right - left).clamp(520.0, 1100.0);
        let r = Rect::new((left + right - w) * 0.5, bottom - HEIGHT, w, HEIGHT);
        ui.panel(r);
        let at = |t: u32| r.x + 16.0 + (r.w - 32.0) * t as f32 / length as f32;

        // Title and clock.
        ui.section(
            r.x + 16.0,
            r.y + 14.0,
            r.w * 0.45,
            &format!("Replay  {}", self.id.as_deref().unwrap_or("")),
        );
        let speed = if s.view.paused {
            "Paused".to_string()
        } else {
            super::speed_label(s.view.speed)
        };
        ui.text_right(
            r.right() - 16.0,
            r.y + 14.0,
            type_scale::VALUE,
            rgb(palette::TEXT, 1.0),
            &format!("{}  /  {}    {speed}", clock(now), clock(length)),
        );

        // The track: played part, where a seek is going, the marks, the pointer.
        let track = Rect::new(r.x + 16.0, r.y + 36.0, r.w - 32.0, 6.0);
        let hit = Rect::new(track.x - 6.0, track.y - 14.0, track.w + 12.0, 30.0);
        let res = ui.interact(id("replay-track", 0), hit, true);
        ui.fill(track, ink(0.8));
        ui.frame(track, rgb(palette::LINE, 0.18));
        ui.fill(
            Rect::new(track.x, track.y, at(now) - track.x, track.h),
            rgb(palette::ACCENT, 0.85),
        );
        if let Some(to) = replay.seeking {
            let (a, b) = (at(now.min(to)), at(now.max(to)));
            ui.fill(
                Rect::new(a, track.y - 2.0, b - a, track.h + 4.0),
                rgb(palette::WARN, 0.35 + 0.25 * (ui.time * 6.0).sin().abs()),
            );
            ui.fill(
                Rect::new(at(to) - 1.0, track.y - 6.0, 2.0, track.h + 12.0),
                rgb(palette::WARN, 1.0),
            );
        }
        ui.fill(
            Rect::new(at(now) - 1.5, track.y - 5.0, 3.0, track.h + 10.0),
            rgb(palette::TEXT, 1.0),
        );
        let frac = |x: f32| ((x - track.x) / track.w).clamp(0.0, 1.0);
        let tick_at = |k: f32| (k * length as f32).round() as u32;
        let mut tip: Option<String> = None;
        let mut jump: Option<u32> = None;
        for (i, m) in self.marks.iter().enumerate() {
            let x = at(m.tick.min(length));
            let pin = Rect::new(x - 6.0, track.y - 16.0, 12.0, 12.0);
            let p = ui.interact(id("replay-mark", i), pin, true);
            ui.fill(
                Rect::new(x - 1.0, track.y - 8.0, 2.0, track.h + 8.0),
                rgb(palette::WARN, 0.8),
            );
            ui.fill(
                Rect::new(x - 4.0, track.y - 16.0, 8.0, 8.0),
                rgb(palette::WARN, 0.75 + 0.25 * p.glow),
            );
            if p.hovered {
                tip = Some(format!(
                    "Mark {}  {}{}",
                    m.number,
                    clock(m.tick),
                    if m.note.is_empty() {
                        String::new()
                    } else {
                        format!("  \u{b7}  {}", m.note)
                    }
                ));
            }
            if p.clicked {
                jump = Some(m.tick.saturating_sub(LEAD_IN));
            }
        }
        if res.held || self.drag.is_some() {
            if ui.input.down {
                self.drag = Some(frac(ui.cursor.x));
            } else if let Some(k) = self.drag.take() {
                jump = Some(tick_at(k));
            }
        }
        let hover = self
            .drag
            .or_else(|| (res.hovered && tip.is_none()).then(|| frac(ui.cursor.x)));
        if let Some(k) = hover {
            let x = track.x + track.w * k;
            ui.fill(
                Rect::new(x - 0.5, track.y - 6.0, 1.0, track.h + 12.0),
                rgb(palette::TEXT, 0.6),
            );
            tip.get_or_insert_with(|| clock(tick_at(k)));
        }
        if let Some(t) = tip {
            let x = ui.cursor.x.clamp(r.x + 60.0, r.right() - 60.0);
            let tw = ui.text_width(type_scale::MICRO, &t) + 16.0;
            let b = Rect::new(x - tw * 0.5, r.y - 26.0, tw, 20.0);
            ui.fill(b, ink(0.85));
            ui.text_centred(
                b.x + b.w * 0.5,
                b.mid_y(),
                type_scale::MICRO,
                rgb(palette::TEXT, 1.0),
                &t,
            );
        }

        // Buttons: to the start, back and on by 30 s, play, and from mark to mark.
        let prev = self.marks.iter().rev().find(|m| m.tick + LEAD_IN < now);
        let next = self.marks.iter().find(|m| m.tick > now + LEAD_IN);
        let row = r.y + 56.0;
        let mut x = r.x + 16.0;
        let button = |ui: &mut Ui, x: &mut f32, label: &str, w: f32, on: bool| {
            let b = Rect::new(*x, row, w, 28.0);
            *x += w + 6.0;
            ui.button(
                id("replay-button", 0) ^ id(label, 0),
                b,
                label,
                ButtonKind::Secondary,
                on,
            )
        };
        if button(ui, &mut x, "Start", 64.0, now > 0) {
            jump = Some(0);
        }
        if button(ui, &mut x, "-30 s", 64.0, now > 0) {
            jump = Some(now.saturating_sub(STEP));
        }
        let play = if s.view.paused { "Play" } else { "Pause" };
        if button(ui, &mut x, play, 72.0, true) {
            actions.push(HudAction::Pause);
        }
        if button(ui, &mut x, "+30 s", 64.0, now < length) {
            jump = Some((now + STEP).min(length));
        }
        x += 12.0;
        if button(ui, &mut x, "Prev Mark", 96.0, prev.is_some()) {
            jump = prev.map(|m| m.tick.saturating_sub(LEAD_IN));
        }
        if button(ui, &mut x, "Next Mark", 96.0, next.is_some()) {
            jump = next.map(|m| m.tick.saturating_sub(LEAD_IN));
        }
        if let Some(t) = jump {
            ui.audio.play(Sfx::Tick);
            actions.push(HudAction::Seek(t.min(length)));
        }
        Some(r)
    }
}

#[cfg(test)]
mod tests {
    use super::clock;

    #[test]
    fn clock_reads_minutes_then_hours() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(5234), "8:43");
        assert_eq!(clock(37_000), "1:01:40");
    }
}
