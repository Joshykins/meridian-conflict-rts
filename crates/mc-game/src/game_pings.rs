//! Map pings from the keyboard and the pointer (`pings.rs`): H takes a ping in hand,
//! the digits 1 to 6 pick its kind while it is, and a click on the ground or the
//! minimap puts it down for the allies (Shift keeps it in hand). A message ping asks
//! for its words first (`hud/pings.rs`). Space looks at the newest ping.

use super::{Game, Mode};
use crate::audio::{Audio, Sfx};
use crate::pings::{Ping, PingKind};
use glam::Vec2;
use winit::keyboard::KeyCode;

impl Game {
    /// The ping keys. True when `code` was one of them.
    pub(super) fn ping_key(&mut self, code: KeyCode, audio: &Audio) -> bool {
        let digits = [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
        ];
        match (code, self.view.mode) {
            (KeyCode::KeyH, _) => {
                self.arm_ping(PingKind::Look, audio);
                true
            }
            (KeyCode::Space, _) => {
                let Some(at) = self.hud.pings.newest else {
                    return false;
                };
                self.track = None;
                self.cancel_orbit_return();
                self.camera.focus = at.extend(self.camera.focus.z);
                self.camera.clamp_focus();
                true
            }
            (_, Mode::Ping(_)) => match digits.iter().position(|k| *k == code) {
                Some(i) => {
                    audio.play(Sfx::Tick);
                    self.view.mode = Mode::Ping(PingKind::ALL[i]);
                    true
                }
                None => false,
            },
            _ => false,
        }
    }

    /// Takes a ping of `kind` in hand, or puts it away if it already is.
    pub(super) fn arm_ping(&mut self, kind: PingKind, audio: &Audio) {
        if self.view.observing {
            audio.play(Sfx::Deny);
            return;
        }
        audio.play(Sfx::Tick);
        self.view.patrol_posts.clear();
        self.view.patrol_inserts.clear();
        self.view.circle_from = None;
        self.view.mode = if self.view.mode == Mode::Ping(kind) {
            Mode::Normal
        } else {
            Mode::Ping(kind)
        };
    }

    /// A click with a ping in hand, on the ground at `at` (`None`: off the map).
    pub(super) fn ping_at(&mut self, at: Option<Vec2>, audio: &Audio) {
        let Mode::Ping(kind) = self.view.mode else {
            return;
        };
        let Some(at) = at else {
            audio.play(Sfx::Deny);
            return;
        };
        if !self.shift {
            self.view.mode = Mode::Normal;
        }
        if kind == PingKind::Message {
            audio.play(Sfx::Tick);
            self.hud.pings.draft = Some(crate::pings::Draft {
                at,
                text: String::new(),
            });
            return;
        }
        self.send_ping(kind, at, String::new(), audio);
    }

    /// Puts a ping up here and sends it to the allies. Too many too fast is refused,
    /// and the HUD says so.
    pub(super) fn send_ping(&mut self, kind: PingKind, at: Vec2, text: String, audio: &Audio) {
        if !self.hud.pings.spend() {
            audio.play(Sfx::Deny);
            self.hud.ping_refused();
            return;
        }
        let local = self.view.local;
        let name = self
            .view
            .status
            .players
            .get(local as usize)
            .map_or_else(|| "You".to_owned(), |p| p.name.clone());
        let to = self.ping_allies();
        if let (Some(net), true) = (&self.sim.net, to != 0) {
            net.request(crate::netplay::NetRequest::MapPing {
                kind,
                at,
                text: text.clone(),
                to,
            });
        }
        self.hud.ping_up(
            Ping {
                kind,
                at,
                from: local,
                name,
                text,
                age: 0.0,
            },
            audio,
        );
    }

    /// The slots on this machine's team, itself left out.
    fn ping_allies(&self) -> mc_core::PlayerMask {
        let local = self.view.local;
        let team = self.team_of(local);
        (0..self.view.status.players.len() as u8)
            .filter(|&s| s != local && self.team_of(s) == team)
            .fold(0, |m, s| m | mc_core::player_bit(s))
    }
}
