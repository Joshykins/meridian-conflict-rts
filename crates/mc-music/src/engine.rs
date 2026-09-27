//! The engine: plays a `Song` in real time. It sequences the sections (straight
//! through, one looped, one pattern, or picked by the director), plays the
//! instruments, and mixes tracks through their effects and sends into buses
//! and the master.
//!
//! Everything happens in `render`, which the audio callback calls. Settings
//! arrive as a whole new `Song` (`set_song`): the engine matches tracks, buses
//! and effects by name and kind and keeps their state, so a knob turned while
//! the song plays changes the sound without a click or a lost reverb tail.

use crate::dsp::effects::{Ctx, Unit};
use crate::dsp::{db_to_gain, gain_to_db, pan_gains};
use crate::patch::{Effect, Instrument};
use crate::song::{SectionKind, Song, Target, PPQ};
use crate::voice::{Mods, Player};
use std::sync::Arc;

/// Samples processed between control updates (automation, intensity, layers).
const CHUNK: usize = 64;
/// Frames the scope keeps for the studio.
pub const SCOPE_LEN: usize = 4096;

#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    /// The arrangement, start to end (or its loop range).
    Song,
    /// One section, looped.
    Section(usize),
    /// One pattern looped on one track: the piano roll's audition.
    Pattern { pattern: usize, track: usize },
    /// The director chooses sections by intensity, as in the game.
    Director,
}

/// Something to tell the engine from another thread.
#[derive(Clone, Debug)]
pub enum Command {
    SetSong(Arc<Song>),
    Play,
    Stop,
    /// Stop and silence every voice at once.
    Panic,
    /// Arrangement tick in `Song` mode, section tick otherwise.
    Seek(u32),
    SetMode(Mode),
    /// A loop range over the arrangement, in ticks.
    SetLoop(Option<(u32, u32)>),
    SetIntensity(f32),
    /// Snap to the intensity without the glide.
    ForceIntensity(f32),
    Cue(String),
    Finish(Option<String>),
    NoteOn {
        track: usize,
        key: u8,
        vel: u8,
    },
    NoteOff {
        track: usize,
        key: u8,
    },
    AllNotesOff,
    Metronome(bool),
    Gain(f32),
    /// In Song mode, set the intensity to the middle of each section's range as it starts,
    /// so playing straight through sounds the way the game would reach each section (default on).
    AutoIntensity(bool),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Meter {
    /// Peak since the last read, per side, linear.
    pub peak: [f32; 2],
    /// Gain reduction of this strip's dynamics, dB (<= 0).
    pub reduction: f32,
}

/// What the engine is doing, for the studio's transport and the game's logic.
#[derive(Clone, Debug, Default)]
pub struct Status {
    pub playing: bool,
    /// The section sounding (main cursor).
    pub section: Option<usize>,
    /// Ticks into that section.
    pub section_tick: u32,
    /// Ticks into the arrangement (Song mode).
    pub song_tick: u32,
    /// Ticks into the pattern (Pattern mode).
    pub pattern_tick: u32,
    pub intensity: f32,
    pub intensity_target: f32,
    /// The song has finished (an ending played out, or the arrangement ended).
    pub finished: bool,
    pub voices: usize,
    /// Output samples rendered.
    pub clock: u64,
    /// Fraction of real time the last block took to render.
    pub load: f32,
}

#[derive(Clone, Debug)]
pub struct Meters {
    pub tracks: Vec<Meter>,
    pub buses: Vec<Meter>,
    pub master: Meter,
    /// Short-term loudness of the master, approximately LUFS (K-weighted, 400 ms).
    pub loudness: f32,
    /// The master's last `SCOPE_LEN` frames, oldest first after `scope_at`.
    pub scope: Vec<[f32; 2]>,
    pub scope_at: usize,
    /// Per-track layer gain the intensity sets now.
    pub layers: Vec<f32>,
}

impl Meters {
    fn new() -> Meters {
        Meters {
            tracks: Vec::new(),
            buses: Vec::new(),
            master: Meter::default(),
            loudness: -70.0,
            scope: vec![[0.0; 2]; SCOPE_LEN],
            scope_at: 0,
            layers: Vec::new(),
        }
    }
    /// Reset peaks after reading (the studio calls this each frame it draws).
    pub fn decay(&mut self) {
        for m in self.tracks.iter_mut().chain(self.buses.iter_mut()) {
            m.peak = [0.0; 2];
            m.reduction = 0.0;
        }
        self.master.peak = [0.0; 2];
        self.master.reduction = 0.0;
    }
    /// The scope in time order.
    pub fn scope_ordered(&self) -> Vec<[f32; 2]> {
        let mut v = Vec::with_capacity(SCOPE_LEN);
        v.extend_from_slice(&self.scope[self.scope_at..]);
        v.extend_from_slice(&self.scope[..self.scope_at]);
        v
    }
}

struct Chain {
    units: Vec<Unit>,
}

impl Chain {
    fn build(effects: &[Effect], old: Option<Chain>, rate: f32) -> Chain {
        let mut old_units: Vec<Option<Unit>> = old
            .map(|c| c.units.into_iter().map(Some).collect())
            .unwrap_or_default();
        let units = effects
            .iter()
            .enumerate()
            .map(|(i, fx)| {
                // Keep the unit in the same slot if it is the same kind; else look for one of that kind.
                let slot = if old_units
                    .get(i)
                    .and_then(|u| u.as_ref())
                    .is_some_and(|u| u.fits(fx))
                {
                    Some(i)
                } else {
                    old_units
                        .iter()
                        .position(|u| u.as_ref().is_some_and(|u| u.fits(fx)))
                };
                match slot.and_then(|s| old_units[s].take()) {
                    Some(mut u) => {
                        u.update(fx, rate);
                        u
                    }
                    None => Unit::new(fx, rate),
                }
            })
            .collect();
        Chain { units }
    }

