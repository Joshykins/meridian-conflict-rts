//! The set-up's chat, down the left of the screen: what people in a lobby
//! say, and a note each time the plan changes (the map, the mode, a seat's
//! team, race or colour, somebody joining), so everyone sees what moved. On one
//! machine it is the notes alone; Open to Others takes the chat to the lobby,
//! where it can be typed in.
//!
//! The notes come from comparing the plan with how it was last seen
//! ([`Facts`]), so a change made anywhere (this screen, the settings sheet, the
//! host's plan arriving over the network) is noted the same way.

use super::roster::Control;
use super::{seats, settings, Catalog, Lineup, Mode, Table};
use crate::setup::TEAM_COLORS;
use crate::ui::{id, ink, palette, rgb, type_scale, Color, Key, Rect, Ui};
use glam::Vec2;
use mc_sim::SurvivalRules;
use std::time::{Duration, Instant};

/// Lines the chat keeps.
const KEPT: usize = 80;
/// A change to the same thing this soon after the last note replaces it,
/// so stepping a seed or a team through several values says only the last.
const MERGE: Duration = Duration::from_secs(4);
/// How long a new note stands out before it settles to the rest.
const FRESH: f32 = 3.0;

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// Someone said it: the seat they sit in, if any, colours the name.
    Person { from: Option<u8>, name: String },
    /// The plan changed; with the colour of the seat it is about, if any.
    Note { swatch: Option<u8> },
    /// Something went wrong or was refused, or the room says so.
    Warn,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub kind: Kind,
    pub text: String,
    /// What the note is about, for merging with the next one.
    topic: Option<String>,
    at: Instant,
}

/// What the input under the lines does.
pub enum Input<'a> {
    /// Type and press Enter to send.
    Live,
    /// No one to talk to: a hint that a click acts on.
    Closed(&'a str),
}

/// What the chat asks of the screen around it.
pub enum Reply {
    /// Send this to the room.
    Send(String),
    /// The closed input's hint was clicked.
    Open,
}

#[derive(Default)]
pub struct Chat {
    pub lines: Vec<Line>,
    pub draft: String,
    /// The plan as last noted.
    seen: Option<Facts>,
}

impl Chat {
    fn push(&mut self, kind: Kind, text: String, topic: Option<String>) {
        let at = Instant::now();
        if let (Some(t), Some(last)) = (&topic, self.lines.last_mut()) {
            if last.topic.as_ref() == Some(t) && last.at.elapsed() < MERGE {
                (last.kind, last.text, last.at) = (kind, text, at);
                return;
            }
        }
        self.lines.push(Line {
            kind,
            text,
            topic,
            at,
        });
        if self.lines.len() > KEPT {
            self.lines.remove(0);
        }
    }

    /// Someone in the room said something.
    pub fn said(&mut self, from: Option<u8>, name: String, text: String) {
        if from.is_none() && name.is_empty() {
            // The room itself.
            return self.warn(text);
        }
        self.push(Kind::Person { from, name }, text, None);
    }

    /// Something happened that is not a change to the plan.
    pub fn note(&mut self, text: impl Into<String>) {
        self.push(Kind::Note { swatch: None }, text.into(), None);
    }

    pub fn warn(&mut self, text: impl Into<String>) {
        self.push(Kind::Warn, text.into(), None);
    }

    /// Notes whatever changed since the plan was last seen. The first look, and
    /// the first after moving between this machine and a lobby, only looks.
    pub fn watch(&mut self, now: Facts) {
        let Some(was) = self.seen.take().filter(|w| w.lobby == now.lobby) else {
            self.seen = Some(now);
            return;
        };
        if was != now {
            for (topic, swatch, text) in changes(&was, &now) {
                self.push(Kind::Note { swatch }, text, Some(topic));
            }
        }
        self.seen = Some(now);
    }
}

/// The plan as the chat reads it: what a note may be written about.
#[derive(Clone, Debug, PartialEq)]
pub struct Facts {
    lobby: bool,
    mode: Mode,
    map: String,
    fog: bool,
    seed: u64,
    rules: Option<SurvivalRules>,
    /// On one machine: every commander is an AI and you watch.
    observe: bool,
    /// This machine's sky, which the plan does not carry.
    sky: Option<String>,
    seats: Vec<SeatFacts>,
}

