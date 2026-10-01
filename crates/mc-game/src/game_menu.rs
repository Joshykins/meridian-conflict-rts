//! What stands over the match: the in-match menu (`ui/pause.rs`), the settings
//! screen over it, and the battle report (`ui/report`), which the match's end
//! opens by itself a moment after the last commander falls.

use super::{Game, GameEvent};
use crate::audio::{Audio, Sfx};
use crate::settings::Settings;
use crate::ui::pause::{self, PauseAction};
use crate::ui::report::{Ctx, Place, Report, ReportAction};
use crate::ui::{options, Ui};

/// Seconds between the match being decided and its report coming up: the last
/// commander's blast is seen first.
const RESULT_DELAY: f32 = 3.0;

pub(super) struct Menu {
    /// Opened by the match's end, on the report: the clock is not held, and
    /// closing the report closes it all.
    result: bool,
    enter: f32,
    closing: bool,
    /// Opened since the last frame: the key press that opened it (Escape) is still in
    /// this frame's input and must not close it again.
    fresh: bool,
    /// The settings screen is up over the menu; how far in it is.
    settings: Option<(f32, bool)>,
    /// The battle report is up over the menu: it, how far in it is, and whether it is going.
    report: Option<(Box<Report>, f32, bool)>,
}

/// Where the match's result is: not decided, decided and its report due in so many
/// seconds, or shown.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub(super) enum ResultCue {
    #[default]
    Undecided,
    Due(f32),
    Shown,
}

impl Game {
    pub(super) fn open_menu(&mut self, audio: &Audio) {
        if self.menu.is_none() {
            // Opened by a key, so there is no control to make the sound.
            audio.play(Sfx::Select);
            self.menu = Some(Menu {
                result: false,
                enter: 0.0,
                closing: false,
                fresh: true,
                settings: None,
                report: None,
            });
            self.release_held();
        }
    }

    /// Held keys and drags must not carry on underneath the menu.
    fn release_held(&mut self) {
        self.keys.clear();
        self.left_down = None;
        self.place_from = None;
        self.orders.cancel();
        self.middle_down = false;
        self.end_orbit();
    }

    /// The report on the match so far, from the sim thread's record of it.
    fn report(&self) -> Box<Report> {
        let chronicle = self.sim.chronicle.lock().unwrap();
        let local = (!self.view.observing).then_some(self.view.local);
        Box::new(Report::new(&chronicle, &self.blueprints, local))
    }

    /// The report can be read once the match is decided, and at any time by an
    /// observer or a side already out of the match (who see every side anyway).
    pub(super) fn report_open_to_all(&self) -> bool {
        self.view.status.winner.is_some() || self.view.observing || self.defeated()
    }

    /// Once the match is decided: the report, a moment later.
    pub(super) fn result_frame(&mut self, dt: f32) {
        match self.result {
            ResultCue::Undecided if self.view.status.winner.is_some() => {
                self.result = ResultCue::Due(RESULT_DELAY);
            }
            ResultCue::Due(left) if left - dt > 0.0 => self.result = ResultCue::Due(left - dt),
            ResultCue::Due(_) => {
                self.result = ResultCue::Shown;
                // No stinger: a commander's end is its own detonation, and a ringing chord
                // over it cut across the blast (user ask, 2026-09-24).
                let report = self.report();
                self.menu = Some(Menu {
                    result: true,
                    enter: 1.0,
                    closing: false,
                    fresh: true,
                    settings: None,
                    report: Some((report, 0.0, false)),
                });
                self.release_held();
            }
            _ => {}
        }
    }

    /// A single-player match holds its clock under the menu, not under the result.
    pub(super) fn menu_holds_clock(&self) -> bool {
        self.menu.as_ref().is_some_and(|m| !m.result && !m.closing)
    }