    fn run(&mut self, effects: &[Effect], buf: &mut [[f32; 2]], ctx: &Ctx) -> f32 {
        let mut red = 0.0f32;
        for (u, fx) in self.units.iter_mut().zip(effects) {
            red = red.min(u.run(fx, buf, ctx));
        }
        red
    }
}

struct TrackState {
    name: String,
    player: Player,
    chain: Chain,
    /// The instrument's dry output this block (sidechain key), then the strip's output.
    dry: Vec<[f32; 2]>,
    /// Smoothed layer gain.
    layer: f32,
    /// Current automation values per target (normalised), or neutral.
    auto: [f32; 4],
    /// Smoothed final gains.
    gain: [f32; 2],
    /// Index of the track named by a sidechained compressor, per effect slot.
    sidechain: Vec<Option<usize>>,
    /// The instrument as played: library names looked up, recordings attached.
    instrument: Instrument,
    /// Kind of instrument the player was built for.
    kind: std::mem::Discriminant<Instrument>,
    /// Chunks in a row with no voices and silent output: past a short tail the strip is skipped.
    quiet: u32,
}

struct BusState {
    name: String,
    chain: Chain,
    buf: Vec<[f32; 2]>,
    /// Chunks in a row with silent input; the bus is skipped once its tail has died.
    quiet: u32,
}

/// A place in a section: the main cursor, or a stinger playing over it.
#[derive(Clone, Debug)]
struct Cursor {
    section: usize,
    /// Ticks into the section, fractional.
    tick: f64,
    /// Samples to wait before starting (quantised cues).
    wait: f64,
    stinger: bool,
}

/// A sequenced note sounding, to release at its end.
#[derive(Clone, Debug)]
struct Held {
    track: usize,
    key: u8,
    /// Ticks left to sound.
    left: f64,
}

/// A note event inside one chunk.
#[derive(Clone, Copy, Debug)]
struct Event {
    at: usize,
    track: usize,
    key: u8,
    /// 0 = off.
    vel: u8,
}

pub struct Engine {
    rate: f32,
    song: Arc<Song>,
    tracks: Vec<TrackState>,
    buses: Vec<BusState>,
    master_chain: Chain,
    playing: bool,
    mode: Mode,
    /// Arrangement position in Song mode.
    song_tick: f64,
    loop_range: Option<(u32, u32)>,
    main: Option<Cursor>,
    stingers: Vec<Cursor>,
    pattern_tick: f64,
    held: Vec<Held>,
    /// Keys held by hand (the studio's keyboard), per track.
    live: Vec<(usize, u8)>,
    intensity: f32,
    intensity_target: f32,
    /// Times the main cursor has repeated its section, for variety.
    repeats: u32,
    finishing: Option<Option<String>>,
    finished: bool,
    rng: crate::dsp::Rng,
    metronome: bool,
    click_phase: f32,
    click_env: f32,
    click_hz: f32,
    master_gain: f32,
    auto_intensity: bool,
    /// Per section, per clip: its (track, pattern) indices, resolved once per song.
    clip_index: Vec<Vec<Option<(usize, usize)>>>,
    /// Per track, per send: the bus index.
    send_index: Vec<Vec<Option<usize>>>,
    /// Copies of the dry output of tracks that key a sidechain, reused every chunk.
    key_bufs: Vec<Option<Vec<[f32; 2]>>>,
    /// Any pattern in the song has automation (else it is skipped).
    has_automation: bool,
    /// Seconds spent per stage, when profiling (`set_profiling`).
    pub profile: Option<Profile>,
    clock: u64,
    meters: Meters,
    events: Vec<Event>,
    mix: Vec<[f32; 2]>,
    k_filter: [crate::dsp::Biquad; 4],
    k_power: f32,
    load: f32,
}

impl Engine {
    pub fn new(rate: f32, song: Arc<Song>) -> Engine {
        let mut e = Engine {
            rate,
            song: Arc::new(Song::empty("")),
            tracks: Vec::new(),
            buses: Vec::new(),
            master_chain: Chain { units: Vec::new() },
            playing: false,
            mode: Mode::Song,
            song_tick: 0.0,
            loop_range: None,
            main: None,
            stingers: Vec::new(),
            pattern_tick: 0.0,
            held: Vec::new(),
            live: Vec::new(),
            intensity: 0.0,
            intensity_target: 0.0,
            repeats: 0,
            finishing: None,
            finished: false,
            rng: crate::dsp::Rng::new(0x5EED),
            metronome: false,
            click_phase: 0.0,
            click_env: 0.0,
            click_hz: 1000.0,
            master_gain: 1.0,
            auto_intensity: true,
            clip_index: Vec::new(),
            send_index: Vec::new(),
            key_bufs: Vec::new(),
            has_automation: false,
            profile: None,
            clock: 0,
            meters: Meters::new(),
            events: Vec::with_capacity(256),
            mix: vec![[0.0; 2]; CHUNK],
            k_filter: Default::default(),
            k_power: 0.0,
            load: 0.0,
        };
        // K-weighting (ITU BS.1770): a high shelf and a high-pass, per side.
        for c in 0..2 {
            e.k_filter[c].set(
                crate::dsp::filter::BiquadKind::HighShelf,
                1681.0,
                0.707,
                4.0,
                rate,
            );
            e.k_filter[2 + c].set(
                crate::dsp::filter::BiquadKind::HighPass,
                38.0,
                0.5,
                0.0,
                rate,
            );
        }
        e.set_song(song);
        e
    }

    pub fn rate(&self) -> f32 {
        self.rate
    }
    pub fn song(&self) -> &Arc<Song> {
        &self.song
    }
    pub fn meters(&self) -> &Meters {
        &self.meters
    }
    pub fn meters_mut(&mut self) -> &mut Meters {
        &mut self.meters
    }
    pub fn mode(&self) -> &Mode {
        &self.mode
    }
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn status(&self) -> Status {
        Status {
            playing: self.playing,
            section: self.main.as_ref().map(|c| c.section),
            section_tick: self.main.as_ref().map(|c| c.tick as u32).unwrap_or(0),
            song_tick: self.song_tick as u32,
            pattern_tick: self.pattern_tick as u32,
            intensity: self.intensity,
            intensity_target: self.intensity_target,
            finished: self.finished,
            voices: self.tracks.iter().map(|t| t.player.active()).sum(),
            clock: self.clock,
            load: self.load,
        }
    }

