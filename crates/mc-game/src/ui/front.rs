//! The front end: main menu, match set-up and settings over the live
//! backdrop, and the transitions between them. It knows nothing about windows
//! or renderers, so the headless screenshot tool drives it the same way the
//! game does.

use super::backdrop::Director;
use super::history::{self, HistoryAction, HistoryState};
use super::lineup::{Mode, ReadAhead};
use super::menu::{self, MenuAction, MenuState, Telemetry};
use super::multiplayer::{self, MultiplayerAction, MultiplayerState};
use super::options;
use super::setup::{self, MatchRequest, SetupAction, SetupState};
use super::{rgb, Rect, Ui};
use crate::settings::Settings;
use mc_data::Blueprints;
use mc_jobs::Pool;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Menu,
    /// Setting up a match, arriving in this mode (skirmish and survival share the screen).
    Setup(Mode),
    /// Match History: recorded matches, their battle reports and replays.
    History,
    Multiplayer,
    Options,
}

impl Screen {
    pub fn parse(s: &str) -> Option<Screen> {
        Some(match s {
            "menu" => Screen::Menu,
            "skirmish" => Screen::Setup(Mode::Skirmish),
            "survival" => Screen::Setup(Mode::Survival),
            "multiplayer" => Screen::Multiplayer,
            "settings" => Screen::Options,
            "history" => Screen::History,
            _ => return None,
        })
    }
}

pub enum FrontEvent {
    Launch(Box<MatchRequest>),
    /// A network lobby started its match.
    LaunchNet(Box<multiplayer::lobby::Launch>),
    /// Open the test range.
    Range,
    /// Watch a replay, jumping to the tick if one is given.
    Replay(std::path::PathBuf, Option<u32>),
    Quit,
}

#[derive(Default)]
pub struct FrontOutcome {
    pub event: Option<FrontEvent>,
    /// Settings changed and should be saved and applied.
    pub settings_changed: bool,
    /// The change involves the window or the renderer.
    pub display_changed: bool,
}

/// What the front end fades out to start.
enum Launching {
    Match(Box<MatchRequest>),
    Net(Box<multiplayer::lobby::Launch>),
    Range,
    /// Anything else that starts after the fade (a replay picked to watch).
    Event(FrontEvent),
}

/// Seconds a screen takes to leave and to arrive.
const LEAVE: f32 = 0.16;
const ARRIVE: f32 = 0.42;
/// The launch sound's riser lands its hit this long after the click; the
/// screen reaches black with it.
const LAUNCH_FADE: f32 = 0.62;

pub struct Front {
    screen: Screen,
    target: Screen,
    /// Presence of the current screen, 0..1.
    enter: f32,
    menu: MenuState,
    setup: Option<SetupState>,
    history: Option<HistoryState>,
    multiplayer: Option<MultiplayerState>,
    /// The maps the set-up and multiplayer screens list, read before they open.
    maps: ReadAhead,
    /// This build's unit data, which network matches must share and Match History's
    /// battle reports are worked out with, and the workers they use.
    blueprints: Arc<Blueprints>,
    pool: Arc<Pool>,
    blueprint_hash: u64,
    /// The backdrop map's preview (`menu::PREVIEW_SLOT`), kept to put back after a
    /// battle report borrowed the slot; whether it is due in the slot.
    preview: Option<Vec<u8>>,
    preview_due: bool,
    pub director: Director,
    /// `None` inside is the test range: there is nothing to set up first.
    launching: Option<(Launching, f32)>,
    quitting: Option<f32>,
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

impl Front {
    /// Over the backdrop's `director`, listing the maps `maps` is reading.
    pub fn new(
        director: Director,
        blueprints: Arc<Blueprints>,
        pool: Arc<Pool>,
        maps: ReadAhead,
    ) -> Front {
        Front {
            screen: Screen::Menu,
            target: Screen::Menu,
            enter: 0.0,
            menu: MenuState::default(),
            setup: None,
            history: None,
            multiplayer: None,
            maps,
            blueprint_hash: blueprints.content_hash(),
            blueprints,
            pool,
            preview: None,
            preview_due: false,
            director,
            launching: None,
            quitting: None,
        }
    }

    /// Leaving for a match: the maps read ahead, for the front end it comes back to.
    pub fn into_maps(self) -> ReadAhead {
        self.maps
    }

