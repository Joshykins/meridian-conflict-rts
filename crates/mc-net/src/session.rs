//! The [`Session`] trait and its two socket-free implementations.
//!
//! See the crate docs for the contract between a session and the game loop.

use std::collections::{BTreeMap, VecDeque};
use std::io::{self, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use mc_core::{PlayerId, TICKS_PER_SECOND};

use crate::protocol::{check_commands, LobbyState, MatchStart, RefuseReason, TickBundle, Welcome};
use crate::replay::{Replay, ReplayWriter};
use crate::wire::NetError;

/// Ticks released by one `poll` unless the caller asks for a different budget.
pub const DEFAULT_TICK_BUDGET: u32 = 10;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EndReason {
    /// The match or replay ran to its end.
    Finished,
    Refused {
        reason: RefuseReason,
        detail: String,
    },
    /// Reconnect with the token from `Joined` to resume from a snapshot.
    ConnectionLost(String),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SessionEvent {
    /// The relay accepted us. Carries the slot and the reconnect token.
    Joined(Welcome),
    Lobby(LobbyState),
    /// Build tick-0 state from this. Always precedes the first `TickReady`.
    Started(MatchStart),
    /// Replace the whole sim state with `blob`: the state right after `tick`.
    /// The next `TickReady` is `tick + 1`.
    SnapshotLoaded {
        tick: u32,
        blob: Vec<u8>,
    },
    /// Apply these commands and step the sim once. Ticks arrive in order with no gaps.
    TickReady(TickBundle),
    /// Right after stepping `tick`, serialise the sim and call `provide_snapshot`.
    /// Always delivered before `TickReady(tick)`.
    SnapshotWanted {
        tick: u32,
    },
    /// Players reported different hashes for `tick`. Sent once per match.
    Desync {
        tick: u32,
        hashes: Vec<(PlayerId, u64)>,
    },
    /// Playback produced a different hash than the recording did.
    ReplayDiverged {
        tick: u32,
        recorded: u64,
        computed: u64,
    },
    PlayerDropped(PlayerId),
    PlayerRejoined(PlayerId),
    Chat {
        from: Option<PlayerId>,
        text: String,
    },
    /// Recording stopped because of an io error; the match itself goes on.
    ReplayWriteFailed(String),
    /// Nothing follows this event.
    Ended(EndReason),
}

/// What the game loop talks to. Identical for single-player, multiplayer and playback.
pub trait Session {
    /// Queues the local player's commands. The session stamps them with the
    /// tick that will execute them (current tick plus input delay) when it
    /// cuts the next turn. Sessions without a local player accept and ignore
    /// commands. An error means nothing was queued.
    fn submit(&mut self, commands: Vec<Vec<u8>>) -> Result<(), NetError>;

    /// Never blocks. Returns what happened since the last call, in order,
    /// with at most `tick_budget` `TickReady` events.
    fn poll(&mut self) -> Vec<SessionEvent>;

    /// Reports the sim's state hash right after stepping `tick`. Any subset of
    /// ticks may be reported as long as every machine reports the same subset.
    fn report_hash(&mut self, tick: u32, hash: u64);

    /// Answers `SnapshotWanted { tick }`.
    fn provide_snapshot(&mut self, tick: u32, blob: Vec<u8>) -> Result<(), NetError>;

    /// Caps `TickReady` events per `poll` so catching up (after a join, a
    /// stall, or in fast playback) never turns into one giant frame.
    fn set_tick_budget(&mut self, max_ticks_per_poll: u32);

    /// The caller has just stepped a tick. A local clock that overran its
    /// slot must not mint extra ticks to catch up: a cheap tick after a
    /// slow one would publish two frames in a few milliseconds and units
    /// would jump forward. Network sessions ignore this — the relay paces.
    fn credit_tick(&mut self) {}

    /// The slot `submit` issues commands for, once known.
    fn local_player(&self) -> Option<PlayerId>;

    /// Stops or restarts the clock, if this session owns one. Returns whether it
    /// does: a single-player match can pause, a network match cannot.
    fn set_paused(&mut self, _paused: bool) -> bool {
        false
    }

    /// Runs the clock at `percent` of real time, if this session owns one.
    /// Returns whether it does, like `set_paused`.
    fn set_speed(&mut self, _percent: u32) -> bool {
        false
    }
}

/// Ordered event buffer that enforces the tick budget.
pub(crate) struct EventQueue {
    queue: VecDeque<SessionEvent>,
    budget: u32,
    ended: bool,
}

impl EventQueue {
    pub fn new() -> EventQueue {
        EventQueue {
            queue: VecDeque::new(),
            budget: DEFAULT_TICK_BUDGET,
            ended: false,
        }
    }

    pub fn set_budget(&mut self, budget: u32) {
        self.budget = budget.max(1);
    }

    pub fn budget(&self) -> u32 {
        self.budget
    }

    pub fn ended(&self) -> bool {
        self.ended
    }

    /// `TickReady` events waiting to be released.
    pub fn queued_ticks(&self) -> usize {
        self.queue
            .iter()
            .filter(|e| matches!(e, SessionEvent::TickReady(_)))
            .count()
    }

    pub fn push(&mut self, event: SessionEvent) {
        if self.ended {
            return;
        }
        self.ended = matches!(event, SessionEvent::Ended(_));
        self.queue.push_back(event);
    }

    /// Stops in front of the first tick over budget so ordering is kept.
    pub fn drain(&mut self) -> Vec<SessionEvent> {
        let mut out = Vec::new();
        let mut ticks = 0;
        while let Some(event) = self.queue.front() {
            if matches!(event, SessionEvent::TickReady(_)) {
                if ticks == self.budget {
                    break;
                }
                ticks += 1;
            }
            out.extend(self.queue.pop_front());
        }
        out
    }
}

/// How fast a socket-free session releases ticks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pacing {
    /// 10 ticks per second of wall-clock time.
    RealTime,
    /// Percent of real time: 100 is normal, 400 is 4x, 0 is paused.
    Speed(u32),
    /// Exactly this many ticks per `poll` (still capped by the tick budget).
    /// For headless runs, tests and benchmarks.
    PerPoll(u32),
}

struct TickClock {
    pacing: Pacing,
    /// Next wall-clock instant a real-time tick may be released.
    next_due: Instant,
}

const TICK_US: u64 = 1_000_000 / TICKS_PER_SECOND as u64;

impl TickClock {
    fn new(pacing: Pacing) -> TickClock {
        TickClock {
            pacing,
            next_due: Instant::now(),
        }
    }

    fn set_pacing(&mut self, pacing: Pacing) {
        self.pacing = pacing;
        self.next_due = Instant::now();
    }

    fn interval(&self) -> Option<Duration> {
        let percent = match self.pacing {
            Pacing::PerPoll(_) => return None,
            Pacing::RealTime => 100,
            Pacing::Speed(p) => p as u64,
        };
        if percent == 0 {
            return None;
        }
        Some(Duration::from_micros(TICK_US.saturating_mul(100) / percent))
    }

    /// Ticks to release now, at most `max`. Wall-clock pacing never banks a
    /// stall into extra ticks: one release, then the next slot starts now.
    fn due(&mut self, max: u32) -> u32 {
        match self.pacing {
            Pacing::PerPoll(n) => return n.min(max),
            Pacing::RealTime | Pacing::Speed(_) => {}
        }
        let Some(interval) = self.interval() else {
            return 0;
        };
        if max == 0 || Instant::now() < self.next_due {
            return 0;
        }
        self.next_due = Instant::now() + interval;
        1
    }

    /// A step that finished after its slot must wait a full interval from now
    /// before the next release, so a cheap follow-up tick cannot bunch.
    fn credit(&mut self) {
        let Some(interval) = self.interval() else {
            return;
        };
        let now = Instant::now();
        if now >= self.next_due {
            self.next_due = now + interval;
        }
    }
}

type BoxedReplayWriter = ReplayWriter<Box<dyn Write + Send>>;

/// Single-player and tools: no sockets, no latency, commands execute on the
/// next tick. Still produces the same bundles and the same replay file a
/// network match would.
pub struct LocalSession {
    start: MatchStart,
    local: Option<PlayerId>,
    pending: BTreeMap<PlayerId, Vec<Vec<u8>>>,
    next_tick: u32,
    clock: TickClock,
    /// The pacing to return to after a pause.
    pacing: Pacing,
    paused: bool,
    events: EventQueue,
    replay: Option<BoxedReplayWriter>,
    finished: bool,
}

impl LocalSession {
    fn from_start(
        start: MatchStart,
        local: Option<PlayerId>,
        pacing: Pacing,
    ) -> Result<LocalSession, NetError> {
        start.validate()?;
        let mut events = EventQueue::new();
        events.push(SessionEvent::Started(start.clone()));
        Ok(LocalSession {
            start,
            local,
            pending: BTreeMap::new(),
            next_tick: 0,
            clock: TickClock::new(pacing),
            pacing,
            paused: false,
            events,
            replay: None,
            finished: false,
        })
    }

    /// `local` must be one of `start.players`.
    pub fn new(
        start: MatchStart,
        local: PlayerId,
        pacing: Pacing,
    ) -> Result<LocalSession, NetError> {
        if !start.players.iter().any(|p| p.slot == local) {
            return Err(NetError::Limit("local player is not part of the match"));
        }
        Self::from_start(start, Some(local), pacing)
    }

    /// A local match with no human slot: AI vs AI, replays of the same kind.
    /// `start.players` is empty so nobody is later rewritten as human.
    pub fn observer(start: MatchStart, pacing: Pacing) -> Result<LocalSession, NetError> {
        Self::from_start(start, None, pacing)
    }

    /// Records to `path`. Call before the first `poll`.
    pub fn record_to(&mut self, path: impl AsRef<Path>) -> io::Result<()> {
        let file = io::BufWriter::new(std::fs::File::create(path)?);
        self.record_to_writer(Box::new(file))
    }

    pub fn record_to_writer(&mut self, out: Box<dyn Write + Send>) -> io::Result<()> {
        if self.next_tick != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "recording must start before tick 0",
            ));
        }
        self.replay = Some(ReplayWriter::new(out, &self.start)?);
        Ok(())
    }

    /// Commands for another slot: scripted opponents, AI that lives outside the sim, tests.
    pub fn submit_as(&mut self, slot: PlayerId, commands: Vec<Vec<u8>>) -> Result<(), NetError> {
        check_commands(&commands)?;
        if !self.start.players.iter().any(|p| p.slot == slot) {
            return Err(NetError::Limit("slot is not part of the match"));
        }
        if !self.finished {
            self.pending.entry(slot).or_default().extend(commands);
        }
        Ok(())
    }

    pub fn set_pacing(&mut self, pacing: Pacing) {
        self.pacing = pacing;
        if !self.paused {
            self.clock.set_pacing(pacing);
        }
    }

    /// Ends the match: completes the replay file and queues `Ended`.
    pub fn finish(&mut self) -> io::Result<()> {
        if self.finished {
            return Ok(());
        }
        self.finished = true;
        self.events.push(SessionEvent::Ended(EndReason::Finished));
        match self.replay.take() {
            Some(mut w) => w.finish(),
            None => Ok(()),
        }
    }

    fn record(&mut self, write: impl FnOnce(&mut BoxedReplayWriter) -> io::Result<()>) {
        if let Some(w) = &mut self.replay {
            if let Err(e) = write(w) {
                self.replay = None;
                self.events
                    .push(SessionEvent::ReplayWriteFailed(e.to_string()));
            }
        }
    }
}

