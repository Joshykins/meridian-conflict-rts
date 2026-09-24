//! Sound: a small mixer, an interface set written here in code, and the sound
//! library (`data/sounds`, `mc_data::sounds`: the battle, and what units answer
//! the player with), all synthesised at start-up.
//!
//! Like the models and textures, every sound is made by code: there are no
//! audio files. The bank is rendered once, on a background thread, at the
//! output device's sample rate; playing a sound is pushing a voice that reads
//! one of those buffers. The mixer runs in the audio callback and owns nothing
//! but a short mutex-guarded list, so `play` never blocks on synthesis or I/O.
//!
//! The device backend is `cpal`. On Linux that needs ALSA's headers at build
//! time, so it is behind the `alsa` feature there and the game runs silent
//! without it; `Audio` is the same type either way.

// Without a backend nothing drives the mixer; it is still built and tested.
#![cfg_attr(not(any(not(target_os = "linux"), feature = "alsa")), allow(dead_code))]

pub mod capital;

use mc_data::sounds::{Layer, Sound};
use mc_data::{SoundId, SoundLibrary};
use std::f32::consts::TAU;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};

/// Interface and notification sounds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sfx {
    /// The pointer or keyboard focus moved onto a control.
    Hover,
    /// A control was activated.
    Select,
    /// Leaving a screen, cancelling.
    Back,
    ToggleOn,
    ToggleOff,
    /// One step of a slider or a cycled value.
    Tick,
    /// The control is unavailable or the action was refused.
    Deny,
    /// The match is starting.
    Launch,
    /// Units acknowledged an order.
    Order,
    Victory,
    Defeat,
}

impl Sfx {
    pub const ALL: [Sfx; 11] = [
        Sfx::Hover,
        Sfx::Select,
        Sfx::Back,
        Sfx::ToggleOn,
        Sfx::ToggleOff,
        Sfx::Tick,
        Sfx::Deny,
        Sfx::Launch,
        Sfx::Order,
        Sfx::Victory,
        Sfx::Defeat,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Sfx::Hover => "hover",
            Sfx::Select => "select",
            Sfx::Back => "back",
            Sfx::ToggleOn => "toggle_on",
            Sfx::ToggleOff => "toggle_off",
            Sfx::Tick => "tick",
            Sfx::Deny => "deny",
            Sfx::Launch => "launch",
            Sfx::Order => "order",
            Sfx::Victory => "victory",
            Sfx::Defeat => "defeat",
        }
    }
}

/// Linear gains, 0..=1.
#[derive(Clone, Copy, Debug)]
pub struct Volumes {
    pub master: f32,
    pub interface: f32,
    /// Weapons, impacts, explosions.
    pub effects: f32,
    /// Rain and thunder.
    pub weather: f32,
}

type Frames = Arc<Vec<[f32; 2]>>;

pub struct Bank {
    /// The interface set, by `Sfx`.
    sounds: Vec<Frames>,
    /// The sound library, by `SoundId`.
    world: Vec<Frames>,
}

/// A library loop that is sounding: movement, mostly. A sound may have a few
/// of these at once (a column split across the view); `Audio::set_loops` says
/// how loud and where each is.
struct LoopVoice {
    id: SoundId,
    frames: Frames,
    at: f64,
    /// Frames advanced per output frame: a little off one so two of the same
    /// loop do not lock into one machine.
    rate: f64,
    /// Where `rate` is gliding to, for a gliding sound (`Audio::set_gliding`).
    rate_target: f64,
    gain: [f32; 2],
    target: [f32; 2],
    /// Last pan, so a new wanted loop of this sound takes the nearest voice.
    pan: f32,
    /// On the weather volume, not the effects volume; set by the caller that owns it.
    weather: bool,
}

struct Voice {
    frames: Frames,
    /// Read position in frames; fractional, because `rate` need not be one. Below
    /// zero the voice has not begun: it waits that many output frames.
    at: f64,
    /// Frames advanced per output frame: above one plays higher and shorter.
    rate: f64,
    /// Left and right gain.
    gain: [f32; 2],
    /// A sound in the world, on the effects volume, not an interface sound.
    world: bool,
    /// A world sound on the weather volume instead.
    weather: bool,
    /// Fading out, because the match it belongs to is over.
    released: bool,
}

/// Interface voices at once, and battle voices at once.
const MAX_INTERFACE_VOICES: usize = 24;
const MAX_WORLD_VOICES: usize = 28;

struct Mixer {
    voices: Vec<Voice>,
    loops: Vec<LoopVoice>,
    volumes: Volumes,
    /// Loops whose pitch follows each `set_loops` call instead of staying where it began:
    /// an engine that works harder (capital ships' drives, `capital.rs`).
    gliding: Vec<SoundId>,
}

struct Shared {
    mixer: Mutex<Mixer>,
    /// `None` until the first synthesis is done; replaced when the library is reloaded.
    bank: RwLock<Option<Arc<Bank>>>,
    library: Mutex<Arc<SoundLibrary>>,
    /// Counts library loads, so users of sound ids know when theirs are stale.
    generation: AtomicU32,
    /// The device's sample rate, once known.
    rate: AtomicU32,
    /// The name of the device the stream plays on; empty when there is none.
    device: Mutex<String>,
    /// Set when the system's default output changed or the device went away:
    /// `Audio::follow_device` reopens the stream on the new default.
    reopen: AtomicBool,
}

impl Shared {
    fn new(volumes: Volumes, library: SoundLibrary) -> Shared {
        Shared {
            mixer: Mutex::new(Mixer {
                voices: Vec::new(),
                loops: Vec::new(),
                volumes,
                gliding: Vec::new(),
            }),
            bank: RwLock::new(None),
            library: Mutex::new(Arc::new(library)),
            generation: AtomicU32::new(0),
            rate: AtomicU32::new(0),
            device: Mutex::new(String::new()),
            reopen: AtomicBool::new(false),
        }
    }

