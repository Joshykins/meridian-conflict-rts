//! A song and the moments played over it.
//!
//! The music does not follow the battle. A song plays straight through (and
//! loops); when something happens in the game (a nuke lands, a commander
//! falls) a *moment* plays: its own short piece of music, from
//! `data/music/moments/<name>.ron`. While it sounds the song dips down, and it
//! comes back up once the moment has rung out. A moment marked as an ending
//! (victory, defeat) stops the song instead of handing back to it.
//!
//! The studio and the game both play through a `Stage`, so what the studio
//! plays when you try a moment is what the game will do.

use crate::engine::{Command, Engine, Mode};
use crate::song::Song;
use std::sync::Arc;

/// How the song makes room for a moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Duck {
    /// How far the song dips, dB (negative).
    pub depth: f32,
    /// Seconds to dip.
    pub down: f32,
    /// Seconds to come back once the moment has ended.
    pub up: f32,
}

impl Default for Duck {
    fn default() -> Duck {
        Duck {
            depth: -16.0,
            down: 0.35,
            up: 2.0,
        }
    }
}

struct Playing {
    name: String,
    engine: Engine,
    /// Seconds of near silence after it finished; it is dropped after its tail.
    quiet: f32,
}

pub struct Stage {
    rate: f32,
    song: Engine,
    moment: Option<Playing>,
    /// The song's gain now, linear.
    gain: f32,
    pub duck: Duck,
    /// An ending moment has taken over: the song does not come back.
    ended: bool,
    scratch: Vec<f32>,
}

impl Stage {
    /// A stage playing `song` straight through, looping at its end.
    pub fn new(rate: f32, song: Arc<Song>) -> Stage {
        let mut e = Engine::new(rate, song.clone());
        e.command(Command::SetMode(Mode::Song));
        loop_whole(&mut e, &song);
        Stage {
            rate,
            song: e,
            moment: None,
            gain: 1.0,
            duck: Duck::default(),
            ended: false,
            scratch: Vec::new(),
        }
    }

    pub fn song(&self) -> &Engine {
        &self.song
    }
    pub fn song_mut(&mut self) -> &mut Engine {
        &mut self.song
    }

    /// Replaces the song (an edit, or a hot reload), keeping its place.
    pub fn set_song(&mut self, song: Arc<Song>) {
        self.song.set_song(song.clone());
        loop_whole(&mut self.song, &song);
    }

    pub fn play(&mut self) {
        self.ended = false;
        self.song.play();
    }

    pub fn stop(&mut self) {
        self.song.stop();
        if let Some(m) = &mut self.moment {
            m.engine.stop();
        }
        self.moment = None;
        self.gain = 1.0;
    }

    /// Plays a moment over the song. A moment already playing is cut short (it fades
    /// out over its own release). `ending` stops the song for good once it has dipped.
    pub fn moment(&mut self, name: &str, piece: Arc<Song>, ending: bool) {
        let mut e = Engine::new(self.rate, piece.clone());
        e.command(Command::SetMode(Mode::Song));
        e.play();
        self.moment = Some(Playing {
            name: name.to_string(),
            engine: e,
            quiet: 0.0,
        });
        if ending {
            self.ended = true;
        }
    }

    /// The moment sounding now, if any.
    pub fn moment_playing(&self) -> Option<&str> {
        self.moment.as_ref().map(|m| m.name.as_str())
    }

    /// The song's level now, 0..1: 1 when no moment plays.
    pub fn song_level(&self) -> f32 {
        self.gain
    }