#[derive(Clone, Debug, PartialEq)]
struct SeatFacts {
    key: u8,
    open: bool,
    /// As its row names it.
    name: String,
    /// The person who sits there: a lobby's occupant, or you on one machine.
    person: Option<String>,
    control: &'static str,
    team: u8,
    zone: String,
    color: u8,
    race: String,
    tuning: Option<String>,
}

impl Facts {
    pub fn of(lineup: &Lineup, catalog: &Catalog, table: &Table) -> Facts {
        let spawns = lineup.theatre(catalog).map(|t| &t.layout.spawns);
        let seats = lineup
            .roster
            .seats
            .iter()
            .enumerate()
            .map(|(i, s)| SeatFacts {
                key: s.key,
                open: s.open(),
                name: seats::who(lineup, table, i, s),
                person: match (s.control, table.occupant(i)) {
                    (Control::Closed, _) => None,
                    (_, Some(p)) => Some(p.name.clone()),
                    (Control::Person, None) if !table.lobby && !table.observe => {
                        Some(table.name.to_owned())
                    }
                    _ => None,
                },
                control: seats::control_label(s),
                team: s.team,
                zone: spawns
                    .and_then(|sp| sp.get(s.start as usize))
                    .map_or_else(|| format!("zone {}", s.start + 1), |z| z.name.clone()),
                color: s.color,
                race: match s.race.race() {
                    Some(r) => format!("the {}", r.name),
                    None => "a random race".to_owned(),
                },
                tuning: (s.control == Control::Ai).then(|| super::ai::summary(&s.ai)),
            })
            .collect();
        Facts {
            lobby: table.lobby,
            mode: lineup.mode,
            map: lineup
                .card(catalog)
                .map_or_else(String::new, |c| c.name.clone()),
            fog: lineup.fog,
            seed: lineup.seed,
            rules: (lineup.mode == Mode::Survival).then_some(lineup.rules),
            observe: table.observe,
            sky: None,
            seats,
        }
    }

    /// With this machine's sky, as picked for `map`.
    pub fn with_sky(
        mut self,
        sky: &mc_data::weather::SkyChoice,
        map: &mc_data::weather::MapConfig,
    ) -> Facts {
        self.sky = Some(crate::ui::sky::summary(sky, map));
        self
    }
}

/// What a note says, keyed by what it is about, and the seat colour it shows.
type Change = (String, Option<u8>, String);

