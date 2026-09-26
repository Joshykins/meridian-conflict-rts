//! Music: the score (`data/music`, played by `mc_music`) under the game's sound.
//!
//! The music does not follow the battle. A song plays straight through and
//! loops (`mc_music::stage::Stage`); when something happens (a nuke lands, a
//! commander falls, the match is won) its *moment* plays, a short piece of its
//! own from `data/music/moments/`, and the song dips under it and comes back
//! after. A moment the score lists as an ending (victory, defeat) stops the song.
//!
//! Four threads take part, and none waits on another for long:
//!
//! - The game thread says what should play (`Audio::music_scene`) and what
//!   happened (`music_cue`, `music_finish`). Each call is a message on a channel.
//! - The loader thread (`mc-music`) owns the score, the songs and the moments: it
//!   reads and parses them (every moment the score names, when the score loads),
//!   builds each song's stage at the device's rate, decides when a new song
//!   cross-fades in, and once a second looks at the files, so a song or a moment
//!   edited in the studio is heard in the running game (hot reload).
//! - The render thread (`mc-music-render`) owns the `Deck`: the stage playing and
//!   any fading out. It renders them about `AHEAD` seconds ahead of the device into
//!   a lock-free ring (`ring.rs`); a finished stage is handed back to the loader
//!   thread to be freed there.
//! - The audio callback only copies frames out of the ring, applying the volume
//!   and the pause duck as it goes (so those act at once), and plays silence for
//!   whatever the ring is short of (counted as an underrun). No lock, no
//!   allocation, no synthesis: a heavy song cannot make the battle's sound glitch.
//!
//! Nothing starts until the device has a sample rate: `Audio::silent` (the
//! headless tools) and builds without a sound backend never load a song.

mod ring;

use super::{Audio, Shared};
use mc_music::score::{self, Score};
use mc_music::stage::Stage;
use mc_music::{Song, Status};
use ring::Ring;
use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant, SystemTime};

/// The paused game's music, against its full level: -6 dB.
const DUCK_GAIN: f32 = 0.5;
/// How often the loader looks at the files for changes.
const POLL: Duration = Duration::from_secs(1);
/// Songs fading out at once; a further one is cut (only rapid scene flipping gets here).
const MAX_OUTGOING: usize = 3;
/// Frames the deck renders per pass.
const SCRATCH_FRAMES: usize = 1024;
/// A volume or duck change glides over this many seconds, so a slider does not zip.
const GAIN_GLIDE: f32 = 0.08;
/// Seconds of music the render thread keeps ready ahead of the device. Also the
/// delay before a moment is heard; volume and duck act at once.
const AHEAD: f32 = 0.2;
/// The ring: over `AHEAD` at any common rate (0.34 s at 96 kHz).
const RING_FRAMES: usize = 32_768;
/// Frames the render thread renders per step.
const RENDER_FRAMES: usize = 256;
/// The render thread's nap when the ring is full enough.
const RENDER_IDLE: Duration = Duration::from_millis(4);
/// The shortest cross-fade: a song swapped for a new device rate, or a score with `fade: 0`.
const MIN_FADE: f32 = 0.05;
/// A stage whose song an ending stopped fades out of the deck over this once its moment is done.
const ENDED_FADE: f32 = 0.5;

/// What the game wants to hear.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scene {
    Silent,
    /// The front end, the set-up screens and the loading screen.
    Menu,
    /// A match, by the local player's faction key ("Aster").
    Battle(String),
    Survival,
}

/// The song the score gives a scene. Faction keys match without regard to case;
/// survival falls back to the default battle song when it has none of its own.
pub fn song_for(score: &Score, scene: &Scene) -> Option<String> {
    let named = |s: &str| (!s.trim().is_empty()).then(|| s.trim().to_string());
    let battle = |faction: &str| {
        score
            .battle
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(faction))
            .or_else(|| score.battle.iter().find(|(k, _)| k.as_str() == "default"))
            .and_then(|(_, v)| named(v))
    };
    match scene {
        Scene::Silent => None,
        Scene::Menu => named(&score.menu),
        Scene::Battle(faction) => battle(faction),
        Scene::Survival => named(&score.survival).or_else(|| battle("default")),
    }
}

/// Equal-power fade: the gain at `p` of the way in. A song fading in at `p` and one
/// fading out at `1 - p` always sum to the same power, so a cross-fade has no dip.
pub fn fade_gain(p: f32) -> f32 {
    (p.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2).sin()
}

/// Where a song is in its fade: `level` 0..1 moves by `step` per frame (up fading in,
/// down fading out); the gain is `fade_gain(level)`.
#[derive(Clone, Copy, Debug)]
struct Fade {
    level: f32,
    step: f32,
}

impl Fade {
    fn per_frame(seconds: f32, rate: f32) -> f32 {
        1.0 / (seconds.max(MIN_FADE) * rate.max(1.0))
    }
    /// Advances `frames`; the gains at the start and the end of them.
    fn advance(&mut self, frames: usize) -> (f32, f32) {
        let from = fade_gain(self.level);
        self.level = (self.level + self.step * frames as f32).clamp(0.0, 1.0);
        (from, fade_gain(self.level))
    }
    fn gone(&self) -> bool {
        self.step <= 0.0 && self.level <= 0.0
    }
}

struct Playing {
    stage: Box<Stage>,
    name: String,
    fade: Fade,
}

impl Playing {
    fn rate(&self) -> f32 {
        self.stage.song().rate()
    }
    /// An ending stopped its song and the ending's moment has rung out.
    fn over(&self) -> bool {
        !self.stage.song().is_playing() && self.stage.moment_playing().is_none()
    }
}

enum DeckMsg {
    /// A new song, faded in over `fade` seconds while the one playing fades out.
    Start { stage: Box<Stage>, name: String, fade: f32 },
    /// A new version of song `name` (hot reload): taken into its running stage.
    Update { name: String, song: Arc<Song> },
    /// A moment over the song playing; `ending` stops the song.
    Moment { name: String, piece: Arc<Song>, ending: bool },
    /// How far songs dip under moments, dB (a new score).
    DuckDepth(f32),
    /// The song playing fades out over this many seconds.
    FadeOut(f32),
}

/// Requests to the loader thread.
enum Req {
    Scene(Scene),
    /// A game event (`score::cue`): its moment, if the score has one.
    Moment(String),
    Finish { victory: bool },
    /// A stage the deck is done with, freed here instead of on the render thread.
    Free(#[allow(dead_code)] Box<Stage>),
}

/// The render thread's side.
struct Deck {
    rx: Receiver<DeckMsg>,
    free: Sender<Req>,
    current: Option<Playing>,
    outgoing: Vec<Playing>,
    scratch: Vec<f32>,
    /// The level applied at the end of the last pass.
    gain: f32,
}

impl Deck {
    fn new(rx: Receiver<DeckMsg>, free: Sender<Req>) -> Deck {
        Deck {
            rx,
            free,
            current: None,
            outgoing: Vec::with_capacity(MAX_OUTGOING + 1),
            scratch: vec![0.0; SCRATCH_FRAMES * 2],
            gain: 1.0,
        }
    }

    /// Something is playing or fading.
    fn busy(&self) -> bool {
        self.current.is_some() || !self.outgoing.is_empty()
    }