impl Session for LocalSession {
    fn submit(&mut self, commands: Vec<Vec<u8>>) -> Result<(), NetError> {
        match self.local {
            Some(slot) => self.submit_as(slot, commands),
            None => {
                check_commands(&commands)?;
                Ok(())
            }
        }
    }

    fn poll(&mut self) -> Vec<SessionEvent> {
        if !self.finished {
            // Only mint ticks the caller can take now; the rest stays in the clock.
            let room = self
                .events
                .budget()
                .saturating_sub(self.events.queued_ticks() as u32);
            for _ in 0..self.clock.due(room) {
                let bundle = TickBundle::new(self.next_tick, std::mem::take(&mut self.pending));
                self.next_tick += 1;
                self.record(|w| w.bundle(&bundle));
                self.events.push(SessionEvent::TickReady(bundle));
            }
        }
        self.events.drain()
    }

    fn report_hash(&mut self, tick: u32, hash: u64) {
        self.record(|w| w.hash(tick, hash));
    }

    fn provide_snapshot(&mut self, _tick: u32, _blob: Vec<u8>) -> Result<(), NetError> {
        Ok(())
    }

    fn set_tick_budget(&mut self, max_ticks_per_poll: u32) {
        self.events.set_budget(max_ticks_per_poll);
    }

