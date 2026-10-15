//! Report a Bug: a panel over the game where a player says what went wrong and
//! sends it to us, with a screenshot of the moment and what every crash report
//! carries (the machine, what the game was doing, the settings, the log:
//! `crash::snapshot`). The in-match menu and the main menu open it; the Worker
//! in `deploy/reports` takes it in and posts it to the team's channel.
//!
//! In a match the screenshot is the frame under the menu: opening hides the menu
//! for the frame the renderer is asked for, then shows the panel.
//! Presentation only: every float here ends on the screen.

mod send;

pub(crate) use send::{available, spawn, Outcome};

use crate::audio::Sfx;
use crate::ui::{id, ink, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use mc_builds::report::{Kind, Report, MAX_NOTE};
use mc_render::Shot;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;

/// The note field grows with what is typed, from this many lines to this many.
const NOTE_LINES: (usize, usize) = (5, 10);
/// Seconds the panel waits for its screenshot before it opens without one.
const SHOT_WAIT: f32 = 1.5;
const W: f32 = 580.0;
const PAD: f32 = 30.0;

#[derive(Default)]
pub(crate) struct BugReport {
    stage: Stage,
    note: String,
    /// The frame the panel was opened over, and whether it goes with the report.
    shot: Option<Arc<Shot>>,
    with_shot: bool,
    /// The panel's arrival, 0 to 1.
    enter: f32,
}

#[derive(Default)]
enum Stage {
    #[default]
    Closed,
    /// The frame under the menu is wanted: how far along asking for it is, and
    /// how long it has been waited for.
    Shooting {
        step: Shooting,
        waited: f32,
    },
    Writing,
    Sending(Receiver<Outcome>),
    Sent(String),
    Failed(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shooting {
    /// The frame the panel was opened in: the menu is already drawn into it.
    Opened,
    /// The menu is hidden from here on; the next frame is the one to ask for.
    Due,
    Asked,
}

impl BugReport {
    /// Opens the panel; `shoot`: with a screenshot of the frame under it.
    pub(crate) fn open(&mut self, shoot: bool) {
        if self.is_open() {
            return;
        }
        self.stage = if shoot {
            Stage::Shooting {
                step: Shooting::Opened,
                waited: 0.0,
            }
        } else {
            Stage::Writing
        };
        self.shot = None;
        self.with_shot = false;
        self.enter = 0.0;
    }

    pub(crate) fn is_open(&self) -> bool {
        !matches!(self.stage, Stage::Closed)
    }

    /// The screenshot is being taken: whatever opened the panel stays out of it.
    pub(crate) fn hides_menu(&self) -> bool {
        matches!(self.stage, Stage::Shooting { .. })
    }

    /// Asks the renderer for the frame to send and takes it when it is back.
    /// Call every frame, before rendering; `busy`: another asker's frame is
    /// still due, and this one waits for it.
    pub(crate) fn pump_capture(&mut self, renderer: &mut mc_render::Renderer, busy: bool) {
        let Stage::Shooting { step, .. } = &mut self.stage else {
            return;
        };
        match *step {
            Shooting::Opened => {
                *step = Shooting::Due;
                return;
            }
            Shooting::Due => {
                if !busy {
                    renderer.capture_next_frame();
                    *step = Shooting::Asked;
                }
                return;
            }
            Shooting::Asked => {}
        }
        match renderer.take_capture() {
            Some(Ok(shot)) => {
                self.shot = Some(Arc::new(shot));
                self.with_shot = true;
                self.stage = Stage::Writing;
            }
            Some(Err(e)) => {
                log::warn!("no screenshot for the bug report: {e}");
                self.stage = Stage::Writing;
            }
            None => {}
        }
    }

    /// Draws the panel over everything when it is open. `player`: the callsign
    /// the report is sent under.
    pub(crate) fn draw(&mut self, ui: &mut Ui, player: &str) {
        match &mut self.stage {
            Stage::Closed => return,
            Stage::Shooting { waited, .. } => {
                *waited += ui.dt;
                if *waited > SHOT_WAIT {
                    log::warn!("no screenshot for the bug report: the frame never came back");
                    self.stage = Stage::Writing;
                }
                return;
            }
            Stage::Sending(rx) => match rx.try_recv() {
                Ok(Ok(reference)) => {
                    self.note.clear();
                    self.stage = Stage::Sent(reference);
                }
                Ok(Err(e)) => self.stage = Stage::Failed(e),
                Err(TryRecvError::Disconnected) => {
                    self.stage = Stage::Failed("the report could not be sent".into())
                }
                Err(TryRecvError::Empty) => {}
            },
            _ => {}
        }
        self.enter = (self.enter + ui.dt / 0.22).min(1.0);
        let k = 1.0 - (1.0 - self.enter).powi(3);
        let (fade, shift) = (ui.fade, ui.shift);
        let (w, h) = (ui.size.x, ui.size.y);
        ui.fill(Rect::new(0.0, 0.0, w, h), ink(0.5 * k));
        ui.fade = k;
        ui.shift.y = 14.0 * (1.0 - k);

        let lead = ui.wrap(
            type_scale::CAPTION,
            "What went wrong, and what did you expect? What you were doing, what you \
             clicked, what you saw: anything helps. The game's log, your settings and \
             your machine's specs are sent with it.",
            W - 2.0 * PAD,
        );
        let field_w = W - 2.0 * PAD;
        let field_h = ui.text_area_height(field_w, &self.note, NOTE_LINES.0, NOTE_LINES.1);
        let toggle_h = if self.shot.is_some() { 56.0 } else { 0.0 };
        let tall = 128.0 + lead.len() as f32 * 19.0 + 16.0 + field_h + toggle_h + 44.0 + 64.0;
        let panel = Rect::new((w - W) * 0.5, (h - tall) * 0.5, W, tall);
        ui.panel(panel);
        let x = panel.x + PAD;
        ui.text_centred(
            panel.x + panel.w * 0.5,
            panel.y + 52.0,
            type_scale::TITLE,
            rgb(palette::TEXT, 1.0),
            "Report a Bug",
        );
        ui.text_centred(
            panel.x + panel.w * 0.5,
            panel.y + 86.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            "Sent straight to the developers",
        );
        let mut y = panel.y + 122.0;
        for line in &lead {
            ui.text(x, y, type_scale::CAPTION, rgb(palette::DIM, 1.0), line);
            y += 19.0;
        }
        y += 6.0;

        let sending = matches!(self.stage, Stage::Sending(_));
        let field = Rect::new(x, y, field_w, field_h);
        let note_id = id("bug-note", 0);
        if sending {
            // Read-only while it goes: what is sent is what was written.
            let mut shown = self.note.clone();
            let interactive = std::mem::replace(&mut ui.interactive, false);
            ui.text_area(note_id, field, &mut shown, MAX_NOTE);
            ui.interactive = interactive;
        } else {
            ui.text_area(note_id, field, &mut self.note, MAX_NOTE);
        }
        if self.note.is_empty() && ui.mem.editing != Some(note_id) {
            ui.text(
                field.x + 12.0,
                field.y + 17.5,
                type_scale::VALUE,
                rgb(palette::FAINT, 1.0),
                "Describe the problem",
            );
        }
        y = field.bottom() + 12.0;
        if self.shot.is_some() {
            let mut with_shot = self.with_shot;
            ui.toggle(
                id("bug-shot", 0),
                Rect::new(x, y, field_w, 44.0),
                "Include screenshot",
                "The screen as it was when you opened this",
                &mut with_shot,
            );
            if !sending {
                self.with_shot = with_shot;
            }
            y += toggle_h;
        }

        let can_send = available();
        let (said, tone) = match &self.stage {
            Stage::Writing if !can_send => (
                "This build has nowhere to send reports (a development build).".to_owned(),
                palette::WARN,
            ),
            Stage::Writing => (String::new(), palette::DIM),
            Stage::Sending(_) => (
                format!("Sending{}", ".".repeat(1 + (ui.time * 3.0) as usize % 3)),
                palette::DIM,
            ),
            Stage::Sent(reference) => (
                format!("Sent. Thank you! Your report is {reference}."),
                palette::ACCENT,
            ),
            Stage::Failed(e) => (format!("Could not send it: {e}"), palette::BAD),
            Stage::Closed | Stage::Shooting { .. } => (String::new(), palette::DIM),
        };
        if !said.is_empty() {
            ui.text_fit_left(
                x,
                y + 14.0,
                field_w,
                type_scale::CAPTION,
                rgb(tone, 1.0),
                &said,
            );
        }
        y += 44.0;

        let (close, send) = if matches!(self.stage, Stage::Sent(_)) {
            let done = ui.button(
                id("bug-done", 0),
                Rect::new(x, y, field_w, 48.0),
                "Done",
                ButtonKind::Primary,
                true,
            );
            (done, false)
        } else {
            let bw = (field_w - 12.0) * 0.5;
            let cancel = ui.button(
                id("bug-cancel", 0),
                Rect::new(x, y, bw, 48.0),
                "Cancel",
                ButtonKind::Secondary,
                !sending,
            );
            let send = ui.button(
                id("bug-send", 0),
                Rect::new(x + bw + 12.0, y, bw, 48.0),
                if matches!(self.stage, Stage::Failed(_)) {
                    "Try Again"
                } else {
                    "Send"
                },
                ButtonKind::Primary,
                can_send && !sending,
            );
            (cancel, send)
        };
        let escape = ui.interactive && !sending && ui.input.key(Key::Escape);
        if close || escape {
            ui.audio.play(Sfx::Back);
            self.close();
        } else if send {
            ui.audio.play(Sfx::Select);
            self.send(player);
        }
        ui.fade = fade;
        ui.shift = shift;
    }

    fn send(&mut self, player: &str) {
        let report = Report {
            kind: Kind::Bug,
            build: env!("MERIDIAN_BUILD").to_owned(),
            player: player.to_owned(),
            note: self.note.clone(),
            text: crate::crash::snapshot(&self.note),
            attachments: Vec::new(),
        };
        let shot = self.shot.clone().filter(|_| self.with_shot);
        self.stage = Stage::Sending(spawn(report, shot));
    }

    /// Closes the panel. An unsent note is kept for the next time it opens.
    fn close(&mut self) {
        self.stage = Stage::Closed;
        self.shot = None;
    }
}
