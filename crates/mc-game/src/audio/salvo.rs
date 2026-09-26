//! How a salvo of strategic missiles is heard (`docs/NUKES.md`, "Sound"). Players
//! fire 20-60 warheads at one mark at once; heard one by one that is a wall of
//! noise that eats every voice the mixer has. Here a salvo coalesces: one alarm,
//! one launch roar that grows deeper (layered, lower pitched) rather than louder,
//! a handful of detonations with later ones folded in, and the warheads' flight
//! and fall loops collapsed to at most three voices.
//!
//! Pure: it takes the time (seconds on the client's clock, never sim state) and
//! what happened, and says what to play, so it is tested without a device.

use std::collections::VecDeque;

/// The launch alarm is not sounded again while one is still sounding (the
/// `nuke_alarm` length when the library has it, this otherwise).
pub const ALARM_FALLBACK: f32 = 7.5;
/// Launches within this long of a roar group's start fold into it (the rate limit
/// on roar groups too: silos firing their queue every 2.5 s are each heard).
pub const ROAR_GROUP: f32 = 0.8;
/// A detonation within this long of the last one played folds into it.
pub const BLAST_FOLD: f32 = 1.0;
/// At most `BLAST_MAX` full detonation voices start within `BLAST_WINDOW` seconds.
pub const BLAST_WINDOW: f32 = 30.0;
pub const BLAST_MAX: usize = 5;
/// Past the budget, a lower, quieter detonation, no oftener than this.
pub const BLAST_LATE_GAP: f32 = 4.0;
/// Interceptor launches, kills and silo doors: one of each per this long.
pub const SMALL_GAP: f32 = 0.25;

/// A sound to start: gain, pan, pitch, delay in seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Play {
    pub gain: f32,
    pub pan: f32,
    pub pitch: f32,
    pub delay: f32,
}

/// The small nuke one-shots rate-limited per sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Small {
    InterceptorLaunch = 0,
    Intercepted = 1,
    SiloDoors = 2,
}

#[derive(Default)]
pub struct SalvoAudio {
    /// The client clock's zero (set on first use).
    pub epoch: Option<std::time::Instant>,
    last_alarm: Option<f32>,
    /// The open roar group: when it started, how many launches it holds, how many
    /// extra layers it has played.
    roar: Option<(f32, u32, u8)>,
    /// Starts of full detonation voices in the last `BLAST_WINDOW`.
    blasts: VecDeque<f32>,
    /// The last detonation played, full or late.
    last_blast: Option<f32>,
    last_small: [Option<f32>; 3],
}

/// The main roar's level for `n` launches: grows slowly, never past 1.
pub fn roar_gain(n: u32) -> f32 {
    (0.7 * (1.0 + 0.3 * (n.max(1) as f32).ln())).min(1.0)
}

/// Extra roar layers a group of `n` gets: (from this many launches, delay, pitch, share of the main gain).
const LAYERS: [(u32, f32, f32, f32); 2] = [(4, 0.14, 0.9, 0.6), (10, 0.36, 0.82, 0.5)];

impl SalvoAudio {
    /// Seconds on the client clock.
    pub fn now(&mut self) -> f32 {
        let now = std::time::Instant::now();
        (now - *self.epoch.get_or_insert(now)).as_secs_f32()
    }

    /// Whether the launch alarm sounds at `t` for launches this tick (it lasts `length`).
    pub fn alarm(&mut self, t: f32, launches: u32, length: f32) -> bool {
        if launches == 0 || self.last_alarm.is_some_and(|at| t - at < length) {
            return false;
        }
        self.last_alarm = Some(t);
        true
    }