    /// Draws the menu and what is over it; returns what the player chose.
    pub(super) fn menu_frame(
        &mut self,
        ui: &mut Ui,
        settings: &mut Settings,
        changed: (&mut bool, &mut bool),
        surrender: bool,
        dt: f32,
    ) -> Option<GameEvent> {
        let (settings_changed, display_changed) = changed;
        let mut event = None;
        // Leaving a network match this side is still in gives it up first: the side is
        // defeated on every machine rather than left standing idle, and the match is
        // left once that is carried out.
        let mut give_up = false;
        let readable = self.report_open_to_all();
        let ctx_map = self.map.clone();
        let blueprints = self.blueprints.clone();
        let colors = self.view.colors;
        let local = (!self.view.observing).then_some(self.view.local);
        let owns_clock = self.view.status.owns_clock;
        let wanted_report = {
            let menu = self.menu.as_mut()?;
            menu.enter =
                (menu.enter + if menu.closing { -dt / 0.14 } else { dt / 0.28 }).clamp(0.0, 1.0);
            let fresh = std::mem::take(&mut menu.fresh);
            let mut wanted = false;
            if let Some((report, enter, closing)) = &mut menu.report {
                *enter = (*enter + if *closing { -dt / 0.18 } else { dt / 0.45 }).clamp(0.0, 1.0);
                ui.interactive = !*closing && !fresh && !menu.closing;
                let ctx = Ctx {
                    blueprints: &blueprints,
                    colors: &colors,
                    local,
                    map_name: ctx_map.name(),
                    thumbs: &self.hud.thumbs,
                    chart: crate::hud::MINIMAP_SLOT,
                    place: Place::Match { surrender },
                };
                // It goes with the menu when that closes (the result's report does).
                let shown = (1.0 - (1.0 - *enter).powi(3)) * (1.0 - (1.0 - menu.enter).powi(3));
                match report.draw(ui, &ctx, shown) {
                    Some(ReportAction::Close) if menu.result => menu.closing = true,
                    Some(ReportAction::Close) => *closing = true,
                    Some(ReportAction::Leave) if surrender => {
                        give_up = true;
                        menu.closing = true;
                    }
                    Some(ReportAction::Leave) => event = Some(GameEvent::Leave),
                    Some(ReportAction::Quit) => event = Some(GameEvent::Quit),
                    Some(ReportAction::Watch) | None => {}
                }
                if *closing && *enter <= 0.0 {
                    menu.report = None;
                }
            } else {
                let in_settings = menu.settings.is_some();
                ui.interactive = !menu.closing && !in_settings && !fresh;
                let eased = 1.0 - (1.0 - menu.enter).powi(3);
                let out = pause::draw(ui, owns_clock, surrender, readable, settings, eased);
                *settings_changed |= out.settings_changed;
                match out.action {
                    Some(PauseAction::Resume) => menu.closing = true,
                    Some(PauseAction::Settings) => menu.settings = Some((0.0, false)),
                    Some(PauseAction::Report) => wanted = true,
                    Some(PauseAction::Leave) if surrender => {
                        give_up = true;
                        menu.closing = true;
                    }
                    Some(PauseAction::Leave) => event = Some(GameEvent::Leave),
                    Some(PauseAction::Quit) => event = Some(GameEvent::Quit),
                    None => {}
                }
                if let (Some((enter, closing)), true) = (&mut menu.settings, in_settings) {
                    *enter =
                        (*enter + if *closing { -dt / 0.14 } else { dt / 0.24 }).clamp(0.0, 1.0);
                    ui.interactive = !*closing;
                    let out = options::draw(ui, settings, 1.0 - (1.0 - *enter).powi(3));
                    *settings_changed |= out.changed;
                    *display_changed |= out.display_changed;
                    *closing |= out.back;
                    if *closing && *enter <= 0.0 {
                        menu.settings = None;
                    }
                }
            }
            if menu.closing && menu.enter <= 0.0 {
                self.menu = None;
            }
            wanted
        };
        if wanted_report {
            let report = self.report();
            if let Some(menu) = &mut self.menu {
                menu.report = Some((report, 0.0, false));
                menu.fresh = true;
            }
        }
        if give_up {
            if let Some(net) = &self.sim.net {
                net.request(crate::netplay::NetRequest::Surrender);
            }
        }
        event
    }
}