    /// Takes a new version of the song, keeping every voice, delay line and tail it can.
    pub fn set_song(&mut self, song: Arc<Song>) {
        let rate = self.rate;
        let old_names: Vec<String> = self.tracks.iter().map(|t| t.name.clone()).collect();
        let mut old: Vec<Option<TrackState>> = std::mem::take(&mut self.tracks)
            .into_iter()
            .map(Some)
            .collect();
        let mut remap = vec![usize::MAX; old.len()];
        let mut tracks = Vec::with_capacity(song.tracks.len());
        for (i, t) in song.tracks.iter().enumerate() {
            let instrument = song.library.voice(&t.instrument);
            let kind = std::mem::discriminant(&instrument);
            let prev = old_names
                .iter()
                .position(|n| *n == t.name)
                .and_then(|j| old[j].take().map(|s| (j, s)))
                .filter(|(_, s)| s.kind == kind);
            let state = match prev {
                Some((j, mut s)) => {
                    remap[j] = i;
                    s.chain = Chain::build(&t.effects, Some(s.chain), rate);
                    s.instrument = instrument;
                    s
                }
                None => TrackState {
                    name: t.name.clone(),
                    player: Player::new(rate, i as u32 + 1),
                    chain: Chain::build(&t.effects, None, rate),
                    dry: vec![[0.0; 2]; CHUNK],
                    layer: t.layer.gain(self.intensity),
                    auto: [0.5, 1.0, 0.5, 1.0],
                    gain: [0.0; 2],
                    sidechain: Vec::new(),
                    instrument,
                    kind,
                    quiet: 0,
                },
            };
            tracks.push(state);
        }
        for (i, t) in song.tracks.iter().enumerate() {
            tracks[i].sidechain = t
                .effects
                .iter()
                .map(|fx| match fx {
                    Effect::Compressor {
                        sidechain: Some(name),
                        ..
                    } => song.track(name),
                    _ => None,
                })
                .collect();
        }
        self.tracks = tracks;
        self.held.retain_mut(|h| {
            h.track = remap.get(h.track).copied().unwrap_or(usize::MAX);
            h.track != usize::MAX
        });
        self.live.retain_mut(|(t, _)| {
            *t = remap.get(*t).copied().unwrap_or(usize::MAX);
            *t != usize::MAX
        });
        let mut old_buses: Vec<Option<BusState>> = std::mem::take(&mut self.buses)
            .into_iter()
            .map(Some)
            .collect();
        self.buses = song
            .buses
            .iter()
            .map(|b| {
                let prev = old_buses
                    .iter()
                    .position(|o| o.as_ref().is_some_and(|o| o.name == b.name));
                match prev.and_then(|p| old_buses[p].take()) {
                    Some(mut s) => {
                        s.chain = Chain::build(&b.effects, Some(s.chain), rate);
                        s
                    }
                    None => BusState {
                        name: b.name.clone(),
                        chain: Chain::build(&b.effects, None, rate),
                        buf: vec![[0.0; 2]; CHUNK],
                        quiet: 0,
                    },
                }
            })
            .collect();
        let old_master = std::mem::replace(&mut self.master_chain, Chain { units: Vec::new() });
        self.master_chain = Chain::build(&song.master.effects, Some(old_master), rate);
        // Cursors pointing past the new song's sections are dropped.
        if let Some(c) = &self.main {
            if c.section >= song.sections.len() {
                self.main = None;
            }
        }
        self.stingers.retain(|c| c.section < song.sections.len());
        match &self.mode {
            Mode::Section(i) if *i >= song.sections.len() => self.mode = Mode::Song,
            Mode::Pattern { pattern, track }
                if *pattern >= song.patterns.len() || *track >= song.tracks.len() =>
            {
                self.mode = Mode::Song
            }
            _ => {}
        }
        self.clip_index = song
            .sections
            .iter()
            .map(|s| {
                s.clips
                    .iter()
                    .map(|c| song.track(&c.track).zip(song.pattern(&c.pattern)))
                    .collect()
            })
            .collect();
        self.send_index = song
            .tracks
            .iter()
            .map(|t| t.sends.iter().map(|s| song.bus(&s.bus)).collect())
            .collect();
        self.key_bufs = (0..song.tracks.len())
            .map(|i| {
                self.tracks
                    .iter()
                    .any(|t| t.sidechain.contains(&Some(i)))
                    .then(|| vec![[0.0; 2]; CHUNK])
            })
            .collect();
        self.has_automation = song.patterns.iter().any(|p| !p.automation.is_empty());
        self.meters.tracks = vec![Meter::default(); song.tracks.len()];
        self.meters.buses = vec![Meter::default(); song.buses.len()];
        self.meters.layers = vec![1.0; song.tracks.len()];
        self.song = song;
    }

    pub fn command(&mut self, c: Command) {
        match c {
            Command::SetSong(s) => self.set_song(s),
            Command::Play => self.play(),
            Command::Stop => self.stop(),
            Command::Panic => self.panic(),
            Command::Seek(t) => self.seek(t),
            Command::SetMode(m) => self.set_mode(m),
            Command::SetLoop(l) => self.loop_range = l,
            Command::SetIntensity(v) => self.intensity_target = v.clamp(0.0, 1.0),
            Command::ForceIntensity(v) => {
                self.intensity_target = v.clamp(0.0, 1.0);
                self.intensity = self.intensity_target;
            }
            Command::Cue(name) => self.cue(&name),
            Command::Finish(name) => self.finish(name),
            Command::NoteOn { track, key, vel } => self.note_on(track, key, vel),
            Command::NoteOff { track, key } => self.note_off(track, key),
            Command::AllNotesOff => {
                for (t, k) in std::mem::take(&mut self.live) {
                    self.note_off(t, k);
                }
            }
            Command::Metronome(on) => self.metronome = on,
            Command::Gain(g) => self.master_gain = g,
            Command::AutoIntensity(on) => self.auto_intensity = on,
        }
    }

    pub fn set_intensity(&mut self, v: f32) {
        self.intensity_target = v.clamp(0.0, 1.0);
    }

    pub fn set_mode(&mut self, mode: Mode) {
        if mode != self.mode {
            self.release_sequenced();
            self.mode = mode;
            self.main = None;
            self.stingers.clear();
            self.pattern_tick = 0.0;
            self.finished = false;
            self.finishing = None;
            if self.playing {
                self.start_cursor();
            }
        }
    }

    pub fn play(&mut self) {
        if !self.playing {
            self.playing = true;
            self.finished = false;
            self.finishing = None;
            self.start_cursor();
        }
    }

    pub fn stop(&mut self) {
        self.playing = false;
        self.release_sequenced();
        self.main = None;
        self.stingers.clear();
    }

    pub fn panic(&mut self) {
        self.stop();
        for t in self.tracks.iter_mut() {
            t.player.panic();
        }
        self.live.clear();
    }

    /// Moves the playhead: arrangement ticks in Song mode, section ticks in Section mode,
    /// pattern ticks in Pattern mode.
    pub fn seek(&mut self, tick: u32) {
        self.release_sequenced();
        match self.mode {
            Mode::Song => {
                self.song_tick = tick as f64;
                self.main = None;
                if self.playing {
                    self.start_cursor();
                }
            }
            Mode::Section(_) | Mode::Director => {
                if let Some(c) = &mut self.main {
                    c.tick = tick as f64;
                }
            }
            Mode::Pattern { .. } => self.pattern_tick = tick as f64,
        }
    }

    pub fn cue(&mut self, name: &str) {
        if let Some(i) = self.song.section(name) {
            // Enter on the next beat so it lands in time.
            let spt = self.song.samples_per_tick(self.rate);
            let wait = match &self.main {
                Some(c) if self.playing => {
                    let into_beat = c.tick % PPQ as f64;
                    if into_beat < 1.0 {
                        0.0
                    } else {
                        (PPQ as f64 - into_beat) * spt
                    }
                }
                _ => 0.0,
            };
            self.stingers.push(Cursor {
                section: i,
                tick: 0.0,
                wait,
                stinger: true,
            });
            if !self.playing {
                self.playing = true;
            }
        }
    }

    /// Ends the music: with an ending section at the next bar line, else by fading at the section's end.
    pub fn finish(&mut self, ending: Option<String>) {
        self.finishing = Some(ending);
    }

