//! Network play inside a match: what the sim thread keeps about the session
//! beyond ticks (links, pause, chat, reconnecting, a desync), and what the
//! interface asks of it.
//!
//! The sim thread owns a [`NetDriver`]; the interface holds a [`NetPlay`]
//! handle. They share [`NetShared`] behind a mutex: the link as it stands now,
//! and notices the interface takes exactly once. Nothing here touches the
//! simulation's state except to dump it when a desync stops the match.

use mc_net::{EndReason, PeerStat, Session, SessionEvent};
use mc_sim::state_hash::{self, SectionHashes, SECTIONS};
use mc_sim::World;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Section hashes of recent ticks, for a desync report: a desync is found a few
/// ticks after it happens, once every player's hash is in.
const KEPT_SECTIONS: usize = 1024;
/// Between reconnect attempts, and how long to keep trying.
const REJOIN_EVERY: Duration = Duration::from_secs(2);
const REJOIN_FOR: Duration = Duration::from_secs(90);

/// What went wrong with a match's lockstep, for the report card and the dump.
#[derive(Clone, Debug)]
pub struct DesyncReport {
    pub tick: u32,
    /// Every player's whole-state hash at `tick`, by slot.
    pub hashes: Vec<(u8, u64)>,
    /// This machine's slot and hash sections at `tick`, when it still had them.
    pub local: Option<u8>,
    pub ours: Option<SectionHashes>,
    /// Other players' sections as the relay passes them on.
    pub theirs: Vec<(u8, Vec<u64>)>,
    /// Where this machine's state and the report were written.
    pub dump: Option<PathBuf>,
}

impl DesyncReport {
    /// The state sections that differ between any two machines that reported them.
    pub fn differing(&self) -> Vec<&'static str> {
        let mut all: Vec<&[u64]> = self.theirs.iter().map(|(_, s)| s.as_slice()).collect();
        if let Some(ours) = &self.ours {
            all.push(ours);
        }
        SECTIONS
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                let mut values = all.iter().filter_map(|s| s.get(*i));
                let first = values.next();
                values.any(|v| Some(v) != first)
            })
            .map(|(_, name)| *name)
            .collect()
    }
}

/// Reconnecting after the connection dropped mid-match.
#[derive(Clone, Debug)]
pub struct Rejoining {
    pub attempts: u32,
    pub since: Instant,
}

/// The session as the interface sees it, refreshed as events arrive.
#[derive(Clone, Debug, Default)]
pub struct NetLink {
    /// Every seat's link and round trip, by the relay.
    pub stats: Vec<PeerStat>,
    /// Ticks between an order and its execution.
    pub input_delay: u32,
    /// Set while the match is paused: by whom (`None`: an observer or the relay).
    pub paused_by: Option<Option<u8>>,
    /// Set after the start until the clock runs: the slots that have loaded.
    pub loading: Option<u8>,
    pub rejoining: Option<Rejoining>,
    pub desync: Option<DesyncReport>,
    /// This player surrendered to leave: since when. The match is left once the
    /// surrender has been carried out (or a moment has passed).
    pub surrendering: Option<Instant>,
}

impl NetLink {
    pub fn stat(&self, slot: u8) -> Option<&PeerStat> {
        self.stats.iter().find(|s| s.slot.0 == slot)
    }
}

/// Something that happened, for the interface to show once.
#[derive(Clone, Debug, PartialEq)]
pub enum NetNotice {
    Chat {
        /// `None`: an observer, or the server itself when `name` is empty.
        from: Option<u8>,
        name: String,
        /// Sent to a few, not to everyone.
        private: bool,
        text: String,
    },
    Dropped(u8),
    Rejoined(u8),
    Paused(Option<u8>),
    Resumed(Option<u8>),
    /// This machine's connection came back.
    Reconnected,
}

#[derive(Default)]
pub struct NetShared {
    pub link: NetLink,
    notices: Vec<NetNotice>,
}

/// What the interface asks the session for.
pub enum NetRequest {
    Chat {
        text: String,
        to: u8,
    },
    Pause(bool),
    /// Give up the match (so this side is defeated, not left standing idle), then leave.
    Surrender,
}

/// The interface's side: read the link, take the notices, send requests.
pub struct NetPlay {
    shared: Arc<Mutex<NetShared>>,
    requests: Sender<NetRequest>,
}

impl NetPlay {
    pub fn link(&self) -> NetLink {
        lock(&self.shared).link.clone()
    }

    /// Notices since the last call, oldest first.
    pub fn notices(&self) -> Vec<NetNotice> {
        std::mem::take(&mut lock(&self.shared).notices)
    }