fn changes(was: &Facts, now: &Facts) -> Vec<Change> {
    let mut out: Vec<Change> = Vec::new();
    let mut say = |topic: &str, swatch: Option<u8>, text: String| {
        out.push((topic.to_owned(), swatch, text));
    };
    let moved = was.mode != now.mode || was.map != now.map;
    if was.mode != now.mode {
        say("mode", None, format!("Mode set to {}", now.mode.label()));
    }
    if was.map != now.map {
        say("map", None, format!("Map set to {}", now.map));
    }
    if was.fog != now.fog {
        let text = if now.fog {
            "Fog of war on"
        } else {
            "Fog of war off"
        };
        say("fog", None, text.to_owned());
    }
    if was.seed != now.seed {
        say(
            "seed",
            None,
            format!("New {}", settings::seed_chip(now.seed)),
        );
    }
    if was.rules != now.rules && was.mode == now.mode && now.rules.is_some() {
        say("rules", None, "Survival rules changed".to_owned());
    }
    if was.observe != now.observe {
        let text = if now.observe {
            "You watch the AI commanders fight"
        } else {
            "You command again"
        };
        say("observe", None, text.to_owned());
    }
    if was.sky != now.sky {
        if let Some(sky) = &now.sky {
            say("sky", None, format!("Sky set to {sky}"));
        }
    }
    for s in now.seats.iter().filter(|s| s.open) {
        let before = was.seats.iter().find(|b| b.key == s.key && b.open);
        let topic = |what: &str| format!("seat{}.{what}", s.key);
        let swatch = Some(s.color);
        let Some(b) = before else {
            let text = match (&s.person, s.control) {
                (Some(p), _) => format!("{p} joins"),
                (None, "Open") => "A seat opens for a player".to_owned(),
                (None, control) => format!("{} joins  \u{b7}  {control}", s.name),
            };
            say(&topic("open"), swatch, text);
            continue;
        };
        if b.person != s.person && was.observe == now.observe {
            match (&b.person, &s.person) {
                // Your callsign, being typed.
                (Some(_), Some(p)) if !now.lobby => {
                    say("callsign", swatch, format!("Your callsign is {p}"));
                }
                (Some(old), Some(p)) => {
                    say(&topic("person"), swatch, format!("{p} takes {old}'s seat"))
                }
                (None, Some(p)) => say(&topic("person"), swatch, format!("{p} takes a seat")),
                (Some(p), None) => say(&topic("person"), swatch, format!("{p} leaves their seat")),
                (None, None) => {}
            }
        }
        if b.control != s.control && s.person.is_none() {
            say(
                &topic("control"),
                swatch,
                format!("{} set to {}", s.name, s.control),
            );
        }
        if b.race != s.race {
            say(
                &topic("race"),
                swatch,
                format!("{} plays {}", s.name, s.race),
            );
        }
        if b.tuning != s.tuning && b.control == s.control {
            if let Some(t) = &s.tuning {
                say(&topic("tuning"), swatch, format!("{} tuned: {t}", s.name));
            }
        }
        // A new map or mode deals teams, zones and colours afresh: its note says enough.
        if moved {
            continue;
        }
        if b.team != s.team && now.mode == Mode::Skirmish {
            say(
                &topic("team"),
                swatch,
                format!("{} joins Team {}", s.name, s.team + 1),
            );
        }
        if b.zone != s.zone {
            say(
                &topic("zone"),
                swatch,
                format!("{} lands at {}", s.name, s.zone),
            );
        }
        if b.color != s.color {
            say(
                &topic("color"),
                swatch,
                format!("{} changes colour", s.name),
            );
        }
    }
    for b in was.seats.iter().filter(|b| b.open) {
        if !now.seats.iter().any(|s| s.key == b.key && s.open) {
            let who = b.person.as_ref().unwrap_or(&b.name);
            say(
                &format!("seat{}.open", b.key),
                None,
                format!("{who} leaves"),
            );
        }
    }
    out
}

/// The chat in `r`: the section title, the lines from the newest up, and the input.
pub fn draw(ui: &mut Ui, chat: &mut Chat, r: Rect, input: Input) -> Option<Reply> {
    ui.section(r.x, r.y, r.w, "Chat");
    let field = Rect::new(r.x, r.bottom() - 40.0, r.w, 40.0);
    let area = Rect::new(r.x, r.y + 22.0, r.w, field.y - r.y - 30.0);
    lines(ui, chat, area);
    if chat.lines.is_empty() {
        let hint = match input {
            Input::Live => "Say hello: type below and press Enter.",
            Input::Closed(_) => "Changes to the match show here.",
        };
        ui.text_fit_left(
            r.x,
            area.y + 12.0,
            r.w,
            type_scale::BODY,
            rgb(palette::FAINT, 1.0),
            hint,
        );
    }
    match input {
        Input::Live => {
            let at = id("lineup-chat", 0);
            // The field gives up the keyboard on Enter, so ask first.
            let editing = ui.mem.editing == Some(at);
            ui.text_field(at, field, &mut chat.draft, 200);
            if chat.draft.is_empty() && ui.mem.editing != Some(at) {
                ui.text_fit_left(
                    field.x + 12.0,
                    field.mid_y(),
                    field.w - 24.0,
                    type_scale::BODY,
                    rgb(palette::FAINT, 1.0),
                    "Message the lobby",
                );
            }
            if editing && ui.input.key(Key::Enter) {
                // Keep the keyboard: chat is a conversation.
                ui.mem.editing = Some(at);
                let text = std::mem::take(&mut chat.draft).trim().to_owned();
                return (!text.is_empty()).then_some(Reply::Send(text));
            }
            None
        }
        Input::Closed(hint) => {
            let res = ui.interact(id("lineup-chat-closed", 0), field, true);
            ui.fill(field, ink(0.35 + 0.15 * res.glow));
            ui.frame(field, rgb(palette::LINE, 0.12 + 0.25 * res.glow));
            ui.text_fit_left(
                field.x + 12.0,
                field.mid_y(),
                field.w - 24.0,
                type_scale::BODY,
                mix(rgb(palette::FAINT, 1.0), rgb(palette::TEXT, 1.0), res.glow),
                hint,
            );
            res.clicked.then_some(Reply::Open)
        }
    }
}