    pub fn note_on(&mut self, track: usize, key: u8, vel: u8) {
        if let Some(t) = self.tracks.get_mut(track) {
            t.player.note_on(&t.instrument, key, vel);
            self.live.push((track, key));
        }
    }

    pub fn note_off(&mut self, track: usize, key: u8) {
        if let Some(t) = self.tracks.get_mut(track) {
            t.player.note_off(&t.instrument, key);
        }
        self.live.retain(|&(t, k)| !(t == track && k == key));
    }

    fn release_sequenced(&mut self) {
        for h in std::mem::take(&mut self.held) {
            if let Some(t) = self.tracks.get_mut(h.track) {
                t.player.note_off(&t.instrument, h.key);
            }
        }
    }

    fn start_cursor(&mut self) {
        let song = self.song.clone();
        match self.mode.clone() {
            Mode::Song => {
                if song.arrangement_ticks() == 0 {
                    return;
                }
                if self.song_tick >= song.arrangement_ticks() as f64 {
                    self.song_tick = 0.0;
                }
                let starts = song.arrangement_starts();
                let at = self.song_tick as u32;
                if let Some(&(start, sec)) = starts.iter().rev().find(|(s, _)| *s <= at) {
                    self.main = Some(Cursor {
                        section: sec,
                        tick: (at - start) as f64,
                        wait: 0.0,
                        stinger: false,
                    });
                }
            }
            Mode::Section(i) => {
                if self.main.is_none() && i < song.sections.len() {
                    self.main = Some(Cursor {
                        section: i,
                        tick: 0.0,
                        wait: 0.0,
                        stinger: false,
                    });
                }
            }
            Mode::Director => {
                if self.main.is_none() {
                    self.repeats = 0;
                    if let Some(i) = self.director_first() {
                        self.main = Some(Cursor {
                            section: i,
                            tick: 0.0,
                            wait: 0.0,
                            stinger: false,
                        });
                    }
                }
            }
            Mode::Pattern { .. } => {}
        }
    }

    /// The section the director opens with: an intro that fits, else a loop that fits.
    fn director_first(&mut self) -> Option<usize> {
        let song = &self.song;
        let fits = |i: usize| {
            let (lo, hi) = song.sections[i].intensity;
            self.intensity_target >= lo - 0.05 && self.intensity_target <= hi + 0.05
        };
        let intro = (0..song.sections.len())
            .find(|&i| song.sections[i].kind == SectionKind::Intro && fits(i));
        intro.or_else(|| self.director_pick(None))
    }

    /// The next section after `from` at the current intensity.
    fn director_pick(&mut self, from: Option<usize>) -> Option<usize> {
        let song = self.song.clone();
        let x = self.intensity;
        let named: Vec<usize> = from
            .map(|f| {
                song.sections[f]
                    .next
                    .iter()
                    .filter_map(|n| song.section(n))
                    .collect()
            })
            .unwrap_or_default();
        let pool: Vec<usize> = if !named.is_empty() {
            named
        } else {
            (0..song.sections.len())
                .filter(|&i| song.sections[i].kind == SectionKind::Loop)
                .collect()
        };
        if pool.is_empty() {
            return from.filter(|&f| song.sections[f].kind == SectionKind::Loop);
        }
        let distance = |i: usize| {
            let (lo, hi) = song.sections[i].intensity;
            if x < lo {
                lo - x
            } else if x > hi {
                x - hi
            } else {
                0.0
            }
        };
        let fitting: Vec<usize> = pool
            .iter()
            .copied()
            .filter(|&i| distance(i) == 0.0)
            .collect();
        if fitting.is_empty() {
            return pool
                .iter()
                .copied()
                .min_by(|&a, &b| distance(a).total_cmp(&distance(b)));
        }
        // Stay a while in a loop that still fits, then move to another that fits too.
        if let Some(f) = from {
            if fitting.contains(&f) && (self.repeats < 2 || fitting.len() == 1) {
                return Some(f);
            }
        }
        let others: Vec<usize> = fitting
            .iter()
            .copied()
            .filter(|&i| Some(i) != from)
            .collect();
        let list = if others.is_empty() { &fitting } else { &others };
        Some(list[(self.rng.next_u32() as usize) % list.len()])
    }

    /// Fills interleaved stereo `out` (overwrites).
    pub fn render(&mut self, out: &mut [f32]) {
        let _ftz = FlushDenormals::on();
        let started = std::time::Instant::now();
        let frames = out.len() / 2;
        let mut done = 0;
        while done < frames {
            let n = (frames - done).min(CHUNK);
            self.render_chunk(n);
            for i in 0..n {
                out[(done + i) * 2] = self.mix[i][0];
                out[(done + i) * 2 + 1] = self.mix[i][1];
            }
            done += n;
        }
        let took = started.elapsed().as_secs_f32();
        let budget = frames as f32 / self.rate;
        if budget > 0.0 {
            self.load = self.load * 0.9 + (took / budget) * 0.1;
        }
    }

    /// Renders into a buffer of frames (overwrites).
    pub fn render_frames(&mut self, out: &mut [[f32; 2]]) {
        let _ftz = FlushDenormals::on();
        let mut done = 0;
        while done < out.len() {
            let n = (out.len() - done).min(CHUNK);
            self.render_chunk(n);
            out[done..done + n].copy_from_slice(&self.mix[..n]);
            done += n;
        }
    }

