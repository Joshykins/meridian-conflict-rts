//! The lobby screen: the line-up (`ui::lineup`) that skirmish draws too, with
//! the room's chat on the left, the game code top right, and a Ready or
//! Start at the foot. The host sets the match up; each person picks their own
//! race, takes a seat and readies up.

use super::lobby::{Lobby, Place};
use super::{choice_of, MultiplayerAction, MultiplayerState, Page, CHART_SLOT};
use crate::audio::Sfx;
use crate::ui::lineup::chat::{self, Facts, Input, Reply};
use crate::ui::lineup::{self, Ask, Lineup, Mode, Table};
use crate::ui::{id, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use glam::Vec2;

/// The last seconds before the start, large.
const COUNTDOWN: crate::ui::Style = crate::ui::style(mc_render::Face::Light, 64.0, 2.0);

pub(super) fn draw(
    ui: &mut Ui,
    state: &mut MultiplayerState,
    lobby: &mut Lobby,
    enter: f32,
) -> Option<MultiplayerAction> {
    let live = ui.interactive;
    let mode = lobby.lineup.as_ref().map(|l| l.mode);
    let caption = if lobby.title.is_empty() {
        "Waiting for commanders".to_owned()
    } else {
        format!("{}  \u{b7}  Lobby", lobby.title)
    };
    lineup::header(ui, mode, &caption, enter);
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
        let over = state.catalog.browsing() || plan.races.is_open();
        ui.interactive = live && !over && !plan.sheet.is_open();
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
        asks.extend(lineup::sheet(
            ui,
            &mut plan,
            &mut state.catalog,
            &table,
            !over,
            |_, _, y| y + 20.0,
        ));
        asks.extend(lineup::overlays(
            ui,
            &mut plan,
            &mut state.catalog,
            &table,
            enter,
            CHART_SLOT,
        ));
        let facts =
            Facts::of(&plan, &state.catalog, &table).with_sky(&plan.sky, &map_config(&plan, state));
        lobby.lineup = Some(plan);
        lobby.chat.watch(facts);
        for ask in asks {
            match ask {
                Ask::Sit(i) => lobby.take_seat(i as u8),
                Ask::Kick(i) => lobby.kick(i as u8),
                Ask::MyRace(pick) => lobby.set_choice(choice_of(pick)),
                Ask::Say(text) => lobby.chat.warn(text),
            }
        }
        countdown(ui, lobby);
    }
    ui.fade = 1.0;
    ui.shift.y = 0.0;
    ui.interactive = live;
    None
}

/// The plan's map's own settings: the regions its sky picks a weather for.
fn map_config(
    plan: &Lineup,
    state: &MultiplayerState,
) -> std::sync::Arc<mc_data::weather::MapConfig> {
    plan.card(&state.catalog)
        .map(|m| m.config.clone())
        .unwrap_or_default()
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
    // Escape puts down a commander being moved before it leaves the lobby.
    let placing = plan.placing();
    let (left, centre, right) = lineup::columns(ui);
    let chips = [
        lineup::settings::fog_chip(plan.fog),
        crate::ui::sky::summary(&plan.sky, &map_config(plan, state)),
    ];
    let below = lineup::match_card(ui, plan, &mut state.catalog, table.host, &chips, left);
    let room = Rect::new(left.x, below + 22.0, left.w, left.bottom() - below - 22.0);
    if let Some(Reply::Send(text)) = chat::draw(ui, &mut lobby.chat, room, Input::Live) {
        lobby.send_chat(&text);
    }
    lineup::chart(ui, plan, &state.catalog, table, CHART_SLOT, centre);
    asks.extend(lineup::commanders(
        ui,
        plan,
        &mut state.catalog,
        table,
        None,
        right,
    ));
    footer(ui, state, lobby, plan, table, placing);
    asks
}

fn footer(
    ui: &mut Ui,
    state: &mut MultiplayerState,
    lobby: &mut Lobby,
    plan: &Lineup,
    table: &Table,
    placing: bool,
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
    // The host leaving ends the room: say so on the button.
    let back = if table.host { "Close Lobby" } else { "Leave" };
    let clicked = lineup::footer(ui, back, launch, None, &line, tone, notice.as_deref());
    let (leave, go) = (clicked.back, clicked.launch);
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
    let escape = ui.input.key(Key::Escape) && !typing && !listing && !placing;
    if leave || (escape && ui.interactive) {
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
    let right = ui.size.x - lineup::margin(ui);
    let y = lineup::title_y(ui);
    ui.text_right(
        right,
        y - 12.0,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        label,
    );
    ui.text_right(
        right,
        y + 14.0,
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
