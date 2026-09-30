//! Hosting a game: skirmish or co-op survival, the map, a title, and where: on
//! the server (public, or private by code) or on this network, where this
//! computer runs the match's relay and announces it to the others on the network.

use super::browse::header;
use super::lobby::{LanHost, Lobby, Place};
use super::{open, MultiplayerAction, MultiplayerState, Page, CHART_SLOT};
use crate::audio::Sfx;
use crate::ui::lineup::{theatre_card, Lineup, Mode};
use crate::ui::maps::BrowserAction;
use crate::ui::{id, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use mc_net::{LanBeacon, LanInfo, RelayConfig, RelayServer, Role};
use std::net::{IpAddr, UdpSocket};

const LEFT: f32 = 64.0;

pub(super) struct Form {
    mode: Mode,
    /// Index into the mode's maps.
    map: usize,
    title: String,
    /// On this network rather than the server.
    lan: bool,
    private: bool,
    /// Waiting for the server to open the room.
    pub(super) busy: bool,
}

impl Form {
    pub(super) fn new(state: &MultiplayerState) -> Form {
        let map = state
            .catalog
            .maps
            .iter()
            .position(|m| m.stem == "twin_shoals")
            .unwrap_or(0);
        Form {
            mode: Mode::Skirmish,
            map,
            title: format!("{}'s Game", state.name.trim()),
            lan: !state.server.online(),
            private: false,
            busy: false,
        }
    }
}

impl Form {
    pub(super) fn mode(&self) -> Mode {
        self.mode
    }
}

/// This computer's address on its network, as the others would reach it. Nothing is
/// sent: connecting a datagram socket only picks the route.
pub(super) fn local_ip() -> Option<IpAddr> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    s.local_addr().ok().map(|a| a.ip())
}

/// A tile with a title and a line under it; lit when chosen.
fn choice(
    ui: &mut Ui,
    key: usize,
    r: Rect,
    title: &str,
    line: &str,
    lit: bool,
    enabled: bool,
) -> bool {
    let res = ui.tile(id("host-choice", key), r, lit, enabled);
    let a = if enabled { 1.0 } else { 0.4 };
    if lit {
        ui.fill(Rect::new(r.x, r.y, 3.0, r.h), rgb(palette::ACCENT, 1.0));
    }
    ui.text(
        r.x + 18.0,
        r.y + 24.0,
        type_scale::ITEM,
        rgb(if lit { 0xFFFFFF } else { palette::TEXT }, a),
        title,
    );
    ui.text_fit_left(
        r.x + 18.0,
        r.y + 48.0,
        r.w - 30.0,
        type_scale::MICRO,
        rgb(
            if enabled { palette::DIM } else { palette::WARN },
            a.max(0.8),
        ),
        line,
    );
    res.clicked && enabled
}

