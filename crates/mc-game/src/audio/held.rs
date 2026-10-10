//! Held sounds: a long sound that belongs to something going on, such as a gravitic
//! engine's 15 s gathering (`gravitic.rs`). It plays once, but it is placed and levelled
//! every tick as a loop is. It can join part of the way in, so a gathering re-aimed or
//! first seen half done is heard from where it is. It fades out the moment it is no
//! longer wanted (the gathering died or stalled). A played-once `Audio::play_world`
//! voice can do none of these.

use super::{Audio, Frames, Mixer};
use mc_data::SoundId;
use std::sync::atomic::Ordering;

/// One held sound wanted this tick.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Held {
    /// Whose it is: one voice per key and sound (an engine's unit id).
    pub key: u64,
    pub sound: SoundId,
    /// Gain with the distance in it, and pan from -1 to 1, as a loop's.
    pub gain: f32,
    pub pan: f32,
    /// Seconds into the sound that the thing it belongs to has reached.
    pub from: f32,
}

pub(super) struct HeldVoice {
    key: u64,
    id: SoundId,
    frames: Frames,
    /// Read position in frames.
    at: usize,
    gain: [f32; 2],
    target: [f32; 2],
    /// Let go (unwanted, or replaced by a voice back in step): it only fades out.
    gone: bool,
}

/// Seconds a held voice may run ahead of what it belongs to (the game slowed, paused or
/// stalled) before a fresh voice takes over from the right point.
const DRIFT: f32 = 2.0;
/// Joining under this far in starts at full level: the sound's own attack is there.
const JOIN_FADE: f32 = 0.1;

impl Audio {
    /// The held sounds that should be sounding now. A key and sound already playing
    /// carries on from where it is, so asking again each tick, or after a re-aim, never
    /// starts it over. Anything not asked for fades out. Call it every tick.
    pub(crate) fn set_held(&self, wanted: &[Held]) {
        let Some(bank) = self.shared.bank() else {
            return;
        };
        let rate = self.shared.rate.load(Ordering::Relaxed) as f32;
        let mut m = self.shared.mixer();
        set_held_on(&mut m, wanted, rate, |id| {
            bank.world
                .get(id.0 as usize)
                .filter(|f| f.len() > 1)
                .cloned()
        });
    }
}

fn set_held_on(
    m: &mut Mixer,
    wanted: &[Held],
    rate: f32,
    frames_of: impl Fn(SoundId) -> Option<Frames>,
) {
    let mut kept = vec![false; m.held.len()];
    for w in wanted {
        let Some(frames) = frames_of(w.sound) else {
            continue;
        };
        let (l, r) = mc_sfx::pan(w.pan);
        let target = [
            w.gain * l * std::f32::consts::SQRT_2,
            w.gain * r * std::f32::consts::SQRT_2,
        ];
        let from = (w.from.max(0.0) * rate) as usize;
        let playing = (0..kept.len())
            .find(|&i| !m.held[i].gone && m.held[i].key == w.key && m.held[i].id == w.sound);
        if let Some(i) = playing {
            let ahead = (m.held[i].at as f32 - from as f32) / rate;
            if ahead <= DRIFT {
                kept[i] = true;
                m.held[i].target = target;
                continue;
            }
            // Ran ahead of the thing it belongs to: this one fades as the new one comes in.
            m.held[i].gone = true;
        }
        if from + 1 >= frames.len() {
            continue;
        }
        let start = if w.from < JOIN_FADE { target } else { [0.0; 2] };
        m.held.push(HeldVoice {
            key: w.key,
            id: w.sound,
            frames,
            at: from,
            gain: start,
            target,
            gone: false,
        });
        kept.push(true);
    }
    for (keep, v) in kept.iter().zip(m.held.iter_mut()) {
        if !keep {
            v.gone = true;
        }
    }
}

/// Lets every held sound go: the match is over, or the bank was replaced.
pub(super) fn release(voices: &mut [HeldVoice]) {
    for v in voices {
        v.gone = true;
    }
}