    /// Jumps straight to a screen, fully arrived (tools and tests).
    pub fn show(&mut self, screen: Screen, settings: &Settings) {
        self.go(screen, settings);
        self.open(settings, true);
        self.screen = screen;
        self.enter = 1.0;
        // `MERIDIAN_HISTORY_REPORT=N[:PAGE]`: Match History with match N's battle report open.
        if let (Screen::History, Some(h)) = (screen, &mut self.history) {
            if let Ok(spec) = std::env::var("MERIDIAN_HISTORY_REPORT") {
                let (n, page) = spec.split_once(':').unwrap_or((&spec, ""));
                if let Ok(n) = n.parse() {
                    h.open_report_now(n, page);
                }
            }
        }
        // Shots see the map thumbnails; `MERIDIAN_MAP_BROWSER=1` opens the browser.
        let browse = std::env::var("MERIDIAN_MAP_BROWSER").is_ok_and(|v| v == "1");
        if let (Screen::Setup(mode), Some(s)) = (screen, &mut self.setup) {
            let browser = match mode {
                Mode::Skirmish => &mut s.catalog.browser,
                Mode::Survival => &mut s.catalog.theatre_browser,
            };
            browser.wait_for_thumbs();
            if browse {
                browser.open_now(0);
            }
            // `MERIDIAN_SKIRMISH_TEAMS=N`: every seat an AI, split into N sides.
            if let Some(n) = std::env::var("MERIDIAN_SKIRMISH_TEAMS")
                .ok()
                .and_then(|v| v.parse().ok())
            {
                s.seat_teams_for_shot(n);
            }
        }
    }

    fn go(&mut self, screen: Screen, settings: &Settings) {
        if let (Screen::Setup(mode), Some(s)) = (screen, &mut self.setup) {
            s.set_mode(mode);
        }
        // Read afresh every visit: a match may have been recorded or marked since.
        if screen == Screen::History {
            self.history = Some(HistoryState::new(
                self.blueprints.clone(),
                self.pool.clone(),
            ));
        }
        // Set-up and multiplayer share an image slot for their charts.
        if let Some(s) = &mut self.setup {
            s.chart_lost();
        }
        if let Some(m) = &mut self.multiplayer {
            m.chart_lost();
        }
        self.target = screen;
        self.open(settings, false);
    }

    /// Makes the state of the screen being gone to once the maps it lists are
    /// read; true when it is there. Without `wait`, a screen whose maps are
    /// still being read is made on a later frame and arrives then: the window
    /// never stalls on reading them.
    fn open(&mut self, settings: &Settings, wait: bool) -> bool {
        let maps = &mut self.maps;
        let mut catalog = || {
            if wait {
                Some(maps.take())
            } else {
                maps.try_take()
            }
        };
        match self.target {
            Screen::Setup(mode) => {
                if self.setup.is_none() {
                    self.setup = catalog()
                        .map(|c| SetupState::with_catalog(c, settings, mode, self.blueprint_hash));
                }
                self.setup.is_some()
            }
            Screen::Multiplayer => {
                if self.multiplayer.is_none() {
                    self.multiplayer =
                        catalog().map(|c| MultiplayerState::new(settings, self.blueprint_hash, c));
                }
                self.multiplayer.is_some()
            }
            _ => true,
        }
    }

    /// Swaps to `screen` at once, without leaving and arriving: the set-up and
    /// its lobby are one screen to the eye.
    fn swap(&mut self, screen: Screen) {
        self.screen = screen;
        self.target = screen;
    }

    /// The backdrop map's preview, put in its slot by the next frame.
    pub fn set_preview(&mut self, chart: Vec<u8>) {
        self.preview = Some(chart);
        self.preview_due = true;
    }

    /// Something drew over every image slot (a battle report's unit pictures and
    /// chart): each screen puts its pictures back.
    fn slots_lost(&mut self) {
        self.preview_due = true;
        super::maps::slots_lost();
        if let Some(s) = &mut self.setup {
            s.chart_lost();
        }
        if let Some(m) = &mut self.multiplayer {
            m.chart_lost();
        }
    }