pub(super) fn draw(
    ui: &mut Ui,
    state: &mut MultiplayerState,
    form: &mut Form,
    enter: f32,
) -> Option<MultiplayerAction> {
    let (w, h) = (ui.size.x, ui.size.y);
    let browsing = state.catalog.browsing();
    let live = ui.interactive;
    ui.interactive = live && !browsing;
    header(
        ui,
        "Host a Game",
        "Pick the battlefield and who can find it",
        enter,
    );

    let top = 160.0;
    let left = Rect::new(LEFT, top, 420.0, 300.0);
    ui.panel(Rect::new(
        left.x - 22.0,
        top - 20.0,
        left.w + 44.0,
        h - 172.0 - top + 40.0,
    ));
    ui.section(left.x, left.y + 6.0, left.w, "Theatre");
    let (browser, cards) = match form.mode {
        Mode::Skirmish => (&mut state.catalog.browser, &state.catalog.maps),
        Mode::Survival => (
            &mut state.catalog.theatre_browser,
            &state.catalog.theatre_cards,
        ),
    };
    if theatre_card(
        ui,
        browser,
        cards,
        form.map,
        Rect::new(left.x, left.y + 30.0, left.w, 250.0),
    ) {
        browser.open(form.map);
    }
    let y = left.y + 310.0;
    ui.section(left.x, y, left.w, "Title");
    ui.text_field(
        id("host-title", 0),
        Rect::new(left.x, y + 24.0, left.w, 40.0),
        &mut form.title,
        mc_net::MAX_TITLE_LEN.min(40),
    );

    let right = Rect::new(
        LEFT + 420.0 + 72.0,
        top,
        w - 2.0 * LEFT - 420.0 - 72.0,
        h - 172.0 - top,
    );
    ui.panel(Rect::new(
        right.x - 22.0,
        top - 20.0,
        right.w + 44.0,
        right.h + 40.0,
    ));
    let tile_w = (right.w - 12.0) * 0.5;
    ui.section(right.x, right.y + 6.0, right.w, "Game");
    let survival_maps = !state.catalog.theatre_cards.is_empty();
    for (k, (mode, line, enabled)) in [
        (
            Mode::Skirmish,
            "Sides and teams on any skirmish map, with AI commanders if you like",
            true,
        ),
        (
            Mode::Survival,
            if survival_maps {
                "Everyone defends together against the Progenitor's rounds"
            } else {
                "No survival maps in maps/"
            },
            survival_maps,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let r = Rect::new(
            right.x + k as f32 * (tile_w + 12.0),
            right.y + 30.0,
            tile_w,
            70.0,
        );
        if choice(ui, 4 + k, r, mode.label(), line, form.mode == mode, enabled) && form.mode != mode
        {
            ui.audio.play(Sfx::Tick);
            form.mode = mode;
            form.map = 0;
        }
    }
    let right = Rect::new(right.x, right.y + 124.0, right.w, right.h - 124.0);
    ui.section(right.x, right.y + 6.0, right.w, "Where");
    let online = state.server.online();
    let server_line = if online {
        format!("Friends anywhere join through {}", state.server.target())
    } else {
        "The server is not connected: connect to one on the previous page".to_owned()
    };
    if choice(
        ui,
        0,
        Rect::new(right.x, right.y + 30.0, tile_w, 70.0),
        "Over the Internet",
        &server_line,
        !form.lan,
        online,
    ) {
        ui.audio.play(Sfx::Tick);
        form.lan = false;
    }
    let lan_line = match local_ip() {
        Some(ip) => format!("Friends on your network join this computer at {ip}"),
        None => "Friends on your network join this computer directly".to_owned(),
    };
    if choice(
        ui,
        1,
        Rect::new(right.x + tile_w + 12.0, right.y + 30.0, tile_w, 70.0),
        "On This Network",
        &lan_line,
        form.lan,
        true,
    ) {
        ui.audio.play(Sfx::Tick);
        form.lan = true;
    }
    if !online {
        form.lan = true;
    }
    if !form.lan {
        let y = right.y + 130.0;
        ui.section(right.x, y, right.w, "Who Can Find It");
        if choice(
            ui,
            2,
            Rect::new(right.x, y + 24.0, tile_w, 70.0),
            "Public",
            "Listed on the server for anyone to join",
            !form.private,
            true,
        ) {
            ui.audio.play(Sfx::Tick);
            form.private = false;
        }
        if choice(
            ui,
            3,
            Rect::new(right.x + tile_w + 12.0, y + 24.0, tile_w, 70.0),
            "Private",
            "Only those you give the code to",
            form.private,
            true,
        ) {
            ui.audio.play(Sfx::Tick);
            form.private = true;
        }
    }
    let notes = [
        "Seats, teams, AI commanders and the rules are set in the lobby.",
        "Open seats nobody takes are played by the AI.",
    ];
    for (i, n) in notes.iter().enumerate() {
        ui.text(
            right.x,
            right.bottom() - 40.0 + i as f32 * 22.0,
            type_scale::BODY,
            rgb(palette::DIM, 1.0),
            n,
        );
    }

    let back = ui.button(
        id("host-back", 0),
        Rect::new(LEFT, h - 64.0 - 52.0, 200.0, 52.0),
        "Back",
        ButtonKind::Secondary,
        true,
    );
    let go_rect = Rect::new(w - LEFT - 300.0, h - 64.0 - 58.0, 300.0, 58.0);
    let ready =
        !form.busy && !state.catalog.cards(form.mode).is_empty() && !form.title.trim().is_empty();
    let go = ui.button(
        id("host-go", 0),
        go_rect,
        if form.busy {
            "Opening\u{2026}"
        } else {
            "Open Lobby"
        },
        ButtonKind::Primary,
        ready,
    );
    if let Some((text, _)) = &state.notice {
        ui.text_right(
            go_rect.x - 24.0,
            go_rect.mid_y(),
            type_scale::CAPTION,
            rgb(palette::WARN, 1.0),
            text,
        );
    }
    let typing = ui.mem.editing.is_some();
    if back || (ui.input.key(Key::Escape) && !typing && ui.interactive) {
        ui.audio.play(Sfx::Back);
        state.creating = None;
        state.next = Some(Page::Browse);
    } else if go && ready {
        ui.audio.play(Sfx::Select);
        open_lobby(state, form);
    }
    ui.fade = 1.0;
    ui.shift.y = 0.0;
    ui.interactive = live;

    // The map browser, over the page.
    let (fade, shift) = (ui.fade, ui.shift);
    ui.fade = enter;
    let (browser, cards) = match form.mode {
        Mode::Skirmish => (&mut state.catalog.browser, &state.catalog.maps),
        Mode::Survival => (
            &mut state.catalog.theatre_browser,
            &state.catalog.theatre_cards,
        ),
    };
    if let Some(BrowserAction::Pick(i)) = browser.draw(ui, cards, "Choose a Map", CHART_SLOT) {
        form.map = i;
    }
    browser.release_slot();
    (ui.fade, ui.shift) = (fade, shift);
    None
}

fn open_lobby(state: &mut MultiplayerState, form: &mut Form) {
    let Some(card) = state.catalog.cards(form.mode).get(form.map) else {
        return;
    };
    // You and one open seat; the rest closed until the lobby opens them.
    let plan = Lineup::new(&state.catalog, form.mode, form.map, 2, 0);
    let content = state.content(card.map.content_id());
    let title = form.title.trim().to_owned();
    if !form.lan {
        let Some(client) = state.server.client() else {
            return state.say("The server is not connected.");
        };
        match client.create_room(&title, mc_core::MAX_PLAYERS as u8, form.private, content) {
            Ok(()) => {
                form.busy = true;
                state.creating = Some((plan, form.private, title));
            }
            Err(e) => state.say(format!("Could not ask the server: {e}")),
        }
        return;
    }
    match host_here(state, plan, &title, content) {
        Ok(lobby) => state.next = Some(Page::Lobby(Box::new(lobby))),
        Err(e) => state.say(format!("Could not host here: {e}")),
    }
}

/// Runs the match's relay on this computer and announces it on the network.
fn host_here(
    state: &MultiplayerState,
    plan: Lineup,
    title: &str,
    content: mc_net::ContentId,
) -> std::io::Result<Lobby> {
    let config = RelayConfig {
        players: mc_core::MAX_PLAYERS as u8,
        title: title.to_owned(),
        // Only the host's own seat until the lobby publishes the plan.
        open_seats: 0b1,
        replay_dir: crate::settings::config_dir().map(|d| d.join("replays")),
        ..RelayConfig::default()
    };
    // The usual port if it is free; any port if not (the address shows it).
    let server = RelayServer::bind(("0.0.0.0", super::server::DEFAULT_PORT), config.clone())
        .or_else(|_| RelayServer::bind("0.0.0.0:0", config))?;
    let port = server.local_addr()?.port();
    let relay = server.spawn()?;
    let Some(card) = plan.card(&state.catalog) else {
        return Err(std::io::Error::other("no such map"));
    };
    let beacon = LanBeacon::start(
        port,
        LanInfo {
            title: title.to_owned(),
            host: state.name.clone(),
            map: card.name.clone(),
            mode: String::new(),
            players: 1,
            seats: plan.roster.in_play() as u8,
            free: 1,
            build: crate::BUILD.to_owned(),
            content,
        },
    )
    .map_err(|e| log::warn!("games on this network will not see this one: {e}"))
    .ok();
    let me = format!("127.0.0.1:{port}");
    let config = crate::app::net_config(&state.name, Role::Player, content);
    let rx = open(me.clone(), config.clone());
    let shown = match local_ip() {
        Some(ip) if port == super::server::DEFAULT_PORT => ip.to_string(),
        Some(ip) => format!("{ip}:{port}"),
        None => format!("port {port}"),
    };
    let mut lobby = Lobby::new(
        rx,
        Place::Lan { addr: shown },
        me,
        config,
        Some(plan),
        title.to_owned(),
    );
    lobby.hosting = Some(LanHost::new(relay, beacon));
    Ok(lobby)
}
