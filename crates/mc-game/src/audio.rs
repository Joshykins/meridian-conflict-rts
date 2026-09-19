//! Sound: a small mixer and an interface sound set synthesised at start-up.
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

use std::f32::consts::TAU;
use std::sync::{Arc, Mutex, OnceLock};

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
    /// A screen slid in or out.
    Whoosh,
    /// The match is starting.
    Launch,
    /// Units acknowledged an order.
    Order,
    Victory,
    Defeat,
}

impl Sfx {
    pub const ALL: [Sfx; 12] = [Sfx::Hover, Sfx::Select, Sfx::Back, Sfx::ToggleOn, Sfx::ToggleOff, Sfx::Tick, Sfx::Deny, Sfx::Whoosh, Sfx::Launch, Sfx::Order, Sfx::Victory, Sfx::Defeat];

    pub fn name(self) -> &'static str {
        match self {
            Sfx::Hover => "hover",
            Sfx::Select => "select",
            Sfx::Back => "back",
            Sfx::ToggleOn => "toggle_on",
            Sfx::ToggleOff => "toggle_off",
            Sfx::Tick => "tick",
            Sfx::Deny => "deny",
            Sfx::Whoosh => "whoosh",
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
    pub ambience: f32,
}

type Frames = Arc<Vec<[f32; 2]>>;

pub struct Bank {
    sounds: Vec<Frames>,
    /// The front end's drone; loops seamlessly.
    ambience: Frames,
}

struct Voice {
    frames: Frames,
    at: usize,
    gain: f32,
}

struct Mixer {
    voices: Vec<Voice>,
    volumes: Volumes,
    ambience_at: usize,
    /// Current and wanted ambience level; the mixer glides between them.
    ambience_gain: f32,
    ambience_target: f32,
    sample_rate: f32,
}

struct Shared {
    mixer: Mutex<Mixer>,
    bank: OnceLock<Bank>,
}

impl Shared {
    /// Fills interleaved output. Runs on the audio thread.
    fn render(&self, out: &mut [f32], channels: usize) {
        out.fill(0.0);
        let Some(bank) = self.bank.get() else { return };
        let mut m = self.mixer.lock().unwrap();
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
        let interface = m.volumes.master * m.volumes.interface;
        for v in &mut m.voices {
            let n = frames.min(v.frames.len() - v.at);
            for (i, f) in v.frames[v.at..v.at + n].iter().enumerate() {
                write(i, f[0] * v.gain * interface, f[1] * v.gain * interface);
            }
            v.at += n;
        }
        m.voices.retain(|v| v.at < v.frames.len());

        // About a second to fade the drone in or out.
        let glide = 1.0 / m.sample_rate;
        let level = m.volumes.master * m.volumes.ambience;
        for i in 0..frames {
            m.ambience_gain += (m.ambience_target - m.ambience_gain).clamp(-glide, glide);
            if m.ambience_gain <= 0.0 && m.ambience_target <= 0.0 {
                break;
            }
            let f = bank.ambience[m.ambience_at];
            m.ambience_at = (m.ambience_at + 1) % bank.ambience.len();
            let g = m.ambience_gain * m.ambience_gain * level;
            write(i, f[0] * g, f[1] * g);
        }
        for s in out.iter_mut() {
            // Many voices at once may add up past full scale; bend instead of clipping.
            *s = if s.abs() > 0.8 { s.signum() * (0.8 + 0.2 * ((s.abs() - 0.8) / 0.2).tanh()) } else { *s };
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
    pub fn new(volumes: Volumes) -> Audio {
        let shared = Arc::new(Shared {
            mixer: Mutex::new(Mixer { voices: Vec::new(), volumes, ambience_at: 0, ambience_gain: 0.0, ambience_target: 0.0, sample_rate: 48_000.0 }),
            bank: OnceLock::new(),
        });
        #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
        {
            let stream = match device::open(shared.clone()) {
                Ok((stream, sample_rate)) => {
                    shared.mixer.lock().unwrap().sample_rate = sample_rate as f32;
                    let bank_for = shared.clone();
                    std::thread::Builder::new()
                        .name("mc-audio-synth".into())
                        .spawn(move || {
                            let started = std::time::Instant::now();
                            let _ = bank_for.bank.set(Bank::synthesise(sample_rate));
                            log::info!("audio: sound bank ready in {:.0} ms at {sample_rate} Hz", started.elapsed().as_secs_f32() * 1000.0);
                        })
                        .expect("spawn synth thread");
                    Some(stream)
                }
                Err(e) => {
                    log::warn!("audio: no sound ({e})");
                    None
                }
            };
            Audio { shared, _stream: stream }
        }
        #[cfg(not(any(not(target_os = "linux"), feature = "alsa")))]
        {
            log::info!("audio: this build has no sound backend (Linux builds need --features alsa)");
            Audio { shared }
        }
    }

    /// No device at all: for tools that draw the interface without a player present.
    pub fn silent() -> Audio {
        let shared = Arc::new(Shared {
            mixer: Mutex::new(Mixer { voices: Vec::new(), volumes: Volumes { master: 0.0, interface: 0.0, ambience: 0.0 }, ambience_at: 0, ambience_gain: 0.0, ambience_target: 0.0, sample_rate: 48_000.0 }),
            bank: OnceLock::new(),
        });
        #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
        return Audio { shared, _stream: None };
        #[cfg(not(any(not(target_os = "linux"), feature = "alsa")))]
        return Audio { shared };
    }

    pub fn play(&self, sfx: Sfx) {
        self.play_at(sfx, 1.0);
    }

    pub fn play_at(&self, sfx: Sfx, gain: f32) {
        let Some(bank) = self.shared.bank.get() else { return };
        let mut m = self.shared.mixer.lock().unwrap();
        // A held key or a fast pointer must not stack up a wall of voices.
        if m.voices.len() < 24 {
            m.voices.push(Voice { frames: bank.sounds[sfx as usize].clone(), at: 0, gain });
        }
    }

    pub fn set_volumes(&self, volumes: Volumes) {
        self.shared.mixer.lock().unwrap().volumes = volumes;
    }

    /// Fades the front end's drone in or out.
    pub fn set_ambience(&self, on: bool) {
        self.shared.mixer.lock().unwrap().ambience_target = if on { 1.0 } else { 0.0 };
    }
}

#[cfg(any(not(target_os = "linux"), feature = "alsa"))]
mod device {
    use super::Shared;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{FromSample, SampleFormat, SizedSample};
    use std::sync::Arc;

    pub fn open(shared: Arc<Shared>) -> Result<(cpal::Stream, u32), String> {
        let device = cpal::default_host().default_output_device().ok_or("no output device")?;
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
        log::info!("audio: {} at {rate} Hz", device.name().unwrap_or_else(|_| "output device".into()));
        Ok((stream, rate))
    }

    fn build<T: SizedSample + FromSample<f32>>(device: &cpal::Device, config: &cpal::StreamConfig, shared: Arc<Shared>) -> Result<cpal::Stream, String> {
        let channels = config.channels as usize;
        let mut mix: Vec<f32> = Vec::new();
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
                |e| log::warn!("audio: stream error: {e}"),
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
        Buf { rate: rate as f32, frames: vec![[0.0; 2]; (rate as f32 * seconds) as usize] }
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
    fn tone(&mut self, start: f32, f0: f32, f1: f32, glide: f32, attack: f32, decay: f32, gain: f32, position: f32) {
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

    /// A burst of band-passed noise.
    #[allow(clippy::too_many_arguments)]
    fn hiss(&mut self, start: f32, cutoff: f32, q: f32, attack: f32, decay: f32, gain: f32, position: f32, seed: u32) {
        let (mut noise, mut air) = (Noise(seed), Air::default());
        let rate = self.rate;
        self.add(start, position, |t| air.step(noise.next(), cutoff, q, rate) * pluck(t, attack, decay) * gain);
    }

    /// Early reflections: a few cross-fed echoes that put a dry blip in a room.
    /// `wrap` folds the tail back onto the start, for loops.
    fn space(&mut self, amount: f32, wrap: bool) {
        let n = self.frames.len();
        for (seconds, gain, swap) in [(0.043, 0.30, true), (0.089, 0.22, false), (0.137, 0.16, true), (0.211, 0.11, false), (0.307, 0.07, true)] {
            let d = (seconds * self.rate) as usize;
            // Every tap echoes the signal as the previous taps left it.
            let dry = self.frames.clone();
            for i in 0..n {
                let src = if i >= d { i - d } else if wrap { i + n - d } else { continue };
                let f = dry[src];
                let (a, b) = if swap { (f[1], f[0]) } else { (f[0], f[1]) };
                self.frames[i][0] += a * gain * amount;
                self.frames[i][1] += b * gain * amount;
            }
        }
    }

    /// Scales to a peak level, and fades the last few milliseconds so a tail
    /// cut short by the buffer's end cannot click.
    fn finish(mut self, peak: f32) -> Frames {
        let top = self.frames.iter().flat_map(|f| f.iter()).fold(1e-6f32, |m, s| m.max(s.abs()));
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
    pub fn synthesise(rate: u32) -> Bank {
        Bank { sounds: Sfx::ALL.iter().map(|s| sound(*s, rate)).collect(), ambience: ambience(rate) }
    }

    pub fn sound(&self, sfx: Sfx) -> &[[f32; 2]] {
        &self.sounds[sfx as usize]
    }

    pub fn ambience(&self) -> &[[f32; 2]] {
        &self.ambience
    }
}

/// The interface set shares one voice: soft sine blips around A, a little air
/// from filtered noise, and the same short room on everything.
fn sound(sfx: Sfx, rate: u32) -> Frames {
    match sfx {
        Sfx::Hover => {
            let mut b = Buf::new(rate, 0.30);
            b.tone(0.0, 1960.0, 1760.0, 0.03, 0.002, 0.020, 0.8, -0.1);
            b.tone(0.0, 3520.0, 3520.0, 0.03, 0.001, 0.010, 0.15, 0.1);
            b.hiss(0.0, 2600.0, 1.2, 0.001, 0.008, 0.5, 0.0, 11);
            b.space(0.5, false);
            b.finish(0.20)
        }
        Sfx::Select => {
            let mut b = Buf::new(rate, 0.60);
            b.tone(0.0, 850.0, 880.0, 0.04, 0.002, 0.070, 1.0, 0.0);
            b.tone(0.0, 1730.0, 1760.0, 0.04, 0.002, 0.045, 0.45, 0.15);
            b.tone(0.045, 1318.5, 1318.5, 0.04, 0.003, 0.090, 0.55, -0.15);
            b.tone(0.0, 150.0, 90.0, 0.06, 0.002, 0.050, 0.7, 0.0);
            b.hiss(0.0, 2400.0, 0.9, 0.0005, 0.006, 0.7, 0.0, 23);
            b.space(0.7, false);
            b.finish(0.42)
        }
        Sfx::Back => {
            let mut b = Buf::new(rate, 0.55);
            b.tone(0.0, 700.0, 587.3, 0.09, 0.003, 0.080, 1.0, 0.0);
            b.tone(0.05, 440.0, 392.0, 0.10, 0.004, 0.100, 0.7, -0.1);
            b.tone(0.0, 130.0, 80.0, 0.06, 0.002, 0.045, 0.5, 0.0);
            b.hiss(0.0, 2600.0, 0.8, 0.001, 0.010, 0.35, 0.0, 31);
            b.space(0.7, false);
            b.finish(0.36)
        }
        Sfx::ToggleOn | Sfx::ToggleOff => {
            let mut b = Buf::new(rate, 0.45);
            let (first, second) = if sfx == Sfx::ToggleOn { (1174.7, 1760.0) } else { (1760.0, 1174.7) };
            b.tone(0.0, first, first, 0.01, 0.002, 0.030, 0.9, -0.2);
            b.tone(0.055, second, second, 0.01, 0.002, 0.050, 1.0, 0.2);
            b.hiss(0.0, 2600.0, 1.0, 0.0005, 0.005, 0.6, 0.0, 41);
            b.space(0.6, false);
            b.finish(0.32)
        }
        Sfx::Tick => {
            let mut b = Buf::new(rate, 0.12);
            b.tone(0.0, 2637.0, 2349.3, 0.02, 0.001, 0.009, 0.9, 0.0);
            b.hiss(0.0, 3000.0, 1.5, 0.0005, 0.004, 0.7, 0.0, 53);
            b.space(0.3, false);
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
            b.space(0.5, false);
            b.finish(0.34)
        }
        Sfx::Whoosh => {
            let mut b = Buf::new(rate, 0.75);
            let (mut noise, mut air) = (Noise(67), Air::default());
            let r = b.rate;
            let mut frames = std::mem::take(&mut b.frames);
            for (i, f) in frames.iter_mut().enumerate() {
                let t = i as f32 / r;
                let k = (t / 0.42).min(1.0);
                // The band rises and falls as the panel passes; the sound crosses left to right.
                let cutoff = 350.0 + 1500.0 * (k * std::f32::consts::PI).sin().max(0.0).powf(1.5);
                let s = air.step(noise.next(), cutoff, 1.4, r) * (k * std::f32::consts::PI).sin().powi(2) * if k < 1.0 { 1.0 } else { 0.0 };
                let (l, rr) = pan(-0.7 + 1.4 * k);
                *f = [s * l, s * rr];
            }
            b.frames = frames;
            b.space(0.8, false);
            b.finish(0.22)
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
                    let gate = if t < hit { k * k } else { (-(t - hit) / 0.03).exp() };
                    air.step(noise.next(), 200.0 * (12.0f32).powf(k), 2.5, r) * gate * 0.8
                });
            }
            b.tone(0.0, 110.0, 220.0, hit, hit * 0.95, 0.05, 0.35, 0.0);
            // The hit: a falling sub, a crack, and an open fifth-and-ninth chord left ringing.
            b.tone(hit, 120.0, 41.2, 0.35, 0.004, 0.55, 1.6, 0.0);
            b.tone(hit, 240.0, 82.4, 0.25, 0.004, 0.22, 0.6, 0.0);
            b.hiss(hit, 1200.0, 0.7, 0.001, 0.09, 1.8, 0.0, 83);
            b.hiss(hit, 3200.0, 0.8, 0.001, 0.30, 0.4, 0.0, 89);
            for (i, (f, gain)) in [(220.0, 0.5), (329.6, 0.42), (440.0, 0.36), (493.9, 0.26), (659.3, 0.2), (880.0, 0.12)].into_iter().enumerate() {
                let side = if i % 2 == 0 { -0.45 } else { 0.45 };
                b.tone(hit, f * 0.996, f * 0.996, 0.1, 0.012, 0.85, gain, side);
                b.tone(hit, f * 1.004, f * 1.004, 0.1, 0.012, 0.85, gain, -side);
            }
            b.space(1.0, false);
            b.finish(0.80)
        }
        Sfx::Order => {
            let mut b = Buf::new(rate, 0.35);
            b.tone(0.0, 987.8, 987.8, 0.01, 0.002, 0.028, 0.8, -0.15);
            b.tone(0.04, 1318.5, 1318.5, 0.01, 0.002, 0.040, 0.9, 0.15);
            b.space(0.5, false);
            b.finish(0.17)
        }
        Sfx::Victory | Sfx::Defeat => {
            let mut b = Buf::new(rate, 3.4);
            // The same four notes, climbing in the major or sinking in the minor.
            let notes: [f32; 4] = if sfx == Sfx::Victory { [293.7, 370.0, 440.0, 587.3] } else { [392.0, 349.2, 311.1, 233.1] };
            for (i, f) in notes.into_iter().enumerate() {
                let (start, last) = (i as f32 * 0.19, i == 3);
                let ring = if last { 1.1 } else { 0.32 };
                for (detune, side) in [(0.997, -0.4), (1.003, 0.4)] {
                    b.tone(start, f * detune, f * detune, 0.1, 0.006, ring, 0.6, side);
                    b.tone(start, f * 2.0 * detune, f * 2.0 * detune, 0.1, 0.004, ring * 0.5, 0.2, -side);
                    b.tone(start, f * 0.5 * detune, f * 0.5 * detune, 0.1, 0.010, ring, 0.35, 0.0);
                }
            }
            b.space(1.0, false);
            b.finish(0.55)
        }
    }
}

/// The front end's drone: a low open chord whose partials swell at different
/// rates, wind, and two distant pings. Every frequency and modulation rate is a
/// whole number of cycles per loop and the echoes wrap, so the loop has no seam.
fn ambience(rate: u32) -> Frames {
    const SECONDS: f32 = 32.0;
    let mut b = Buf::new(rate, SECONDS);
    let n = b.frames.len();
    let cycle = |f: f32| (f * SECONDS).round() / SECONDS;

    // D, A, D, E, A: root, fifths and a ninth. Each partial is a detuned pair
    // drifting against each other, under a slow swell of its own.
    for (i, (f, gain)) in [(36.71, 0.45), (73.42, 1.0), (110.0, 0.75), (146.83, 0.6), (164.81, 0.4), (220.0, 0.34), (293.66, 0.2), (440.0, 0.07)].into_iter().enumerate() {
        let swell = (2 + i % 4) as f32 / SECONDS;
        let offset = i as f32 * 1.7;
        for (detune, side) in [(-0.09f32, -0.6f32), (0.09, 0.6)] {
            let f = cycle(f + detune * (1.0 + i as f32 * 0.4));
            let (l, r) = pan(side);
            for (k, frame) in b.frames.iter_mut().enumerate() {
                // Doubles: half a minute of phase is past what an f32 resolves.
                let t = k as f64 / rate as f64;
                let level = 0.55 + 0.45 * (std::f64::consts::TAU * swell as f64 * t + offset as f64).sin() as f32;
                let s = (std::f64::consts::TAU * f as f64 * t).sin() as f32 * level * gain;
                frame[0] += s * l;
                frame[1] += s * r;
            }
        }
    }

    // Wind: noise through a slowly wandering band. Noise does not repeat, so the
    // last two seconds are cross-faded into the first two.
    let wind: Vec<[f32; 2]> = {
        let (mut noise, mut left, mut right) = (Noise(97), Air::default(), Air::default());
        let overlap = 2 * rate as usize;
        let mut raw: Vec<[f32; 2]> = (0..n + overlap)
            .map(|k| {
                let t = k as f32 / rate as f32;
                let cutoff = 330.0 + 170.0 * (TAU * 3.0 / SECONDS * t).sin();
                let gust = 0.5 + 0.5 * (TAU * 5.0 / SECONDS * t + 1.0).sin();
                let (a, c) = (noise.next(), noise.next());
                [left.step(a, cutoff, 0.8, rate as f32) * gust, right.step(c, cutoff * 1.1, 0.8, rate as f32) * gust]
            })
            .collect();
        for k in 0..overlap {
            let w = k as f32 / overlap as f32;
            let (head, tail) = (raw[k], raw[n + k]);
            raw[k] = [head[0] * w.sqrt() + tail[0] * (1.0 - w).sqrt(), head[1] * w.sqrt() + tail[1] * (1.0 - w).sqrt()];
        }
        raw.truncate(n);
        raw
    };
    for (frame, w) in b.frames.iter_mut().zip(&wind) {
        frame[0] += w[0] * 1.6;
        frame[1] += w[1] * 1.6;
    }

    // Two far-off pings, like a sonar sweep somewhere over the horizon.
    for (at, f, side) in [(7.0, 1174.7, -0.5), (21.5, 880.0, 0.5)] {
        b.tone(at, f, f, 0.1, 0.01, 0.9, 0.16, side);
        b.tone(at, f * 1.5, f * 1.5, 0.1, 0.01, 0.5, 0.05, -side);
    }
    b.space(1.0, true);

    let top = b.frames.iter().flat_map(|f| f.iter()).fold(1e-6f32, |m, s| m.max(s.abs()));
    for f in &mut b.frames {
        f[0] *= 0.5 / top;
        f[1] *= 0.5 / top;
    }
    Arc::new(b.frames)
}

/// Writes the whole bank as 16-bit WAV files, for listening to outside the game.
pub fn dump(dir: &std::path::Path) -> Result<(), String> {
    const RATE: u32 = 48_000;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let bank = Bank::synthesise(RATE);
    let named = Sfx::ALL.iter().map(|s| (s.name(), bank.sound(*s))).chain([("ambience_loop", bank.ambience())]);
    for (name, frames) in named {
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
        println!("wrote {} ({:.2} s)", path.display(), frames.len() as f32 / RATE as f32);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bank() -> &'static Bank {
        static BANK: OnceLock<Bank> = OnceLock::new();
        BANK.get_or_init(|| Bank::synthesise(44_100))
    }

    #[test]
    fn sounds_are_clean() {
        for sfx in Sfx::ALL {
            let frames = bank().sound(sfx);
            let peak = frames.iter().flat_map(|f| f.iter()).fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(frames.iter().flat_map(|f| f.iter()).all(|s| s.is_finite()), "{sfx:?} has a non-finite sample");
            assert!((0.1..=0.85).contains(&peak), "{sfx:?} peaks at {peak}");
            // No click at either end, and the tail has really died away before the fade.
            assert!(frames[0].iter().all(|s| s.abs() < 0.02), "{sfx:?} starts at {:?}", frames[0]);
            assert!(frames[frames.len() - 1].iter().all(|s| s.abs() < 1e-4), "{sfx:?} ends at {:?}", frames[frames.len() - 1]);
            let tail = &frames[frames.len() - 1200..frames.len() - 600];
            let tail_peak = tail.iter().flat_map(|f| f.iter()).fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(tail_peak < 0.06 * peak, "{sfx:?} is cut off while still at {tail_peak} (peak {peak})");
        }
    }

    #[test]
    fn ambience_loops_without_a_seam() {
        let a = bank().ambience();
        let n = a.len();
        // The step across the loop point is no bigger than the steps either side of it.
        for c in 0..2 {
            let seam = (a[0][c] - a[n - 1][c]).abs();
            let nearby = (1..200).map(|i| (a[i][c] - a[i - 1][c]).abs().max((a[n - i][c] - a[n - i - 1][c]).abs())).fold(0.0f32, f32::max);
            assert!(seam <= nearby * 1.5 + 1e-4, "channel {c}: seam step {seam}, nearby steps {nearby}");
        }
        let peak = a.iter().flat_map(|f| f.iter()).fold(0.0f32, |m, s| m.max(s.abs()));
        assert!((0.45..=0.55).contains(&peak));
    }

    #[test]
    fn mixer_plays_voices_to_the_end_and_fades_ambience() {
        let shared = Shared {
            mixer: Mutex::new(Mixer { voices: Vec::new(), volumes: Volumes { master: 1.0, interface: 1.0, ambience: 1.0 }, ambience_at: 0, ambience_gain: 0.0, ambience_target: 0.0, sample_rate: 44_100.0 }),
            bank: OnceLock::new(),
        };
        let mut out = vec![1.0f32; 512];
        shared.render(&mut out, 2);
        assert!(out.iter().all(|s| *s == 0.0), "silent until the bank is ready");

        let _ = shared.bank.set(Bank::synthesise(44_100));
        let select = shared.bank.get().unwrap().sounds[Sfx::Select as usize].clone();
        shared.mixer.lock().unwrap().voices.push(Voice { frames: select.clone(), at: 0, gain: 1.0 });
        let mut heard = 0.0f32;
        for _ in 0..select.len() / 256 + 2 {
            shared.render(&mut out, 2);
            heard = out.iter().fold(heard, |m, s| m.max(s.abs()));
        }
        assert!(heard > 0.2);
        assert!(shared.mixer.lock().unwrap().voices.is_empty());

        shared.mixer.lock().unwrap().ambience_target = 1.0;
        let mut out = vec![0.0f32; 44_100 * 2 * 2];
        shared.render(&mut out, 2);
        assert!(out[..200].iter().all(|s| s.abs() < 1e-3), "the drone fades in");
        assert!(out[44_100 * 2..].iter().any(|s| s.abs() > 0.05));
    }
}