    fn retire(&mut self, mut p: Playing, seconds: f32) {
        p.fade.step = -Fade::per_frame(seconds, p.rate());
        if self.outgoing.len() >= MAX_OUTGOING {
            let cut = self.outgoing.remove(0);
            let _ = self.free.send(Req::Free(cut.stage));
        }
        self.outgoing.push(p);
    }

    fn take_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                DeckMsg::Start { stage, name, fade } => {
                    if let Some(old) = self.current.take() {
                        self.retire(old, fade);
                    }
                    let step = Fade::per_frame(fade, stage.song().rate());
                    let level = if fade <= 0.0 { 1.0 } else { 0.0 };
                    self.current = Some(Playing { stage, name, fade: Fade { level, step } });
                }
                DeckMsg::Update { name, song } => {
                    for p in self.current.iter_mut().chain(self.outgoing.iter_mut()) {
                        if p.name == name {
                            p.stage.set_song(song.clone());
                        }
                    }
                }
                DeckMsg::Moment { name, piece, ending } => {
                    if let Some(p) = &mut self.current {
                        p.stage.moment(&name, piece, ending);
                    }
                }
                DeckMsg::DuckDepth(db) => {
                    for p in self.current.iter_mut().chain(self.outgoing.iter_mut()) {
                        p.stage.duck.depth = db;
                    }
                }
                DeckMsg::FadeOut(seconds) => {
                    if let Some(old) = self.current.take() {
                        self.retire(old, seconds);
                    }
                }
            }
        }
    }

    /// Adds the music into interleaved `out` at `target`.
    fn render(&mut self, out: &mut [f32], channels: usize, target: f32) {
        self.take_messages();
        if self.current.as_ref().is_some_and(Playing::over) {
            let done = self.current.take().unwrap();
            self.retire(done, ENDED_FADE);
        }
        if self.current.is_none() && self.outgoing.is_empty() {
            self.gain = target;
            return;
        }
        if target <= 0.0 && self.gain <= 1e-4 {
            self.gain = 0.0;
            return;
        }
        let channels = channels.max(1);
        let frames = out.len() / channels;
        let mut done = 0;
        while done < frames {
            let n = (frames - done).min(SCRATCH_FRAMES);
            let rate = self.current.as_ref().or(self.outgoing.first()).map_or(48_000.0, Playing::rate);
            let g0 = self.gain;
            let g1 = target + (g0 - target) * (-(n as f32) / (GAIN_GLIDE * rate)).exp();
            let (scratch, current, outgoing) = (&mut self.scratch, &mut self.current, &mut self.outgoing);
            for p in current.iter_mut().chain(outgoing.iter_mut()) {
                let (l0, l1) = p.fade.advance(n);
                if l0 <= 0.0 && l1 <= 0.0 {
                    continue;
                }
                let buf = &mut scratch[..n * 2];
                buf.fill(0.0);
                p.stage.render(buf);
                let (a, b) = (g0 * l0, g1 * l1);
                let inv = 1.0 / n as f32;
                for i in 0..n {
                    let g = a + (b - a) * (i as f32 * inv);
                    let (l, r) = (buf[i * 2] * g, buf[i * 2 + 1] * g);
                    let o = &mut out[(done + i) * channels..(done + i + 1) * channels];
                    if channels == 1 {
                        o[0] += (l + r) * 0.5;
                    } else {
                        o[0] += l;
                        o[1] += r;
                    }
                }
            }
            self.gain = g1;
            done += n;
        }
        let mut i = 0;
        while i < self.outgoing.len() {
            if self.outgoing[i].fade.gone() {
                let p = self.outgoing.swap_remove(i);
                let _ = self.free.send(Req::Free(p.stage));
            } else {
                i += 1;
            }
        }
    }

    /// Everything playing is dropped (after a panic in a song).
    fn clear(&mut self) {
        for p in self.current.take().into_iter().chain(self.outgoing.drain(..)) {
            let _ = self.free.send(Req::Free(p.stage));
        }
    }
}

/// What the render thread last saw of the stage playing.
#[derive(Clone, Debug)]
struct DeckStatus {
    status: Status,
    moment: Option<String>,
    song_level: f32,
}

/// What the music is doing, for logs and tuning.
#[derive(Clone, Debug)]
pub struct MusicStatus {
    pub song: String,
    pub section: Option<String>,
    /// The moment sounding over the song, and the song's level under it (1: no dip).
    pub moment: Option<String>,
    pub song_level: f32,
    /// Share of real time the song's engine takes (its own measure).
    pub load: f32,
    /// Share of real time the render thread spends rendering, over the last second
    /// of music, and the most any second has taken.
    pub render_load: f32,
    pub render_peak: f32,
    /// Seconds of music ready in the ring.
    pub ahead: f32,
    /// Callbacks that found the ring short while music played, and frames missed.
    pub underruns: u64,
    pub underrun_frames: u64,
}