    fn render_chunk(&mut self, n: usize) {
        let song = self.song.clone();
        let rate = self.rate;
        let spt = song.samples_per_tick(rate);
        let beat = (spt * PPQ as f64) as f32;
        // Intensity glides: up over about two seconds, down over about eight.
        let dt = n as f32 / rate;
        let tc = if self.intensity_target > self.intensity {
            2.0
        } else {
            8.0
        };
        self.intensity += (self.intensity_target - self.intensity) * (1.0 - (-dt / tc).exp());

        self.events.clear();
        if self.playing {
            self.sequence(&song, n, spt);
        }
        // Sequenced notes run down and release.
        let ticks = n as f64 / spt;
        if self.playing {
            let mut i = 0;
            while i < self.held.len() {
                self.held[i].left -= ticks;
                if self.held[i].left <= 0.0 {
                    let h = self.held.swap_remove(i);
                    let at = (((h.left + ticks) * spt).max(0.0) as usize).min(n.saturating_sub(1));
                    self.events.push(Event {
                        at,
                        track: h.track,
                        key: h.key,
                        vel: 0,
                    });
                } else {
                    i += 1;
                }
            }
        }
        self.events.sort_by_key(|e| (e.at, e.vel != 0));

        // Instruments.
        let any_solo = song.tracks.iter().any(|t| t.solo);
        let timing = self.profile.is_some();
        let mut clock = timing.then(std::time::Instant::now);
        let lap = |slot: &mut f64, clock: &mut Option<std::time::Instant>| {
            if let Some(c) = clock {
                let now = std::time::Instant::now();
                *slot += (now - *c).as_secs_f64();
                *c = now;
            }
        };
        let mut synth_times = vec![0.0f64; if timing { self.tracks.len() } else { 0 }];
        let mut chain_times = vec![0.0f64; if timing { self.tracks.len() } else { 0 }];
        let mut bus_times = vec![0.0f64; if timing { self.buses.len() } else { 0 }];
        let mut master_time = 0.0f64;
        let mut other_time = 0.0f64;
        lap(&mut other_time, &mut clock);
        for (ti, t) in self.tracks.iter_mut().enumerate() {
            let spec = &song.tracks[ti];
            t.dry.resize(n, [0.0; 2]);
            t.dry[..n].fill([0.0; 2]);
            let mods = Mods {
                cutoff: (t.auto[0] - 0.5) * 8.0
                    + follow(spec, Target::Cutoff, self.intensity, 0.5) * 8.0,
                beat,
            };
            let mut at = 0;
            for e in self.events.iter().filter(|e| e.track == ti) {
                if e.at > at {
                    t.player.render(&t.instrument, &mut t.dry[at..e.at], mods);
                    at = e.at;
                }
                if e.vel > 0 {
                    t.player.note_on(&t.instrument, e.key, e.vel);
                } else {
                    t.player.note_off(&t.instrument, e.key);
                }
            }
            if at < n {
                t.player.render(&t.instrument, &mut t.dry[at..n], mods);
            }
            if timing {
                lap(&mut synth_times[ti], &mut clock);
            }
        }
        // Inserts, faders, sends. Sidechains key off the source's dry output, so run
        // the chains from copies: keys first.
        for b in self.buses.iter_mut() {
            b.buf.resize(n, [0.0; 2]);
            b.buf[..n].fill([0.0; 2]);
        }
        self.mix.resize(n.max(CHUNK), [0.0; 2]);
        self.mix[..n].fill([0.0; 2]);
        for (i, k) in self.key_bufs.iter_mut().enumerate() {
            if let Some(k) = k {
                k.resize(n, [0.0; 2]);
                k[..n].copy_from_slice(&self.tracks[i].dry[..n]);
            }
        }
        let keys = std::mem::take(&mut self.key_bufs);
        let intensity = self.intensity;
        let layer_k = 1.0 - (-dt / 0.35).exp();
        for (ti, t) in self.tracks.iter_mut().enumerate() {
            let spec = &song.tracks[ti];
            let target_layer = spec.layer.gain(intensity);
            t.layer += (target_layer - t.layer) * layer_k;
            self.meters.layers[ti] = t.layer;
            // A strip with no voices and nothing coming out is skipped after its effects have
            // had a second to ring out (delays and reverbs on the strip itself).
            let silent =
                t.player.active() == 0 && t.dry[..n].iter().all(|f| f[0] == 0.0 && f[1] == 0.0);
            t.quiet = if silent { t.quiet.saturating_add(1) } else { 0 };
            let tail_chunks = if spec
                .effects
                .iter()
                .any(|fx| matches!(fx, Effect::Delay { .. } | Effect::Reverb { .. }))
            {
                (8.0 * rate / CHUNK as f32) as u32
            } else {
                (0.25 * rate / CHUNK as f32) as u32
            };
            if t.quiet > tail_chunks {
                if timing {
                    lap(&mut chain_times[ti], &mut clock);
                }
                continue;
            }
            let mut buf = std::mem::take(&mut t.dry);
            let mut red = 0.0f32;
            for (slot, (u, fx)) in t.chain.units.iter_mut().zip(&spec.effects).enumerate() {
                let key = t
                    .sidechain
                    .get(slot)
                    .copied()
                    .flatten()
                    .and_then(|k| keys.get(k)?.as_deref().map(|b| &b[..n]));
                let ctx = Ctx {
                    rate,
                    beat,
                    sidechain: key,
                };
                red = red.min(u.run(fx, &mut buf[..n], &ctx));
            }
            let audible = !spec.mute && (!any_solo || spec.solo);
            let vol = db_to_gain(spec.db)
                * t.layer
                * t.auto[1]
                * follow(spec, Target::Volume, intensity, 1.0)
                * if audible { 1.0 } else { 0.0 };
            let pan_auto =
                (t.auto[2] - 0.5) * 2.0 + (follow(spec, Target::Pan, intensity, 0.5) - 0.5) * 2.0;
            let (pl, pr) = pan_gains((spec.pan + pan_auto).clamp(-1.0, 1.0));
            let target = [
                vol * pl * std::f32::consts::SQRT_2,
                vol * pr * std::f32::consts::SQRT_2,
            ];
            let sends_scale = t.auto[3]
                * follow(spec, Target::Sends, intensity, 1.0)
                * if audible { 1.0 } else { 0.0 };
            let mut peak = [0.0f32; 2];
            let step = 1.0 / n as f32;
            let g0 = t.gain;
            for (i, f) in buf[..n].iter_mut().enumerate() {
                let k = (i + 1) as f32 * step;
                let gl = g0[0] + (target[0] - g0[0]) * k;
                let gr = g0[1] + (target[1] - g0[1]) * k;
                f[0] *= gl;
                f[1] *= gr;
                peak[0] = peak[0].max(f[0].abs());
                peak[1] = peak[1].max(f[1].abs());
            }
            t.gain = target;
            for (si, s) in spec.sends.iter().enumerate() {
                if let Some(bi) = self
                    .send_index
                    .get(ti)
                    .and_then(|v| v.get(si))
                    .copied()
                    .flatten()
                {
                    let g = db_to_gain(s.db) * sends_scale;
                    if g > 0.0 {
                        let bus = &mut self.buses[bi].buf;
                        for i in 0..n {
                            bus[i][0] += buf[i][0] * g;
                            bus[i][1] += buf[i][1] * g;
                        }
                    }
                }
            }
            for (m, b) in self.mix[..n].iter_mut().zip(&buf[..n]) {
                m[0] += b[0];
                m[1] += b[1];
            }
            t.dry = buf;
            if timing {
                lap(&mut chain_times[ti], &mut clock);
            }
            let m = &mut self.meters.tracks[ti];
            m.peak[0] = m.peak[0].max(peak[0]);
            m.peak[1] = m.peak[1].max(peak[1]);
            m.reduction = m.reduction.min(red);
        }
        self.key_bufs = keys;
        for (bi, b) in self.buses.iter_mut().enumerate() {
            let spec = &song.buses[bi];
            // A bus whose sends have been silent longer than any tail it could have is skipped.
            let silent = b.buf[..n].iter().all(|f| f[0] == 0.0 && f[1] == 0.0);
            b.quiet = if silent { b.quiet.saturating_add(1) } else { 0 };
            if b.quiet > (10.0 * rate / CHUNK as f32) as u32 {
                if timing {
                    lap(&mut bus_times[bi], &mut clock);
                }
                continue;
            }
            let ctx = Ctx {
                rate,
                beat,
                sidechain: None,
            };
            let red = b.chain.run(&spec.effects, &mut b.buf[..n], &ctx);
            let g = if spec.mute { 0.0 } else { db_to_gain(spec.db) };
            let m = &mut self.meters.buses[bi];
            for i in 0..n {
                let (l, r) = (b.buf[i][0] * g, b.buf[i][1] * g);
                m.peak[0] = m.peak[0].max(l.abs());
                m.peak[1] = m.peak[1].max(r.abs());
                self.mix[i][0] += l;
                self.mix[i][1] += r;
            }
            m.reduction = m.reduction.min(red);
            if timing {
                lap(&mut bus_times[bi], &mut clock);
            }
        }
        // Metronome, before the master chain would colour it: mixed after.
        let ctx = Ctx {
            rate,
            beat,
            sidechain: None,
        };
        let g = db_to_gain(song.master.db) * self.master_gain;
        for f in self.mix[..n].iter_mut() {
            f[0] *= g;
            f[1] *= g;
        }
        let mut mix = std::mem::take(&mut self.mix);
        let red = self
            .master_chain
            .run(&song.master.effects, &mut mix[..n], &ctx);
        self.mix = mix;
        if self.metronome && self.playing {
            self.render_click(n, spt);
        }
        let mut peak = [0.0f32; 2];
        for i in 0..n {
            let f = &mut self.mix[i];
            for c in 0..2 {
                if !f[c].is_finite() {
                    f[c] = 0.0;
                }
                peak[c] = peak[c].max(f[c].abs());
            }
            let kl = {
                let x = self.k_filter[0].process(f[0]);
                self.k_filter[2].process(x)
            };
            let kr = {
                let x = self.k_filter[1].process(f[1]);
                self.k_filter[3].process(x)
            };
            self.k_power += (kl * kl + kr * kr - self.k_power) * (1.0 / (0.4 * rate));
            self.meters.scope[self.meters.scope_at] = *f;
            self.meters.scope_at = (self.meters.scope_at + 1) % SCOPE_LEN;
        }
        for k in self.k_filter.iter_mut() {
            k.sanitise();
        }
        self.meters.loudness = -0.691 + 10.0 * self.k_power.max(1e-12).log10();
        self.meters.master.peak[0] = self.meters.master.peak[0].max(peak[0]);
        self.meters.master.peak[1] = self.meters.master.peak[1].max(peak[1]);
        self.meters.master.reduction = self.meters.master.reduction.min(red);
        lap(&mut master_time, &mut clock);
        if let Some(p) = &mut self.profile {
            p.add(
                &synth_times,
                &chain_times,
                &bus_times,
                master_time,
                other_time,
            );
        }
        self.clock += n as u64;
        let _ = gain_to_db;
    }