    fn credit_tick(&mut self) {
        self.clock.credit();
    }

    fn local_player(&self) -> Option<PlayerId> {
        self.local
    }

    fn set_paused(&mut self, paused: bool) -> bool {
        self.paused = paused;
        self.clock.set_pacing(if paused {
            Pacing::Speed(0)
        } else {
            self.pacing
        });
        true
    }

    fn set_speed(&mut self, percent: u32) -> bool {
        // A session stepped by its caller (tests, benchmarks) has no clock to speed up.
        if matches!(self.pacing, Pacing::PerPoll(_)) {
            return false;
        }
        self.set_pacing(Pacing::Speed(percent.max(1)));
        true
    }
}

impl Drop for LocalSession {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

/// Plays a replay through the same interface as a live match.
pub struct ReplaySession {
    replay: Replay,
    position: usize,
    clock: TickClock,
    events: EventQueue,
}

impl ReplaySession {
    pub fn new(replay: Replay, pacing: Pacing) -> ReplaySession {
        let mut events = EventQueue::new();
        events.push(SessionEvent::Started(replay.start.clone()));
        ReplaySession {
            replay,
            position: 0,
            clock: TickClock::new(pacing),
            events,
        }
    }

    pub fn open(path: impl AsRef<Path>, pacing: Pacing) -> Result<ReplaySession, NetError> {
        Ok(ReplaySession::new(Replay::load(path)?, pacing))
    }

