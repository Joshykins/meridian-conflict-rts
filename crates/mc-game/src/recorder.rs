//! Recording a match from what the sim thread carries out, for the matches whose
//! session does not record itself: a network match (its session is a socket, and
//! is swapped for a fresh one when the connection comes back) and the test range
//! (its opening set-up is staged by the sim thread, not given through the session).
//!
//! The file is the same `.mcreplay` a single-player session writes. A tick the
//! range staged is written with the staged commands in front of the session's,
//! which is the order the sim thread applies them in, so playback needs to know
//! nothing about scenes. Staged commands are all the first side's (`setup`), so
//! grouping the bundle by side keeps that order.

use std::fs::File;
use std::io::{self, BufWriter};
use std::path::PathBuf;

use mc_net::{MatchStart, ReplayWriter, TickBundle};
use mc_sim::PlayerCommand;

/// Ticks between flushes: five seconds of play, as a single-player recording.
const FLUSH_EVERY: u32 = 50;
/// The most a note from a replay file may take to decode.
const MAX_NOTE_BYTES: u64 = 1 << 10;

/// What a recording keeps that the sim does not: shown on this machine only.
/// The variants' order is the file format: add new ones at the end, never reorder.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Note {
    /// The test range's weather, from its Sky tab.
    RangeSky(crate::range::RangeSky),
}

impl Note {
    pub fn encode(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }

    /// `None` for a note this build does not know, or damaged bytes.
    pub fn decode(bytes: &[u8]) -> Option<Note> {
        mc_sim::decode_untrusted(bytes, MAX_NOTE_BYTES).ok()
    }
}

enum State {
    /// Waiting for the start message; notes given before it wait with it.
    Waiting {
        path: PathBuf,
        notes: Vec<Vec<u8>>,
    },
    Writing {
        path: PathBuf,
        writer: ReplayWriter<BufWriter<File>>,
    },
    Stopped,
}

pub struct Recorder {
    state: State,
}

impl Recorder {
    /// Records to `path` once the match starts.
    pub fn new(path: PathBuf) -> Recorder {
        Recorder {
            state: State::Waiting {
                path,
                notes: Vec::new(),
            },
        }
    }

    /// The match starts: the file is opened. A second start (the connection came
    /// back) changes nothing.
    pub fn started(&mut self, start: &MatchStart) {
        let State::Waiting { path, notes } = std::mem::replace(&mut self.state, State::Stopped)
        else {
            return;
        };
        let origin = crate::build_info::origin(start.content);
        let opened = ReplayWriter::create(&path, &origin, start).and_then(|mut writer| {
            for note in &notes {
                writer.note(note)?;
            }
            Ok(writer)
        });
        match opened {
            Ok(writer) => {
                log::info!("recording match to {}", path.display());
                self.state = State::Writing { path, writer };
            }
            Err(e) => log::warn!("this match will not be recorded: {e}"),
        }
    }

    /// Tick `bundle.tick` was carried out with `staged` in front of the bundle's commands.
    pub fn tick(&mut self, bundle: &TickBundle, staged: &[PlayerCommand]) {
        let tick = bundle.tick;
        if staged.is_empty() {
            self.write(|w| w.bundle(bundle));
        } else {
            let merged = TickBundle::new(
                tick,
                staged
                    .iter()
                    .map(|c| (mc_core::PlayerId(c.player), vec![c.command.encode()]))
                    .chain(
                        bundle
                            .commands()
                            .map(|(slot, bytes)| (slot, vec![bytes.to_vec()])),
                    ),
            );
            self.write(|w| w.bundle(&merged));
        }
        if (tick + 1).is_multiple_of(FLUSH_EVERY) {
            self.write(|w| w.flush());
        }
    }

    /// Orders carried out while the clock was held.
    pub fn held(&mut self, bundle: &TickBundle) {
        self.write(|w| w.held(bundle));
    }

    pub fn hash(&mut self, tick: u32, hash: u64) {
        self.write(|w| w.hash(tick, hash));
    }

    /// Something shown, not simulated, changed (`ReplayWriter::note`).
    pub fn note(&mut self, note: Vec<u8>) {
        match &mut self.state {
            State::Waiting { notes, .. } => notes.push(note),
            State::Writing { .. } => self.write(|w| w.note(&note)),
            State::Stopped => {}
        }
    }