    pub fn request(&self, request: NetRequest) {
        // A closed channel means the match is over; there is nobody to ask.
        let _ = self.requests.send(request);
    }
}

/// How to come back after the connection drops: the relay, and who we are
/// (with the reconnect token).
pub struct Rejoin {
    pub addr: String,
    pub config: mc_net::ClientConfig,
}

/// The sim thread's side.
pub struct NetDriver {
    shared: Arc<Mutex<NetShared>>,
    requests: Receiver<NetRequest>,
    rejoin: Option<Rejoin>,
    local: Option<u8>,
    recent: VecDeque<(u32, SectionHashes)>,
    next_attempt: Instant,
    /// A desync stopped the match: ticks are no longer stepped.
    pub frozen: bool,
}

/// A driver for the sim thread and the handle for the interface, sharing one link.
pub fn pair(rejoin: Option<Rejoin>, local: Option<u8>) -> (NetDriver, NetPlay) {
    let shared: Arc<Mutex<NetShared>> = Arc::default();
    let (tx, rx) = std::sync::mpsc::channel();
    (
        NetDriver {
            shared: shared.clone(),
            requests: rx,
            rejoin,
            local,
            recent: VecDeque::with_capacity(KEPT_SECTIONS),
            next_attempt: Instant::now(),
            frozen: false,
        },
        NetPlay {
            shared,
            requests: tx,
        },
    )
}

fn lock(m: &Mutex<NetShared>) -> std::sync::MutexGuard<'_, NetShared> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl NetDriver {
    /// Passes the interface's requests on to the session.
    pub fn serve(&mut self, session: &mut dyn Session) {
        while let Ok(request) = self.requests.try_recv() {
            match request {
                NetRequest::Chat { text, to } => {
                    if let Err(e) = session.chat(&text, to) {
                        log::warn!("chat not sent: {e}");
                    }
                }
                NetRequest::Pause(paused) => {
                    session.set_paused(paused);
                }
                NetRequest::Surrender => {
                    let resign = mc_sim::Command::Resign.encode();
                    if let Err(e) = session.submit(vec![resign]) {
                        log::warn!("surrender not sent: {e}");
                    }
                    lock(&self.shared).link.surrendering = Some(Instant::now());
                }
            }
        }
    }

    /// Keeps a stepped tick's section hashes for a possible desync report.
    pub fn record(&mut self, tick: u32, sections: SectionHashes) {
        if self.recent.len() == KEPT_SECTIONS {
            self.recent.pop_front();
        }
        self.recent.push_back((tick, sections));
    }

    /// A snapshot replaced the state: the hashes kept before it describe another history.
    pub fn restored(&mut self) {
        self.recent.clear();
    }

    /// Takes the session events that are about the link rather than the ticks.
    /// Returns false for the ones the sim thread handles itself.
    pub fn on_event(&mut self, event: &SessionEvent) -> bool {
        let mut s = lock(&self.shared);
        match event {
            SessionEvent::NetStats(stats) => s.link.stats = stats.clone(),
            SessionEvent::Loading { loaded } => {
                // The final `Loading` comes as the clock starts; the first tick clears it.
                s.link.loading = Some(*loaded);
            }
            SessionEvent::Clock {
                paused,
                by,
                input_delay,
            } => {
                let by = by.map(|p| p.0);
                let was = s.link.paused_by.is_some();
                s.link.input_delay = *input_delay;
                s.link.paused_by = paused.then_some(by);
                match (was, *paused) {
                    (false, true) => s.notices.push(NetNotice::Paused(by)),
                    (true, false) => s.notices.push(NetNotice::Resumed(by)),
                    _ => {}
                }
            }
            SessionEvent::Chat {
                from,
                name,
                to,
                text,
            } => s.notices.push(NetNotice::Chat {
                from: from.map(|p| p.0),
                name: name.clone(),
                private: *to != 0,
                text: text.clone(),
            }),
            SessionEvent::PlayerDropped(p) => s.notices.push(NetNotice::Dropped(p.0)),
            SessionEvent::PlayerRejoined(p) => {
                if Some(p.0) == self.local {
                    s.link.rejoining = None;
                    s.notices.push(NetNotice::Reconnected);
                } else {
                    s.notices.push(NetNotice::Rejoined(p.0));
                }
            }
            SessionEvent::DesyncDetail { slot, sections, .. } => {
                if let Some(d) = &mut s.link.desync {
                    if Some(slot.0) != d.local && !d.theirs.iter().any(|(t, _)| *t == slot.0) {
                        d.theirs.push((slot.0, sections.clone()));
                    }
                }
            }
            _ => return false,
        }
        true
    }

    /// The clock has started: the load barrier is behind us.
    pub fn ticking(&mut self) {
        let mut s = lock(&self.shared);
        if s.link.loading.is_some() {
            s.link.loading = None;
        }
    }

    /// The match is out of step. Reports our sections, writes our state for the
    /// post-mortem, and stops stepping. Returns what to tell the player.
    pub fn desync(
        &mut self,
        tick: u32,
        hashes: &[(mc_core::PlayerId, u64)],
        world: &mut World,
        session: &mut dyn Session,
    ) -> String {
        self.frozen = true;
        let ours = self
            .recent
            .iter()
            .find(|(t, _)| *t == tick)
            .map(|(_, s)| *s);
        if let Some(sections) = &ours {
            session.report_desync(tick, sections);
        }
        let mut report = DesyncReport {
            tick,
            hashes: hashes.iter().map(|(p, h)| (p.0, *h)).collect(),
            local: self.local,
            ours,
            theirs: Vec::new(),
            dump: None,
        };
        report.dump = dump(&report, world);
        let seconds = tick / mc_core::TICKS_PER_SECOND;
        let message = format!(
            "The machines fell out of step at {}:{:02}.",
            seconds / 60,
            seconds % 60
        );
        lock(&self.shared).link.desync = Some(report);
        message
    }

    /// The connection dropped. True if it is worth trying to come back.
    pub fn lost(&mut self, why: &EndReason) -> bool {
        if self.frozen || self.rejoin.is_none() {
            return false;
        }
        let EndReason::ConnectionLost(why) = why else {
            return false;
        };
        let mut s = lock(&self.shared);
        if s.link.rejoining.is_none() {
            log::warn!("connection lost: {why}; reconnecting");
            s.link.rejoining = Some(Rejoining {
                attempts: 0,
                since: Instant::now(),
            });
        }
        true
    }

    /// While reconnecting: tries again when it is time. `Err` once it is time to give up.
    pub fn try_rejoin(&mut self) -> Result<Option<mc_net::NetSession>, String> {
        let now = Instant::now();
        let since = {
            let s = lock(&self.shared);
            match &s.link.rejoining {
                None => return Ok(None),
                Some(r) => r.since,
            }
        };
        if now.duration_since(since) > REJOIN_FOR {
            return Err("the connection to the match was lost and did not come back".into());
        }
        if now < self.next_attempt {
            return Ok(None);
        }
        self.next_attempt = now + REJOIN_EVERY;
        let Some(rejoin) = &self.rejoin else {
            return Ok(None);
        };
        if let Some(r) = &mut lock(&self.shared).link.rejoining {
            r.attempts += 1;
        }
        match mc_net::NetSession::connect(rejoin.addr.as_str(), rejoin.config.clone()) {
            Ok(session) => {
                self.recent.clear();
                Ok(Some(session))
            }
            Err(e) => {
                log::info!("reconnect attempt failed: {e}");
                Ok(None)
            }
        }
    }

    pub fn rejoining(&self) -> bool {
        lock(&self.shared).link.rejoining.is_some()
    }
}