/// The music's part of `Shared`.
pub(super) struct Music {
    /// The deck, until the render thread takes it.
    deck: Mutex<Option<Deck>>,
    ring: Ring,
    /// The volume applied to the last frame copied out, f32 bits (audio thread only).
    gain: AtomicU32,
    /// The device rate the render thread last saw.
    rate: AtomicU32,
    /// The deck has something playing; `primed`: and the ring has been filled since.
    active: AtomicBool,
    primed: AtomicBool,
    underruns: AtomicU64,
    underrun_frames: AtomicU64,
    /// Render thread load (f32 bits): last second, and the worst second.
    render_load: AtomicU32,
    render_peak: AtomicU32,
    to_loader: Sender<Req>,
    /// The loader's end, until the loader starts (once the device has a rate).
    loader_rx: Mutex<Option<(Receiver<Req>, Sender<DeckMsg>)>>,
    started: AtomicBool,
    /// The scene last asked for, handed to the loader when it starts.
    scene: Mutex<Option<Scene>>,
    /// The song the newest stage plays (set by the loader), for the status.
    playing: Mutex<Option<(String, Arc<Song>)>>,
    /// Game events that have a moment ready (set by the loader).
    moments: Mutex<Vec<String>>,
    /// The playing stage's status, left by the render thread.
    status: Mutex<Option<DeckStatus>>,
    /// Master times music volume, f32 bits.
    volume: AtomicU32,
    duck: AtomicBool,
    /// A song panicked while rendering: logged once.
    faulted: AtomicBool,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Music {
    pub(super) fn new() -> Music {
        let (to_loader, loader_rx) = mpsc::channel();
        let (to_deck, deck_rx) = mpsc::channel();
        Music {
            deck: Mutex::new(Some(Deck::new(deck_rx, to_loader.clone()))),
            ring: Ring::new(RING_FRAMES),
            gain: AtomicU32::new(0.0f32.to_bits()),
            rate: AtomicU32::new(0),
            active: AtomicBool::new(false),
            primed: AtomicBool::new(false),
            underruns: AtomicU64::new(0),
            underrun_frames: AtomicU64::new(0),
            render_load: AtomicU32::new(0.0f32.to_bits()),
            render_peak: AtomicU32::new(0.0f32.to_bits()),
            to_loader,
            loader_rx: Mutex::new(Some((loader_rx, to_deck))),
            started: AtomicBool::new(false),
            scene: Mutex::new(None),
            playing: Mutex::new(None),
            moments: Mutex::new(Vec::new()),
            status: Mutex::new(None),
            volume: AtomicU32::new(0.0f32.to_bits()),
            duck: AtomicBool::new(false),
            faulted: AtomicBool::new(false),
        }
    }

    /// Adds the music into `out` from the ring, at the volume and duck of the moment.
    /// Runs on the audio thread, before the mixer's soft clip: no lock, no allocation.
    pub(super) fn render(&self, out: &mut [f32], channels: usize) {
        if !self.started.load(Ordering::Relaxed) {
            return;
        }
        let channels = channels.max(1);
        let frames = out.len() / channels;
        if frames == 0 {
            return;
        }
        let duck = if self.duck.load(Ordering::Relaxed) { DUCK_GAIN } else { 1.0 };
        let target = f32::from_bits(self.volume.load(Ordering::Relaxed)) * duck;
        let g0 = f32::from_bits(self.gain.load(Ordering::Relaxed));
        // Music off: the ring is left as it is, and the render thread idles on a full one.
        if target <= 0.0 && g0 <= 1e-4 {
            self.gain.store(0.0f32.to_bits(), Ordering::Relaxed);
            return;
        }
        let rate = self.rate.load(Ordering::Relaxed).max(1) as f32;
        let g1 = target + (g0 - target) * (-(frames as f32) / (GAIN_GLIDE * rate)).exp();
        let inv = 1.0 / frames as f32;
        let got = self.ring.pop(frames, |i, l, r| {
            let g = g0 + (g1 - g0) * (i as f32 * inv);
            let o = &mut out[i * channels..(i + 1) * channels];
            if channels == 1 {
                o[0] += (l + r) * 0.5 * g;
            } else {
                o[0] += l * g;
                o[1] += r * g;
            }
        });
        self.gain.store(g1.to_bits(), Ordering::Relaxed);
        if got < frames && self.active.load(Ordering::Relaxed) && self.primed.load(Ordering::Relaxed) {
            self.underruns.fetch_add(1, Ordering::Relaxed);
            self.underrun_frames.fetch_add((frames - got) as u64, Ordering::Relaxed);
        }
    }

    fn send(&self, req: Req) {
        if self.started.load(Ordering::Relaxed) {
            let _ = self.to_loader.send(req);
        }
    }

    /// Starts the loader once the device has a rate; cheap when it has started or cannot.
    fn ensure_loader(&self, shared: &Arc<Shared>) {
        if self.started.load(Ordering::Relaxed) || shared.rate.load(Ordering::Relaxed) == 0 {
            return;
        }
        let Some((rx, to_deck)) = lock(&self.loader_rx).take() else {
            return;
        };
        let Some(deck) = lock(&self.deck).take() else {
            return;
        };
        self.rate.store(shared.rate.load(Ordering::Relaxed), Ordering::Relaxed);
        let weak = Arc::downgrade(shared);
        let spawned = std::thread::Builder::new()
            .name("mc-music-render".into())
            .spawn(move || render_thread(weak, deck));
        if let Err(e) = spawned {
            log::warn!("music: no render thread ({e}); the game plays without music");
            return;
        }
        let weak = Arc::downgrade(shared);
        let spawned = std::thread::Builder::new()
            .name("mc-music".into())
            .spawn(move || Loader::new(weak, rx, to_deck).run());
        if let Err(e) = spawned {
            log::warn!("music: no loader thread ({e}); the game plays without music");
            return;
        }
        self.started.store(true, Ordering::Relaxed);
        if let Some(scene) = lock(&self.scene).clone() {
            self.send(Req::Scene(scene));
        }
    }
}

impl Audio {
    /// What should play. Call it every frame: only a change does anything.
    pub fn music_scene(&self, scene: Scene) {
        let music = &self.shared.music;
        music.ensure_loader(&self.shared);
        if scene == Scene::Menu {
            // Nothing pauses the front end: a match left while paused does not keep the duck.
            music.duck.store(false, Ordering::Relaxed);
        }
        {
            let mut last = lock(&music.scene);
            if last.as_ref() == Some(&scene) {
                return;
            }
            *last = Some(scene.clone());
        }
        music.send(Req::Scene(scene));
    }

    /// Whether the score has a moment ready for game event `event` (`score::cue`).
    pub fn music_answers(&self, event: &str) -> bool {
        lock(&self.shared.music.moments).iter().any(|e| e == event)
    }

    /// Plays the moment for game event `event` over the song, if there is one; true if so.
    pub fn music_cue(&self, event: &str) -> bool {
        if !self.music_answers(event) {
            return false;
        }
        self.shared.music.send(Req::Moment(event.to_string()));
        true
    }

    /// The match is over: its `victory`/`defeat` moment, or a fade.
    pub fn music_finish(&self, victory: bool) {
        self.shared.music.send(Req::Finish { victory });
    }

    /// The game is paused: the music plays on, 6 dB down.
    pub fn music_duck(&self, duck: bool) {
        self.shared.music.duck.store(duck, Ordering::Relaxed);
    }

    /// The music's linear gain: master volume times music volume.
    pub fn set_music_volume(&self, gain: f32) {
        let gain = if gain.is_finite() { gain.clamp(0.0, 1.0) } else { 0.0 };
        self.shared.music.volume.store(gain.to_bits(), Ordering::Relaxed);
    }