/// `a` eased towards `b` by `t`.
fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

/// The lines that fit in `area`, the newest at the foot. A new line eases in,
/// and a new note stands out a moment before it settles.
fn lines(ui: &mut Ui, chat: &Chat, area: Rect) {
    const LINE_H: f32 = 21.0;
    const MARK: f32 = 14.0;
    struct Row<'a> {
        line: &'a Line,
        who: String,
        text: String,
        first: bool,
    }
    let mut rows: Vec<Row> = Vec::new();
    for line in chat.lines.iter().rev() {
        let who = match &line.kind {
            Kind::Person { name, .. } => format!("{name}:"),
            _ => String::new(),
        };
        let indent = if who.is_empty() {
            MARK
        } else {
            ui.text_width(type_scale::VALUE, &who) + 8.0
        };
        let wrapped = ui.wrap(type_scale::BODY, &line.text, area.w - indent);
        for (k, text) in wrapped.into_iter().enumerate().rev() {
            rows.push(Row {
                line,
                who: if k == 0 { who.clone() } else { String::new() },
                text,
                first: k == 0,
            });
        }
        if rows.len() as f32 * LINE_H > area.h {
            break;
        }
    }
    let mut y = area.bottom() - LINE_H * 0.5;
    for row in &rows {
        if y < area.y {
            break;
        }
        let age = row.line.at.elapsed().as_secs_f32();
        let ease = (age / 0.35).min(1.0);
        let fresh = 1.0 - (age / FRESH).min(1.0);
        let x = area.x + 10.0 * (1.0 - ease);
        let (mark, body) = match &row.line.kind {
            Kind::Person { from, .. } => {
                let tone = from.map_or(rgb(palette::DIM, ease), |s| {
                    let c = TEAM_COLORS[s as usize % TEAM_COLORS.len()];
                    [c[0], c[1], c[2], ease]
                });
                let mut at = x;
                if !row.who.is_empty() {
                    at = ui.text(x, y, type_scale::VALUE, tone, &row.who) + 8.0;
                }
                ui.text(
                    at,
                    y,
                    type_scale::BODY,
                    rgb(palette::TEXT, 0.9 * ease),
                    &row.text,
                );
                y -= LINE_H;
                continue;
            }
            Kind::Note { swatch } => (
                swatch.map_or(rgb(palette::ACCENT, ease), |s| {
                    let c = TEAM_COLORS[s as usize % TEAM_COLORS.len()];
                    [c[0], c[1], c[2], ease]
                }),
                mix(rgb(palette::DIM, ease), rgb(palette::TEXT, ease), fresh),
            ),
            Kind::Warn => (rgb(palette::WARN, ease), rgb(palette::WARN, ease)),
        };
        if row.first {
            ui.dot(Vec2::new(x + 3.0, y), 2.5 + 1.0 * fresh, mark);
        }
        ui.text(x + MARK, y, type_scale::BODY, body, &row.text);
        y -= LINE_H;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seat(key: u8, name: &str) -> SeatFacts {
        SeatFacts {
            key,
            open: true,
            name: name.to_owned(),
            person: None,
            control: "AI \u{b7} Normal",
            team: key,
            zone: format!("zone {}", key + 1),
            color: key,
            race: "the Asterian Reach Command".to_owned(),
            tuning: Some("Commander".to_owned()),
        }
    }

    fn plan() -> Facts {
        let mut you = seat(0, "Josh");
        you.person = Some("Josh".to_owned());
        you.control = "Open";
        you.tuning = None;
        Facts {
            lobby: false,
            mode: Mode::Skirmish,
            map: "Serac Divide".to_owned(),
            fog: true,
            seed: 7,
            rules: None,
            observe: false,
            sky: None,
            seats: vec![you, seat(1, "ARC AI 1")],
        }
    }

    fn texts(chat: &Chat) -> Vec<&str> {
        chat.lines.iter().map(|l| l.text.as_str()).collect()
    }

    #[test]
    fn the_first_look_says_nothing() {
        let mut chat = Chat::default();
        chat.watch(plan());
        chat.watch(plan());
        assert!(chat.lines.is_empty());
    }

    #[test]
    fn each_change_is_noted() {
        let mut chat = Chat::default();
        chat.watch(plan());
        let mut now = plan();
        now.fog = false;
        now.seats[1].team = 0;
        now.seats[1].control = "AI \u{b7} Hard";
        now.seats.push(seat(2, "ARC AI 2"));
        chat.watch(now);
        assert_eq!(
            texts(&chat),
            [
                "Fog of war off",
                "ARC AI 1 set to AI \u{b7} Hard",
                "ARC AI 1 joins Team 1",
                "ARC AI 2 joins  \u{b7}  AI \u{b7} Normal",
            ]
        );
    }

    #[test]
    fn a_new_map_says_so_not_every_seat_it_moved() {
        let mut chat = Chat::default();
        chat.watch(plan());
        let mut now = plan();
        now.map = "Halden's Grip".to_owned();
        now.seats[1].zone = "zone 5".to_owned();
        now.seats[1].color = 4;
        chat.watch(now);
        assert_eq!(texts(&chat), ["Map set to Halden's Grip"]);
    }

    #[test]
    fn stepping_one_thing_keeps_one_note() {
        let mut chat = Chat::default();
        chat.watch(plan());
        for team in 2..5 {
            let mut now = plan();
            now.seats[1].team = team;
            chat.watch(now);
        }
        assert_eq!(texts(&chat), ["ARC AI 1 joins Team 5"]);
    }

    #[test]
    fn seats_closing_and_people_coming_and_going() {
        let mut chat = Chat::default();
        let mut was = plan();
        was.lobby = true;
        was.seats[0].person = None;
        chat.watch(was.clone());
        let mut now = was.clone();
        now.seats[0].person = Some("Ana".to_owned());
        now.seats[1].open = false;
        chat.watch(now);
        assert_eq!(texts(&chat), ["Ana takes a seat", "ARC AI 1 leaves"]);
    }

    #[test]
    fn moving_to_a_lobby_starts_afresh() {
        let mut chat = Chat::default();
        chat.watch(plan());
        let mut lobby = plan();
        lobby.lobby = true;
        lobby.seats[0].person = None;
        chat.watch(lobby);
        assert!(chat.lines.is_empty());
    }

    #[test]
    fn the_room_speaks_as_a_warning() {
        let mut chat = Chat::default();
        chat.said(None, String::new(), "Server restarting".to_owned());
        chat.said(Some(1), "Ana".to_owned(), "hi".to_owned());
        assert_eq!(chat.lines[0].kind, Kind::Warn);
        assert!(matches!(
            chat.lines[1].kind,
            Kind::Person { from: Some(1), .. }
        ));
    }

    /// One frame of the live chat with the keyboard in its field.
    fn frame(
        chat: &mut Chat,
        mem: &mut crate::ui::Memory,
        input: crate::ui::Input,
    ) -> Option<Reply> {
        let mut o = mc_render::Overlay::default();
        let audio = crate::audio::Audio::silent();
        mem.editing = Some(id("lineup-chat", 0));
        mem.begin_frame();
        let mut ui = Ui::new(
            &mut o,
            &input,
            mem,
            &audio,
            Vec2::new(1920.0, 1080.0),
            1.0,
            0.0,
            0.016,
        );
        draw(
            &mut ui,
            chat,
            Rect::new(20.0, 100.0, 360.0, 600.0),
            Input::Live,
        )
    }

    #[test]
    fn enter_sends_and_keeps_the_keyboard() {
        let mut chat = Chat::default();
        let mut mem = crate::ui::Memory::default();
        let typed = crate::ui::Input {
            typed: "gl hf".into(),
            ..Default::default()
        };
        assert!(frame(&mut chat, &mut mem, typed).is_none());
        let enter = crate::ui::Input {
            keys: vec![Key::Enter],
            ..Default::default()
        };
        let sent = frame(&mut chat, &mut mem, enter);
        assert!(matches!(sent, Some(Reply::Send(t)) if t == "gl hf"));
        assert!(chat.draft.is_empty());
        assert_eq!(mem.editing, Some(id("lineup-chat", 0)));
    }
}
