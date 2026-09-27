//! Joining a running match: late observers and reconnecting players enter from
//! a snapshot one healthy player provides, then the bundles after it.

use std::sync::Arc;
use std::time::Instant;

use super::hub::{frame, Hub, Kind, SnapshotJob};
use super::ConnId;
use crate::protocol::{snapshot_chunks, Message, SnapshotAssembler};

impl Hub {
    /// Attaches waiting joiners to a snapshot job, starting one if needed.
    pub(super) fn service_joiners(&mut self, avoid: Option<ConnId>) {
        let Some(m) = &mut self.game else { return };
        if m.waiting.is_empty() {
            return;
        }
        if let Some(job) = &mut m.snapshot {
            // Any snapshot still in flight serves later joiners just as well.
            job.joiners.append(&mut m.waiting);
            return;
        }
        let tick = m.log.len() as u32;
        if tick == 0 {
            // Nothing has happened yet: the joiner starts from `MatchStart` like everyone else.
            let joiners = std::mem::take(&mut m.waiting);
            return self.go_live(&joiners, 0, 0);
        }
        let candidates: Vec<(bool, ConnId)> = self
            .slots
            .iter()
            .filter(|s| s.synced)
            .filter_map(|s| s.conn.map(|c| (s.lagging, c)))
            .collect();
        let provider = candidates
            .iter()
            .filter(|(_, c)| Some(*c) != avoid)
            .min()
            .or(candidates.iter().min())
            .map(|(_, c)| *c);
        match provider {
            Some(provider) => {
                m.snapshot = Some(SnapshotJob {
                    tick,
                    provider,
                    deadline: Instant::now() + self.config.snapshot_timeout,
                    assembler: SnapshotAssembler::new(),
                    joiners: std::mem::take(&mut m.waiting),
                });
                self.send(provider, &Message::SnapshotRequest { tick });
            }
            None => {
                // Nobody holds the state. The log from tick 0 rebuilds it, slowly but exactly.
                let joiners = std::mem::take(&mut m.waiting);
                self.go_live(&joiners, 0, 0);
            }
        }
    }

    pub(super) fn deliver_snapshot(&mut self, tick: u32, blob: &[u8]) {
        let Some(job) = self.game.as_mut().and_then(|m| m.snapshot.take()) else {
            return;
        };
        let Ok(chunks) = snapshot_chunks(tick, blob) else {
            return;
        };
        let frames: Vec<Arc<[u8]>> = chunks.iter().filter_map(frame).collect();
        for &id in &job.joiners {
            for f in &frames {
                self.send_frame(id, f);
            }
        }
        self.go_live(&job.joiners, tick + 1, tick + 1);
        self.service_joiners(None);
    }

    /// Sends the log from `from_tick` and switches the joiners to the live stream. The hub is
    /// single-threaded, so no tick can close between the two.
    fn go_live(&mut self, joiners: &[ConnId], from_tick: u32, hash_from: u32) {
        for &id in joiners {
            let Some(m) = &self.game else { return };
            let backlog: Vec<Arc<[u8]>> =
                m.log.get(from_tick as usize..).unwrap_or_default().to_vec();
            for f in &backlog {
                self.send_frame(id, f);
            }
            let Some(conn) = self.conns.get_mut(&id) else {
                continue;
            };
            conn.live = true;
            if let Kind::Player(slot) = conn.kind {
                let s = &mut self.slots[slot.index()];
                s.synced = true;
                s.hash_from = hash_from;
                // Coming in after the clock started needs no load barrier; at tick 0 it still does.
                s.loaded |= from_tick > 0;
                self.broadcast(&Message::PlayerRejoined(slot));
            }
        }
    }
}