    /// What is playing, for tuning logs; `None` when nothing is.
    pub fn music_status(&self) -> Option<MusicStatus> {
        let music = &self.shared.music;
        let deck = lock(&music.status).clone()?;
        let playing = lock(&music.playing);
        let (song, s) = playing.as_ref()?;
        Some(MusicStatus {
            song: song.clone(),
            section: deck.status.section.and_then(|i| s.sections.get(i)).map(|x| x.name.clone()),
            moment: deck.moment,
            song_level: deck.song_level,
            load: deck.status.load,
            render_load: f32::from_bits(music.render_load.load(Ordering::Relaxed)),
            render_peak: f32::from_bits(music.render_peak.load(Ordering::Relaxed)),
            ahead: music.ring.len() as f32 / music.rate.load(Ordering::Relaxed).max(1) as f32,
            underruns: music.underruns.load(Ordering::Relaxed),
            underrun_frames: music.underrun_frames.load(Ordering::Relaxed),
        })
    }
}

/// The render thread: keeps the ring `AHEAD` seconds full from the deck. Ends with the `Audio`.
fn render_thread(shared: Weak<Shared>, mut deck: Deck) {
    crate::app::set_this_thread_priority(1);
    let mut chunk = vec![0.0f32; RENDER_FRAMES * 2];
    // Rendering time against music time, per second of music, for the load figures.
    let (mut busy, mut made, mut peak) = (0.0f64, 0.0f64, 0.0f32);
    // `MERIDIAN_MUSIC_LOG`: the render figures every few seconds, front end included.
    let log = std::env::var_os("MERIDIAN_MUSIC_LOG").is_some();
    let mut logged = Instant::now();
    loop {
        let Some(shared) = shared.upgrade() else {
            return;
        };
        let music = &shared.music;
        let rate = shared.rate.load(Ordering::Relaxed);
        if rate != 0 {
            music.rate.store(rate, Ordering::Relaxed);
        }
        let rate = music.rate.load(Ordering::Relaxed).max(1) as f32;
        let ahead = ((rate * AHEAD) as usize).min(music.ring.capacity() - RENDER_FRAMES);
        deck.take_messages();
        if deck.busy() {
            music.active.store(true, Ordering::Relaxed);
            let started = Instant::now();
            let mut frames = 0usize;
            while music.ring.len() < ahead && deck.busy() {
                chunk.fill(0.0);
                let ok = std::panic::catch_unwind(AssertUnwindSafe(|| deck.render(&mut chunk, 2, 1.0))).is_ok();
                if !ok {
                    deck.clear();
                    if !music.faulted.swap(true, Ordering::Relaxed) {
                        log::warn!("music: a song failed while playing and was stopped");
                    }
                    break;
                }
                music.ring.push(&chunk);
                frames += RENDER_FRAMES;
            }
            if music.ring.len() >= ahead {
                music.primed.store(true, Ordering::Relaxed);
            }
            busy += started.elapsed().as_secs_f64();
            made += frames as f64 / rate as f64;
            if made >= 1.0 {
                let load = (busy / made) as f32;
                peak = peak.max(load);
                music.render_load.store(load.to_bits(), Ordering::Relaxed);
                music.render_peak.store(peak.to_bits(), Ordering::Relaxed);
                (busy, made) = (0.0, 0.0);
            }
            if log && logged.elapsed() >= Duration::from_secs(5) {
                logged = Instant::now();
                log::info!(
                    "music render: {:.1}% of real time (worst second {:.1}%), {:.0} ms ahead, {} underruns ({} frames)",
                    f32::from_bits(music.render_load.load(Ordering::Relaxed)) * 100.0,
                    peak * 100.0,
                    music.ring.len() as f32 * 1000.0 / rate,
                    music.underruns.load(Ordering::Relaxed),
                    music.underrun_frames.load(Ordering::Relaxed)
                );
            }
            *lock(&music.status) = deck.current.as_ref().map(|p| DeckStatus {
                status: p.stage.song().status(),
                moment: p.stage.moment_playing().map(str::to_string),
                song_level: p.stage.song_level(),
            });
        } else {
            music.active.store(false, Ordering::Relaxed);
            music.primed.store(false, Ordering::Relaxed);
            *lock(&music.status) = None;
        }
        drop(shared);
        std::thread::sleep(RENDER_IDLE);
    }
}

/// A file's modification time, `None` when it cannot be read.
fn stamp(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// The directory songs are read from: `MERIDIAN_MUSIC_DIR`, else `data/music`.
fn music_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("MERIDIAN_MUSIC_DIR") {
        return Some(PathBuf::from(dir));
    }
    mc_data::Blueprints::locate_data_dir().map(|d| d.join("music"))
}

/// Cache key of a moment, beside the songs': `moments/<name>`.
fn moment_key(name: &str) -> String {
    format!("moments/{name}")
}

/// The loader thread: the score, the songs, the moments, and what plays.
struct Loader {
    shared: Weak<Shared>,
    rx: Receiver<Req>,
    to_deck: Sender<DeckMsg>,
    dir: Option<PathBuf>,
    score: Score,
    /// The score file's time as last read (`Some(None)`: read while missing).
    score_stamp: Option<Option<SystemTime>>,
    /// Parsed songs and moments by key (`name`, `moments/name`), with the file time they were read at.
    pieces: HashMap<String, (Arc<Song>, Option<SystemTime>)>,
    /// Files that failed to parse, at the file time that failed: not tried (or warned about) again until it changes.
    broken: HashMap<String, Option<SystemTime>>,
    scene: Scene,
    /// The song the deck's current stage plays.
    playing: Option<String>,
    /// That song was ended (an ending moment, or faded out): a new scene starts it afresh.
    ended: bool,
    /// The rate the current stage was built at.
    rate: u32,
}

impl Loader {
    fn new(shared: Weak<Shared>, rx: Receiver<Req>, to_deck: Sender<DeckMsg>) -> Loader {
        let mut l = Loader {
            shared,
            rx,
            to_deck,
            dir: None,
            score: Score::default(),
            score_stamp: None,
            pieces: HashMap::new(),
            broken: HashMap::new(),
            scene: Scene::Silent,
            playing: None,
            ended: false,
            rate: 0,
        };
        if l.read_score() {
            l.load_moments();
        }
        l
    }