    /// Fills interleaved stereo `out`.
    pub fn render(&mut self, out: &mut [f32]) {
        let frames = out.len() / 2;
        self.song.render(out);
        let floor = crate::dsp::db_to_gain(self.duck.depth);
        // Where the song's level is heading: down while a moment plays, silent after an
        // ending has dipped it, else back up.
        let target = if self.ended {
            0.0
        } else if self.moment.is_some() {
            floor
        } else {
            1.0
        };
        let secs = if target < self.gain {
            self.duck.down
        } else {
            self.duck.up
        };
        let k = 1.0 - (-1.0 / (secs.max(0.01) * self.rate)).exp();
        for f in 0..frames {
            self.gain += (target - self.gain) * k;
            out[f * 2] *= self.gain;
            out[f * 2 + 1] *= self.gain;
        }
        if self.ended && self.gain < 1e-3 && self.song.is_playing() {
            self.song.stop();
        }
        if let Some(m) = &mut self.moment {
            self.scratch.resize(out.len(), 0.0);
            m.engine.render(&mut self.scratch);
            let mut loud = 0.0f32;
            for (o, s) in out.iter_mut().zip(&self.scratch) {
                *o += s;
                loud = loud.max(s.abs());
            }
            // Done once it has finished and its tail is below -60 dB for half a second.
            if m.engine.status().finished && loud < 1e-3 {
                m.quiet += frames as f32 / self.rate;
            } else {
                m.quiet = 0.0;
            }
            if m.quiet > 0.5 {
                self.moment = None;
            }
        }
    }
}

/// Loops the whole arrangement, so the song plays on until it is stopped.
fn loop_whole(e: &mut Engine, song: &Song) {
    let end = song.arrangement_ticks();
    e.command(Command::SetLoop(if end > 0 {
        Some((0, end))
    } else {
        None
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::{Instrument, Synth};
    use crate::song::{Clip, Note, Pattern, Section, Track};

    fn tone(key: u8, bars: u32) -> Song {
        let mut s = Song::empty("t");
        s.tracks.push(Track {
            name: "a".into(),
            instrument: Instrument::Synth(Synth::default()),
            db: 0.0,
            pan: 0.0,
            mute: false,
            solo: false,
            sends: vec![],
            effects: vec![],
            layer: Default::default(),
            follow: vec![],
            colour: 0,
        });
        s.patterns.push(Pattern {
            name: "p".into(),
            beats: 4,
            notes: vec![Note(0, 380, key, 100)],
            automation: vec![],
        });
        s.sections.push(Section {
            name: "s".into(),
            bars,
            kind: Default::default(),
            intensity: (0.0, 1.0),
            next: vec![],
            exit_every: 0,
            clips: vec![Clip {
                track: "a".into(),
                pattern: "p".into(),
                at: 0,
                times: 0,
                transpose: 0,
            }],
        });
        s.arrangement = vec!["s".into()];
        s
    }

    /// Renders `secs` in audio-callback-sized blocks; the RMS of the last quarter.
    fn level(st: &mut Stage, secs: f32) -> f32 {
        let mut buf = vec![0.0f32; (secs * 48000.0) as usize * 2];
        for block in buf.chunks_mut(1024) {
            st.render(block);
        }
        let tail = &buf[buf.len() * 3 / 4..];
        (tail.iter().map(|x| x * x).sum::<f32>() / tail.len() as f32).sqrt()
    }

    #[test]
    fn a_moment_ducks_the_song_and_the_song_comes_back() {
        let mut st = Stage::new(48000.0, Arc::new(tone(57, 2)));
        st.play();
        let before = level(&mut st, 2.0);
        assert!(before > 0.01);
        st.moment("hit", Arc::new(tone(69, 1)), false);
        level(&mut st, 1.5);
        assert!(
            st.song_level() < 0.2,
            "the song dipped: {}",
            st.song_level()
        );
        assert_eq!(st.moment_playing(), Some("hit"));
        // One bar at 110 bpm is about 2.2 s; then its tail, then the song comes back.
        level(&mut st, 8.0);
        assert_eq!(st.moment_playing(), None);
        assert!(
            st.song_level() > 0.95,
            "the song came back: {}",
            st.song_level()
        );
        assert!(
            st.song().is_playing(),
            "the song kept its place and plays on"
        );
    }

    #[test]
    fn an_ending_moment_stops_the_song() {
        let mut st = Stage::new(48000.0, Arc::new(tone(57, 2)));
        st.play();
        level(&mut st, 1.0);
        st.moment("victory", Arc::new(tone(60, 1)), true);
        level(&mut st, 6.0);
        assert!(!st.song().is_playing());
    }
}
