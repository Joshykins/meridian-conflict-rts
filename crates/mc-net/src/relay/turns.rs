//! The match clock: loading, closing turns, pause, round trips and the input
//! delay that follows them, link stats, and desync detection.

use std::time::{Duration, Instant};

use mc_core::PlayerId;

use super::hub::{frame, Hub, Kind, MAX_SLOT_BACKLOG_BYTES};
use super::ConnId;
use crate::protocol::{
    command_cost, take_commands, Link, Message, PeerStat, TickBundle, MAX_ADAPTIVE_DELAY,
    MAX_COMMANDS_BYTES, MAX_INPUT_DELAY, MAX_SECTIONS,
};
use crate::wire::NetError;

/// Hash reports are compared once complete, or once this many newer ticks have piled up.
const HASH_WINDOW: usize = 512;
/// Added to the slowest round trip before it is turned into ticks of delay.
const DELAY_MARGIN_MS: u32 = 30;
/// A higher delay is taken at most this often...
const RAISE_EVERY: Duration = Duration::from_secs(1);
/// ...a lower one only after it would have done for this long.
const LOWER_AFTER: Duration = Duration::from_secs(10);

impl Hub {
    // ---- loading -----------------------------------------------------------------------------

    pub(super) fn on_loaded(&mut self, slot: PlayerId) {
        let s = &mut self.slots[slot.index()];
        if s.loaded {
            return;
        }
        s.loaded = true;
        if self
            .game
            .as_ref()
            .is_some_and(|m| m.loading_until.is_some())
        {
            self.send_loading();
            self.check_loaded();
        }
    }

    /// Starts the clock once every connected player has loaded.
    pub(super) fn check_loaded(&mut self) {
        let loading = self
            .game
            .as_ref()
            .is_some_and(|m| m.loading_until.is_some());
        let all = self
            .slots
            .iter()
            .all(|s| !s.occupied || s.conn.is_none() || s.loaded);
        if loading && all {
            self.end_loading();
        }
    }

    /// Starts the clock. Anyone still loading is treated as lagging: nobody waits for
    /// it, and it catches up from the bundles when it is ready.
    pub(super) fn end_loading(&mut self) {
        let Some(m) = &mut self.game else { return };
        if m.loading_until.take().is_none() {
            return;
        }
        m.due = Instant::now() + self.config.tick_interval;
        for (i, s) in self.slots.iter_mut().enumerate() {
            if s.occupied && !s.loaded {
                eprintln!("mc-relay: slot {i} did not load in time; starting without it");
                s.lagging = true;
            }
        }
        self.send_loading();
        self.publish_status();
    }

