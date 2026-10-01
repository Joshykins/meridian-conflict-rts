//! Opening a set-up to others: the set-up screen's Open to Others sheet. The
//! match set up on this machine, skirmish or survival, becomes a lobby's plan,
//! on the server (public, or private by code) or on this network, where this
//! computer runs the match's relay and announces it to the others near it.
//! The multiplayer browser's Host Game comes here too: a game is set up on the
//! set-up screen, the same one every mode and every lobby draws.

use super::lobby::{LanHost, Lobby, Place};
use super::server::{Answer, Server, Status};
use super::{identity, open};
use crate::audio::Sfx;
use crate::ui::lineup::roster::Control;
use crate::ui::lineup::{settings::Sheet, Catalog, Lineup};
use crate::ui::{id, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_net::{Identity, LanBeacon, LanInfo, RelayConfig, RelayServer, Role};
use std::net::{IpAddr, UdpSocket};

/// The sheet's size: one column of choices.
const SIZE: Vec2 = Vec2::new(760.0, 560.0);

/// A lobby opened from the set-up screen, and the server link it came through
/// (its sign-in ticket is what lets a dropped player back in).
pub struct Hosted {
    pub lobby: Lobby,
    pub server: Server,
    pub identity: Option<Identity>,
}

/// What the sheet asks of the set-up screen under it.
pub enum ShareAsk {
    /// Open a lobby on this network with the plan.
    HereNow,
    /// The server opened a room: take the plan to it.
    Created { code: mc_net::RoomCode },
}

#[derive(Default)]
pub struct Share {
    sheet: Sheet,
    /// Made when the sheet first opens, and kept while the screen is up.
    server: Option<Server>,
    identity: Option<Identity>,
    title: String,
    /// On this network rather than the server.
    lan: bool,
    private: bool,
    /// Waiting for the server to open the room.
    busy: bool,
    /// Why the last try did not open a lobby.
    notice: Option<String>,
}

impl Share {
    pub fn is_open(&self) -> bool {
        self.sheet.is_open()
    }

    /// Opens the sheet, signing in to `address` (the server last used) the first time.
    pub fn open(&mut self, address: &str, name: &str) {
        if self.server.is_none() {
            self.identity = identity();
            self.server = Some(Server::new(address, name, self.identity.clone()));
            self.lan = address.trim().is_empty();
        }
        if self.title.trim().is_empty() {
            self.title = format!("{}'s Game", name.trim());
        }
        self.notice = None;
        self.sheet.open();
    }

    pub fn title(&self) -> String {
        self.title.trim().to_owned()
    }

    fn online(&self) -> bool {
        self.server.as_ref().is_some_and(Server::online)
    }

    /// Takes the server link for the lobby the plan went to.
    pub fn hosted(&mut self, lobby: Lobby) -> Option<Box<Hosted>> {
        self.sheet.close();
        self.busy = false;
        Some(Box::new(Hosted {
            lobby,
            server: self.server.take()?,
            identity: self.identity.take(),
        }))
    }

    /// Something went wrong opening the lobby: say so on the sheet.
    pub fn say(&mut self, text: String) {
        self.busy = false;
        self.notice = Some(text);
    }

    /// The sheet over the screen, when open; the server link is kept going
    /// while the screen is up. `callsign` must be one a server takes.
    pub fn draw(
        &mut self,
        ui: &mut Ui,
        lineup: &Lineup,
        catalog: &Catalog,
        content: mc_net::ContentId,
        callsign: &str,
        live: bool,
    ) -> Option<ShareAsk> {
        let mut ask = None;
        if let Some(server) = &mut self.server {
            server.pump();
            for answer in std::mem::take(&mut server.answers) {
                match answer {
                    Answer::Created(code) if self.busy => ask = Some(ShareAsk::Created { code }),
                    Answer::Refused(why) if self.busy => {
                        self.say(format!("The server would not open the game: {why}."));
                    }
                    _ => {}
                }
            }
        }
        if !self.online() {
            self.lan = true;
        }
        let name_ok = mc_net::check_name(callsign);
        let ready = !self.busy
            && name_ok.is_ok()
            && !self.title.trim().is_empty()
            && lineup.problem(catalog).is_none()
            && lineup.card(catalog).is_some();
        let label = if self.busy {
            "Opening\u{2026}"
        } else {
            "Open Lobby"
        };
        let mut sheet = std::mem::take(&mut self.sheet);
        let go = sheet.draw_with(
            ui,
            "Open to Others",
            SIZE,
            live,
            (label, ready),
            |ui, body| {
                self.body(ui, lineup, catalog, name_ok.err(), body);
            },
        );
        self.sheet = sheet;
        if go {
            ui.audio.play(Sfx::Select);
            if self.lan {
                ask = Some(ShareAsk::HereNow);
            } else {
                self.create_room(content);
            }
        }
        ask
    }

    fn create_room(&mut self, content: mc_net::ContentId) {
        let title = self.title();
        let private = self.private;
        let Some(client) = self.server.as_mut().and_then(Server::client) else {
            return self.say("The server is not connected.".to_owned());
        };
        match client.create_room(&title, mc_core::MAX_PLAYERS as u8, private, content) {
            Ok(()) => {
                self.busy = true;
                self.notice = None;
            }
            Err(e) => self.say(format!("Could not ask the server: {e}")),
        }
    }

    fn body(
        &mut self,
        ui: &mut Ui,
        lineup: &Lineup,
        catalog: &Catalog,
        bad_name: Option<&'static str>,
        area: Rect,
    ) {
        let what = lineup.card(catalog).map_or(String::new(), |m| {
            format!("{} on {}", lineup.mode.label(), m.name)
        });
        // Opening is one way: say so before it is done.
        ui.text_fit_left(
            area.x,
            area.y + 4.0,
            area.w,
            type_scale::BODY,
            rgb(palette::DIM, 1.0),
            &format!("{what}, as set up here. Once open it stays open: leaving closes the lobby."),
        );
        let tile_w = (area.w - 12.0) * 0.5;
        let mut y = area.y + 36.0;
        ui.section(area.x, y, area.w, "Where");
        let online = self.online();
        let server_line = match self.server.as_ref().map(|s| (&s.status, s.target())) {
            Some((Status::Online { .. }, target)) => {
                format!("Friends anywhere join through {target}")
            }
            Some((Status::Connecting { .. }, target)) => format!("Reaching {target}\u{2026}"),
            Some((Status::NoAddress, _)) | None => {
                "No server yet: set one in Multiplayer".to_owned()
            }
            Some((_, target)) => format!("{target} is not answering"),
        };
        if choice(
            ui,
            0,
            Rect::new(area.x, y + 24.0, tile_w, 70.0),
            "Over the Internet",
            &server_line,
            !self.lan,
            online && !self.busy,
        ) {
            ui.audio.play(Sfx::Tick);
            self.lan = false;
        }
        let lan_line = match local_ip() {
            Some(ip) => format!("Friends on your network join this computer at {ip}"),
            None => "Friends on your network join this computer directly".to_owned(),
        };
        if choice(
            ui,
            1,
            Rect::new(area.x + tile_w + 12.0, y + 24.0, tile_w, 70.0),
            "On This Network",
            &lan_line,
            self.lan,
            !self.busy,
        ) {
            ui.audio.play(Sfx::Tick);
            self.lan = true;
        }
        y += 118.0;
        // A game on this network is seen by everyone on it; only a server's can be private.
        if !self.lan {
            ui.section(area.x, y, area.w, "Who Can Find It");
            if choice(
                ui,
                2,
                Rect::new(area.x, y + 24.0, tile_w, 70.0),
                "Public",
                "Listed on the server for anyone to join",
                !self.private,
                !self.busy,
            ) {
                ui.audio.play(Sfx::Tick);
                self.private = false;
            }
            if choice(
                ui,
                3,
                Rect::new(area.x + tile_w + 12.0, y + 24.0, tile_w, 70.0),
                "Private",
                "Only those you give the code to",
                self.private,
                !self.busy,
            ) {
                ui.audio.play(Sfx::Tick);
                self.private = true;
            }
            y += 118.0;
        }
        ui.section(area.x, y, area.w, "Title");
        ui.text_field(
            id("share-title", 0),
            Rect::new(area.x, y + 24.0, area.w, 40.0),
            &mut self.title,
            mc_net::MAX_TITLE_LEN.min(40),
        );
        let (line, tone) = if let Some(text) = &self.notice {
            (text.clone(), palette::WARN)
        } else if let Some(e) = bad_name {
            (
                format!("Your callsign will not do on a server: {e}. Change it in Match Settings."),
                palette::WARN,
            )
        } else {
            // Your seat aside, the seats a person may take.
            let open = lineup
                .roster
                .seats
                .iter()
                .filter(|s| s.control == Control::Person)
                .count()
                .saturating_sub(1);
            let text = if open == 0 {
                "Every seat has a commander: in the lobby, set an AI seat to Open for a friend."
            } else {
                "Open seats wait for people; any nobody takes are played by the AI."
            };
            (text.to_owned(), palette::DIM)
        };
        // Beside the sheet's Open Lobby, under the body.
        ui.text_fit_left(
            area.x,
            area.bottom() + 41.0,
            area.w - 240.0,
            type_scale::CAPTION,
            rgb(tone, 1.0),
            &line,
        );
    }
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
    let res = ui.tile(id("share-choice", key), r, lit, enabled);
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
        rgb(palette::DIM, a.max(0.8)),
        line,
    );
    res.clicked && enabled
}