    pub fn frame(
        &mut self,
        ui: &mut Ui,
        settings: &mut Settings,
        telemetry: &Telemetry,
    ) -> FrontOutcome {
        let mut out = FrontOutcome::default();
        if let (true, Some(preview)) = (std::mem::take(&mut self.preview_due), &self.preview) {
            ui.o.set_image(
                menu::PREVIEW_SLOT,
                super::preview::SIZE,
                super::preview::SIZE,
                preview,
            );
        }
        self.director.update(ui.dt);
        ui.fill(
            Rect::new(0.0, 0.0, ui.size.x, ui.size.y),
            rgb(0x000000, self.director.dip()),
        );

        // Leave, swap, arrive.
        let leaving =
            self.target != self.screen || self.launching.is_some() || self.quitting.is_some();
        if self.target != self.screen {
            self.enter -= ui.dt / LEAVE;
            if self.enter <= 0.0 {
                self.enter = 0.0;
                // Leaving multiplayer signs out; coming back signs in afresh.
                if self.screen == Screen::Multiplayer {
                    self.multiplayer = None;
                }
                self.screen = self.target;
            }
        } else if !leaving && self.open(settings, false) {
            self.enter = (self.enter + ui.dt / ARRIVE).min(1.0);
        }
        ui.interactive = !leaving && self.enter > 0.6;
        let enter = smooth(self.enter);

        match self.screen {
            Screen::Menu => {
                match menu::draw(ui, &mut self.menu, &mut self.director, telemetry, enter) {
                    Some(MenuAction::Skirmish) => self.go(Screen::Setup(Mode::Skirmish), settings),
                    Some(MenuAction::Survival) => self.go(Screen::Setup(Mode::Survival), settings),
                    Some(MenuAction::Multiplayer) => self.go(Screen::Multiplayer, settings),
                    Some(MenuAction::Range) => self.launching = Some((Launching::Range, 0.0)),
                    Some(MenuAction::History) => self.go(Screen::History, settings),
                    Some(MenuAction::Options) => self.go(Screen::Options, settings),
                    Some(MenuAction::Quit) => self.quitting = Some(0.0),
                    None => {}
                }
            }
            Screen::Setup(_) => {
                let Some(state) = self.setup.as_mut() else {
                    // Handed on to the lobby it opened.
                    return out;
                };
                match setup::draw(ui, state, enter) {
                    Some(SetupAction::Back) => self.target = Screen::Menu,
                    Some(SetupAction::Start(request)) => {
                        self.launching = Some((Launching::Match(request), 0.0))
                    }
                    Some(SetupAction::Host(hosted)) => {
                        // The set-up becomes its lobby in place: the same screen, now with
                        // the room's chat and code. Its maps go with it.
                        if let Some(setup) = self.setup.take() {
                            self.multiplayer = Some(MultiplayerState::hosted(
                                settings,
                                self.blueprint_hash,
                                setup.catalog,
                                *hosted,
                            ));
                            self.swap(Screen::Multiplayer);
                        }
                        return out;
                    }
                    None => {}
                }
                // What was set up is what the screen opens with next time.
                if state.store(settings, ui.mem.editing.is_some()) {
                    out.settings_changed = true;
                }
            }
            Screen::History => {
                let state = self.history.as_mut().expect("created on the way in");
                match history::draw(ui, state, enter) {
                    Some(HistoryAction::Back) => self.target = Screen::Menu,
                    Some(HistoryAction::Watch(path, at)) => {
                        self.launching =
                            Some((Launching::Event(FrontEvent::Replay(path, at)), 0.0));
                    }
                    None => {}
                }
                if state.take_slots_back() {
                    self.slots_lost();
                }
            }
            Screen::Multiplayer => {
                // After a launch the connection has been handed on; the fade to
                // black runs over an empty screen.
                let action = self
                    .multiplayer
                    .as_mut()
                    .and_then(|state| multiplayer::draw(ui, state, enter));
                match action {
                    Some(MultiplayerAction::Back) => self.target = Screen::Menu,
                    Some(MultiplayerAction::Launch(launch)) => {
                        self.multiplayer = None;
                        self.launching = Some((Launching::Net(launch), 0.0));
                    }
                    Some(MultiplayerAction::Host) => {
                        let mode = self.setup.as_ref().map_or(Mode::Skirmish, SetupState::mode);
                        self.go(Screen::Setup(mode), settings);
                        self.open(settings, true);
                        if let Some(s) = &mut self.setup {
                            s.open_share();
                        }
                    }
                    None => {}
                }
                // The callsign and server are remembered once they are typed.
                if let Some(state) = &self.multiplayer {
                    let name = state.name.trim();
                    if ui.mem.editing.is_none()
                        && (settings.server != state.address.trim()
                            || (settings.player_name != name && mc_net::check_name(name).is_ok()))
                    {
                        settings.server = state.address.trim().to_owned();
                        if mc_net::check_name(name).is_ok() {
                            settings.player_name = name.to_owned();
                        }
                        out.settings_changed = true;
                    }
                }
            }
            Screen::Options => {
                let result = options::draw(ui, settings, enter);
                out.settings_changed = result.changed;
                out.display_changed = result.display_changed;
                if result.back {
                    self.target = Screen::Menu;
                }
            }
        }
        if settings.backdrop_auto_advance != self.director.auto_advance {
            settings.backdrop_auto_advance = self.director.auto_advance;
            out.settings_changed = true;
        }

        // An open dropdown's list, over the screen that opened it.
        ui.popups();

        // Launching and quitting fade the whole front end to black.
        if let Some((_, t)) = &mut self.launching {
            *t += ui.dt;
            let k = (*t / LAUNCH_FADE).clamp(0.0, 1.0);
            ui.fill(
                Rect::new(0.0, 0.0, ui.size.x, ui.size.y),
                rgb(0x000000, k * k),
            );
            if *t >= LAUNCH_FADE + 0.05 {
                out.event = self.launching.take().map(|(what, _)| match what {
                    Launching::Range => FrontEvent::Range,
                    Launching::Match(request) => FrontEvent::Launch(request),
                    Launching::Net(launch) => FrontEvent::LaunchNet(launch),
                    Launching::Event(event) => event,
                });
            }
        }
        if let Some(t) = &mut self.quitting {
            *t += ui.dt;
            ui.fill(
                Rect::new(0.0, 0.0, ui.size.x, ui.size.y),
                rgb(0x000000, (*t / 0.3).min(1.0)),
            );
            if *t >= 0.32 {
                out.event = Some(FrontEvent::Quit);
            }
        }
        out
    }
}