    pub fn set_pacing(&mut self, pacing: Pacing) {
        self.clock.set_pacing(pacing);
    }

    pub fn total_ticks(&self) -> u32 {
        self.replay.bundles.len() as u32
    }

    /// Ticks released so far.
    pub fn position(&self) -> u32 {
        self.position as u32
    }

    pub fn replay(&self) -> &Replay {
        &self.replay
    }
}

impl Session for ReplaySession {
    fn submit(&mut self, _commands: Vec<Vec<u8>>) -> Result<(), NetError> {
        Ok(())
    }

    fn poll(&mut self) -> Vec<SessionEvent> {
        if !self.events.ended() {
            let queued = self.events.queued_ticks() as u32;
            if self.position == self.replay.bundles.len() && queued == 0 {
                // Only once the caller has been handed the last tick in an earlier poll, so a
                // divergence in the final ticks is still reported in front of `Ended`.
                self.events.push(SessionEvent::Ended(EndReason::Finished));
            }
            let room = self.events.budget().saturating_sub(queued);
            for _ in 0..self.clock.due(room) {
                match self.replay.bundles.get(self.position) {
                    Some(b) => {
                        self.events.push(SessionEvent::TickReady(b.clone()));
                        self.position += 1;
                    }
                    None => break,
                }
            }
        }
        self.events.drain()
    }

    fn report_hash(&mut self, tick: u32, hash: u64) {
        if let Some(&recorded) = self.replay.hashes.get(&tick) {
            if recorded != hash {
                self.events.push(SessionEvent::ReplayDiverged {
                    tick,
                    recorded,
                    computed: hash,
                });
            }
        }
    }

    fn provide_snapshot(&mut self, _tick: u32, _blob: Vec<u8>) -> Result<(), NetError> {
        Ok(())
    }

    fn set_tick_budget(&mut self, max_ticks_per_poll: u32) {
        self.events.set_budget(max_ticks_per_poll);
    }

    fn credit_tick(&mut self) {
        self.clock.credit();
    }

    fn local_player(&self) -> Option<PlayerId> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ContentId, PlayerSetup, MAX_COMMAND_LEN};

    fn start() -> MatchStart {
        MatchStart {
            content: ContentId {
                map_id: 1,
                blueprint_hash: 2,
            },
            seed: 3,
            input_delay: 0,
            players: vec![
                PlayerSetup {
                    slot: PlayerId(0),
                    name: "me".into(),
                    data: vec![],
                },
                PlayerSetup {
                    slot: PlayerId(1),
                    name: "bot".into(),
                    data: vec![],
                },
            ],
            options: vec![],
        }
    }

    fn ticks(events: &[SessionEvent]) -> Vec<&TickBundle> {
        events
            .iter()
            .filter_map(|e| {
                if let SessionEvent::TickReady(b) = e {
                    Some(b)
                } else {
                    None
                }
            })
            .collect()
    }

    #[test]
    fn local_session_orders_by_slot_and_respects_budget() {
        let mut s = LocalSession::new(start(), PlayerId(0), Pacing::PerPoll(100)).unwrap();
        s.set_tick_budget(3);
        s.submit_as(PlayerId(1), vec![vec![2]]).unwrap();
        s.submit(vec![vec![1]]).unwrap();
        let events = s.poll();
        assert!(matches!(events[0], SessionEvent::Started(_)));
        let got = ticks(&events);
        assert_eq!(got.len(), 3);
        assert_eq!(
            got[0],
            &TickBundle::new(
                0,
                [(PlayerId(0), vec![vec![1]]), (PlayerId(1), vec![vec![2]])]
            )
        );
        assert!(got[1].is_empty() && got[1].tick == 1 && got[2].tick == 2);
        assert_eq!(ticks(&s.poll())[0].tick, 3);
    }