    fn mixer(&self) -> MutexGuard<'_, Mixer> {
        self.mixer.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn bank(&self) -> Option<Arc<Bank>> {
        self.bank.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Synthesises the whole bank on a background thread and swaps it in.
    fn synthesise_in_background(self: &Arc<Self>) {
        let rate = self.rate.load(Ordering::Relaxed);
        if rate == 0 {
            return;
        }
        let shared = self.clone();
        std::thread::Builder::new()
            .name("mc-audio-synth".into())
            .spawn(move || {
                let started = std::time::Instant::now();
                let library = shared
                    .library
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                let bank = Arc::new(Bank::synthesise(rate, &library));
                // Loops hold buffers of the bank they were started from, and ids may have moved.
                shared.mixer().loops.clear();
                *shared.bank.write().unwrap_or_else(|e| e.into_inner()) = Some(bank);
                log::info!(
                    "audio: {} sounds ready in {:.0} ms at {rate} Hz",
                    Sfx::ALL.len() + library.sounds.len(),
                    started.elapsed().as_secs_f32() * 1000.0
                );
            })
            .expect("spawn synth thread");
    }
}

impl Shared {
    /// Fills interleaved output. Runs on the audio thread.
    fn render(&self, out: &mut [f32], channels: usize) {
        out.fill(0.0);
        let mut m = self.mixer();
        // A panic here used to kill the WASAPI callback; cpal 0.15 then panics
        // again in Stream::drop. Keep the device thread alive.
        let _ = std::panic::catch_unwind(AssertUnwindSafe(|| m.mix(out, channels)));
    }
}

impl Mixer {
    fn mix(&mut self, out: &mut [f32], channels: usize) {
        let frames = out.len() / channels;
        let mut write = |i: usize, l: f32, r: f32| {
            let o = &mut out[i * channels..(i + 1) * channels];
            if channels == 1 {
                o[0] += (l + r) * 0.5;
            } else {
                o[0] += l;
                o[1] += r;
            }
        };
        let (interface, effects, weather) = (
            self.volumes.master * self.volumes.interface,
            self.volumes.master * self.volumes.effects,
            self.volumes.master * self.volumes.weather,
        );
        for v in &mut self.voices {
            if v.frames.len() < 2 {
                continue;
            }
            let bus = match (v.world, v.weather) {
                (false, _) => interface,
                (true, false) => effects,
                (true, true) => weather,
            };
            let last = v.frames.len() - 1;
            for i in 0..frames {
                if v.at < 0.0 {
                    v.at = (v.at + 1.0).min(0.0);
                    continue;
                }
                let k = v.at as usize;
                if k >= last {
                    break;
                }
                if v.released {
                    // The same glide the loops use, down to nothing.
                    for g in &mut v.gain {
                        *g -= *g * 0.0006;
                    }
                }
                let t = (v.at - k as f64) as f32;
                let (a, b) = (v.frames[k], v.frames[k + 1]);
                write(
                    i,
                    (a[0] + (b[0] - a[0]) * t) * v.gain[0] * bus,
                    (a[1] + (b[1] - a[1]) * t) * v.gain[1] * bus,
                );
                v.at += v.rate;
            }
        }
        self.voices.retain(|v| {
            (v.at < 0.0 || (v.frames.len() >= 2 && (v.at as usize) < v.frames.len() - 1))
                && !(v.released && v.gain[0].max(v.gain[1]) < 1e-4)
        });
        // Loops glide to their new level over a few hundredths of a second, so nothing clicks.
        for l in &mut self.loops {
            if l.frames.is_empty() {
                continue;
            }
            let n = l.frames.len() as f64;
            let bus = if l.weather { weather } else { effects };
            for i in 0..frames {
                for c in 0..2 {
                    l.gain[c] += (l.target[c] - l.gain[c]) * 0.0006;
                }
                // Pitch glides over about half a second: a drive winding up, not a jump.
                l.rate += (l.rate_target - l.rate) * 0.00005;
                let k = l.at as usize % l.frames.len();
                let (a, b) = (l.frames[k], l.frames[(k + 1) % l.frames.len()]);
                let t = (l.at - l.at.floor()) as f32;
                write(
                    i,
                    (a[0] + (b[0] - a[0]) * t) * l.gain[0] * bus,
                    (a[1] + (b[1] - a[1]) * t) * l.gain[1] * bus,
                );
                l.at = (l.at + l.rate).rem_euclid(n);
            }
        }
        self.loops
            .retain(|l| l.target != [0.0; 2] || l.gain[0].max(l.gain[1]) > 1e-4);
        for s in out.iter_mut() {
            // Many voices at once may add up past full scale; bend instead of clipping.
            *s = if s.abs() > 0.8 {
                s.signum() * (0.8 + 0.2 * ((s.abs() - 0.8) / 0.2).tanh())
            } else {
                *s
            };
        }
    }
}

pub struct Audio {
    shared: Arc<Shared>,
    /// Keeps the device stream alive; `None` when there is no device or no backend.
    #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
    _stream: Option<cpal::Stream>,
}

impl Audio {
    /// Opens the default output device. Never fails: without a device (or
    /// without a backend in this build) the game is simply silent.
    pub fn new(volumes: Volumes, library: SoundLibrary) -> Audio {
        let shared = Arc::new(Shared::new(volumes, library));
        #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
        {
            let mut audio = Audio {
                shared,
                _stream: None,
            };
            audio.open();
            device::watch(&audio.shared);
            audio
        }
        #[cfg(not(any(not(target_os = "linux"), feature = "alsa")))]
        {
            log::info!(
                "audio: this build has no sound backend (Linux builds need --features alsa)"
            );
            Audio { shared }
        }
    }

    /// No device at all: for tools that draw the interface without a player present.
    pub fn silent() -> Audio {
        let shared = Arc::new(Shared::new(
            Volumes {
                master: 0.0,
                interface: 0.0,
                effects: 0.0,
                weather: 0.0,
            },
            SoundLibrary::default(),
        ));
        #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
        return Audio {
            shared,
            _stream: None,
        };
        #[cfg(not(any(not(target_os = "linux"), feature = "alsa")))]
        return Audio { shared };
    }

    /// The sound library in use, and a number that changes whenever it is
    /// replaced: ids looked up under one number are no good under the next.
    pub fn library(&self) -> (Arc<SoundLibrary>, u32) {
        (
            self.shared
                .library
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            self.shared.generation.load(Ordering::Relaxed),
        )
    }

    /// Takes a freshly loaded library into use (the test range's reload). The
    /// old sounds keep playing until the new ones are synthesised.
    pub fn set_library(&self, library: SoundLibrary) {
        *self
            .shared
            .library
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Arc::new(library);
        self.shared.generation.fetch_add(1, Ordering::Relaxed);
        self.shared.synthesise_in_background();
    }

    pub fn play(&self, sfx: Sfx) {
        self.play_at(sfx, 1.0);
    }

    pub fn play_at(&self, sfx: Sfx, gain: f32) {
        let Some(bank) = self.shared.bank() else {
            return;
        };
        let mut m = self.shared.mixer();
        // A held key or a fast pointer must not stack up a wall of voices.
        if m.voices.iter().filter(|v| !v.world).count() < MAX_INTERFACE_VOICES {
            m.voices.push(Voice {
                frames: bank.sounds[sfx as usize].clone(),
                at: 0.0,
                rate: 1.0,
                gain: [gain; 2],
                world: false,
                weather: false,
                released: false,
            });
        }
    }

    /// A library sound played to the player alone, as the interface set is: on
    /// the interface volume and from no place in the world (a unit answering a
    /// selection).
    pub fn play_response(&self, sound: SoundId, gain: f32) {
        let Some(bank) = self.shared.bank() else {
            return;
        };
        let Some(frames) = bank.world.get(sound.0 as usize).filter(|f| f.len() > 1) else {
            return;
        };
        let mut m = self.shared.mixer();
        if m.voices.iter().filter(|v| !v.world).count() < MAX_INTERFACE_VOICES {
            m.voices.push(Voice {
                frames: frames.clone(),
                at: 0.0,
                rate: 1.0,
                gain: [gain; 2],
                world: false,
                weather: false,
                released: false,
            });
        }
    }

    /// A sound out in the world. `gain` already carries the distance, `pan`
    /// runs from -1 (left) to 1, `pitch` is a playback rate around one. When
    /// the battle is louder than the mixer has voices for, the quietest voice
    /// makes way, or this sound is dropped if it is the quietest.
    pub fn play_world(&self, sound: SoundId, gain: f32, pan: f32, pitch: f32) {
        self.play_world_after(sound, gain, pan, pitch, 0.0);
    }

    /// [`Self::play_world`], `delay` seconds from now: for something that happens part
    /// of the way through the tick that reported it, like a foot coming down.
    pub fn play_world_after(&self, sound: SoundId, gain: f32, pan: f32, pitch: f32, delay: f32) {
        self.play_in_world(sound, gain, pan, pitch, delay, false);
    }

    /// [`Self::play_world_after`] on the weather volume: thunder.
    pub fn play_weather_after(&self, sound: SoundId, gain: f32, pan: f32, pitch: f32, delay: f32) {
        self.play_in_world(sound, gain, pan, pitch, delay, true);
    }

