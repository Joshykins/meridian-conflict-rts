//! MIDI keyboards: every connected input, all channels, plays the selected
//! track through the engine with the key's velocity. The callback runs on
//! midir's thread and goes straight to the audio channel, so playing never
//! waits on a UI frame.

use mc_music::Command;
use std::sync::atomic::{AtomicU32, AtomicUsize};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub struct Midi {
    /// The track MIDI plays; the UI updates it as the selection moves.
    pub track: Arc<AtomicUsize>,
    /// Last note seen, `key | vel << 8 | count << 16`, for the activity light.
    pub last: Arc<AtomicU32>,
    pub ports: Vec<String>,
    pub error: Option<String>,
    /// Notes copied here while a sketch is recorded: (time, key, velocity; 0 = off).
    tap: Arc<Mutex<Vec<(Instant, u8, u8)>>>,
    tap_on: Arc<std::sync::atomic::AtomicBool>,
    #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
    connections: Vec<midir::MidiInputConnection<()>>,
}

impl Midi {
    pub fn disabled() -> Midi {
        Midi {
            track: Arc::new(AtomicUsize::new(0)),
            last: Arc::new(AtomicU32::new(0)),
            ports: Vec::new(),
            error: Some("MIDI is not in this build".into()),
            tap: Arc::new(Mutex::new(Vec::new())),
            tap_on: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
            connections: Vec::new(),
        }
    }

    pub fn tapping(&self, on: bool) {
        self.tap_on.store(on, std::sync::atomic::Ordering::Relaxed);
        if !on {
            self.take_tap();
        }
    }

    /// The notes tapped since the last call.
    pub fn take_tap(&self) -> Vec<(Instant, u8, u8)> {
        self.tap
            .lock()
            .map(|mut t| std::mem::take(&mut *t))
            .unwrap_or_default()
    }

    /// Connects to every input port there is now.
    pub fn connect(tx: Sender<Command>) -> Midi {
        let mut m = Midi::disabled();
        m.error = None;
        m.rescan(tx);
        m
    }

    #[cfg(not(any(not(target_os = "linux"), feature = "alsa")))]
    pub fn rescan(&mut self, _tx: Sender<Command>) {
        self.error = Some("MIDI is not in this build (Linux without --features alsa)".into());
    }

    #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
    pub fn rescan(&mut self, tx: Sender<Command>) {
        self.connections.clear();
        self.ports.clear();
        let probe = match midir::MidiInput::new("mc-studio") {
            Ok(m) => m,
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        };
        let ports = probe.ports();
        for port in ports.iter() {
            let name = probe
                .port_name(port)
                .unwrap_or_else(|_| "MIDI input".into());
            let Ok(input) = midir::MidiInput::new("mc-studio") else {
                continue;
            };
            let tx = tx.clone();
            let track = self.track.clone();
            let last = self.last.clone();
            let (tap, tap_on) = (self.tap.clone(), self.tap_on.clone());
            let conn = input.connect(
                port,
                "mc-studio",
                move |_, msg, _| {
                    if let Some(c) = decode(msg, track.load(std::sync::atomic::Ordering::Relaxed)) {
                        if tap_on.load(std::sync::atomic::Ordering::Relaxed) {
                            let e = match c {
                                Command::NoteOn { key, vel, .. } => Some((key, vel)),
                                Command::NoteOff { key, .. } => Some((key, 0)),
                                _ => None,
                            };
                            if let (Some((k, v)), Ok(mut t)) = (e, tap.lock()) {
                                t.push((Instant::now(), k, v));
                            }
                        }
                        if let Command::NoteOn { key, vel, .. } = c {
                            let n = (last.load(std::sync::atomic::Ordering::Relaxed) >> 16)
                                .wrapping_add(1)
                                & 0xFFFF;
                            last.store(
                                key as u32 | (vel as u32) << 8 | n << 16,
                                std::sync::atomic::Ordering::Relaxed,
                            );
                        }
                        let _ = tx.send(c);
                    }
                },
                (),
            );
            match conn {
                Ok(c) => {
                    self.connections.push(c);
                    self.ports.push(name);
                }
                Err(e) => self.error = Some(format!("{name}: {e}")),
            }
        }
    }
}

/// A note message as an engine command for `track`; other messages are ignored.
#[cfg_attr(not(any(not(target_os = "linux"), feature = "alsa")), allow(dead_code))]
pub fn decode(msg: &[u8], track: usize) -> Option<Command> {
    let status = *msg.first()? & 0xF0;
    let key = *msg.get(1)? & 0x7F;
    let vel = *msg.get(2)? & 0x7F;
    match status {
        0x90 if vel > 0 => Some(Command::NoteOn { track, key, vel }),
        0x90 | 0x80 => Some(Command::NoteOff { track, key }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_notes_on_any_channel() {
        assert!(matches!(
            decode(&[0x93, 60, 100], 2),
            Some(Command::NoteOn {
                track: 2,
                key: 60,
                vel: 100
            })
        ));
        assert!(matches!(
            decode(&[0x90, 60, 0], 0),
            Some(Command::NoteOff { key: 60, .. })
        ));
        assert!(matches!(
            decode(&[0x8F, 61, 40], 0),
            Some(Command::NoteOff { key: 61, .. })
        ));
        assert!(decode(&[0xB0, 1, 64], 0).is_none());
        assert!(decode(&[0x90], 0).is_none());
    }
}