    /// The world was replaced by a snapshot of the state after `tick` (a late join,
    /// or back after a lost connection). Ticks the recording missed cannot be
    /// played, so it ends where it is, unless it missed none.
    pub fn restored(&mut self, tick: u32) {
        let State::Writing { path, writer } = &mut self.state else {
            self.state = State::Stopped;
            return;
        };
        if writer.ticks() == tick + 1 {
            return;
        }
        if writer.ticks() == 0 {
            // Joined a match already running: there is nothing to play.
            let path = path.clone();
            self.state = State::Stopped;
            let _ = std::fs::remove_file(&path);
            log::info!("joined at tick {tick}: this match is not recorded");
        } else {
            log::info!(
                "the recording ends at tick {}: play went on from a snapshot of tick {tick}",
                writer.ticks()
            );
            self.finish();
        }
    }

    /// The match is over: the end marker goes in.
    pub fn finish(&mut self) {
        if let State::Writing { mut writer, .. } =
            std::mem::replace(&mut self.state, State::Stopped)
        {
            if let Err(e) = writer.finish() {
                log::warn!("the recording could not be finished: {e}");
            }
        }
    }

    fn write(&mut self, f: impl FnOnce(&mut ReplayWriter<BufWriter<File>>) -> io::Result<()>) {
        if let State::Writing { writer, .. } = &mut self.state {
            if let Err(e) = f(writer) {
                log::warn!("recording stopped: {e}");
                self.state = State::Stopped;
            }
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_core::PlayerId;
    use mc_sim::Command;

    fn start() -> MatchStart {
        MatchStart {
            content: Default::default(),
            seed: 1,
            input_delay: 0,
            players: vec![mc_net::PlayerSetup {
                slot: PlayerId(0),
                name: "me".into(),
                data: Vec::new(),
            }],
            options: vec![1],
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mc-recorder-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn staged_commands_go_in_front_and_notes_wait_for_the_start() {
        let path = scratch("staged.mcreplay");
        let mut r = Recorder::new(path.clone());
        r.note(vec![7]);
        r.started(&start());
        let staged = [PlayerCommand {
            player: 0,
            command: Command::DebugFreeBuild {
                player: 0,
                on: true,
            },
        }];
        let own = TickBundle::new(0, [(PlayerId(0), vec![vec![42]])]);
        r.tick(&own, &staged);
        r.hash(0, 9);
        r.tick(&TickBundle::empty(1), &[]);
        r.note(vec![8]);
        drop(r);
        let replay = mc_net::Replay::load(&path).unwrap();
        assert!(replay.complete);
        assert_eq!(replay.origin.build, crate::BUILD);
        assert_eq!(replay.origin.sim, crate::build_info::sim());
        let first: Vec<&[u8]> = replay.bundles[0].commands().map(|(_, c)| c).collect();
        assert_eq!(first, [staged[0].command.encode().as_slice(), &[42]]);
        assert_eq!(replay.notes, [(0, vec![7]), (2, vec![8])]);
        assert_eq!(replay.hashes.get(&0), Some(&9));
    }

    #[test]
    fn a_snapshot_ends_the_recording_unless_nothing_was_missed() {
        let path = scratch("rejoin.mcreplay");
        let mut r = Recorder::new(path.clone());
        r.started(&start());
        for t in 0..5 {
            r.tick(&TickBundle::empty(t), &[]);
        }
        // Back after a drop, from the tick it had reached: it goes on.
        r.restored(4);
        r.tick(&TickBundle::empty(5), &[]);
        // From further on: it ends at what it has.
        r.restored(20);
        r.tick(&TickBundle::empty(21), &[]);
        drop(r);
        let replay = mc_net::Replay::load(&path).unwrap();
        assert_eq!(replay.bundles.len(), 6);
        assert!(replay.complete);

        // Joined a running match: no file is left behind.
        let late = scratch("late.mcreplay");
        let mut r = Recorder::new(late.clone());
        r.started(&start());
        r.restored(300);
        assert!(!late.exists());
    }
}