    fn play_in_world(
        &self,
        sound: SoundId,
        gain: f32,
        pan: f32,
        pitch: f32,
        delay: f32,
        weather: bool,
    ) {
        let Some(bank) = self.shared.bank() else {
            return;
        };
        let Some(frames) = bank.world.get(sound.0 as usize) else {
            return;
        };
        if gain < 0.004 {
            return;
        }
        let (l, r) = self::pan(pan);
        // Equal power leaves the centre 3 dB down on each side; bring it back to unity.
        let gains = [
            gain * l * std::f32::consts::SQRT_2,
            gain * r * std::f32::consts::SQRT_2,
        ];
        let mut m = self.shared.mixer();
        if m.voices.iter().filter(|v| v.world).count() >= MAX_WORLD_VOICES {
            let loudness = |v: &Voice| v.gain[0].max(v.gain[1]);
            let Some(quietest) = (0..m.voices.len())
                .filter(|&i| m.voices[i].world)
                .min_by(|&a, &b| loudness(&m.voices[a]).total_cmp(&loudness(&m.voices[b])))
            else {
                return;
            };
            if loudness(&m.voices[quietest]) >= gain {
                return;
            }
            m.voices.swap_remove(quietest);
        }
        let wait = delay.clamp(0.0, 2.0) as f64 * self.shared.rate.load(Ordering::Relaxed) as f64;
        m.voices.push(Voice {
            frames: frames.clone(),
            at: -wait,
            rate: pitch.clamp(0.5, 2.0) as f64,
            gain: gains,
            world: true,
            weather,
            released: false,
        });
    }

    /// The loops that should be sounding now, as (sound, gain, pan, pitch).
    /// Several wanted rows may share a sound: each is its own voice. A voice
    /// already playing that sound is reused by the nearest pan, so a column
    /// sliding across the view glides instead of spawning a new machine.
    /// Anything not wanted fades out. Call it every tick.
    pub fn set_loops(&self, wanted: &[(SoundId, f32, f32, f32)]) {
        self.set_loops_on(wanted, false);
    }

    /// [`Self::set_loops`] for the loops on the weather volume (rain), which
    /// are wanted apart from the battle's and leave its loops alone.
    pub fn set_weather_loops(&self, wanted: &[(SoundId, f32, f32, f32)]) {
        self.set_loops_on(wanted, true);
    }

    fn set_loops_on(&self, wanted: &[(SoundId, f32, f32, f32)], weather: bool) {
        let Some(bank) = self.shared.bank() else {
            return;
        };
        let mut m = self.shared.mixer();
        let mut assigned = vec![false; m.loops.len()];
        for &(id, gain, pan, pitch) in wanted {
            let Some(frames) = bank.world.get(id.0 as usize).filter(|f| f.len() > 1) else {
                continue;
            };
            let (l, r) = self::pan(pan);
            let target = [
                gain * l * std::f32::consts::SQRT_2,
                gain * r * std::f32::consts::SQRT_2,
            ];
            // Walk the shorter of the two: a new voice used to grow `loops`
            // without `assigned`, and the next row then indexed past it.
            let nearest = (0..assigned.len().min(m.loops.len()))
                .filter(|&i| !assigned[i] && m.loops[i].id == id && m.loops[i].weather == weather)
                .min_by(|&a, &b| {
                    (m.loops[a].pan - pan)
                        .abs()
                        .total_cmp(&(m.loops[b].pan - pan).abs())
                });
            match nearest {
                Some(i) => {
                    assigned[i] = true;
                    if m.gliding.contains(&id) {
                        m.loops[i].rate_target = pitch.clamp(0.5, 2.0) as f64;
                    }
                    m.loops[i].target = target;
                    m.loops[i].pan = pan;
                }
                None => {
                    let n = frames.len() as f64;
                    let siblings = m.loops.iter().filter(|v| v.id == id && v.weather == weather).count() as f64;
                    m.loops.push(LoopVoice {
                        id,
                        frames: frames.clone(),
                        at: (siblings * n * 0.37) % n,
                        rate: pitch.clamp(0.5, 2.0) as f64,
                        rate_target: pitch.clamp(0.5, 2.0) as f64,
                        gain: [0.0; 2],
                        target,
                        pan,
                        weather,
                    });
                    assigned.push(true);
                }
            }
        }
        for (used, l) in assigned.iter().zip(m.loops.iter_mut()) {
            if !used && l.weather == weather {
                l.target = [0.0; 2];
            }
        }
    }

    /// Loops of these sounds follow the pitch each `set_loops` asks for, gliding to it,
    /// instead of keeping the pitch they started at. Replaces the last call's list.
    pub fn set_gliding(&self, sounds: &[SoundId]) {
        self.shared.mixer().gliding = sounds.to_vec();
    }

    /// The match is over: its loops (movement, rain) fade out, sounds still
    /// ringing fade with them, and ones waiting on a delay never start. Loops
    /// are only let go by `set_loops`, which nobody calls once the match is
    /// gone, so without this they would hold into the menu. Interface sounds
    /// are left alone.
    pub fn release_world(&self) {
        let mut m = self.shared.mixer();
        for l in &mut m.loops {
            l.target = [0.0; 2];
        }
        m.voices.retain(|v| !v.world || v.at >= 0.0);
        for v in m.voices.iter_mut().filter(|v| v.world) {
            v.released = true;
        }
    }

    pub fn set_volumes(&self, volumes: Volumes) {
        self.shared.mixer().volumes = volumes;
    }

    /// Moves to the system's default output when it has changed (the player
    /// switched speakers or headphones, or the device was unplugged). cpal
    /// streams stay on the device they were opened on, so this reopens. Cheap
    /// when nothing changed: call it every frame.
    pub fn follow_device(&mut self) {
        #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
        if self.shared.reopen.swap(false, Ordering::Relaxed) {
            self.close();
            self.open();
        }
    }

    /// Opens the default output device; on a new sample rate the bank is
    /// synthesised again, and the game is silent until it is ready.
    #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
    fn open(&mut self) {
        let shared = &self.shared;
        match device::open(shared.clone()) {
            Ok((stream, name, rate)) => {
                *shared.device.lock().unwrap_or_else(|e| e.into_inner()) = name;
                if shared.rate.swap(rate, Ordering::Relaxed) != rate {
                    {
                        let mut m = shared.mixer();
                        m.voices.clear();
                        m.loops.clear();
                    }
                    *shared.bank.write().unwrap_or_else(|e| e.into_inner()) = None;
                    shared.synthesise_in_background();
                }
                self._stream = Some(stream);
            }
            Err(e) => {
                shared.device.lock().unwrap_or_else(|e| e.into_inner()).clear();
                log::warn!("audio: no sound ({e})");
            }
        }
    }

    #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
    fn close(&mut self) {
        if let Some(stream) = self._stream.take() {
            use cpal::traits::StreamTrait;
            let _ = stream.pause();
            // cpal 0.15 WASAPI unwraps IAudioClient::Stop(); a dead callback
            // makes that an Err and a panic on the way out of the process.
            let _ = std::panic::catch_unwind(AssertUnwindSafe(|| drop(stream)));
        }
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
        self.close();
    }
}

/// How a column of movers is heard: a few loops per sound, split across the
/// view and slightly detuned, so they stay a column and not one machine.
/// `movers` is (sound, gain, pan) for each unit covering ground.
pub fn mix_moving(movers: &[(SoundId, f32, f32)]) -> Vec<(SoundId, f32, f32, f32)> {
    const MOVE_GAIN: f32 = 0.26;
    const MOVE_CAP: f32 = 0.36;
    const BIN_EDGE: f32 = 0.28;
    const VOICES_PER_BIN: usize = 2;
    const DETUNE: [f32; 5] = [0.97, 1.03, 0.94, 1.055, 0.92];

    if movers.is_empty() {
        return Vec::new();
    }
    let mut order: Vec<usize> = (0..movers.len()).collect();
    order.sort_by(|&a, &b| movers[a].0 .0.cmp(&movers[b].0 .0));

    let mut out = Vec::new();
    let mut i = 0;
    while i < order.len() {
        let sound = movers[order[i]].0;
        let start = i;
        while i < order.len() && movers[order[i]].0 == sound {
            i += 1;
        }
        let mut bins: [Vec<(f32, f32)>; 3] = Default::default();
        for &k in &order[start..i] {
            let (_, gain, pan) = movers[k];
            let bin = if pan < -BIN_EDGE {
                0
            } else if pan > BIN_EDGE {
                2
            } else {
                1
            };
            bins[bin].push((gain, pan));
        }
        let mut slot = 0usize;
        for bin in bins.iter_mut() {
            if bin.is_empty() {
                continue;
            }
            bin.sort_by(|a, c| c.0.total_cmp(&a.0));
            let near = bin.len().min(VOICES_PER_BIN);
            let extra = bin.len().saturating_sub(near);
            for (v, &(gain, pan)) in bin.iter().take(near).enumerate() {
                let crowd = if v == 0 {
                    1.0 + (extra as f32 * 0.03).min(0.18)
                } else {
                    1.0
                };
                let pitch = if slot == 0 {
                    1.0
                } else {
                    DETUNE[(slot - 1) % DETUNE.len()]
                };
                slot += 1;
                out.push((sound, (gain * MOVE_GAIN * crowd).min(MOVE_CAP), pan, pitch));
            }
        }
    }
    out
}

#[cfg(any(not(target_os = "linux"), feature = "alsa"))]
mod device {
    use super::Shared;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{FromSample, SampleFormat, SizedSample};
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, Weak};
    use std::time::Duration;

