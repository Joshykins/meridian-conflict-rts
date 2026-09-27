//! Multiplayer: find a game on a server or on this network, host one, and the
//! lobby before the start.
//!
//! Three pages share one state: the browser (`browse.rs`), the host form
//! (`host.rs`) and the lobby (`lobby_view.rs`, over the model in `lobby.rs`).
//! The server link (`server.rs`) lives as long as the screen does, and on into
//! the match: its sign-in ticket is what lets a dropped player back in.

mod browse;
mod host;
pub mod lobby;
mod lobby_view;
pub mod server;
#[cfg(test)]
mod tests;

use super::maps::{self, Browser, MapCard};
use super::Ui;
use crate::match_options::SeatChoice;
use lobby::{Launch, Lobby, Place, Plan};
use mc_net::{ClientConfig, Identity, LanGame, LanScanner, NetSession, Role, RoomListing};
use server::{Answer, Server};
use std::sync::mpsc::{self, Receiver};
use std::time::Instant;

/// Image slot of the lobby's chart: the skirmish screen's, which redraws its own on return.
const CHART_SLOT: usize = 0;

pub enum MultiplayerAction {
    Back,
    Launch(Box<Launch>),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab {
    Online,
    Network,
}

enum Page {
    Browse,
    Host(host::Form),
    Lobby(Box<Lobby>),
}

pub struct MultiplayerState {
    pub maps: Vec<MapCard>,
    browser: Browser,
    pub server: Server,
    scanner: Option<LanScanner>,
    lan: Vec<LanGame>,
    identity: Option<Identity>,
    /// As typed in the fields.
    pub name: String,
    pub address: String,
    code: String,
    direct: String,
    tab: Tab,
    page: Page,
    /// The page to show from the next frame, when a page has moved on.
    next: Option<Page>,
    /// This build's unit data, which a match's content must share.
    blueprint_hash: u64,
    /// A line for the player about the last thing tried, and when it was said.
    notice: Option<(String, Instant)>,
    /// A room asked for and not yet created: its plan and whether it is private.
    creating: Option<(Plan, bool, String)>,
    /// The chart in the image slot is this map's.
    chart_of: Option<usize>,
}

/// The key that keeps this computer's name its own on a server, made on first use.
fn identity() -> Option<Identity> {
    let path = crate::settings::config_dir()?.join(mc_net::IDENTITY_FILE);
    match Identity::load_or_create(&path) {
        Ok(id) => Some(id),
        Err(e) => {
            log::error!("device key at {}: {e}", path.display());
            None
        }
    }
}

/// Opens a match connection on a thread of its own; the lobby picks it up.
fn open(addr: String, config: ClientConfig) -> Receiver<std::io::Result<NetSession>> {
    let (tx, rx) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("mc-net-connect".into())
        .spawn(move || {
            let _ = tx.send(NetSession::connect(addr.as_str(), config));
        });
    if let Err(e) = spawned {
        log::error!("no thread to connect with: {e}");
    }
    rx
}

impl MultiplayerState {
    pub fn new(settings: &crate::settings::Settings, blueprint_hash: u64) -> MultiplayerState {
        let maps: Vec<MapCard> = crate::setup::list_maps()
            .into_iter()
            .filter_map(|path| {
                let config = mc_data::weather::MapConfig::for_map(&path).unwrap_or_default();
                config
                    .survival
                    .is_none()
                    .then(|| maps::open_card(&path, &config))
                    .flatten()
            })
            .collect();
        let browser = Browser::new(&maps);
        let identity = identity();
        let name = settings.player_name.clone();
        let server = Server::new(&settings.server, &name, identity.clone());
        let scanner = match LanScanner::start() {
            Ok(s) => Some(s),
            Err(e) => {
                log::warn!("cannot listen for games on this network: {e}");
                None
            }
        };
        MultiplayerState {
            maps,
            browser,
            server,
            scanner,
            lan: Vec::new(),
            identity,
            name,
            address: settings.server.clone(),
            code: String::new(),
            direct: String::new(),
            tab: if settings.server.is_empty() {
                Tab::Network
            } else {
                Tab::Online
            },
            page: Page::Browse,
            next: None,
            blueprint_hash,
            notice: None,
            creating: None,
            chart_of: None,
        }
    }

    /// The chart slot was used by something else: draw ours again.
    pub fn chart_lost(&mut self) {
        self.chart_of = None;
    }