/// This computer's address on its network, as the others would reach it. Nothing is
/// sent: connecting a datagram socket only picks the route.
fn local_ip() -> Option<IpAddr> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    s.local_addr().ok().map(|a| a.ip())
}

/// Takes `plan` to the room the server opened for it.
pub fn lobby_on_server(
    share: &mut Share,
    code: mc_net::RoomCode,
    plan: Lineup,
    content: mc_net::ContentId,
) -> Result<Lobby, String> {
    let private = share.private;
    let title = share.title();
    let joined = share
        .server
        .as_mut()
        .and_then(Server::client)
        .and_then(|c| {
            Some((
                c.server_addr().to_string(),
                c.join_config(code, Role::Player, content)?,
            ))
        });
    let Some((addr, config)) = joined else {
        return Err("The server went away before the game could open.".to_owned());
    };
    let rx = open(addr.clone(), config.clone());
    Ok(Lobby::new(
        rx,
        Place::Server { code, private },
        addr,
        config,
        Some(plan),
        title,
    ))
}

/// Runs the match's relay on this computer and announces it on the network.
pub fn host_here(
    name: &str,
    title: &str,
    catalog: &Catalog,
    plan: Lineup,
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
    let Some(card) = plan.card(catalog) else {
        return Err(std::io::Error::other("no such map"));
    };
    let beacon = LanBeacon::start(
        port,
        LanInfo {
            title: title.to_owned(),
            host: name.to_owned(),
            map: card.name.clone(),
            mode: plan.mode.label().to_owned(),
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
    let config = crate::app::net_config(name, Role::Player, content);
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