    /// The roar voices to start at `t` for `launches` this tick (all heard the same,
    /// from the middle).
    pub fn roar(&mut self, t: f32, launches: u32) -> Vec<Play> {
        let mut out = Vec::new();
        if launches == 0 {
            return out;
        }
        let (start, count, layers) = match self.roar {
            Some((start, count, layers)) if t - start < ROAR_GROUP => (start, count + launches, layers),
            _ => {
                let gain = roar_gain(launches);
                out.push(Play { gain, pan: 0.0, pitch: 1.0, delay: 0.0 });
                (t, launches, 0)
            }
        };
        let gain = roar_gain(count);
        let mut played = layers;
        for (i, &(from, delay, pitch, share)) in LAYERS.iter().enumerate() {
            if (i as u8) < played || count < from {
                continue;
            }
            // A layer joining a group already under way comes in at once.
            let delay = if t > start { 0.0 } else { delay };
            out.push(Play { gain: gain * share, pan: 0.0, pitch, delay });
            played = i as u8 + 1;
        }
        self.roar = Some((start, count, played));
        out
    }

    /// The detonation to start at `t` for this tick's bursts, given as (gain, pan)
    /// where each is heard: at most one voice.
    pub fn detonation(&mut self, t: f32, bursts: &[(f32, f32)]) -> Option<Play> {
        let &(gain, pan) = bursts.iter().max_by(|a, b| a.0.total_cmp(&b.0))?;
        while self.blasts.front().is_some_and(|&at| t - at >= BLAST_WINDOW) {
            self.blasts.pop_front();
        }
        let since = self.last_blast.map_or(f32::INFINITY, |at| t - at);
        if since < BLAST_FOLD {
            return None;
        }
        let gain = gain.max(0.85);
        if self.blasts.len() < BLAST_MAX {
            self.blasts.push_back(t);
            self.last_blast = Some(t);
            return Some(Play { gain, pan, pitch: 1.0, delay: 0.0 });
        }
        if since < BLAST_LATE_GAP {
            return None;
        }
        self.last_blast = Some(t);
        Some(Play { gain: gain * 0.6, pan, pitch: 0.85, delay: 0.0 })
    }

    /// One `kind` sound at `t` for this tick's (gain, pan): the loudest, unless one
    /// was started less than `SMALL_GAP` ago.
    pub fn small(&mut self, t: f32, kind: Small, heard: &[(f32, f32)]) -> Option<Play> {
        let &(gain, pan) = heard.iter().max_by(|a, b| a.0.total_cmp(&b.0))?;
        let last = &mut self.last_small[kind as usize];
        if last.is_some_and(|at| t - at < SMALL_GAP) {
            return None;
        }
        *last = Some(t);
        Some(Play { gain, pan, pitch: 1.0, delay: 0.0 })
    }
}

/// Several loops of one sound as one: gain is the root of the summed squares
/// (capped at `cap`), pan the gain-weighted mean.
pub fn merge(parts: impl IntoIterator<Item = (f32, f32)>, cap: f32) -> Option<(f32, f32)> {
    let (mut power, mut panned, mut weight) = (0.0f32, 0.0f32, 0.0f32);
    for (gain, pan) in parts {
        power += gain * gain;
        panned += gain * pan;
        weight += gain;
    }
    (power > 0.0).then(|| (power.sqrt().min(cap), panned / weight.max(1e-9)))
}

