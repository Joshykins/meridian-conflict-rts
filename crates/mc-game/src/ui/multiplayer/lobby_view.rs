//! The lobby screen: the line-up (`ui::lineup`) that skirmish draws too, with
//! the room's chat under the rules, the game code top right, and a Ready or
//! Start at the foot. The host sets the match up; each person picks their own
//! race, takes a seat and readies up.

use super::lobby::{Lobby, Place};
use super::{choice_of, MultiplayerAction, MultiplayerState, Page, CHART_SLOT};
use crate::audio::Sfx;
use crate::setup::TEAM_COLORS;
use crate::ui::lineup::{self, Ask, Lineup, Mode, Table};
use crate::ui::{id, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use glam::Vec2;

const LEFT: f32 = lineup::LEFT;
/// The last seconds before the start, large.
const COUNTDOWN: crate::ui::Style = crate::ui::style(mc_render::Face::Light, 64.0, 2.0);
/// Height of the theatre card's block over the rules.
const THEATRE_H: f32 = 290.0;

pub(super) fn draw(
    ui: &mut Ui,
    state: &mut MultiplayerState,
    lobby: &mut Lobby,
    enter: f32,
) -> Option<MultiplayerAction> {
    let live = ui.interactive;
    let mode = lobby.lineup.as_ref().map(|l| l.mode);
    let caption = match (lobby.title.is_empty(), mode) {
        (true, _) => "Waiting for commanders".to_owned(),
        (false, Some(m)) => format!("{}  \u{b7}  {}", lobby.title, m.label()),
        (false, None) => lobby.title.clone(),
    };
    lineup::header(ui, "Lobby", &caption, enter);
    where_chip(ui, lobby);

    let problem = lobby.error.clone().or_else(|| {
        lobby
            .missing_map
            .as_ref()
            .map(|m| format!("The host picked {m}, which this game does not have. Ask for another map, or update the game."))
    });
    if let Some(error) = problem {
        failed(ui, state, &error);
    } else if lobby.connecting() || lobby.lineup.is_none() {
        joining(ui, state, lobby);
    } else if let Some(mut plan) = lobby.lineup.take() {
        // The line-up is lent out for the frame; the lobby gets it back below.
        let browsing = state.catalog.browsing() || plan.races.is_open();
        ui.interactive = live && !browsing;
        let people = lobby.occupants();
        let host = lobby.is_host() && lobby.planning();
        let name = state.name.clone();
        let table = Table {
            host,
            me: lobby.slot.map(usize::from),
            lobby: true,
            people: &people,
            observe: false,
            name: &name,
        };
        let mut asks = screen(ui, state, lobby, &mut plan, &table);
        ui.interactive = live;
        asks.extend(lineup::overlays(
            ui,
            &mut plan,
            &mut state.catalog,
            &table,
            enter,
            CHART_SLOT,
        ));
        lobby.lineup = Some(plan);
        for ask in asks {
            match ask {
                Ask::Sit(i) => lobby.take_seat(i as u8),
                Ask::Kick(i) => lobby.kick(i as u8),
                Ask::MyRace(pick) => lobby.set_choice(choice_of(pick)),
                Ask::Say(text) => state.say(text),
            }
        }
        countdown(ui, lobby);
    }
    ui.fade = 1.0;
    ui.shift.y = 0.0;
    ui.interactive = live;
    None
}

/// The line-up, the chat and the footer.
fn screen(
    ui: &mut Ui,
    state: &mut MultiplayerState,
    lobby: &mut Lobby,
    plan: &mut Lineup,
    table: &Table,
) -> Vec<Ask> {
    let mut asks = Vec::new();
    let (left, centre, right) = lineup::columns(ui);
    lineup::theatre(
        ui,
        plan,
        &mut state.catalog,
        table.host,
        Rect::new(left.x, left.y, left.w, THEATRE_H),
    );
    let rules = Rect::new(left.x, left.y + THEATRE_H, left.w, left.h - THEATRE_H);
    let (y, ask) = lineup::rules(ui, plan, &state.catalog, table, rules);
    asks.extend(ask);
    let chat_top = y + 16.0;
    chat(
        ui,
        lobby,
        Rect::new(left.x, chat_top, left.w, left.bottom() - chat_top),
    );
    lineup::chart(ui, plan, &state.catalog, table, CHART_SLOT, centre);
    asks.extend(lineup::commanders(
        ui,
        plan,
        &mut state.catalog,
        table,
        None,
        right,
    ));
    footer(ui, state, lobby, plan, table);
    asks
}

fn footer(
    ui: &mut Ui,
    state: &mut MultiplayerState,
    lobby: &mut Lobby,
    plan: &Lineup,
    table: &Table,
) {
    let host_slot = lobby.state.as_ref().and_then(|s| s.host);
    let waiting: Vec<String> = lobby
        .state
        .as_ref()
        .map(|s| {
            s.players
                .iter()
                .filter(|p| !p.ready && Some(p.slot) != host_slot)
                .map(|p| p.name.clone())
                .collect()
        })
        .unwrap_or_default();
    let in_play = plan.roster.in_play();
    let empty = (0..in_play)
        .filter(|&i| {
            plan.roster.seats[i].control == lineup::roster::Control::Person && !table.occupied(i)
        })
        .count();
    let watching = lobby.state.as_ref().map_or(0, |s| s.observers);
    let (mut line, tone, can) = if let Some(p) = plan.problem(&state.catalog) {
        (p.to_owned(), palette::WARN, false)
    } else if !waiting.is_empty() {
        (
            format!("Waiting for {}", waiting.join(", ")),
            palette::DIM,
            false,
        )
    } else if empty > 0 {
        (
            format!(
                "{empty} open seat{} will be played by the AI",
                if empty == 1 { "" } else { "s" }
            ),
            palette::DIM,
            true,
        )
    } else {
        (
            format!(
                "{}  \u{b7}  everyone is ready",
                lineup::summary(plan, &state.catalog)
            ),
            palette::DIM,
            true,
        )
    };
    if watching > 0 {
        line.push_str(&format!("  \u{b7}  {watching} watching"));
    }
    let notice = state.notice.as_ref().map(|(t, _)| t.clone());
    let ready = lobby.ready();
    let launch = if table.host {
        let label = match plan.mode {
            Mode::Skirmish => "Start Match",
            Mode::Survival => "Begin Survival",
        };
        (label, ButtonKind::Primary, can)
    } else if ready {
        ("Not Ready", ButtonKind::Secondary, true)
    } else {
        ("Ready", ButtonKind::Primary, true)
    };
    let (leave, go) = lineup::footer(ui, "Leave", launch, &line, tone, notice.as_deref());
    if go {
        if table.host {
            ui.audio.play(Sfx::Launch);
            lobby.start();
        } else {
            ui.audio
                .play(if ready { Sfx::ToggleOff } else { Sfx::ToggleOn });
            lobby.set_ready(!ready);
        }
    }
    let typing = ui.mem.editing.is_some();
    let listing = ui.mem.popup.is_some();
    if leave || (ui.input.key(Key::Escape) && !typing && !listing && ui.interactive) {
        ui.audio.play(Sfx::Back);
        state.next = Some(Page::Browse);
    }
}

/// The room's code (or address), top right: what to tell friends.
fn where_chip(ui: &mut Ui, lobby: &Lobby) {
    let (label, value) = match &lobby.place {
        Place::Server { code, private } => (
            if *private {
                "Private Game  \u{b7}  Code"
            } else {
                "Game Code"
            },
            code.to_string(),
        ),
        Place::Lan { addr } => ("On This Network  \u{b7}  Join At", addr.clone()),
    };
    let right = ui.size.x - LEFT;
    ui.text_right(
        right,
        72.0,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        label,
    );
    ui.text_right(
        right,
        98.0,
        type_scale::TITLE,
        rgb(palette::ACCENT, 1.0),
        &value,
    );
}

fn failed(ui: &mut Ui, state: &mut MultiplayerState, error: &str) {
    let (w, h) = (ui.size.x, ui.size.y);
    let r = Rect::new((w - 620.0) * 0.5, h * 0.34, 620.0, 210.0);
    ui.panel(r);
    ui.text_centred(
        w * 0.5,
        r.y + 46.0,
        type_scale::TITLE,
        rgb(palette::WARN, 1.0),
        "Could Not Join",
    );
    let mut y = r.y + 92.0;
    for line in ui.wrap(type_scale::BODY, error, r.w - 80.0) {
        ui.text_centred(w * 0.5, y, type_scale::BODY, rgb(palette::TEXT, 0.9), &line);
        y += 22.0;
    }
    if ui.button(
        id("lobby-failed-back", 0),
        Rect::new(w * 0.5 - 120.0, r.bottom() - 62.0, 240.0, 42.0),
        "Back to Games",
        ButtonKind::Primary,
        true,
    ) || ui.input.key(Key::Escape)
    {
        ui.audio.play(Sfx::Back);
        state.next = Some(Page::Browse);
    }
}

fn joining(ui: &mut Ui, state: &mut MultiplayerState, lobby: &Lobby) {
    let (w, h) = (ui.size.x, ui.size.y);
    let t = ui.time;
    let c = Vec2::new(w * 0.5, h * 0.42);
    ui.arc(
        c,
        26.0,
        t * 4.0,
        t * 4.0 + 4.2,
        2.4,
        rgb(palette::TEXT, 1.0),
    );
    ui.arc(
        c,
        34.0,
        -t * 2.0,
        -t * 2.0 + 1.4,
        1.2,
        rgb(palette::ACCENT, 0.6),
    );
    let what = if lobby.planning() {
        "Opening the Lobby"
    } else {
        "Joining"
    };
    ui.text_centred(
        c.x,
        c.y + 70.0,
        type_scale::TITLE,
        rgb(palette::TEXT, 0.95),
        what,
    );
    if ui.button(
        id("lobby-join-cancel", 0),
        Rect::new(c.x - 100.0, c.y + 112.0, 200.0, 42.0),
        "Cancel",
        ButtonKind::Secondary,
        true,
    ) || ui.input.key(Key::Escape)
    {
        ui.audio.play(Sfx::Back);
        state.next = Some(Page::Browse);
    }
}

fn chat(ui: &mut Ui, lobby: &mut Lobby, r: Rect) {
    ui.section(r.x, r.y, r.w, "Chat");
    let input = Rect::new(r.x, r.bottom() - 40.0, r.w, 40.0);
    let area = Rect::new(r.x, r.y + 22.0, r.w, input.y - r.y - 30.0);
    let line_h = 21.0;
    let mut lines: Vec<(Option<u8>, String, String)> = Vec::new();
    for c in lobby.chat.iter().rev() {
        let who = if c.from.is_none() && c.name.is_empty() {
            String::new()
        } else {
            format!("{}:", c.name)
        };
        let prefix_w = ui.text_width(type_scale::VALUE, &who) + 8.0;
        let wrapped = ui.wrap(type_scale::BODY, &c.text, r.w - prefix_w);
        for (k, l) in wrapped.iter().enumerate().rev() {
            lines.push((
                c.from,
                if k == 0 { who.clone() } else { String::new() },
                l.clone(),
            ));
        }
        if lines.len() as f32 * line_h > area.h {
            break;
        }
    }
    let mut y = area.bottom() - line_h * 0.5;
    for (from, who, text) in &lines {
        if y < area.y {
            break;
        }
        let mut x = r.x;
        if !who.is_empty() {
            let tone = from.map_or(rgb(palette::DIM, 1.0), |s| {
                let c = TEAM_COLORS[s as usize % 8];
                [c[0], c[1], c[2], 1.0]
            });
            x = ui.text(x, y, type_scale::VALUE, tone, who) + 8.0;
        } else if from.is_none() {
            ui.text(x, y, type_scale::BODY, rgb(palette::WARN, 1.0), text);
            y -= line_h;
            continue;
        }
        ui.text(x, y, type_scale::BODY, rgb(palette::TEXT, 0.9), text);
        y -= line_h;
    }
    if lobby.chat.is_empty() {
        ui.text(
            r.x,
            area.y + 12.0,
            type_scale::BODY,
            rgb(palette::FAINT, 1.0),
            "Say hello: type below and press Enter.",
        );
    }
    let field = id("lobby-chat", 0);
    ui.text_field(field, input, &mut lobby.draft, 200);
    if lobby.draft.is_empty() && ui.mem.editing != Some(field) {
        ui.text(
            input.x + 12.0,
            input.mid_y(),
            type_scale::BODY,
            rgb(palette::FAINT, 1.0),
            "Message the lobby",
        );
    }
    if ui.mem.editing == Some(field) && ui.input.key(Key::Enter) {
        lobby.send_chat();
        // Keep the keyboard: chat is a conversation.
        ui.mem.editing = Some(field);
    }
}

/// The last seconds before the start, over everything.
fn countdown(ui: &mut Ui, lobby: &Lobby) {
    let Some(ms) = lobby.countdown_left() else {
        return;
    };
    let (w, h) = (ui.size.x, ui.size.y);
    let band = Rect::new(0.0, h * 0.5 - 70.0, w, 140.0);
    ui.scrim(Rect::new(0.0, band.y, w * 0.5, band.h), 0.0, 0.8, true);
    ui.scrim(Rect::new(w * 0.5, band.y, w * 0.5, band.h), 0.8, 0.0, true);
    let secs = (ms as f32 / 1000.0).ceil().max(1.0);
    ui.text_centred(
        w * 0.5,
        band.y + 50.0,
        COUNTDOWN,
        rgb(0xFFFFFF, 1.0),
        &format!("{secs:.0}"),
    );
    ui.text_centred(
        w * 0.5,
        band.y + 104.0,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        "Deploying  \u{b7}  anyone who changes their mind stops the count",
    );
}