    fn say(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), Instant::now()));
    }

    fn content(&self, map_id: u64) -> mc_net::ContentId {
        mc_net::ContentId {
            map_id,
            blueprint_hash: self.blueprint_hash,
        }
    }

    /// Joins a room on the server, as a player or to watch.
    fn join_room(&mut self, listing: &RoomListing, role: Role) {
        let map_id = listing.content.map_or(0, |c| c.map_id);
        let content = self.content(map_id);
        let addr = match self.server.client() {
            Some(c) => c.server_addr().to_string(),
            None => return self.say("The server is not connected."),
        };
        let Some(config) = self
            .server
            .client()
            .and_then(|c| c.join_config(listing.code, role, content))
        else {
            return self.say("Sign in to the server first.");
        };
        let rx = open(addr.clone(), config.clone());
        self.next = Some(Page::Lobby(Box::new(Lobby::new(
            rx,
            Place::Server {
                code: listing.code,
                private: listing.private,
            },
            addr,
            config,
            None,
            listing.title.clone(),
        ))));
    }

    /// Joins a game hosted on another computer, by its address.
    fn join_direct(&mut self, addr: &str, title: &str) {
        let addr = server::with_port(addr);
        let config = crate::app::net_config(&self.name, Role::Player, self.content(0));
        let rx = open(addr.clone(), config.clone());
        self.next = Some(Page::Lobby(Box::new(Lobby::new(
            rx,
            Place::Lan { addr: addr.clone() },
            addr,
            config,
            None,
            title.to_owned(),
        ))));
    }

    /// Takes what the server has answered since the last frame.
    fn answers(&mut self) {
        for answer in std::mem::take(&mut self.server.answers) {
            match answer {
                Answer::Created(code) => {
                    let Some((plan, private, title)) = self.creating.take() else {
                        continue;
                    };
                    let map_id = self.maps.get(plan.map).map_or(0, |m| m.map.content_id());
                    let content = self.content(map_id);
                    let joined = self.server.client().and_then(|c| {
                        Some((
                            c.server_addr().to_string(),
                            c.join_config(code, Role::Player, content)?,
                        ))
                    });
                    let Some((addr, config)) = joined else {
                        self.say("The server went away before the game could open.");
                        continue;
                    };
                    let rx = open(addr.clone(), config.clone());
                    self.next = Some(Page::Lobby(Box::new(Lobby::new(
                        rx,
                        Place::Server { code, private },
                        addr,
                        config,
                        Some(plan),
                        title,
                    ))));
                }
                Answer::Refused(why) => {
                    self.creating = None;
                    self.say(format!("The server would not open the game: {why}."));
                    if let Some(Page::Host(form)) = &mut self.next {
                        form.busy = false;
                    }
                }
                Answer::Found(listing) => self.join_room(&listing, Role::Player),
                Answer::NotFound(code) => self.say(format!(
                    "No game with the code {code} is open on this server."
                )),
            }
        }
    }
}

/// The whole screen; `enter` eases it in and out.
pub fn draw(ui: &mut Ui, state: &mut MultiplayerState, enter: f32) -> Option<MultiplayerAction> {
    state.server.pump();
    state.answers();
    if let Some(scanner) = &mut state.scanner {
        state.lan = scanner.poll();
    }
    state.browser.pump(ui);
    if state
        .notice
        .as_ref()
        .is_some_and(|(_, at)| at.elapsed().as_secs_f32() > 7.0)
    {
        state.notice = None;
    }
    let mut page = std::mem::replace(&mut state.page, Page::Browse);
    let action = match &mut page {
        Page::Browse => browse::draw(ui, state, enter),
        Page::Host(form) => host::draw(ui, state, form, enter),
        Page::Lobby(lobby) => {
            lobby.pump(&state.maps);
            match lobby.launch(&state.maps, Vec::new()) {
                Some(Ok(mut l)) => {
                    // The server link travels with the match: its ticket brings a dropped
                    // player back in.
                    let link = std::mem::replace(
                        &mut state.server,
                        Server::new("", &state.name, state.identity.clone()),
                    );
                    l.keep.push(Box::new(link));
                    return Some(MultiplayerAction::Launch(Box::new(l)));
                }
                Some(Err(e)) => {
                    state.say(e);
                    state.next = Some(Page::Browse);
                    None
                }
                None => lobby_view::draw(ui, state, lobby, enter),
            }
        }
    };
    state.page = state.next.take().unwrap_or(page);
    action
}

/// A seat's own choices from a race pick.
fn choice_of(pick: super::faction::Pick) -> SeatChoice {
    SeatChoice {
        faction: pick.race().map_or_else(String::new, |r| r.key.clone()),
    }
}