/// Warheads falling, as (gain, pan, seconds to impact): the nearest to landing on
/// its own, the rest merged into one more. At most two.
pub fn falls(mut parts: Vec<(f32, f32, f32)>) -> Vec<(f32, f32)> {
    let Some(first) = (0..parts.len()).min_by(|&a, &b| parts[a].2.total_cmp(&parts[b].2)) else {
        return Vec::new();
    };
    let (gain, pan, _) = parts.swap_remove(first);
    let mut out = vec![(gain, pan)];
    out.extend(merge(parts.iter().map(|p| (p.0, p.1)), 1.0));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_alarm_per_salvo() {
        let mut s = SalvoAudio::default();
        assert!(s.alarm(0.0, 40, 7.5));
        assert!(!s.alarm(0.1, 20, 7.5));
        assert!(!s.alarm(5.0, 1, 7.5));
        assert!(!s.alarm(8.0, 0, 7.5));
        assert!(s.alarm(8.0, 1, 7.5));
    }

    #[test]
    fn a_salvo_is_one_deeper_roar() {
        let mut s = SalvoAudio::default();
        let plays = s.roar(0.0, 60);
        assert_eq!(plays.len(), 3);
        assert!(plays.iter().all(|p| p.gain <= 1.0));
        assert_eq!(plays[0].pitch, 1.0);
        assert!(plays[1].pitch < 1.0 && plays[2].pitch < plays[1].pitch);
        assert!(plays[1].delay > 0.1 && plays[2].delay <= 0.4);
        // More of the same salvo a moment later: nothing new.
        assert!(s.roar(0.3, 20).is_empty());
        // A single launch is a single roar at the old level.
        let mut s = SalvoAudio::default();
        let plays = s.roar(0.0, 1);
        assert_eq!(plays, vec![Play { gain: 0.7, pan: 0.0, pitch: 1.0, delay: 0.0 }]);
        // Launches trickling in within the group add layers as the count grows, once.
        assert_eq!(s.roar(0.1, 3).len(), 1);
        assert!(s.roar(0.2, 3).is_empty());
        // The queue firing every 2.5 s is heard each time.
        assert_eq!(s.roar(2.5, 1).len(), 1);
        assert_eq!(s.roar(5.0, 1).len(), 1);
    }

    #[test]
    fn detonations_are_budgeted() {
        let mut s = SalvoAudio::default();
        let burst = [(0.2, 0.5), (0.4, -0.1)];
        let first = s.detonation(0.0, &burst).unwrap();
        assert_eq!((first.gain, first.pan, first.pitch), (0.85, -0.1, 1.0));
        assert!(s.detonation(0.5, &burst).is_none(), "folds into the last");
        let mut full = 1;
        let mut late = 0;
        let mut t = 0.0;
        while t < 29.0 {
            t += 1.1;
            match s.detonation(t, &burst) {
                Some(p) if p.pitch == 1.0 => full += 1,
                Some(_) => late += 1,
                None => {}
            }
        }
        assert_eq!(full, BLAST_MAX);
        assert!((4..=6).contains(&late), "{late}");
        // A fresh window, a full blast again.
        assert_eq!(s.detonation(70.0, &burst).unwrap().pitch, 1.0);
        assert!(s.detonation(70.0, &[]).is_none());
    }

    #[test]
    fn small_sounds_rate_limited() {
        let mut s = SalvoAudio::default();
        let doors: Vec<(f32, f32)> = (0..60).map(|i| (i as f32 / 100.0, 0.0)).collect();
        assert_eq!(s.small(0.0, Small::SiloDoors, &doors).unwrap().gain, 0.59);
        assert!(s.small(0.1, Small::SiloDoors, &doors).is_none());
        assert!(s.small(0.1, Small::Intercepted, &doors).is_some());
        assert!(s.small(0.3, Small::SiloDoors, &doors).is_some());
    }

    #[test]
    fn loops_collapse() {
        let (gain, pan) = merge([(0.5, -1.0), (0.5, 1.0)], 1.0).unwrap();
        assert!((gain - 0.5f32.sqrt()).abs() < 1e-6 && pan.abs() < 1e-6);
        assert_eq!(merge((0..60).map(|_| (0.5, 0.2)), 1.0).unwrap().0, 1.0);
        assert!(merge([], 1.0).is_none());
        let f = falls(vec![(0.3, 0.0, 9.0), (0.9, 0.5, 2.0), (0.3, 1.0, 5.0)]);
        assert_eq!(f[0], (0.9, 0.5));
        assert_eq!(f.len(), 2);
        assert_eq!(falls(vec![(0.3, 0.0, 9.0)]).len(), 1);
        assert!(falls(Vec::new()).is_empty());
    }
}