    fn render_click(&mut self, n: usize, spt: f64) {
        let Some(tick) = self.position_tick() else {
            return;
        };
        let bar = self.song.bar_ticks() as f64;
        for i in 0..n {
            let t = tick + i as f64 / spt;
            let prev = t - 1.0 / spt;
            if (t / PPQ as f64).floor() != (prev / PPQ as f64).floor() || t < 1.0 / spt {
                self.click_env = 1.0;
                self.click_hz = if (t % bar) < PPQ as f64 {
                    1760.0
                } else {
                    1175.0
                };
            }
            if self.click_env > 1e-3 {
                self.click_phase = (self.click_phase + self.click_hz / self.rate).fract();
                let v = (self.click_phase * std::f32::consts::TAU).sin() * self.click_env * 0.25;
                self.click_env *= 0.9985;
                self.mix[i][0] += v;
                self.mix[i][1] += v;
            }
        }
    }

    /// Ticks into what the metronome counts from (section or pattern), before this chunk.
    fn position_tick(&self) -> Option<f64> {
        match self.mode {
            Mode::Pattern { .. } => Some(self.pattern_tick),
            _ => self.main.as_ref().map(|c| c.tick),
        }
    }

    /// Advances the cursors by `n` samples and emits their notes into `events`.
    fn sequence(&mut self, song: &Arc<Song>, n: usize, spt: f64) {
        let ticks = n as f64 / spt;
        if let Mode::Pattern { pattern, track } = self.mode {
            let p = &song.patterns[pattern];
            let len = p.ticks().max(1) as f64;
            let t0 = self.pattern_tick;
            self.emit_pattern(song, pattern, track, 0, t0, t0 + ticks, len, spt, 0.0, n);
            self.pattern_tick = (t0 + ticks) % len;
            // Automation of the auditioned pattern.
            self.apply_automation_pattern(song, pattern, track, self.pattern_tick);
            return;
        }
        // Main cursor.
        let mut guard = 0;
        let mut offset = 0.0f64; // samples into the chunk already covered
        while let Some(mut c) = self.main.take() {
            guard += 1;
            if guard > 16 {
                self.main = Some(c);
                break;
            }
            let sec = &song.sections[c.section];
            let len = song.section_ticks(sec) as f64;
            if len <= 0.0 {
                self.main = None;
                break;
            }
            if self.mode == Mode::Song && self.auto_intensity && sec.kind != SectionKind::Stinger {
                let (lo, hi) = sec.intensity;
                self.intensity_target = ((lo + hi) * 0.5).clamp(0.0, 1.0);
            }
            let remaining_samples = n as f64 - offset;
            let t0 = c.tick;
            let mut t1 = t0 + remaining_samples / spt;
            // Loop range in Song mode is in arrangement ticks.
            let arr_start = if self.mode == Mode::Song {
                self.song_tick - t0
            } else {
                0.0
            };
            let mut jump_to: Option<f64> = None;
            if self.mode == Mode::Song {
                if let Some((a, b)) = self.loop_range {
                    let end_local = b as f64 - arr_start;
                    if b > a && t0 < end_local && t1 >= end_local {
                        t1 = end_local;
                        jump_to = Some(a as f64);
                    }
                }
            }
            // Early exit at a bar line when the intensity has left the section's range.
            let mut cut_at: Option<f64> = None;
            if self.mode == Mode::Director && sec.exit_every > 0 && !c.stinger {
                let (lo, hi) = sec.intensity;
                let off = self.intensity < lo - 0.1 || self.intensity > hi + 0.1;
                if off {
                    let every = (sec.exit_every * song.bar_ticks()) as f64;
                    let next_line = ((t0 / every).floor() + 1.0) * every;
                    if next_line < t1 && next_line < len {
                        cut_at = Some(next_line);
                    }
                }
            }
            if self.mode == Mode::Director {
                if let Some(Some(_)) = &self.finishing {
                    // Go to the ending at the next bar line.
                    let bar = song.bar_ticks() as f64;
                    let next_line = if t0 % bar < 1e-6 {
                        t0
                    } else {
                        ((t0 / bar).floor() + 1.0) * bar
                    };
                    if next_line < t1 {
                        cut_at = Some(cut_at.map_or(next_line, |c: f64| c.min(next_line)));
                    }
                }
            }
            let end = cut_at.unwrap_or(len).min(t1);
            self.emit_section(song, c.section, t0, end, spt, offset, n);
            self.apply_automation(song, c.section, end.min(len - 1e-6));
            offset += (end - t0) * spt;
            if self.mode == Mode::Song {
                self.song_tick += end - t0;
            }
            if let Some(a) = jump_to {
                self.release_sequenced();
                self.song_tick = a;
                let starts = song.arrangement_starts();
                match starts.iter().rev().find(|(s, _)| *s as f64 <= a) {
                    Some(&(s, i)) => {
                        c = Cursor {
                            section: i,
                            tick: a - s as f64,
                            wait: 0.0,
                            stinger: false,
                        };
                        self.main = Some(c);
                        continue;
                    }
                    None => break,
                }
            }
            if end < len && cut_at.is_none() {
                c.tick = end;
                self.main = Some(c);
                break;
            }
            // The section is over (or cut): what comes next.
            let next = self.after_section(song, c.section);
            match next {
                Some(i) => {
                    if Some(i) == Some(c.section) {
                        self.repeats += 1;
                    } else {
                        self.repeats = 0;
                    }
                    self.main = Some(Cursor {
                        section: i,
                        tick: 0.0,
                        wait: 0.0,
                        stinger: false,
                    });
                }
                None => {
                    self.main = None;
                    if self.stingers.is_empty() {
                        self.playing = false;
                        self.finished = true;
                        self.release_sequenced();
                    }
                    break;
                }
            }
        }
        // Stingers.
        let mut keep = Vec::new();
        for mut c in std::mem::take(&mut self.stingers) {
            let mut start = 0.0;
            if c.wait > 0.0 {
                if c.wait >= n as f64 {
                    c.wait -= n as f64;
                    keep.push(c);
                    continue;
                }
                start = c.wait;
                c.wait = 0.0;
            }
            let sec = &song.sections[c.section];
            let len = song.section_ticks(sec) as f64;
            let t1 = c.tick + (n as f64 - start) / spt;
            let end = t1.min(len);
            self.emit_section(song, c.section, c.tick, end, spt, start, n);
            c.tick = end;
            if end < len {
                keep.push(c);
            }
        }
        self.stingers = keep;
        if self.main.is_none()
            && self.stingers.is_empty()
            && self.mode == Mode::Director
            && self.finishing.is_some()
        {
            self.playing = false;
            self.finished = true;
        }
    }