    #[test]
    fn limits_reach_the_caller() {
        let mut s = LocalSession::new(start(), PlayerId(0), Pacing::PerPoll(1)).unwrap();
        assert!(matches!(
            s.submit(vec![vec![0; MAX_COMMAND_LEN + 1]]),
            Err(NetError::Limit(_))
        ));
        assert!(matches!(
            s.submit_as(PlayerId(5), vec![]),
            Err(NetError::Limit(_))
        ));
        assert!(LocalSession::new(start(), PlayerId(4), Pacing::RealTime).is_err());
    }

    #[test]
    fn paused_and_real_time_clocks() {
        let mut s = LocalSession::new(start(), PlayerId(0), Pacing::Speed(0)).unwrap();
        assert!(ticks(&s.poll()).is_empty());
        assert!(ticks(&s.poll()).is_empty());
        // 100x real time is one tick per millisecond; wait for a few with a generous deadline.
        s.set_pacing(Pacing::Speed(10_000));
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut seen = 0;
        while seen < 5 {
            assert!(Instant::now() < deadline);
            for b in ticks(&s.poll()) {
                assert_eq!(b.tick, seen);
                seen += 1;
            }
            std::thread::yield_now();
        }
    }

    #[test]
    fn real_time_clock_does_not_catch_up_after_a_slow_step() {
        let mut s = LocalSession::new(start(), PlayerId(0), Pacing::RealTime).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut first = None;
        while first.is_none() {
            assert!(Instant::now() < deadline, "first tick did not arrive");
            first = ticks(&s.poll()).first().map(|b| b.tick);
            if first.is_none() {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        // A 250 ms step is two and a half tick slots. Catch-up would release
        // another tick at once; credit must hold the next one off.
        std::thread::sleep(Duration::from_millis(250));
        s.credit_tick();
        assert!(
            ticks(&s.poll()).is_empty(),
            "a slow step must not mint a follow-up tick"
        );
        std::thread::sleep(Duration::from_millis(40));
        assert!(
            ticks(&s.poll()).is_empty(),
            "credit must cover the rest of the interval, not only this poll"
        );
    }

    #[test]
    fn observer_session_has_no_slot_and_drops_commands() {
        let start = MatchStart {
            players: Vec::new(),
            ..start()
        };
        let mut s = LocalSession::observer(start, Pacing::PerPoll(2)).unwrap();
        assert_eq!(s.local_player(), None);
        s.submit(vec![vec![1]]).unwrap();
        let events = s.poll();
        assert!(matches!(events[0], SessionEvent::Started(_)));
        let got = ticks(&events);
        assert_eq!(got.len(), 2);
        assert!(got[0].is_empty() && got[1].is_empty());
        assert!(s.set_paused(true));
    }

    #[test]
    fn replay_session_flags_divergence_and_ends() {
        let replay = Replay {
            start: start(),
            bundles: (0..5).map(TickBundle::empty).collect(),
            hashes: BTreeMap::from([(2, 22)]),
            complete: true,
        };
        let mut s = ReplaySession::new(replay, Pacing::PerPoll(2));
        s.submit(vec![vec![1]]).unwrap();
        let mut all = Vec::new();
        for _ in 0..4 {
            all.extend(s.poll());
        }
        assert_eq!(ticks(&all).len(), 5);
        assert_eq!(all.last(), Some(&SessionEvent::Ended(EndReason::Finished)));
        assert!(s.poll().is_empty());

        s.report_hash(2, 22);
        s.report_hash(3, 1);
        assert!(s.poll().is_empty());
    }

    #[test]
    fn replay_divergence_is_reported() {
        let replay = Replay {
            start: start(),
            bundles: (0..50).map(TickBundle::empty).collect(),
            hashes: BTreeMap::from([(2, 22)]),
            complete: true,
        };
        let mut s = ReplaySession::new(replay, Pacing::PerPoll(5));
        s.poll();
        s.report_hash(2, 23);
        assert!(s.poll().contains(&SessionEvent::ReplayDiverged {
            tick: 2,
            recorded: 22,
            computed: 23
        }));
    }
}
