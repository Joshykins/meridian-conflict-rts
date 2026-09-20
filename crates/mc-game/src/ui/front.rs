//! The front end: main menu, skirmish set-up and settings over the live
//! backdrop, and the transitions between them. It knows nothing about windows
//! or renderers, so the headless screenshot tool drives it the same way the
//! game does.

use super::backdrop::Director;
use super::menu::{self, MenuAction, MenuState, Telemetry};
use super::options;
use super::skirmish::{self, MatchRequest, SkirmishAction, SkirmishState};
use super::{ink, palette, rgb, Rect, Ui};
use crate::settings::Settings;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Menu,
    Skirmish,
    Options,
}

impl Screen {
    pub fn parse(s: &str) -> Option<Screen> {
        Some(match s {
            "menu" => Screen::Menu,
            "skirmish" => Screen::Skirmish,
            "settings" => Screen::Options,
            _ => return None,
        })
    }
}

pub enum FrontEvent {
    Launch(MatchRequest),
    /// Open the test range.
    Range,
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
    pub director: Director,
    /// `None` inside is the test range: there is nothing to set up first.
    launching: Option<(Option<MatchRequest>, f32)>,
    quitting: Option<f32>,
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

impl Front {
    pub fn new(director: Director) -> Front {
        Front {
            screen: Screen::Menu,
            target: Screen::Menu,
            enter: 0.0,
            menu: MenuState::default(),
            skirmish: None,
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
    }

    fn go(&mut self, screen: Screen, settings: &Settings) {
        if screen == Screen::Skirmish && self.skirmish.is_none() {
            self.skirmish = Some(SkirmishState::new(
                &settings.skirmish_map,
                settings.skirmish_fog,
                &settings.player_name,
            ));
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
                    Some(MenuAction::Range) => self.launching = Some((None, 0.0)),
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
                        self.launching = Some((Some(request), 0.0))
                    }
                    None => {}
                }
                // What was set up is what the screen opens with next time.
                let name = crate::settings::clean_name(&state.name);
                if settings.skirmish_map != state.selected_stem()
                    || settings.skirmish_fog != state.fog
                    || (settings.player_name != name && ui.mem.editing.is_none())
                {
                    settings.skirmish_map = state.selected_stem().to_owned();
                    settings.skirmish_fog = state.fog;
                    if ui.mem.editing.is_none() {
                        state.name = name.clone();
                        settings.player_name = name;
                    }
                    out.settings_changed = true;
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

        // Launching and quitting fade the whole front end to black.
        if let Some((_, t)) = &mut self.launching {
            *t += ui.dt;
            let k = (*t / LAUNCH_FADE).clamp(0.0, 1.0);
            ui.fill(
                Rect::new(0.0, 0.0, ui.size.x, ui.size.y),
                rgb(0x000000, k * k),
            );
            if *t >= LAUNCH_FADE + 0.05 {
                out.event = self
                    .launching
                    .take()
                    .map(|(request, _)| request.map_or(FrontEvent::Range, FrontEvent::Launch));
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

/// The card shown while a match (or the front end) is being loaded.
pub fn loading_card(ui: &mut Ui, title: &str, detail: &str) {
    let (w, h) = (ui.size.x, ui.size.y);
    ui.fill(Rect::new(0.0, 0.0, w, h), ink(1.0));
    let c = glam::Vec2::new(w * 0.5, h * 0.5 - 40.0);
    let t = ui.time;
    ui.reticle(c, 20.0, rgb(palette::TEXT, 0.9));
    for (radius, speed, sweep) in [(40.0, 1.6, 1.2), (54.0, -1.1, 2.0), (70.0, 0.7, 0.8)] {
        ui.arc(
            c,
            radius,
            t * speed,
            t * speed + sweep,
            1.4,
            rgb(palette::ACCENT, 0.7),
        );
    }
    ui.text_centred(
        w * 0.5 + 7.0,
        c.y + 120.0,
        super::type_scale::OVERLINE,
        rgb(palette::TEXT, 1.0),
        title,
    );
    ui.text_centred(
        w * 0.5,
        c.y + 150.0,
        super::type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        detail,
    );
}