    /// Where the main cursor goes when `section` ends; `None` stops.
    fn after_section(&mut self, song: &Arc<Song>, section: usize) -> Option<usize> {
        match self.mode {
            Mode::Section(i) => Some(i),
            Mode::Song => {
                let starts = song.arrangement_starts();
                let at = self.song_tick;
                match starts.iter().find(|(s, _)| (*s as f64 - at).abs() < 0.5) {
                    Some(&(s, i)) => {
                        self.song_tick = s as f64;
                        Some(i)
                    }
                    None => {
                        if self.loop_range.is_none() {
                            None
                        } else {
                            self.song_tick = 0.0;
                            starts.first().map(|s| s.1)
                        }
                    }
                }
            }
            Mode::Director => {
                if song.sections[section].kind == SectionKind::Ending {
                    return None;
                }
                if let Some(ending) = self.finishing.clone() {
                    return match ending {
                        Some(name) => {
                            self.finishing = Some(None);
                            song.section(&name)
                        }
                        None => None,
                    };
                }
                self.director_pick(Some(section))
            }
            Mode::Pattern { .. } => None,
        }
    }

    /// Emits the notes of `section` whose starts fall in [t0, t1), `offset` samples into the chunk.
    fn emit_section(
        &mut self,
        song: &Arc<Song>,
        section: usize,
        t0: f64,
        t1: f64,
        spt: f64,
        offset: f64,
        n: usize,
    ) {
        let sec = &song.sections[section];
        let sec_ticks = song.section_ticks(sec);
        for (ci, clip) in sec.clips.iter().enumerate() {
            let Some((track, pi)) = self
                .clip_index
                .get(section)
                .and_then(|v| v.get(ci))
                .copied()
                .flatten()
            else {
                continue;
            };
            let p = &song.patterns[pi];
            let plen = p.ticks();
            if plen == 0 {
                continue;
            }
            let start = (clip.at * PPQ) as f64;
            let span = clip.span(plen, sec_ticks) as f64;
            let (a, b) = ((t0 - start).max(0.0), (t1 - start).min(span));
            if b <= a {
                continue;
            }
            // Local clip time [a, b) maps to pattern loops.
            let base_offset = offset + (start + a - t0).max(0.0) * spt;
            let _ = base_offset;
            self.emit_range(
                song,
                pi,
                track,
                clip.transpose,
                a,
                b,
                plen as f64,
                span,
                spt,
                offset + (start - t0) * spt,
                n,
            );
        }
    }

    fn emit_pattern(
        &mut self,
        song: &Arc<Song>,
        pi: usize,
        track: usize,
        transpose: i8,
        t0: f64,
        t1: f64,
        len: f64,
        spt: f64,
        offset: f64,
        n: usize,
    ) {
        // Pattern mode loops: split at the wrap.
        if t1 > len {
            self.emit_range(
                song,
                pi,
                track,
                transpose,
                t0,
                len,
                len,
                f64::MAX,
                spt,
                offset - t0 * spt,
                n,
            );
            let rest = t1 - len;
            self.emit_range(
                song,
                pi,
                track,
                transpose,
                0.0,
                rest,
                len,
                f64::MAX,
                spt,
                offset + (len - t0) * spt,
                n,
            );
        } else {
            self.emit_range(
                song,
                pi,
                track,
                transpose,
                t0,
                t1,
                len,
                f64::MAX,
                spt,
                offset - t0 * spt,
                n,
            );
        }
    }