/// Writes this machine's state and the report beside the settings, for comparing
/// with the other players' (`meridian --desync-diff`).
fn dump(report: &DesyncReport, world: &mut World) -> Option<PathBuf> {
    let dir = crate::settings::config_dir()?.join("desync");
    std::fs::create_dir_all(&dir).ok()?;
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let slot = report
        .local
        .map_or("observer".to_owned(), |s| format!("p{s}"));
    let stem = dir.join(format!("desync-{unix}-t{}-{slot}", report.tick));
    let snapshot = stem.with_extension("mcsnap");
    let mut text = format!(
        "{}\ndesync at tick {} (state below is from tick {})\nhashes: {:x?}\n",
        crate::BUILD,
        report.tick,
        world.tick_count(),
        report.hashes
    );
    match &report.ours {
        Some(ours) => {
            for (name, h) in SECTIONS.iter().zip(ours) {
                text.push_str(&format!("  {name:<12} {h:016x}\n"));
            }
            text.push_str(&format!(
                "  whole        {:016x}\n",
                state_hash::combine(ours)
            ));
        }
        None => text.push_str("  (this machine no longer had that tick's sections)\n"),
    }
    let written = std::fs::write(&snapshot, world.snapshot())
        .and_then(|_| std::fs::write(stem.with_extension("txt"), text));
    match written {
        Ok(()) => {
            log::error!("desync: state written to {}", snapshot.display());
            Some(snapshot)
        }
        Err(e) => {
            log::error!("desync: could not write the state: {e}");
            None
        }
    }
}