    /// Checks the system's default output once a second and flags a reopen
    /// when it is not the device in use. Ends with the `Audio` it serves.
    pub fn watch(shared: &Arc<Shared>) {
        let shared: Weak<Shared> = Arc::downgrade(shared);
        let _ = std::thread::Builder::new()
            .name("mc-audio-device".into())
            .spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                let Some(shared) = shared.upgrade() else {
                    return;
                };
                if shared.reopen.load(Ordering::Relaxed) {
                    continue;
                }
                let now = cpal::default_host()
                    .default_output_device()
                    .map(|d| d.name().unwrap_or_default())
                    .unwrap_or_default();
                let playing = shared.device.lock().unwrap_or_else(|e| e.into_inner()).clone();
                if now != playing {
                    log::info!("audio: default output changed to {now:?}");
                    shared.reopen.store(true, Ordering::Relaxed);
                }
            });
    }

    pub fn open(shared: Arc<Shared>) -> Result<(cpal::Stream, String, u32), String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("no output device")?;
        let config = device.default_output_config().map_err(|e| e.to_string())?;
        let rate = config.sample_rate().0;
        let stream = match config.sample_format() {
            SampleFormat::F32 => build::<f32>(&device, &config.into(), shared),
            SampleFormat::I16 => build::<i16>(&device, &config.into(), shared),
            SampleFormat::U16 => build::<u16>(&device, &config.into(), shared),
            SampleFormat::I32 => build::<i32>(&device, &config.into(), shared),
            other => Err(format!("unsupported sample format {other:?}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        let name = device.name().unwrap_or_default();
        log::info!("audio: {name} at {rate} Hz");
        Ok((stream, name, rate))
    }

    fn build<T: SizedSample + FromSample<f32>>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        shared: Arc<Shared>,
    ) -> Result<cpal::Stream, String> {
        let channels = config.channels as usize;
        let mut mix: Vec<f32> = Vec::new();
        let lost = Arc::downgrade(&shared);
        device
            .build_output_stream(
                config,
                move |out: &mut [T], _| {
                    mix.resize(out.len(), 0.0);
                    shared.render(&mut mix, channels);
                    for (o, s) in out.iter_mut().zip(&mix) {
                        *o = T::from_sample(*s);
                    }
                },
                move |e| {
                    log::warn!("audio: stream error: {e}");
                    // Unplugged: move to whatever the system plays on now.
                    if let (cpal::StreamError::DeviceNotAvailable, Some(s)) = (&e, lost.upgrade()) {
                        s.reopen.store(true, Ordering::Relaxed);
                    }
                },
                None,
            )
            .map_err(|e| e.to_string())
    }
}

// -- synthesis -------------------------------------------------------------------

/// A stereo buffer being built.
struct Buf {
    rate: f32,
    frames: Vec<[f32; 2]>,
}

/// White noise, deterministic so every run sounds the same.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / 8_388_608.0 - 1.0
    }
}

/// A state-variable filter: band-pass and low-pass of the same input.
#[derive(Default)]
struct Svf {
    low: f32,
    band: f32,
}

impl Svf {
    /// Returns `(low, band)`. `q` around 0.5 is wide, 4 and up rings.
    fn step(&mut self, input: f32, cutoff: f32, q: f32, rate: f32) -> (f32, f32) {
        let f = (2.0 * (std::f32::consts::PI * cutoff / rate).sin()).min(0.9);
        self.low += f * self.band;
        let high = input - self.low - self.band / q;
        self.band += f * high;
        (self.low, self.band)
    }
}

/// A one-pole low-pass: 6 dB an octave. Two in a row after the band-pass take
/// the fizz off noise, which the band-pass's gentle skirt lets through.
#[derive(Default)]
struct OnePole(f32);

impl OnePole {
    fn step(&mut self, input: f32, cutoff: f32, rate: f32) -> f32 {
        self.0 += (1.0 - (-TAU * cutoff / rate).exp()) * (input - self.0);
        self.0
    }
}

/// Band-passed noise with the top rolled off above the band: soft air, no hiss.
#[derive(Default)]
struct Air {
    band: Svf,
    smooth: [OnePole; 2],
}

impl Air {
    fn step(&mut self, white: f32, cutoff: f32, q: f32, rate: f32) -> f32 {
        let b = self.band.step(white, cutoff, q, rate).1;
        let top = cutoff * 1.5;
        let once = self.smooth[0].step(b, top, rate);
        self.smooth[1].step(once, top, rate)
    }
}

/// Linear attack, exponential decay.
fn pluck(t: f32, attack: f32, decay: f32) -> f32 {
    if t < 0.0 {
        0.0
    } else if t < attack {
        t / attack
    } else {
        (-(t - attack) / decay).exp()
    }
}