/// Mixes the held voices into `out` on the effects bus `bus`, and drops the finished.
pub(super) fn mix(voices: &mut Vec<HeldVoice>, out: &mut [f32], channels: usize, bus: f32) {
    let frames = out.len() / channels;
    for v in voices.iter_mut() {
        let target = if v.gone { [0.0; 2] } else { v.target };
        for i in 0..frames {
            let Some(s) = v.frames.get(v.at) else {
                break;
            };
            // The loops' glide: a few hundredths of a second, so nothing clicks.
            for (g, t) in v.gain.iter_mut().zip(target) {
                *g += (t - *g) * 0.0006;
            }
            let (l, r) = (s[0] * v.gain[0] * bus, s[1] * v.gain[1] * bus);
            let o = &mut out[i * channels..(i + 1) * channels];
            if channels == 1 {
                o[0] += (l + r) * 0.5;
            } else {
                o[0] += l;
                o[1] += r;
            }
            v.at += 1;
        }
    }
    voices.retain(|v| v.at < v.frames.len() && !(v.gone && v.gain[0].max(v.gain[1]) < 1e-4));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{Shared, Volumes};
    use std::sync::Arc;

    fn mixer() -> Shared {
        Shared::new(
            Volumes {
                master: 1.0,
                interface: 1.0,
                effects: 1.0,
                weather: 1.0,
            },
            mc_data::SoundLibrary::default(),
        )
    }

    /// A one-second ramp, so a voice's position can be read off what it plays.
    fn ramp() -> Frames {
        Arc::new((0..1000).map(|i| [i as f32 / 1000.0; 2]).collect())
    }

    fn want(key: u64, from: f32) -> Held {
        Held {
            key,
            sound: SoundId(0),
            gain: 1.0,
            pan: 0.0,
            from,
        }
    }

    #[test]
    fn asked_again_it_carries_on_and_joins_part_way_in() {
        let shared = mixer();
        let frames = |_| Some(ramp());
        let mut m = shared.mixer();
        set_held_on(&mut m, &[want(7, 0.0)], 1000.0, frames);
        assert_eq!(m.held.len(), 1);
        assert_eq!(
            m.held[0].gain, m.held[0].target,
            "from the start: no fade in"
        );
        let mut out = vec![0.0f32; 200];
        mix(&mut m.held, &mut out, 2, 1.0);
        assert_eq!(m.held[0].at, 100);
        // Re-aimed: the sim says the same place; the voice is not started over.
        set_held_on(&mut m, &[want(7, 0.1)], 1000.0, frames);
        assert_eq!((m.held.len(), m.held[0].at), (1, 100));
        // Another engine first seen half way through its gathering joins there, fading in.
        set_held_on(&mut m, &[want(7, 0.1), want(8, 0.5)], 1000.0, frames);
        assert_eq!(m.held.len(), 2);
        assert_eq!(m.held[1].at, 500);
        assert_eq!(m.held[1].gain, [0.0; 2]);
    }

    #[test]
    fn unwanted_it_fades_and_is_dropped_and_ahead_it_is_put_back() {
        let shared = mixer();
        let long = |_| Some(Arc::new(vec![[0.5f32; 2]; 48_000 * 10]));
        let mut m = shared.mixer();
        set_held_on(&mut m, &[want(1, 0.0)], 48_000.0, long);
        let mut out = vec![0.0f32; 2 * 48_000 * 3];
        mix(&mut m.held, &mut out, 2, 1.0);
        // Three seconds played while the thing it belongs to stood still at zero.
        set_held_on(&mut m, &[want(1, 0.5)], 48_000.0, long);
        assert_eq!(
            m.held.len(),
            2,
            "the stray fades while a voice in step comes in"
        );
        assert!(m.held[0].gone && !m.held[1].gone);
        assert_eq!(m.held[1].at, 24_000);
        set_held_on(&mut m, &[], 48_000.0, long);
        let mut out = vec![0.0f32; 2 * 48_000];
        mix(&mut m.held, &mut out, 2, 1.0);
        assert!(m.held.is_empty(), "let go, both fade out and leave");
        assert!(out[out.len() - 2].abs() < 1e-3);
    }

    #[test]
    fn a_voice_ends_with_its_sound() {
        let shared = mixer();
        let mut m = shared.mixer();
        set_held_on(&mut m, &[want(1, 0.0)], 1000.0, |_| Some(ramp()));
        let mut out = vec![0.0f32; 2 * 1500];
        mix(&mut m.held, &mut out, 2, 1.0);
        assert!(m.held.is_empty());
        assert!(out[2 * 1200..].iter().all(|s| *s == 0.0));
    }
}