    fn run(mut self) {
        let mut next_poll = Instant::now() + POLL;
        loop {
            let wait = next_poll.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(wait) {
                Ok(req) => self.handle(req),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            if Instant::now() >= next_poll {
                if self.shared.strong_count() == 0 {
                    return;
                }
                self.poll();
                next_poll = Instant::now() + POLL;
            }
        }
    }

    fn device_rate(&self) -> u32 {
        self.shared.upgrade().map_or(0, |s| s.rate.load(Ordering::Relaxed))
    }

    fn publish(&self) {
        if let Some(shared) = self.shared.upgrade() {
            let now = self
                .playing
                .as_ref()
                .filter(|_| !self.ended)
                .and_then(|n| self.pieces.get(n).map(|(s, _)| (n.clone(), s.clone())));
            *lock(&shared.music.playing) = now;
        }
    }

    /// Tells the game which events have a moment ready.
    fn publish_moments(&self) {
        if let Some(shared) = self.shared.upgrade() {
            *lock(&shared.music.moments) = self
                .score
                .moments
                .iter()
                .filter(|(_, name)| self.pieces.contains_key(&moment_key(name)))
                .map(|(event, _)| event.clone())
                .collect();
        }
    }

    fn fade(&self) -> f32 {
        if self.score.fade.is_finite() {
            self.score.fade.max(0.0)
        } else {
            3.0
        }
    }

    fn duck_db(&self) -> f32 {
        if self.score.duck_db.is_finite() {
            self.score.duck_db.min(0.0)
        } else {
            -16.0
        }
    }

    fn handle(&mut self, req: Req) {
        match req {
            Req::Scene(scene) => {
                if scene != self.scene {
                    self.scene = scene;
                    self.follow_scene();
                }
            }
            Req::Moment(event) => {
                self.moment(&event);
            }
            Req::Finish { victory } => {
                if self.playing.is_none() || self.ended {
                    return;
                }
                let event = if victory { score::cue::VICTORY } else { score::cue::DEFEAT };
                if !self.moment(event) {
                    let _ = self.to_deck.send(DeckMsg::FadeOut(self.fade().max(1.0)));
                    self.ended = true;
                    self.publish();
                }
            }
            Req::Free(_) => {}
        }
    }

    /// Plays `event`'s moment over the song playing; false if there is none to play.
    fn moment(&mut self, event: &str) -> bool {
        if self.playing.is_none() || self.ended {
            return false;
        }
        let Some((name, ending)) = self.score.moment_for(event).map(|(n, e)| (n.to_string(), e)) else {
            return false;
        };
        let Some(piece) = self.piece(true, &name) else {
            return false;
        };
        let _ = self.to_deck.send(DeckMsg::Moment { name, piece, ending });
        if ending {
            self.ended = true;
            self.publish();
        }
        true
    }

    /// Plays what the scene calls for: carries on when that is the song already playing.
    fn follow_scene(&mut self) {
        match song_for(&self.score, &self.scene) {
            None => self.fade_out(),
            Some(name) if self.playing.as_ref() == Some(&name) && !self.ended => {}
            Some(name) => self.start(name, self.fade()),
        }
    }

    fn fade_out(&mut self) {
        if self.playing.take().is_some() && !self.ended {
            let _ = self.to_deck.send(DeckMsg::FadeOut(self.fade()));
        }
        self.ended = false;
        self.publish();
    }

    fn start(&mut self, name: String, fade: f32) {
        let rate = self.device_rate();
        let Some(song) = self.piece(false, &name).filter(|_| rate != 0) else {
            self.fade_out();
            return;
        };
        let mut stage = Stage::new(rate as f32, song);
        stage.duck.depth = self.duck_db();
        stage.play();
        let _ = self.to_deck.send(DeckMsg::Start { stage: Box::new(stage), name: name.clone(), fade });
        log::info!("music: {name}");
        self.playing = Some(name);
        self.ended = false;
        self.rate = rate;
        self.publish();
    }

    /// A song (or with `moment`, a moment), parsed; read again only when its file changed.
    fn piece(&mut self, moment: bool, name: &str) -> Option<Arc<Song>> {
        let dir = self.dir.as_ref()?;
        let (key, path) = if moment {
            (moment_key(name), dir.join("moments").join(format!("{name}.ron")))
        } else {
            (name.to_string(), dir.join(format!("{name}.ron")))
        };
        let at = stamp(&path);
        if let Some((song, when)) = self.pieces.get(&key) {
            if *when == at {
                return Some(song.clone());
            }
        }
        if self.broken.get(&key) == Some(&at) {
            return self.pieces.get(&key).map(|(s, _)| s.clone());
        }
        match Song::load(&path) {
            Ok(song) => {
                for p in song.problems() {
                    log::warn!("music: {key}: {p}");
                }
                self.broken.remove(&key);
                let song = Arc::new(song);
                self.pieces.insert(key, (song.clone(), at));
                Some(song)
            }
            Err(e) => {
                log::warn!("music: {e}");
                self.broken.insert(key.clone(), at);
                // An older good version keeps playing.
                self.pieces.get(&key).map(|(s, _)| s.clone())
            }
        }
    }

    /// Every moment the score names, read now so none is parsed when its event comes;
    /// changed ones are read again. True if any was (re)loaded.
    fn load_moments(&mut self) -> bool {
        let names: Vec<String> = self.score.moments.values().cloned().collect();
        let mut any = false;
        for name in names {
            let key = moment_key(&name);
            let before = self.pieces.get(&key).map(|(s, _)| s.clone());
            if let Some(now) = self.piece(true, &name) {
                if !before.is_some_and(|b| Arc::ptr_eq(&b, &now)) {
                    any = true;
                }
            }
        }
        self.publish_moments();
        any
    }

    /// Reads the score if it changed. True if a new one was taken.
    fn read_score(&mut self) -> bool {
        if self.dir.as_ref().is_none_or(|d| !d.is_dir()) {
            self.dir = music_dir().filter(|d| d.is_dir());
        }
        let Some(dir) = self.dir.clone() else {
            if self.score_stamp.is_none() {
                log::info!("music: no data/music directory; playing without music");
                self.score_stamp = Some(None);
            }
            return false;
        };
        let at = stamp(&dir.join("score.ron"));
        if self.score_stamp == Some(at) {
            return false;
        }
        self.score_stamp = Some(at);
        match Score::load(&dir) {
            Ok(s) => {
                self.score = s;
                true
            }
            Err(e) => {
                log::warn!("music: {e}");
                false
            }
        }
    }

    /// Once a second: the score, the moments, the playing song's file, and the device's rate.
    fn poll(&mut self) {
        if self.read_score() {
            let _ = self.to_deck.send(DeckMsg::DuckDepth(self.duck_db()));
            // The score may name another song for this scene now.
            self.follow_scene();
        }
        if self.load_moments() {
            log::info!("music: moments reloaded");
        }
        let Some(name) = self.playing.clone().filter(|_| !self.ended) else {
            return;
        };
        if let Some(dir) = &self.dir {
            let at = stamp(&dir.join(format!("{name}.ron")));
            let known = self.pieces.get(&name).map(|(_, w)| *w);
            if known.is_some_and(|w| w != at) && self.broken.get(&name) != Some(&at) {
                let before = self.pieces.get(&name).map(|(s, _)| s.clone());
                if let Some(song) = self.piece(false, &name) {
                    if !before.is_some_and(|b| Arc::ptr_eq(&b, &song)) {
                        log::info!("music: {name} changed on disk, reloaded");
                        let _ = self.to_deck.send(DeckMsg::Update { name: name.clone(), song });
                        self.publish();
                    }
                }
            }
        }
        let rate = self.device_rate();
        if rate != 0 && rate != self.rate {
            // A new device: the stage is rebuilt at its rate.
            self.start(name, MIN_FADE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_music::patch::{Instrument, Synth};
    use mc_music::{Clip, Note, Pattern, Section, Track};

    fn score() -> Score {
        ron::from_str(
            r#"(menu: "calm", battle: {"Aster": "reach", "default": "war"}, survival: "", fade: 2.0)"#,
        )
        .unwrap()
    }

    #[test]
    fn music_song_for_each_scene() {
        let s = score();
        assert_eq!(song_for(&s, &Scene::Menu).as_deref(), Some("calm"));
        assert_eq!(song_for(&s, &Scene::Battle("Aster".into())).as_deref(), Some("reach"));
        assert_eq!(song_for(&s, &Scene::Battle("aster".into())).as_deref(), Some("reach"));
        assert_eq!(song_for(&s, &Scene::Battle("Naga".into())).as_deref(), Some("war"));
        // No survival song: the default battle song.
        assert_eq!(song_for(&s, &Scene::Survival).as_deref(), Some("war"));
        assert_eq!(song_for(&s, &Scene::Silent), None);
        // A missing score plays nothing anywhere.
        let none = Score::default();
        for scene in [Scene::Menu, Scene::Battle("Aster".into()), Scene::Survival] {
            assert_eq!(song_for(&none, &scene), None);
        }
    }

    #[test]
    fn music_crossfade_is_equal_power() {
        for i in 0..=20 {
            let p = i as f32 / 20.0;
            let power = fade_gain(p).powi(2) + fade_gain(1.0 - p).powi(2);
            assert!((power - 1.0).abs() < 1e-5, "p {p}: {power}");
        }
        assert_eq!(fade_gain(0.0), 0.0);
        assert!((fade_gain(1.0) - 1.0).abs() < 1e-6);
        // In and out moving together stay equal power the whole way.
        let rate = 48_000.0;
        let mut fin = Fade { level: 0.0, step: Fade::per_frame(2.0, rate) };
        let mut fout = Fade { level: 1.0, step: -Fade::per_frame(2.0, rate) };
        let mut frames = 0;
        while !fout.gone() {
            let (_, a) = fin.advance(1024);
            let (_, b) = fout.advance(1024);
            assert!((a * a + b * b - 1.0).abs() < 1e-3);
            frames += 1024;
        }
        assert!((frames as f32 / rate - 2.0).abs() < 0.05, "took {frames} frames");
        assert_eq!(fin.level, 1.0);
    }

    /// A song of one held note of `key`, `bars` long, looping.
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
        s.patterns.push(Pattern { name: "p".into(), beats: 4, notes: vec![Note(0, 380, key, 100)], automation: vec![] });
        s.sections.push(Section {
            name: "s".into(),
            bars,
            kind: Default::default(),
            intensity: (0.0, 1.0),
            next: vec![],
            exit_every: 0,
            clips: vec![Clip { track: "a".into(), pattern: "p".into(), at: 0, times: 0, transpose: 0 }],
        });
        s.arrangement = vec!["s".into()];
        s
    }

    fn stage() -> Box<Stage> {
        let mut s = Stage::new(48_000.0, Arc::new(Song::empty("t")));
        s.play();
        Box::new(s)
    }

    fn tone_stage() -> Box<Stage> {
        let mut s = Stage::new(48_000.0, Arc::new(tone(57, 2)));
        s.play();
        Box::new(s)
    }

    /// Renders `secs` through the deck in 1024-frame blocks.
    fn run(deck: &mut Deck, secs: f32) {
        let mut out = vec![0.0f32; 2048];
        for _ in 0..(secs * 48_000.0 / 1024.0) as usize {
            out.fill(0.0);
            deck.render(&mut out, 2, 1.0);
        }
    }

    #[test]
    fn music_deck_crossfades_and_frees_the_old_song() {
        let (to_deck, rx) = mpsc::channel();
        let (free, freed) = mpsc::channel();
        let mut deck = Deck::new(rx, free);
        let mut out = vec![0.0f32; 2048];
        to_deck.send(DeckMsg::Start { stage: stage(), name: "a".into(), fade: 0.5 }).unwrap();
        for _ in 0..30 {
            deck.render(&mut out, 2, 1.0);
        }
        assert_eq!(deck.current.as_ref().unwrap().name, "a");
        assert_eq!(deck.current.as_ref().unwrap().fade.level, 1.0);
        to_deck.send(DeckMsg::Start { stage: stage(), name: "b".into(), fade: 0.5 }).unwrap();
        deck.render(&mut out, 2, 1.0);
        assert_eq!(deck.current.as_ref().unwrap().name, "b");
        assert_eq!(deck.outgoing.len(), 1);
        // Half a second later the old song is gone, freed off the render thread.
        for _ in 0..(48_000 / 1024 / 2 + 2) {
            deck.render(&mut out, 2, 1.0);
        }
        assert!(deck.outgoing.is_empty());
        assert!(matches!(freed.try_recv(), Ok(Req::Free(_))));
        assert!(deck.current.as_ref().unwrap().fade.level >= 1.0);
        // Rapid scene flipping never piles up more than a few fading songs.
        for i in 0..10 {
            to_deck.send(DeckMsg::Start { stage: stage(), name: format!("x{i}"), fade: 5.0 }).unwrap();
        }
        deck.render(&mut out, 2, 1.0);
        assert!(deck.outgoing.len() <= MAX_OUTGOING);
        // A fade out empties the deck.
        to_deck.send(DeckMsg::FadeOut(0.1)).unwrap();
        for _ in 0..20 {
            deck.render(&mut out, 1, 1.0);
        }
        assert!(deck.current.is_none() && deck.outgoing.is_empty());
    }

    #[test]
    fn music_volume_glides_and_ducks() {
        let (to_deck, rx) = mpsc::channel();
        let (free, _freed) = mpsc::channel();
        let mut deck = Deck::new(rx, free);
        to_deck.send(DeckMsg::Start { stage: stage(), name: "a".into(), fade: 0.0 }).unwrap();
        let mut out = vec![0.0f32; 2048];
        deck.render(&mut out, 2, 1.0);
        for _ in 0..40 {
            deck.render(&mut out, 2, DUCK_GAIN);
        }
        assert!((deck.gain - DUCK_GAIN).abs() < 0.01, "{}", deck.gain);
    }

    #[test]
    fn music_a_moment_dips_the_song_and_it_comes_back() {
        let (to_deck, rx) = mpsc::channel();
        let (free, _freed) = mpsc::channel();
        let mut deck = Deck::new(rx, free);
        to_deck.send(DeckMsg::Start { stage: tone_stage(), name: "song".into(), fade: 0.0 }).unwrap();
        to_deck.send(DeckMsg::DuckDepth(-20.0)).unwrap();
        run(&mut deck, 1.0);
        let piece = Arc::new(tone(69, 1));
        to_deck.send(DeckMsg::Moment { name: "nuke".into(), piece, ending: false }).unwrap();
        run(&mut deck, 1.5);
        let st = &deck.current.as_ref().unwrap().stage;
        assert_eq!(st.moment_playing(), Some("nuke"));
        assert!(st.song_level() < 0.15, "dipped: {}", st.song_level());
        // The moment rings out, the song comes back where it was and plays on.
        run(&mut deck, 9.0);
        let st = &deck.current.as_ref().unwrap().stage;
        assert_eq!(st.moment_playing(), None);
        assert!(st.song_level() > 0.95, "came back: {}", st.song_level());
        assert!(st.song().is_playing());
    }

    #[test]
    fn music_an_ending_stops_the_song_and_leaves_the_deck() {
        let (to_deck, rx) = mpsc::channel();
        let (free, _freed) = mpsc::channel();
        let mut deck = Deck::new(rx, free);
        to_deck.send(DeckMsg::Start { stage: tone_stage(), name: "song".into(), fade: 0.0 }).unwrap();
        run(&mut deck, 1.0);
        let piece = Arc::new(tone(60, 1));
        to_deck.send(DeckMsg::Moment { name: "victory".into(), piece, ending: true }).unwrap();
        run(&mut deck, 1.0);
        assert!(deck.busy(), "the ending plays out");
        // Once the ending has rung out there is nothing left to render.
        run(&mut deck, 10.0);
        assert!(!deck.busy());
    }

    #[test]
    fn music_silent_audio_never_loads() {
        let audio = Audio::silent();
        audio.music_scene(Scene::Menu);
        assert!(!audio.shared.music.started.load(Ordering::Relaxed));
        assert!(!audio.music_cue(score::cue::NUKE));
        let mut out = vec![0.0; 64];
        audio.shared.render(&mut out, 2);
        assert!(out.iter().all(|s| *s == 0.0));
    }

    /// Drains what the loader sent the deck, as short labels.
    fn sent(rx: &Receiver<DeckMsg>) -> Vec<String> {
        rx.try_iter()
            .map(|m| match m {
                DeckMsg::Start { name, .. } => format!("start {name}"),
                DeckMsg::Update { name, .. } => format!("update {name}"),
                DeckMsg::FadeOut(_) => "fade".into(),
                DeckMsg::Moment { name, ending, .. } => {
                    format!("moment {name}{}", if ending { " (ending)" } else { "" })
                }
                DeckMsg::DuckDepth(db) => format!("duck {db}"),
            })
            .collect()
    }

    /// A music folder in the temp dir with songs `one` and `two`, a broken song, and
    /// moments for nuke (`boom`) and victory (`won`, an ending).
    fn folder(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mc-music-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("moments")).unwrap();
        Song::empty("One").save(&dir.join("one.ron")).unwrap();
        Song::empty("Two").save(&dir.join("two.ron")).unwrap();
        std::fs::write(dir.join("broken.ron"), "(name: ").unwrap();
        Song::empty("Boom").save(&dir.join("moments").join("boom.ron")).unwrap();
        Song::empty("Won").save(&dir.join("moments").join("won.ron")).unwrap();
        std::fs::write(
            dir.join("score.ron"),
            r#"(menu: "one", battle: {"Aster": "one", "Naga": "two", "Precursor": "broken"}, survival: "missing",
                fade: 1.0, moments: {"nuke": "boom", "victory": "won", "titan": "absent"}, endings: ["victory"],
                duck_db: -12.0)"#,
        )
        .unwrap();
        dir
    }

    fn loader(dir: &std::path::Path, shared: &Arc<Shared>) -> (Loader, Receiver<DeckMsg>) {
        shared.rate.store(48_000, Ordering::Relaxed);
        let (_to_loader, rx) = mpsc::channel();
        let (to_deck, deck) = mpsc::channel();
        let mut l = Loader::new(Arc::downgrade(shared), rx, to_deck);
        l.dir = Some(dir.to_path_buf());
        l.score_stamp = None;
        assert!(l.read_score());
        l.load_moments();
        (l, deck)
    }

    fn shared() -> Arc<Shared> {
        Arc::new(Shared::new(
            super::super::Volumes { master: 0.0, interface: 0.0, effects: 0.0, weather: 0.0 },
            mc_data::SoundLibrary::default(),
        ))
    }

    #[test]
    fn music_loader_follows_the_game_and_reloads() {
        let dir = folder("follow");
        let shared = shared();
        let (mut l, deck) = loader(&dir, &shared);

        l.handle(Req::Scene(Scene::Menu));
        assert_eq!(sent(&deck), ["start one"]);
        // Into a match whose song is the menu's: it carries on, no cross-fade from itself.
        l.handle(Req::Scene(Scene::Battle("Aster".into())));
        assert!(sent(&deck).is_empty());
        // Won: the victory moment, an ending. Then back to the menu, where the song starts afresh.
        l.handle(Req::Finish { victory: true });
        assert_eq!(sent(&deck), ["moment won (ending)"]);
        l.handle(Req::Finish { victory: true });
        assert!(sent(&deck).is_empty());
        l.handle(Req::Scene(Scene::Menu));
        assert_eq!(sent(&deck), ["start one"]);
        // Another faction's song cross-fades in; lost with no defeat moment: a fade.
        l.handle(Req::Scene(Scene::Battle("Naga".into())));
        assert_eq!(sent(&deck), ["start two"]);
        l.handle(Req::Finish { victory: false });
        assert_eq!(sent(&deck), ["fade"]);
        // A song that does not parse, or is missing: silence, not a crash.
        l.handle(Req::Scene(Scene::Menu));
        l.handle(Req::Scene(Scene::Battle("Precursor".into())));
        assert_eq!(sent(&deck), ["start one", "fade"]);
        l.handle(Req::Scene(Scene::Survival));
        assert!(sent(&deck).is_empty());
        // Hot reload: the playing song's file changes, its stage takes the new version.
        l.handle(Req::Scene(Scene::Menu));
        assert_eq!(sent(&deck), ["start one"]);
        let mut changed = Song::empty("One");
        changed.tempo = 90.0;
        std::thread::sleep(Duration::from_millis(20));
        changed.save(&dir.join("one.ron")).unwrap();
        if stamp(&dir.join("one.ron")) != l.pieces["one"].1 {
            l.poll();
            assert_eq!(sent(&deck), ["update one"]);
            assert_eq!(l.pieces["one"].0.tempo, 90.0);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn music_moments_play_on_events_and_hot_reload() {
        let dir = folder("moments");
        let shared = shared();
        let (mut l, deck) = loader(&dir, &shared);
        let audio_knows = |e: &str| lock(&shared.music.moments).iter().any(|x| x == e);
        // Loaded with the score, before any event: the game knows which it can cue.
        assert!(audio_knows("nuke") && audio_knows("victory"));
        assert!(!audio_knows("titan"), "its file is missing");
        assert!(!audio_knows("wave"), "no moment in the score");
        // Nothing playing: a moment has nothing to dip.
        l.handle(Req::Moment("nuke".into()));
        assert!(sent(&deck).is_empty());
        l.handle(Req::Scene(Scene::Battle("Aster".into())));
        assert_eq!(sent(&deck), ["start one"]);
        l.handle(Req::Moment("nuke".into()));
        assert_eq!(sent(&deck), ["moment boom"]);
        // Unknown events, and events whose moment is missing, do nothing.
        l.handle(Req::Moment("wave".into()));
        l.handle(Req::Moment("titan".into()));
        l.handle(Req::Moment("no_such_event".into()));
        assert!(sent(&deck).is_empty());
        // The song plays on after a moment that is not an ending.
        l.handle(Req::Moment("nuke".into()));
        assert_eq!(sent(&deck), ["moment boom"]);
        // A moment edited on disk is read again, and the next event plays the new one.
        let before = l.pieces[&moment_key("boom")].0.clone();
        let mut changed = Song::empty("Boom");
        changed.tempo = 150.0;
        std::thread::sleep(Duration::from_millis(20));
        changed.save(&dir.join("moments").join("boom.ron")).unwrap();
        if stamp(&dir.join("moments").join("boom.ron")) != l.pieces[&moment_key("boom")].1 {
            l.poll();
            let now = &l.pieces[&moment_key("boom")].0;
            assert!(!Arc::ptr_eq(&before, now));
            assert_eq!(now.tempo, 150.0);
        }
        // The ending ends it: no further moments until a new song starts.
        l.handle(Req::Moment("victory".into()));
        assert_eq!(sent(&deck), ["moment won (ending)"]);
        l.handle(Req::Moment("nuke".into()));
        assert!(sent(&deck).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The real score, when there is one: it parses, plays straight through, dips under
    /// a moment, and makes finite, non-silent sound.
    #[test]
    fn music_the_real_score_plays() {
        let Some(dir) = music_dir().filter(|d| d.join("score.ron").is_file()) else {
            return;
        };
        let score = match Score::load(&dir) {
            Ok(s) => s,
            Err(e) => panic!("{e}"),
        };
        let Some(name) = song_for(&score, &Scene::Menu) else { return };
        let song = match score::load_song(&dir, &name) {
            Ok(s) => Arc::new(s),
            Err(e) => {
                // The song is being written in another session: say so, do not fail here.
                eprintln!("music: {e}");
                return;
            }
        };
        let (to_deck, rx) = mpsc::channel();
        let (free, _freed) = mpsc::channel();
        let mut deck = Deck::new(rx, free);
        let mut stage = Stage::new(48_000.0, song);
        stage.play();
        to_deck.send(DeckMsg::Start { stage: Box::new(stage), name, fade: 0.0 }).unwrap();
        let mut out = vec![0.0f32; 2 * 48_000 * 4];
        deck.render(&mut out, 2, 1.0);
        assert!(out.iter().all(|s| s.is_finite()));
        let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak > 1e-3, "four seconds of silence");
        if let Some((moment, ending)) = score.moment_for(score::cue::NUKE) {
            match score::load_moment(&dir, moment) {
                Ok(piece) => {
                    let piece = Arc::new(piece);
                    to_deck.send(DeckMsg::Moment { name: moment.into(), piece, ending }).unwrap();
                    deck.render(&mut out, 2, 1.0);
                    assert!(out.iter().all(|s| s.is_finite()));
                }
                Err(e) => eprintln!("music: {e}"),
            }
        }
    }

    // -- the ring between the render thread and the callback --------------------

    /// Counts this thread's allocations, to prove the callback's path makes none.
    struct Counting;
    thread_local! {
        static ALLOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }
    unsafe impl std::alloc::GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
            let _ = ALLOCATIONS.try_with(|c| c.set(c.get() + 1));
            unsafe { std::alloc::System.alloc(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
            unsafe { std::alloc::System.dealloc(ptr, layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
            let _ = ALLOCATIONS.try_with(|c| c.set(c.get() + 1));
            unsafe { std::alloc::System.realloc(ptr, layout, size) }
        }
    }
    #[global_allocator]
    static COUNTING: Counting = Counting;

    /// A `Music` as the callback sees it once started, playing at full volume.
    fn playing_music() -> Music {
        let m = Music::new();
        m.started.store(true, Ordering::Relaxed);
        m.rate.store(48_000, Ordering::Relaxed);
        m.volume.store(1.0f32.to_bits(), Ordering::Relaxed);
        m.gain.store(1.0f32.to_bits(), Ordering::Relaxed);
        m.active.store(true, Ordering::Relaxed);
        m.primed.store(true, Ordering::Relaxed);
        m
    }

    #[test]
    fn music_ring_keeps_order_and_never_overfills() {
        let ring = Ring::new(1000);
        assert_eq!(ring.capacity(), 1024);
        let frames: Vec<f32> = (0..3000).flat_map(|i| [i as f32, -(i as f32)]).collect();
        assert_eq!(ring.push(&frames), 1024);
        assert_eq!(ring.push(&frames), 0);
        let mut next = 0.0;
        // Round and round the wrap: what comes out is what went in, in order.
        for _ in 0..50 {
            let got = ring.pop(700, |_, l, r| {
                assert_eq!((l, r), (next, -next));
                next += 1.0;
            });
            assert!(got > 0);
            let from = next as usize + ring.len();
            ring.push(&frames[from * 2..(from + 700).min(3000) * 2]);
            if from + 700 >= 3000 {
                break;
            }
        }
    }

    #[test]
    fn music_ring_steady_state_has_no_underruns() {
        let m = playing_music();
        let ahead = (48_000.0 * AHEAD) as usize;
        let mut chunk = vec![0.0f32; RENDER_FRAMES * 2];
        let (mut made, mut heard) = (0u32, 0u32);
        let mut out = vec![0.0f32; 480 * 2];
        // A render thread that tops the ring up between callbacks, a device taking 10 ms at a time.
        for _ in 0..3000 {
            while m.ring.len() < ahead {
                for f in chunk.chunks_mut(2) {
                    f[0] = (made % 1000) as f32 / 1000.0;
                    f[1] = f[0];
                    made += 1;
                }
                m.ring.push(&chunk);
            }
            out.fill(0.0);
            m.render(&mut out, 2);
            for f in out.chunks(2) {
                assert!((f[0] - (heard % 1000) as f32 / 1000.0).abs() < 1e-6);
                heard += 1;
            }
        }
        assert_eq!(m.underruns.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn music_ring_underrun_plays_silence_and_is_counted() {
        let m = playing_music();
        m.ring.push(&[0.5f32; 200]);
        let mut out = vec![0.0f32; 480 * 2];
        m.render(&mut out, 2);
        assert!(out[..200].iter().all(|&s| (s - 0.5).abs() < 1e-6));
        assert!(out[200..].iter().all(|&s| s == 0.0));
        assert_eq!(m.underruns.load(Ordering::Relaxed), 1);
        assert_eq!(m.underrun_frames.load(Ordering::Relaxed), 380);
        // Nothing playing (between songs, or before the first): an empty ring is no underrun.
        m.active.store(false, Ordering::Relaxed);
        m.render(&mut out, 2);
        assert_eq!(m.underruns.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn music_callback_does_not_allocate() {
        let m = playing_music();
        let mut out = vec![0.0f32; 512 * 2];
        let mut mono = vec![0.0f32; 512];
        let chunk = vec![0.25f32; 4096 * 2];
        let before = ALLOCATIONS.with(|c| c.get());
        for i in 0..200 {
            m.ring.push(&chunk);
            m.duck.store(i % 2 == 0, Ordering::Relaxed);
            m.render(&mut out, 2);
            m.render(&mut mono, 1);
        }
        // Underruns too.
        for _ in 0..50 {
            m.render(&mut out, 2);
        }
        assert_eq!(ALLOCATIONS.with(|c| c.get()), before);
    }

    #[test]
    fn music_volume_and_duck_act_at_copy_time() {
        let m = playing_music();
        m.ring.push(&vec![1.0f32; 48_000 * 2]);
        let mut out = vec![0.0f32; 480 * 2];
        m.duck.store(true, Ordering::Relaxed);
        out.fill(0.0);
        m.render(&mut out, 2);
        // The duck starts at once (the ring's 200 ms of music are not waited out)...
        assert!(out[out.len() - 1] < 0.95, "{}", out[out.len() - 1]);
        for _ in 0..30 {
            out.fill(0.0);
            m.render(&mut out, 2);
        }
        // ...and 300 ms on the ring's full-level frames come out 6 dB down.
        assert!((out[out.len() - 1] - DUCK_GAIN).abs() < 0.02, "{}", out[out.len() - 1]);
        m.volume.store(0.0f32.to_bits(), Ordering::Relaxed);
        for _ in 0..40 {
            out.fill(0.0);
            m.render(&mut out, 2);
        }
        assert!(out.iter().all(|s| s.abs() < 1e-3));
    }

    #[test]
    fn music_render_thread_keeps_the_ring_ahead() {
        let shared = Arc::new(Shared::new(
            super::super::Volumes { master: 0.0, interface: 0.0, effects: 0.0, weather: 0.0 },
            mc_data::SoundLibrary::default(),
        ));
        shared.rate.store(48_000, Ordering::Relaxed);
        let music = &shared.music;
        let deck = lock(&music.deck).take().unwrap();
        let (_, to_deck) = lock(&music.loader_rx).take().unwrap();
        music.started.store(true, Ordering::Relaxed);
        music.volume.store(1.0f32.to_bits(), Ordering::Relaxed);
        to_deck.send(DeckMsg::Start { stage: stage(), name: "a".into(), fade: 0.0 }).unwrap();
        let weak = Arc::downgrade(&shared);
        let thread = std::thread::spawn(move || render_thread(weak, deck));
        // Wait for it to fill, then play a second of 10 ms callbacks in real time.
        let begun = Instant::now();
        while !music.primed.load(Ordering::Relaxed) && begun.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(music.primed.load(Ordering::Relaxed), "the ring never filled");
        let mut out = vec![0.0f32; 480 * 2];
        for _ in 0..100 {
            std::thread::sleep(Duration::from_millis(10));
            music.render(&mut out, 2);
        }
        assert_eq!(music.underruns.load(Ordering::Relaxed), 0);
        assert!(music.ring.len() as f32 > 48_000.0 * AHEAD * 0.5);
        drop(shared);
        thread.join().unwrap();
    }
}