/// Equal-power pan, -1 left to 1 right.
fn pan(p: f32) -> (f32, f32) {
    let a = (p.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    (a.cos(), a.sin())
}

impl Buf {
    fn new(rate: u32, seconds: f32) -> Buf {
        Buf {
            rate: rate as f32,
            frames: vec![[0.0; 2]; (rate as f32 * seconds) as usize],
        }
    }

    /// Adds `f(t)` (mono, `t` from `start`) at a pan position.
    fn add(&mut self, start: f32, position: f32, mut f: impl FnMut(f32) -> f32) {
        let (l, r) = pan(position);
        let first = (start * self.rate) as usize;
        for i in first..self.frames.len() {
            let s = f((i - first) as f32 / self.rate);
            self.frames[i][0] += s * l;
            self.frames[i][1] += s * r;
        }
    }

    /// A sine partial gliding from `f0` to `f1` over `glide` seconds, shaped by `pluck`.
    #[allow(clippy::too_many_arguments)]
    fn tone(
        &mut self,
        start: f32,
        f0: f32,
        f1: f32,
        glide: f32,
        attack: f32,
        decay: f32,
        gain: f32,
        position: f32,
    ) {
        let mut phase = 0.0f32;
        let rate = self.rate;
        self.add(start, position, |t| {
            let k = (t / glide).min(1.0);
            // Ease out, so a glide settles into its note.
            let f = f0 + (f1 - f0) * (1.0 - (1.0 - k) * (1.0 - k));
            phase += TAU * f / rate;
            phase.sin() * pluck(t, attack, decay) * gain
        });
    }

    /// A sine shaken by another at `ratio` times its frequency, `index` deep at first and
    /// falling away over `fade`: bright at the start, a plain note by the end.
    #[allow(clippy::too_many_arguments)]
    fn fm(
        &mut self,
        start: f32,
        f0: f32,
        f1: f32,
        glide: f32,
        ratio: f32,
        index: f32,
        fade: f32,
        attack: f32,
        decay: f32,
        gain: f32,
        position: f32,
    ) {
        let (mut carrier, mut shaker) = (0.0f32, 0.0f32);
        let rate = self.rate;
        self.add(start, position, |t| {
            let k = (t / glide).min(1.0);
            let f = f0 + (f1 - f0) * (1.0 - (1.0 - k) * (1.0 - k));
            carrier = (carrier + TAU * f / rate) % TAU;
            shaker = (shaker + TAU * f * ratio / rate) % TAU;
            (carrier + index * (-t / fade).exp() * shaker.sin()).sin()
                * pluck(t, attack, decay)
                * gain
        });
    }

    /// A burst of band-passed noise.
    #[allow(clippy::too_many_arguments)]
    fn hiss(
        &mut self,
        start: f32,
        cutoff: f32,
        q: f32,
        attack: f32,
        decay: f32,
        gain: f32,
        position: f32,
        seed: u32,
    ) {
        let (mut noise, mut air) = (Noise(seed), Air::default());
        let rate = self.rate;
        self.add(start, position, |t| {
            air.step(noise.next(), cutoff, q, rate) * pluck(t, attack, decay) * gain
        });
    }

    /// A burst of wide noise for the crack and the bark of a gun: a gentle corner
    /// below, a steep one (18 dB an octave) above. Anything left over `high_cut`
    /// is heard as static, not as a bang, so keep that low and the burst short.
    #[allow(clippy::too_many_arguments)]
    fn burst(
        &mut self,
        start: f32,
        low_cut: f32,
        high_cut: f32,
        attack: f32,
        decay: f32,
        gain: f32,
        position: f32,
        seed: u32,
    ) {
        let (mut noise, mut top, mut bottom) = (
            Noise(seed),
            [OnePole::default(), OnePole::default(), OnePole::default()],
            OnePole::default(),
        );
        let rate = self.rate;
        self.add(start, position, |t| {
            let mut bright = noise.next();
            for pole in &mut top {
                bright = pole.step(bright, high_cut, rate);
            }
            (bright - bottom.step(bright, low_cut, rate)) * pluck(t, attack, decay) * gain
        });
    }

    /// Noise through a band that glides from `f0` to `f1` over `glide` seconds:
    /// a report rolling away over the ground, or a shell whistling off.
    #[allow(clippy::too_many_arguments)]
    fn sweep(
        &mut self,
        start: f32,
        f0: f32,
        f1: f32,
        glide: f32,
        q: f32,
        attack: f32,
        decay: f32,
        gain: f32,
        position: f32,
        seed: u32,
    ) {
        let (mut noise, mut band, mut smooth) = (Noise(seed), Svf::default(), OnePole::default());
        let rate = self.rate;
        self.add(start, position, |t| {
            let k = (t / glide).min(1.0);
            // Falls fast at first, like a pitch heard going away.
            let cutoff = f1 + (f0 - f1) * (1.0 - k) * (1.0 - k);
            let b = band.step(noise.next(), cutoff, q, rate).1;
            smooth.step(b, cutoff * 2.0, rate) * pluck(t, attack, decay) * gain
        });
    }

    /// Soft saturation of the whole buffer, `amount` around 1 to 3: the loud
    /// part is squashed against the ceiling and everything under it comes up,
    /// which is most of what makes a bang sound like a bang and not like a drum.
    fn drive(&mut self, amount: f32) {
        let top = self
            .frames
            .iter()
            .flat_map(|f| f.iter())
            .fold(1e-6f32, |m, s| m.max(s.abs()));
        for f in &mut self.frames {
            for s in f {
                *s = (*s / top * amount).tanh() / amount.tanh();
            }
        }
    }

    /// Early reflections: a few cross-fed echoes that put a dry blip in a room.
    fn space(&mut self, amount: f32) {
        let n = self.frames.len();
        for (seconds, gain, swap) in [
            (0.043, 0.30, true),
            (0.089, 0.22, false),
            (0.137, 0.16, true),
            (0.211, 0.11, false),
            (0.307, 0.07, true),
        ] {
            let d = (seconds * self.rate) as usize;
            // Every tap echoes the signal as the previous taps left it.
            let dry = self.frames.clone();
            for i in d..n {
                let f = dry[i - d];
                let (a, b) = if swap { (f[1], f[0]) } else { (f[0], f[1]) };
                self.frames[i][0] += a * gain * amount;
                self.frames[i][1] += b * gain * amount;
            }
        }
    }

    /// Scales to a peak level and folds the last `fold` seconds back over the
    /// first, so the buffer can play round and round without a seam.
    fn finish_loop(mut self, peak: f32, fold: f32) -> Frames {
        let n = (fold * self.rate) as usize;
        let keep = self.frames.len().saturating_sub(n).max(2);
        for i in 0..n.min(keep) {
            let x = i as f32 / n as f32;
            for c in 0..2 {
                self.frames[i][c] = self.frames[i][c] * x + self.frames[keep + i][c] * (1.0 - x);
            }
        }
        self.frames.truncate(keep);
        let top = self
            .frames
            .iter()
            .flat_map(|f| f.iter())
            .fold(1e-6f32, |m, s| m.max(s.abs()));
        for f in &mut self.frames {
            f[0] *= peak / top;
            f[1] *= peak / top;
        }
        Arc::new(self.frames)
    }

    /// Scales to a peak level, and fades the last few milliseconds so a tail
    /// cut short by the buffer's end cannot click.
    fn finish(mut self, peak: f32) -> Frames {
        let top = self
            .frames
            .iter()
            .flat_map(|f| f.iter())
            .fold(1e-6f32, |m, s| m.max(s.abs()));
        let fade = (0.012 * self.rate) as usize;
        let n = self.frames.len();
        for (i, f) in self.frames.iter_mut().enumerate() {
            let edge = ((n - 1 - i) as f32 / fade as f32).min(1.0);
            f[0] *= peak / top * edge;
            f[1] *= peak / top * edge;
        }
        Arc::new(self.frames)
    }
}

impl Bank {
    pub fn synthesise(rate: u32, library: &SoundLibrary) -> Bank {
        Bank {
            sounds: Sfx::ALL.iter().map(|s| sound(*s, rate)).collect(),
            world: library
                .sounds
                .iter()
                .map(|s| from_recipe(s, rate))
                .collect(),
        }
    }

    pub fn world(&self, id: SoundId) -> &[[f32; 2]] {
        &self.world[id.0 as usize]
    }

    pub fn sound(&self, sfx: Sfx) -> &[[f32; 2]] {
        &self.sounds[sfx as usize]
    }
}

/// The interface set shares one voice: soft sine blips around A, a little air
/// from filtered noise, and the same short room on everything.
fn sound(sfx: Sfx, rate: u32) -> Frames {
    match sfx {
        Sfx::Hover => {
            // Heard constantly, so it is the quietest sound in the set: a soft,
            // low tap with a slow attack, nothing bright in it and no noise.
            let mut b = Buf::new(rate, 0.30);
            b.tone(0.0, 622.3, 587.3, 0.04, 0.006, 0.022, 0.8, 0.0);
            b.tone(0.0, 311.1, 293.7, 0.04, 0.008, 0.030, 0.4, 0.0);
            b.space(0.35);
            b.finish(0.12)
        }
        // Into and out of a screen are the hover's tap given a body: up into D
        // going in, down off it coming out. Low and short, with nothing ringing on.
        Sfx::Select => {
            let mut b = Buf::new(rate, 0.45);
            b.tone(0.0, 493.9, 587.3, 0.03, 0.003, 0.035, 0.9, 0.0);
            b.tone(0.0, 246.9, 293.7, 0.03, 0.004, 0.045, 0.5, 0.0);
            b.tone(0.0, 140.0, 85.0, 0.05, 0.002, 0.035, 0.6, 0.0);
            b.hiss(0.0, 1500.0, 1.0, 0.0005, 0.004, 0.35, 0.0, 23);
            b.space(0.45);
            b.finish(0.30)
        }
        Sfx::Back => {
            let mut b = Buf::new(rate, 0.45);
            b.tone(0.0, 587.3, 440.0, 0.05, 0.004, 0.040, 0.9, 0.0);
            b.tone(0.0, 293.7, 220.0, 0.05, 0.005, 0.050, 0.5, 0.0);
            b.tone(0.0, 120.0, 70.0, 0.06, 0.002, 0.040, 0.5, 0.0);
            b.space(0.45);
            b.finish(0.26)
        }
        Sfx::ToggleOn | Sfx::ToggleOff => {
            let mut b = Buf::new(rate, 0.45);
            let (first, second) = if sfx == Sfx::ToggleOn {
                (1174.7, 1760.0)
            } else {
                (1760.0, 1174.7)
            };
            b.tone(0.0, first, first, 0.01, 0.002, 0.030, 0.9, -0.2);
            b.tone(0.055, second, second, 0.01, 0.002, 0.050, 1.0, 0.2);
            b.hiss(0.0, 2600.0, 1.0, 0.0005, 0.005, 0.6, 0.0, 41);
            b.space(0.6);
            b.finish(0.32)
        }
        Sfx::Tick => {
            let mut b = Buf::new(rate, 0.12);
            b.tone(0.0, 2637.0, 2349.3, 0.02, 0.001, 0.009, 0.9, 0.0);
            b.hiss(0.0, 3000.0, 1.5, 0.0005, 0.004, 0.7, 0.0, 53);
            b.space(0.3);
            b.finish(0.16)
        }
        Sfx::Deny => {
            let mut b = Buf::new(rate, 0.55);
            for start in [0.0, 0.11] {
                // A reedy buzz: a few odd harmonics, slightly detuned between the ears.
                for (k, gain) in [(1.0, 1.0), (3.0, 0.45), (5.0, 0.22), (7.0, 0.1)] {
                    b.tone(start, 155.6 * k, 146.8 * k, 0.09, 0.004, 0.045, gain, -0.3);
                    b.tone(start, 156.9 * k, 148.0 * k, 0.09, 0.004, 0.045, gain, 0.3);
                }
            }
            b.space(0.5);
            b.finish(0.34)
        }
        Sfx::Launch => {
            let mut b = Buf::new(rate, 3.2);
            let hit = 0.62;
            // Riser: noise through a band that climbs into the hit.
            {
                let (mut noise, mut air) = (Noise(79), Air::default());
                let r = b.rate;
                b.add(0.0, 0.0, |t| {
                    let k = (t / hit).min(1.0);
                    let gate = if t < hit {
                        k * k
                    } else {
                        (-(t - hit) / 0.03).exp()
                    };
                    air.step(noise.next(), 200.0 * (12.0f32).powf(k), 2.5, r) * gate * 0.8
                });
            }
            b.tone(0.0, 110.0, 220.0, hit, hit * 0.95, 0.05, 0.35, 0.0);
            // The hit: a falling sub, a crack, and an open fifth-and-ninth chord left ringing.
            b.tone(hit, 120.0, 41.2, 0.35, 0.004, 0.55, 1.6, 0.0);
            b.tone(hit, 240.0, 82.4, 0.25, 0.004, 0.22, 0.6, 0.0);
            b.hiss(hit, 1200.0, 0.7, 0.001, 0.09, 1.8, 0.0, 83);
            b.hiss(hit, 3200.0, 0.8, 0.001, 0.30, 0.4, 0.0, 89);
            for (i, (f, gain)) in [
                (220.0, 0.5),
                (329.6, 0.42),
                (440.0, 0.36),
                (493.9, 0.26),
                (659.3, 0.2),
                (880.0, 0.12),
            ]
            .into_iter()
            .enumerate()
            {
                let side = if i % 2 == 0 { -0.45 } else { 0.45 };
                b.tone(hit, f * 0.996, f * 0.996, 0.1, 0.012, 0.85, gain, side);
                b.tone(hit, f * 1.004, f * 1.004, 0.1, 0.012, 0.85, gain, -side);
            }
            b.space(1.0);
            b.finish(0.80)
        }
        Sfx::Order => {
            let mut b = Buf::new(rate, 0.35);
            b.tone(0.0, 987.8, 987.8, 0.01, 0.002, 0.028, 0.8, -0.15);
            b.tone(0.04, 1318.5, 1318.5, 0.01, 0.002, 0.040, 0.9, 0.15);
            b.space(0.5);
            b.finish(0.17)
        }
        Sfx::Victory | Sfx::Defeat => {
            let mut b = Buf::new(rate, 3.4);
            // The same four notes, climbing in the major or sinking in the minor.
            let notes: [f32; 4] = if sfx == Sfx::Victory {
                [293.7, 370.0, 440.0, 587.3]
            } else {
                [392.0, 349.2, 311.1, 233.1]
            };
            for (i, f) in notes.into_iter().enumerate() {
                let (start, last) = (i as f32 * 0.19, i == 3);
                let ring = if last { 1.1 } else { 0.32 };
                for (detune, side) in [(0.997, -0.4), (1.003, 0.4)] {
                    b.tone(start, f * detune, f * detune, 0.1, 0.006, ring, 0.6, side);
                    b.tone(
                        start,
                        f * 2.0 * detune,
                        f * 2.0 * detune,
                        0.1,
                        0.004,
                        ring * 0.5,
                        0.2,
                        -side,
                    );
                    b.tone(
                        start,
                        f * 0.5 * detune,
                        f * 0.5 * detune,
                        0.1,
                        0.010,
                        ring,
                        0.35,
                        0.0,
                    );
                }
            }
            b.space(1.0);
            b.finish(0.55)
        }
    }
}

/// A library sound from its recipe. Noise layers that name no seed get one
/// from the sound's name and their place in it, so every run sounds the same
/// and two sounds `like` each other still differ in their grain.
fn from_recipe(sound: &Sound, rate: u32) -> Frames {
    // A loop is made a little long and its end folded back over its start.
    let fold = if sound.looped { 0.25 } else { 0.0 };
    let mut b = Buf::new(rate, sound.length + fold);
    let name_seed = sound.name.bytes().fold(0x811C_9DC5u32, |h, c| {
        (h ^ c as u32).wrapping_mul(0x0100_0193)
    }) | 1;
    // A loop's steady layers are tuned to whole cycles of it, so the fold is seamless.
    let whole = |hz: f32| {
        if sound.looped {
            (hz * sound.length).round().max(0.0) / sound.length
        } else {
            hz
        }
    };
    for (i, layer) in sound.layers.iter().enumerate() {
        let auto = name_seed.wrapping_add(i as u32 * 7919) | 1;
        match layer {
            Layer::Tone {
                at,
                from,
                to,
                glide,
                attack,
                decay,
                gain,
                pan,
            } => b.tone(*at, *from, *to, *glide, *attack, *decay, *gain, *pan),
            Layer::Stack {
                at,
                from,
                to,
                glide,
                attack,
                decay,
                gain,
                detune,
                width,
                partials,
            } => {
                for (multiple, level) in partials {
                    b.tone(
                        *at,
                        from * multiple,
                        to * multiple,
                        *glide,
                        *attack,
                        *decay,
                        gain * level,
                        -width,
                    );
                    b.tone(
                        *at,
                        from * multiple * detune,
                        to * multiple * detune,
                        *glide,
                        *attack,
                        *decay,
                        gain * level,
                        *width,
                    );
                }
            }
            Layer::Fm {
                at,
                from,
                to,
                glide,
                ratio,
                index,
                fade,
                attack,
                decay,
                gain,
                pan,
            } => b.fm(
                *at, *from, *to, *glide, *ratio, *index, *fade, *attack, *decay, *gain, *pan,
            ),
            Layer::Burst {
                at,
                low,
                high,
                attack,
                decay,
                gain,
                pan,
                seed,
            } => b.burst(
                *at,
                *low,
                *high,
                *attack,
                *decay,
                *gain,
                *pan,
                seed.unwrap_or(auto),
            ),
            Layer::Hiss {
                at,
                freq,
                q,
                attack,
                decay,
                gain,
                pan,
                seed,
            } => b.hiss(
                *at,
                *freq,
                *q,
                *attack,
                *decay,
                *gain,
                *pan,
                seed.unwrap_or(auto),
            ),
            Layer::Sweep {
                at,
                from,
                to,
                glide,
                q,
                attack,
                decay,
                gain,
                pan,
                seed,
            } => b.sweep(
                *at,
                *from,
                *to,
                *glide,
                *q,
                *attack,
                *decay,
                *gain,
                *pan,
                seed.unwrap_or(auto),
            ),
            Layer::Rumble {
                freq,
                q,
                gain,
                wobble,
                depth,
                pan,
                seed,
            } => {
                let (mut noise, mut air, wobble) =
                    (Noise(seed.unwrap_or(auto)), Air::default(), whole(*wobble));
                let r = b.rate;
                b.add(0.0, *pan, |t| {
                    air.step(noise.next(), *freq, *q, r)
                        * gain
                        * (1.0 - depth * 0.5 * (1.0 + (TAU * wobble * t).sin()))
                });
            }
            Layer::Drone {
                freq,
                gain,
                wobble,
                depth,
                pan,
            } => {
                let (freq, wobble) = (whole(*freq), whole(*wobble));
                b.add(0.0, *pan, |t| {
                    (TAU * freq * t).sin()
                        * gain
                        * (1.0 - depth * 0.5 * (1.0 + (TAU * wobble * t).sin()))
                });
            }
            Layer::Drive(amount) => b.drive(*amount),
        }
    }
    if sound.room > 0.0 {
        b.space(sound.room);
    }
    if sound.looped {
        b.finish_loop(sound.peak, fold)
    } else {
        b.finish(sound.peak)
    }
}

/// Writes the interface set and the whole sound library as 16-bit WAV files
/// named after the sounds, for listening to outside the game.
pub fn dump(dir: &std::path::Path, library: &SoundLibrary) -> Result<(), String> {
    const RATE: u32 = 48_000;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let bank = Bank::synthesise(RATE, library);
    let interface = Sfx::ALL
        .iter()
        .map(|s| (s.name().to_owned(), bank.sound(*s)));
    let world = library
        .sounds
        .iter()
        .enumerate()
        .map(|(i, s)| (s.name.clone(), bank.world(SoundId(i as u16))));
    for (name, frames) in interface.chain(world) {
        let mut bytes = Vec::with_capacity(44 + frames.len() * 4);
        let data_len = (frames.len() * 4) as u32;
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&RATE.to_le_bytes());
        bytes.extend_from_slice(&(RATE * 4).to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for f in frames {
            for s in f {
                bytes.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
            }
        }
        let path = dir.join(format!("{name}.wav"));
        std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        println!(
            "wrote {} ({:.2} s)",
            path.display(),
            frames.len() as f32 / RATE as f32
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    fn library() -> SoundLibrary {
        SoundLibrary::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
            .unwrap()
    }

    fn bank() -> &'static Bank {
        static BANK: OnceLock<Bank> = OnceLock::new();
        BANK.get_or_init(|| Bank::synthesise(44_100, &library()))
    }

    /// No click at either end, nothing cut off while still sounding, a sane level.
    fn assert_clean(name: &str, frames: &[[f32; 2]]) {
        let peak = frames
            .iter()
            .flat_map(|f| f.iter())
            .fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(
            frames.iter().flat_map(|f| f.iter()).all(|s| s.is_finite()),
            "{name} has a non-finite sample"
        );
        let minimum = if name == "bomb_release" { 0.02 } else { 0.1 };
        assert!((minimum..=0.9).contains(&peak), "{name} peaks at {peak}");
        assert!(
            frames[0].iter().all(|s| s.abs() < 0.02),
            "{name} starts at {:?}",
            frames[0]
        );
        assert!(
            frames[frames.len() - 1].iter().all(|s| s.abs() < 1e-4),
            "{name} ends at {:?}",
            frames[frames.len() - 1]
        );
        let tail = &frames[frames.len() - 1200..frames.len() - 600];
        let tail_peak = tail
            .iter()
            .flat_map(|f| f.iter())
            .fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(
            tail_peak < 0.06 * peak,
            "{name} is cut off while still at {tail_peak} (peak {peak})"
        );
    }

    #[test]
    fn sounds_are_clean() {
        for sfx in Sfx::ALL {
            assert_clean(sfx.name(), bank().sound(sfx));
        }
    }

    /// The airbase's set on its own, so a problem elsewhere in the library does not hide it.
    #[test]
    fn fulgur_main_bore_dominates_the_compact_pair() {
        let library = library();
        let samples = |name| bank().world(library.id_of(name).unwrap());
        let peak = |frames: &[[f32; 2]]| frames.iter().flatten().fold(0.0f32, |m, x| m.max(x.abs()));
        let energy = |frames: &[[f32; 2]]| frames.iter().flatten().map(|x| (*x as f64).powi(2)).sum::<f64>();
        for (main, compact) in [
            ("aster_bore_heavy", "aster_bore_compact"),
            ("aster_bore_heavy_strike", "aster_bore_compact_strike"),
        ] {
            let (main, compact) = (samples(main), samples(compact));
            assert!(peak(compact) * 2.0 < peak(main), "even simultaneous compact shots must peak below the main gun");
            assert!(energy(compact) * 4.0 < energy(main), "repeated compact tails must leave room for the main gun");
            assert!(compact.len() < main.len());
        }
        assert_clean("aster_bore_compact", samples("aster_bore_compact"));
        assert_clean("aster_bore_compact_strike", samples("aster_bore_compact_strike"));
    }

    #[test]
    fn airbase_sounds_are_clean() {
        let library = library();
        let names = ["hatch_open", "hatch_close", "aircraft_stored", "tunnel_launch"];
        for name in names {
            let id = library.id_of(name).unwrap_or_else(|| panic!("{name} is in the library"));
            assert_clean(name, bank().world(id));
        }
    }
    /// Survival's set on its own, so a problem elsewhere in the library does not hide it.
    #[test]
    fn survival_sounds_are_clean() {
        let library = library();
        let mut seen = 0;
        for (i, sound) in library.sounds.iter().enumerate() {
            if !(sound.name.starts_with("survival_") || sound.name.starts_with("replication_")) {
                continue;
            }
            seen += 1;
            let frames = bank().world(SoundId(i as u16));
            if sound.looped {
                let peak = frames.iter().flat_map(|f| f.iter()).fold(0.0f32, |m, s| m.max(s.abs()));
                assert!((0.1..=0.9).contains(&peak), "{} peaks at {peak}", sound.name);
            } else {
                assert_clean(&sound.name, frames);
            }
        }
        assert_eq!(seen, 7);
    }

    #[test]
    fn library_sounds_are_clean_and_loops_join_up() {
        let library = library();
        assert!(
            library.sounds.iter().any(|s| s.looped) && library.sounds.iter().any(|s| !s.looped)
        );
        for (i, sound) in library.sounds.iter().enumerate() {
            let frames = bank().world(SoundId(i as u16));
            if !sound.looped {
                assert_clean(&sound.name, frames);
                continue;
            }
            let peak = frames
                .iter()
                .flat_map(|f| f.iter())
                .fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(
                (0.1..=0.9).contains(&peak),
                "{} peaks at {peak}",
                sound.name
            );
            assert!(
                (frames.len() as f32 / 44_100.0 - sound.length).abs() < 0.01,
                "{} is as long as it says",
                sound.name
            );
            // Across the seam the signal moves no more than it does anywhere else.
            let step = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs().max((a[1] - b[1]).abs());
            let usual = frames
                .windows(2)
                .map(|w| step(w[0], w[1]))
                .fold(0.0f32, f32::max);
            let seam = step(frames[frames.len() - 1], frames[0]);
            assert!(
                seam <= usual * 1.5 + 1e-4,
                "{}: a jump of {seam} at the seam, {usual} elsewhere",
                sound.name
            );
        }
    }

    #[test]
    fn mixer_plays_voices_to_the_end_and_fades_loops() {
        let shared = Shared::new(
            Volumes {
                master: 1.0,
                interface: 1.0,
                effects: 1.0,
                weather: 1.0,
            },
            SoundLibrary::default(),
        );
        let mut out = vec![1.0f32; 512];
        shared.render(&mut out, 2);
        assert!(
            out.iter().all(|s| *s == 0.0),
            "silent until the bank is ready"
        );

        let select = bank().sounds[Sfx::Select as usize].clone();
        shared.mixer.lock().unwrap().voices.push(Voice {
            frames: select.clone(),
            at: 0.0,
            rate: 1.0,
            gain: [1.0; 2],
            world: false,
            weather: false,
            released: false,
        });
        let mut heard = 0.0f32;
        for _ in 0..select.len() / 256 + 2 {
            shared.render(&mut out, 2);
            heard = out.iter().fold(heard, |m, s| m.max(s.abs()));
        }
        assert!(heard > 0.2);
        assert!(shared.mixer.lock().unwrap().voices.is_empty());

        // A loop comes up to its level, keeps going past its own length, and leaves once it is told to stop.
        let tracks = library().id_of("tracks").unwrap();
        let frames = bank().world[tracks.0 as usize].clone();
        shared.mixer.lock().unwrap().loops.push(LoopVoice {
            id: tracks,
            frames: frames.clone(),
            at: 0.0,
            rate: 1.0,
            rate_target: 1.0,
            gain: [0.0; 2],
            target: [0.8; 2],
            pan: 0.0,
            weather: false,
        });
        let mut loud = 0.0f32;
        for _ in 0..frames.len() / 256 * 2 {
            shared.render(&mut out, 2);
            loud = out.iter().fold(loud, |m, s| m.max(s.abs()));
        }
        assert!(loud > 0.1, "the loop is heard: {loud}");
        shared.mixer.lock().unwrap().loops[0].target = [0.0; 2];
        for _ in 0..200 {
            shared.render(&mut out, 2);
        }
        assert!(
            shared.mixer.lock().unwrap().loops.is_empty(),
            "a stopped loop fades and is dropped"
        );
    }

    /// Leaving a match lets its sound go: loops that nobody will ask to stop
    /// again, a long sound mid-play, and one still waiting on its delay. The
    /// interface keeps playing.
    #[test]
    fn release_world_quiets_the_battle_but_not_the_interface() {
        let audio = Audio::silent();
        let shared = &audio.shared;
        *shared.bank.write().unwrap() = Some(Arc::new(Bank {
            sounds: bank().sounds.clone(),
            world: bank().world.clone(),
        }));
        shared.rate.store(44_100, Ordering::Relaxed);
        let tracks = library().id_of("tracks").unwrap();
        audio.set_loops(&[(tracks, 0.8, 0.0, 1.0)]);
        audio.set_weather_loops(&[(tracks, 0.5, 0.3, 1.0)]);
        audio.play_world_after(tracks, 0.8, 0.0, 1.0, 1.0);
        let long = shared.mixer().voices.len();
        {
            // A world sound partway through, long enough to outlast the fade.
            let mut m = shared.mixer();
            m.voices.push(Voice {
                frames: Arc::new(vec![[0.5; 2]; 44_100]),
                at: 10.0,
                rate: 1.0,
                gain: [1.0; 2],
                world: true,
                weather: false,
                released: false,
            });
        }
        audio.play(Sfx::Select);
        let mut out = vec![0.0f32; 512];
        shared.render(&mut out, 2);

        audio.release_world();
        assert_eq!(long, 1);
        assert_eq!(
            shared.mixer().voices.iter().filter(|v| !v.world).count(),
            1,
            "the interface sound plays on"
        );
        assert!(
            !shared.mixer().voices.iter().any(|v| v.world && v.at < 0.0),
            "a delayed battle sound never starts"
        );
        for _ in 0..200 {
            shared.render(&mut out, 2);
        }
        let m = shared.mixer();
        assert!(m.loops.is_empty(), "the battle's loops fade out");
        assert!(!m.voices.iter().any(|v| v.world), "ringing battle sounds fade out");
    }

    #[test]
    fn mix_moving_keeps_a_column_from_becoming_one_machine() {
        let a = SoundId(1);
        let b = SoundId(2);

        let one = mix_moving(&[(a, 0.8, 0.0)]);
        assert_eq!(one.len(), 1);
        assert!(one[0].1 < 0.8 * 0.5, "quieter than the old half-gain sum");
        assert!((one[0].3 - 1.0).abs() < 1e-5);

        // A blob of twenty at the same place: two voices, neither loud, detuned.
        let blob: Vec<_> = (0..20).map(|_| (a, 0.8, 0.0)).collect();
        let mixed = mix_moving(&blob);
        assert_eq!(mixed.len(), 2);
        assert!(mixed.iter().all(|l| l.1 <= 0.36 + 1e-5));
        assert_ne!(mixed[0].3, mixed[1].3);

        // Spread across the view: a voice on each side, not one in the middle.
        let line = [
            (a, 0.7, -0.7),
            (a, 0.7, -0.6),
            (a, 0.7, 0.0),
            (a, 0.7, 0.6),
            (a, 0.7, 0.7),
        ];
        let spread = mix_moving(&line);
        assert!(spread.len() >= 3);
        assert!(spread.iter().any(|l| l.2 < -0.4));
        assert!(spread.iter().any(|l| l.2 > 0.4));

        // Two sounds stay separate.
        let both = mix_moving(&[(a, 0.8, 0.0), (b, 0.8, 0.0)]);
        assert_eq!(both.len(), 2);
        assert_eq!(both[0].0, a);
        assert_eq!(both[1].0, b);
    }

    /// A column split across the view asks for several loops in one call.
    /// Each newly pushed voice used to leave `assigned` short, and the next
    /// row then indexed it out of bounds.
    #[test]
    fn set_loops_can_grow_several_voices_in_one_call() {
        let audio = Audio::silent();
        *audio.shared.bank.write().unwrap() = Some(Arc::new(Bank::synthesise(8_000, &library())));
        let tracks = library().id_of("tracks").unwrap();

        audio.set_loops(&[(tracks, 0.5, -0.6, 1.0), (tracks, 0.5, 0.6, 1.0)]);
        assert_eq!(audio.shared.mixer.lock().unwrap().loops.len(), 2);

        // One voice already playing, then a wider column: reuse one, push two.
        audio.set_loops(&[
            (tracks, 0.5, -0.7, 1.0),
            (tracks, 0.5, 0.0, 1.0),
            (tracks, 0.5, 0.7, 1.03),
        ]);
        assert_eq!(audio.shared.mixer.lock().unwrap().loops.len(), 3);
    }

    #[test]
    fn render_survives_a_poisoned_mixer() {
        let audio = Audio::silent();
        let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let _guard = audio.shared.mixer();
            panic!("poison the mixer");
        }));
        let mut out = vec![0.0; 64];
        audio.shared.render(&mut out, 2);
    }
}