    /// Notes of pattern `pi` looped every `plen` ticks, starting in [a, b) of clip time;
    /// clip time 0 is `zero` samples into the chunk (may be negative). Notes are cut at `span`.
    fn emit_range(
        &mut self,
        song: &Arc<Song>,
        pi: usize,
        track: usize,
        transpose: i8,
        a: f64,
        b: f64,
        plen: f64,
        span: f64,
        spt: f64,
        zero: f64,
        n: usize,
    ) {
        let p = &song.patterns[pi];
        let first_loop = (a / plen).floor() as i64;
        let last_loop = ((b - 1e-9) / plen).floor() as i64;
        for lp in first_loop..=last_loop {
            let base = lp as f64 * plen;
            for note in &p.notes {
                if note.at() >= p.ticks() {
                    continue;
                }
                let t = base + note.at() as f64;
                if t < a || t >= b {
                    continue;
                }
                let at = ((zero + t * spt).round().max(0.0) as usize).min(n.saturating_sub(1));
                let key = (note.key() as i32 + transpose as i32).clamp(0, 127) as u8;
                // A note never rings past its pattern's end or the clip's.
                let len = (note.len().max(1) as f64)
                    .min(p.ticks() as f64 - note.at() as f64)
                    .min(span - t)
                    .max(1.0);
                // Retrigger of a key already held on this track: release it first.
                if let Some(pos) = self
                    .held
                    .iter()
                    .position(|h| h.track == track && h.key == key)
                {
                    self.held.swap_remove(pos);
                    self.events.push(Event {
                        at,
                        track,
                        key,
                        vel: 0,
                    });
                }
                self.events.push(Event {
                    at,
                    track,
                    key,
                    vel: note.vel().max(1),
                });
                // Time left counts from the chunk start, where `held` is decremented.
                let into = (at as f64) / spt;
                self.held.push(Held {
                    track,
                    key,
                    left: len + into,
                });
            }
        }
    }

    fn apply_automation(&mut self, song: &Arc<Song>, section: usize, tick: f64) {
        if !self.has_automation {
            return;
        }
        let sec = &song.sections[section];
        let sec_ticks = song.section_ticks(sec);
        let mut touched = vec![[false; 4]; self.tracks.len()];
        for (ci, clip) in sec.clips.iter().enumerate() {
            let Some((track, pi)) = self
                .clip_index
                .get(section)
                .and_then(|v| v.get(ci))
                .copied()
                .flatten()
            else {
                continue;
            };
            let p = &song.patterns[pi];
            if p.automation.is_empty() || p.ticks() == 0 {
                continue;
            }
            let start = (clip.at * PPQ) as f64;
            let span = clip.span(p.ticks(), sec_ticks) as f64;
            if tick < start || tick >= start + span {
                continue;
            }
            let local = (tick - start) % p.ticks() as f64;
            for lane in &p.automation {
                if let Some(v) = lane.value_at(local as f32) {
                    let k = target_slot(lane.target);
                    self.tracks[track].auto[k] = v;
                    touched[track][k] = true;
                }
            }
        }
        for (t, tt) in self.tracks.iter_mut().zip(touched) {
            for (k, was) in tt.iter().enumerate() {
                if !was {
                    t.auto[k] = [0.5, 1.0, 0.5, 1.0][k];
                }
            }
        }
    }

    fn apply_automation_pattern(&mut self, song: &Arc<Song>, pi: usize, track: usize, tick: f64) {
        let p = &song.patterns[pi];
        for t in self.tracks.iter_mut() {
            t.auto = [0.5, 1.0, 0.5, 1.0];
        }
        for lane in &p.automation {
            if let Some(v) = lane.value_at(tick as f32) {
                self.tracks[track].auto[target_slot(lane.target)] = v;
            }
        }
    }
}

fn target_slot(t: Target) -> usize {
    match t {
        Target::Cutoff => 0,
        Target::Volume => 1,
        Target::Pan => 2,
        Target::Sends => 3,
    }
}

/// A track's `follow` value for `target` at `intensity`, or `neutral` when none.
fn follow(spec: &crate::song::Track, target: Target, intensity: f32, neutral: f32) -> f32 {
    let mut v = neutral;
    for f in spec.follow.iter().filter(|f| f.target == target) {
        v = f.from + (f.to - f.from) * intensity;
    }
    if target == Target::Cutoff {
        v - 0.5
    } else {
        v
    }
}

/// Treats denormal floats as zero while rendering. Echo and reverb tails decay
/// into denormals and keep circulating in their feedback loops, and on x86 every
/// operation on one is many times slower: the calm sections, with long tails and
/// few notes, cost twice the busy ones before this. The flags are put back when
/// this is dropped, so the caller's thread (the game's audio callback) is left as it was.
struct FlushDenormals {
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    saved: u32,
}

impl FlushDenormals {
    #[cfg_attr(
        any(target_arch = "x86_64", target_arch = "x86"),
        expect(
            deprecated,
            reason = "_mm_getcsr/_mm_setcsr are deprecated but std has no other way to set flush-to-zero/denormals-are-zero for the audio thread"
        )
    )]
    fn on() -> FlushDenormals {
        #[cfg(target_arch = "x86_64")]
        {
            use std::arch::x86_64::{_mm_getcsr, _mm_setcsr};
            // SAFETY: only the flush-to-zero (bit 15) and denormals-are-zero (bit 6) bits change.
            unsafe {
                let saved = _mm_getcsr();
                _mm_setcsr(saved | 0x8040);
                FlushDenormals { saved }
            }
        }
        #[cfg(target_arch = "x86")]
        {
            use std::arch::x86::{_mm_getcsr, _mm_setcsr};
            // SAFETY: as above.
            unsafe {
                let saved = _mm_getcsr();
                _mm_setcsr(saved | 0x8040);
                FlushDenormals { saved }
            }
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
        FlushDenormals {}
    }
}

impl Drop for FlushDenormals {
    #[cfg_attr(
        any(target_arch = "x86_64", target_arch = "x86"),
        expect(
            deprecated,
            reason = "_mm_getcsr/_mm_setcsr are deprecated but std has no other way to set flush-to-zero/denormals-are-zero for the audio thread"
        )
    )]
    fn drop(&mut self) {
        #[cfg(target_arch = "x86_64")]
        // SAFETY: restores the value read in `on`.
        unsafe {
            std::arch::x86_64::_mm_setcsr(self.saved)
        }
        #[cfg(target_arch = "x86")]
        // SAFETY: restores the value read in `on`.
        unsafe {
            std::arch::x86::_mm_setcsr(self.saved)
        }
    }
}

/// Seconds spent per stage, summed over the chunks rendered while profiling.
#[derive(Clone, Debug, Default)]
pub struct Profile {
    pub synth: Vec<f64>,
    pub chain: Vec<f64>,
    pub buses: Vec<f64>,
    pub master: f64,
    /// Sequencing and everything before the instruments.
    pub other: f64,
}

impl Profile {
    fn add(&mut self, synth: &[f64], chain: &[f64], buses: &[f64], master: f64, other: f64) {
        let grow = |v: &mut Vec<f64>, a: &[f64]| {
            if v.len() < a.len() {
                v.resize(a.len(), 0.0);
            }
            for (x, y) in v.iter_mut().zip(a) {
                *x += y;
            }
        };
        grow(&mut self.synth, synth);
        grow(&mut self.chain, chain);
        grow(&mut self.buses, buses);
        self.master += master;
        self.other += other;
    }
}