    pub(super) fn send_loading(&mut self) {
        let loaded = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.loaded)
            .fold(0u8, |m, (i, _)| m | 1 << i);
        self.broadcast(&Message::Loading { loaded });
    }

    // ---- pause and delay ---------------------------------------------------------------------

    pub(super) fn set_paused(&mut self, paused: bool, by: Option<PlayerId>) {
        let interval = self.config.tick_interval;
        let Some(m) = &mut self.game else { return };
        if paused == m.paused.is_some() {
            return;
        }
        if paused {
            m.paused = Some(by);
        } else {
            m.paused = None;
            m.due = Instant::now() + interval;
        }
        // Resuming names who resumed.
        self.send_clock_by(None, by);
    }

    /// The clock as it stands, to `to` (everyone when `None`).
    pub(super) fn send_clock(&mut self, to: Option<ConnId>) {
        let by = self.game.as_ref().and_then(|m| m.paused).flatten();
        self.send_clock_by(to, by);
    }

    fn send_clock_by(&mut self, to: Option<ConnId>, by: Option<PlayerId>) {
        let paused = self.game.as_ref().is_some_and(|m| m.paused.is_some());
        let msg = Message::Clock {
            paused,
            by,
            input_delay: self.input_delay,
        };
        match to {
            Some(id) => self.send(id, &msg),
            None => self.broadcast(&msg),
        }
    }

    pub(super) fn ping_all(&mut self, now: Instant) {
        let stamp = self.stamp(now);
        let Some(f) = frame(&Message::Ping(stamp)) else {
            return;
        };
        let ids: Vec<ConnId> = self
            .conns
            .iter()
            .filter(|(_, c)| c.kind != Kind::Pending)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.send_frame(id, &f);
        }
    }

    pub(super) fn on_pong(&mut self, id: ConnId, stamp: u32) {
        let now = self.stamp(Instant::now());
        let rtt_ms = now.wrapping_sub(stamp) / 1000;
        // A pong to a ping from long ago (or a forged stamp) says nothing useful.
        if rtt_ms > 60_000 {
            return;
        }
        if let Some(conn) = self.conns.get_mut(&id) {
            conn.rtt.push(rtt_ms);
        }
    }

    /// Covers the slowest connected player's recent round trip with the input delay.
    pub(super) fn adapt_delay(&mut self, now: Instant) {
        if !self.config.adaptive_delay || self.game.is_none() {
            return;
        }
        let worst = self
            .slots
            .iter()
            .filter_map(|s| s.conn)
            .filter_map(|c| self.conns.get(&c).and_then(|c| c.rtt.recent_max()))
            .max();
        let Some(worst) = worst else { return };
        let tick_ms = (self.config.tick_interval.as_millis() as u32).max(1);
        let want = (worst + DELAY_MARGIN_MS)
            .div_ceil(tick_ms)
            .clamp(1, MAX_ADAPTIVE_DELAY);
        let since_change = now.duration_since(self.delay_changed);
        if want > self.input_delay {
            self.lower_since = None;
            if since_change >= RAISE_EVERY {
                self.change_delay(want, now);
            }
        } else if want < self.input_delay {
            let since = *self.lower_since.get_or_insert(now);
            if now.duration_since(since) >= LOWER_AFTER {
                // Down a step at a time: a round trip that dipped once is not a trend.
                self.change_delay(self.input_delay - 1, now);
                self.lower_since = None;
            }
        } else {
            self.lower_since = None;
        }
    }

    fn change_delay(&mut self, delay: u32, now: Instant) {
        self.input_delay = delay;
        self.delay_changed = now;
        self.send_clock(None);
    }

    pub(super) fn send_net_stats(&mut self) {
        let loading = self
            .game
            .as_ref()
            .is_some_and(|m| m.loading_until.is_some());
        let stats: Vec<PeerStat> = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.occupied)
            .map(|(i, s)| {
                let conn = s.conn.and_then(|c| self.conns.get(&c));
                let link = match conn {
                    None => Link::Dropped,
                    Some(_) if loading && !s.loaded => Link::Loading,
                    Some(_) if s.lagging && self.game.is_some() => Link::Lagging,
                    Some(_) => Link::Connected,
                };
                PeerStat {
                    slot: PlayerId(i as u8),
                    rtt_ms: conn
                        .and_then(|c| c.rtt.typical())
                        .map_or(0, |ms| ms.min(u16::MAX as u32) as u16),
                    link,
                }
            })
            .collect();
        self.broadcast(&Message::NetStats(stats));
    }

    // ---- turns -------------------------------------------------------------------------------

    pub(super) fn on_commands(
        &mut self,
        slot: PlayerId,
        tick: u32,
        commands: Vec<Vec<u8>>,
    ) -> Result<(), NetError> {
        let Some(m) = &self.game else {
            return Err(NetError::Malformed("Commands outside a match"));
        };
        let next_tick = m.log.len() as u32;
        let s = &mut self.slots[slot.index()];
        if s.received_through.is_some_and(|r| tick <= r) {
            return Err(NetError::Malformed("Commands ticks must increase"));
        }
        // Commands answer bundle R with R + delay, and R is below the next tick to close. The
        // delay may have just changed, so anything the protocol allows is accepted.
        if tick >= next_tick.saturating_add(MAX_INPUT_DELAY) {
            return Err(NetError::Malformed("Commands for a tick too far ahead"));
        }
        s.received_through = Some(tick);
        if tick >= next_tick {
            s.lagging = false;
        }
        if !commands.is_empty() {
            s.queue_bytes += commands.iter().map(|c| command_cost(c)).sum::<usize>();
            if s.queue_bytes > MAX_SLOT_BACKLOG_BYTES {
                return Err(NetError::Limit("command backlog at the relay"));
            }
            s.queue.entry(tick).or_default().extend(commands);
        }
        Ok(())
    }

    /// Slots tick `tick` has to wait for.
    pub(super) fn awaited(&self, tick: u32) -> Vec<usize> {
        let first = self.game.as_ref().map_or(0, |m| m.start_delay);
        if tick < first {
            return Vec::new();
        }
        (0..self.slots.len())
            .filter(|&i| {
                let s = &self.slots[i];
                s.conn.is_some()
                    && s.synced
                    && !s.lagging
                    && s.received_through.is_none_or(|r| r < tick)
            })
            .collect()
    }

    /// Closes at most one tick; the next one is due an interval later at the earliest.
    pub(super) fn try_close_tick(&mut self, now: Instant) {
        let Some(m) = &self.game else { return };
        if now < m.due || m.loading_until.is_some() || m.paused.is_some() {
            return;
        }
        let tick = m.log.len() as u32;
        let stragglers = self.awaited(tick);
        if !stragglers.is_empty() && now < m.due + self.config.turn_timeout {
            return;
        }
        for i in stragglers {
            self.slots[i].lagging = true;
        }

        let mut per_slot = Vec::new();
        for (i, s) in self.slots.iter_mut().enumerate() {
            let mut budget = MAX_COMMANDS_BYTES;
            let mut commands = Vec::new();
            // Late entries first, in the order they were stamped; what exceeds the budget stays queued.
            let keys: Vec<u32> = s.queue.range(..=tick).map(|(k, _)| *k).collect();
            for key in keys {
                let Some(entry) = s.queue.get_mut(&key) else {
                    continue;
                };
                commands.extend(take_commands(entry, &mut budget));
                if !entry.is_empty() {
                    break;
                }
                s.queue.remove(&key);
            }
            s.queue_bytes -= MAX_COMMANDS_BYTES - budget;
            per_slot.push((PlayerId(i as u8), commands));
        }
        let bundle = TickBundle::new(tick, per_slot);
        let Some(bundle_frame) = frame(&Message::Bundle(bundle.clone())) else {
            self.done = true;
            return;
        };

        let Some(m) = self.game.as_mut() else {
            return;
        };
        m.log.push(bundle_frame.clone());
        m.due = (m.due + self.config.tick_interval).max(now);
        let recorded = m.replay.as_mut().map(|w| w.bundle(&bundle));
        if let Some(Err(e)) = recorded {
            self.replay_failed(e);
        }
        let live: Vec<ConnId> = self
            .conns
            .iter()
            .filter(|(_, c)| c.live)
            .map(|(id, _)| *id)
            .collect();
        for id in live {
            self.send_frame(id, &bundle_frame);
        }
    }

    // ---- desync detection --------------------------------------------------------------------

    pub(super) fn on_hash(
        &mut self,
        player: Option<PlayerId>,
        tick: u32,
        hash: u64,
    ) -> Result<(), NetError> {
        let Some(m) = &mut self.game else {
            return Err(NetError::Malformed("Hash outside a match"));
        };
        if tick as usize >= m.log.len() {
            return Err(NetError::Malformed("Hash for a tick that has not closed"));
        }
        if let Some(slot) = player {
            let reports = m.hashes.entry(tick).or_default();
            let wanted =
                self.summary.desync_tick.is_none() && tick >= self.slots[slot.index()].hash_from;
            if wanted && !reports.iter().any(|(s, _)| *s == slot) {
                reports.push((slot, hash));
            }
            self.evaluate_hashes();
        }
        Ok(())
    }

    pub(super) fn evaluate_hashes(&mut self) {
        let Some(m) = &mut self.game else { return };
        if self.summary.desync_tick.is_some() {
            m.hashes.clear();
            return;
        }
        let overflow = m.hashes.len().saturating_sub(HASH_WINDOW);
        let ticks: Vec<u32> = m.hashes.keys().copied().collect();
        for (n, tick) in ticks.into_iter().enumerate() {
            let reports = &m.hashes[&tick];
            let complete = self
                .slots
                .iter()
                .enumerate()
                .filter(|(_, s)| s.conn.is_some() && s.synced && s.hash_from <= tick)
                .all(|(i, _)| reports.iter().any(|(slot, _)| slot.index() == i));
            if !complete && n >= overflow {
                continue;
            }
            let Some(mut reports) = m.hashes.remove(&tick) else {
                continue;
            };
            if reports.is_empty() {
                continue;
            }
            if reports.iter().all(|(_, h)| *h == reports[0].1) {
                let recorded = m.replay.as_mut().map(|w| w.hash(tick, reports[0].1));
                if let Some(Err(e)) = recorded {
                    self.replay_failed(e);
                    return self.evaluate_hashes();
                }
            } else {
                reports.sort_by_key(|(slot, _)| *slot);
                self.summary.desync_tick = Some(tick);
                eprintln!("mc-relay: desync at tick {tick}: {reports:x?}");
                m.hashes.clear();
                self.broadcast(&Message::Desync {
                    tick,
                    hashes: reports,
                });
                return;
            }
        }
    }

    /// Passes a player's hash sections at the desync tick on to everyone, once per player.
    pub(super) fn on_desync_report(&mut self, slot: PlayerId, tick: u32, sections: Vec<u64>) {
        let Some(m) = &mut self.game else { return };
        let bit = 1u8 << slot.0;
        if self.summary.desync_tick != Some(tick)
            || m.desync_reported & bit != 0
            || sections.len() > MAX_SECTIONS
        {
            return;
        }
        m.desync_reported |= bit;
        self.broadcast(&Message::DesyncDetail {
            tick,
            slot,
            sections,
        });
    }
}
