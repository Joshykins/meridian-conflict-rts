//! The front end: main menu, skirmish set-up and settings over the live
//! backdrop, and the transitions between them. It knows nothing about windows
//! or renderers, so the headless screenshot tool drives it the same way the
//! game does.

use super::backdrop::Director;
use super::menu::{self, MenuAction, MenuState, Telemetry};
use super::multiplayer::{self, MultiplayerAction, MultiplayerState};
use super::options;
use super::replays::{self, ReplaysAction, ReplaysState};
use super::skirmish::{self, MatchRequest, SkirmishAction, SkirmishState};
use super::survival::{self, SurvivalAction, SurvivalState};
use super::{rgb, Rect, Ui};
use crate::settings::Settings;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Menu,
    Skirmish,
    Survival,
    Replays,
    Multiplayer,
    Options,
}

impl Screen {
    pub fn parse(s: &str) -> Option<Screen> {
        Some(match s {
            "menu" => Screen::Menu,
            "skirmish" => Screen::Skirmish,
            "survival" => Screen::Survival,
            "multiplayer" => Screen::Multiplayer,
            "settings" => Screen::Options,
            "replays" => Screen::Replays,
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
    skirmish: Option<SkirmishState>,
    survival: Option<SurvivalState>,
    replays: Option<ReplaysState>,
    multiplayer: Option<MultiplayerState>,
    /// This build's unit data, which network matches must share.
    blueprint_hash: u64,
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
    pub fn new(director: Director, blueprint_hash: u64) -> Front {
        Front {
            screen: Screen::Menu,
            target: Screen::Menu,
            enter: 0.0,
            menu: MenuState::default(),
            skirmish: None,
            survival: None,
            replays: None,
            multiplayer: None,
            blueprint_hash,
            director,
            launching: None,
            quitting: None,
        }
    }

    /// Jumps straight to a screen, fully arrived (tools and tests).
    pub fn show(&mut self, screen: Screen, settings: &Settings) {
        self.go(screen, settings);
        self.screen = screen;
        self.enter = 1.0;
        // Shots see the map thumbnails; `MERIDIAN_MAP_BROWSER=1` opens the browser.
        let browse = std::env::var("MERIDIAN_MAP_BROWSER").is_ok_and(|v| v == "1");
        if let (Screen::Skirmish, Some(s)) = (screen, &mut self.skirmish) {
            s.browser.wait_for_thumbs();
            if browse {
                s.browser.open_now(0);
            }
            // `MERIDIAN_SKIRMISH_TEAMS=N`: every seat an AI, split into N sides.
            if let Some(n) = std::env::var("MERIDIAN_SKIRMISH_TEAMS")
                .ok()
                .and_then(|v| v.parse().ok())
            {
                s.seat_teams_for_shot(n);
            }
        }
        if let (Screen::Survival, Some(s)) = (screen, &mut self.survival) {
            s.browser.wait_for_thumbs();
            if browse {
                s.browser.open_now(0);
            }
        }
    }

    fn go(&mut self, screen: Screen, settings: &Settings) {
        if screen == Screen::Skirmish && self.skirmish.is_none() {
            let mut state = SkirmishState::new(
                &settings.skirmish_map,
                settings.skirmish_fog,
                &settings.player_name,
            );
            state.sky = settings.skirmish_sky;
            self.skirmish = Some(state);
        }
        if screen == Screen::Survival && self.survival.is_none() {
            self.survival = Some(SurvivalState::new(settings));
        }
        // Read afresh every visit: a match may have been recorded or marked since.
        if screen == Screen::Replays {
            self.replays = Some(ReplaysState::new());
        }
        if screen == Screen::Multiplayer && self.multiplayer.is_none() {
            self.multiplayer = Some(MultiplayerState::new(settings, self.blueprint_hash));
        }
        // Skirmish and multiplayer share an image slot for their charts.
        if let Some(s) = &mut self.skirmish {
            s.chart_lost();
        }
        if let Some(m) = &mut self.multiplayer {
            m.chart_lost();
        }
        self.target = screen;
    }

    pub fn frame(
        &mut self,
        ui: &mut Ui,
        settings: &mut Settings,
        telemetry: &Telemetry,
    ) -> FrontOutcome {
        let mut out = FrontOutcome::default();
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
        } else if !leaving {
            self.enter = (self.enter + ui.dt / ARRIVE).min(1.0);
        }
        ui.interactive = !leaving && self.enter > 0.6;
        let enter = smooth(self.enter);

        match self.screen {
            Screen::Menu => {
                match menu::draw(ui, &mut self.menu, &mut self.director, telemetry, enter) {
                    Some(MenuAction::Skirmish) => self.go(Screen::Skirmish, settings),
                    Some(MenuAction::Survival) => self.go(Screen::Survival, settings),
                    Some(MenuAction::Multiplayer) => self.go(Screen::Multiplayer, settings),
                    Some(MenuAction::Range) => self.launching = Some((Launching::Range, 0.0)),
                    Some(MenuAction::Replays) => self.go(Screen::Replays, settings),
                    Some(MenuAction::Options) => self.go(Screen::Options, settings),
                    Some(MenuAction::Quit) => self.quitting = Some(0.0),
                    None => {}
                }
            }
            Screen::Skirmish => {
                let state = self.skirmish.as_mut().expect("created on the way in");
                match skirmish::draw(ui, state, enter) {
                    Some(SkirmishAction::Back) => self.target = Screen::Menu,
                    Some(SkirmishAction::Start(request)) => {
                        self.launching = Some((Launching::Match(request), 0.0))
                    }
                    None => {}
                }
                // What was set up is what the screen opens with next time.
                let name = crate::settings::clean_name(&state.name);
                if settings.skirmish_map != state.selected_stem()
                    || settings.skirmish_fog != state.fog
                    || settings.skirmish_sky != state.sky
                    || (settings.player_name != name && ui.mem.editing.is_none())
                {
                    settings.skirmish_map = state.selected_stem().to_owned();
                    settings.skirmish_fog = state.fog;
                    settings.skirmish_sky = state.sky;
                    if ui.mem.editing.is_none() {
                        state.name = name.clone();
                        settings.player_name = name;
                    }
                    out.settings_changed = true;
                }
            }
            Screen::Survival => {
                let state = self.survival.as_mut().expect("created on the way in");
                match survival::draw(ui, state, enter) {
                    Some(SurvivalAction::Back) => self.target = Screen::Menu,
                    Some(SurvivalAction::Start(request)) => {
                        self.launching = Some((Launching::Match(request), 0.0))
                    }
                    None => {}
                }
                if state.store(settings, ui.mem.editing.is_some()) {
                    out.settings_changed = true;
                }
            }
            Screen::Replays => {
                let state = self.replays.as_mut().expect("created on the way in");
                match replays::draw(ui, state, enter) {
                    Some(ReplaysAction::Back) => self.target = Screen::Menu,
                    Some(ReplaysAction::Watch(path, at)) => {
                        self.launching =
                            Some((Launching::Event(FrontEvent::Replay(path, at)), 0.0));
                    }
                    None => {}
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
