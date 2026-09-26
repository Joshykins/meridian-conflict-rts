//! The audio thread. A `Stage` (the song, and any moment played over it)
//! lives inside the device callback; the UI
//! talks to it only through channels (drained at the top of each callback)
//! and reads back what it is doing through a mutex the callback only ever
//! `try_lock`s, so a slow UI frame can never stall the sound.
//!
//! A reference recording can play through the same output: it runs alongside
//! the song, and a flip crossfades which of the two is heard, so both keep
//! their place and the comparison is at the same moment.
//!
//! Without a device (no cpal on this build, or opening it failed) the engine
//! runs on a plain thread at real-time pace into nothing, so the transport,
//! playhead and meters still work and the studio can be tested headless.

use mc_music::stage::Stage;
use mc_music::{Command, Engine, Meters, Song, Status};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

/// What the callback last reported.
pub struct Published {
    pub status: Status,
    pub meters: Meters,
    /// Reference playhead, frames at the output rate.
    pub ref_pos: f64,
    pub ref_playing: bool,
    /// The song's level under a moment, 0..1 (1 = no moment).
    pub song_level: f32,
    /// The moment sounding, if any.
    pub moment: Option<String>,
}

/// What the stage and the reference player are doing, besides the song.
#[derive(Clone, Debug, Default)]
pub struct Side {
    pub ref_pos: f64,
    pub ref_playing: bool,
    pub song_level: f32,
    pub moment: Option<String>,
}

/// Commands for the reference player and for moments.
pub enum RefCmd {
    /// Play a moment over the song now; `true` = an ending (the song stops).
    Moment(String, Arc<Song>, bool),
    /// How far the song dips under a moment, dB.
    Duck(f32),
    /// Frames already at the output rate.
    Load(Arc<Vec<[f32; 2]>>),
    Unload,
    Play(bool),
    Seek(f64),
    /// Hear the reference (true) or the song (false).
    Audible(bool),
    Gain(f32),
    /// Loop between these frames.
    Span(Option<(usize, usize)>),
}

#[derive(Default)]
struct RefPlayer {
    frames: Option<Arc<Vec<[f32; 2]>>>,
    pos: f64,
    playing: bool,
    audible: bool,
    /// 0 = the song is heard, 1 = the reference.
    mix: f32,
    gain: f32,
    span: Option<(usize, usize)>,
}

pub struct Audio {
    tx: Sender<Command>,
    ref_tx: Sender<RefCmd>,
    shared: Arc<Mutex<Published>>,
    pub rate: u32,
    /// The output device's name, or why there is none.
    pub device: String,
    pub silent: bool,
    stop: Arc<AtomicBool>,
    #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
    stream: Option<cpal::Stream>,
}

/// Everything the audio thread owns.
struct Worker {
    stage: Stage,
    /// A loop range the user set (else the whole song loops).
    user_loop: Option<(u32, u32)>,
    rx: Receiver<Command>,
    ref_rx: Receiver<RefCmd>,
    reference: RefPlayer,
    shared: Arc<Mutex<Published>>,
    buf: Vec<f32>,
}

impl Worker {
    fn new(
        rate: u32,
        song: Arc<Song>,
        rx: Receiver<Command>,
        ref_rx: Receiver<RefCmd>,
        shared: Arc<Mutex<Published>>,
    ) -> Worker {
        let stage = Stage::new(rate as f32, song);
        Worker {
            stage,
            user_loop: None,
            rx,
            ref_rx,
            reference: RefPlayer {
                gain: 1.0,
                ..Default::default()
            },
            shared,
            buf: Vec::new(),
        }
    }

    /// Renders `frames` stereo frames into `self.buf` and publishes.
    fn run(&mut self, frames: usize) {
        self.buf.resize(frames * 2, 0.0);
        let rate = self.stage.song().rate();
        let (stage, user_loop, rx, ref_rx, buf, r) = (
            &mut self.stage,
            &mut self.user_loop,
            &self.rx,
            &self.ref_rx,
            &mut self.buf,
            &mut self.reference,
        );
        // A panic in the engine must not kill the device thread (WASAPI then
        // panics again when the stream drops). Output silence for the block.
        let ok = std::panic::catch_unwind(AssertUnwindSafe(|| {
            while let Ok(c) = rx.try_recv() {
                match c {
                    Command::Play => stage.play(),
                    Command::Stop => stage.stop(),
                    Command::Panic => {
                        stage.stop();
                        stage.song_mut().command(Command::Panic);
                    }
                    Command::SetSong(s) => {
                        stage.set_song(s);
                        if let Some(l) = *user_loop {
                            stage.song_mut().command(Command::SetLoop(Some(l)));
                        }
                    }
                    Command::SetLoop(l) => {
                        *user_loop = l;
                        // No range of the user's: the whole song loops, as on the stage.
                        let end = stage.song().song().arrangement_ticks();
                        let whole = (end > 0).then_some((0, end));
                        stage.song_mut().command(Command::SetLoop(l.or(whole)));
                    }
                    c => stage.song_mut().command(c),
                }
            }
            while let Ok(c) = ref_rx.try_recv() {
                match c {
                    RefCmd::Moment(name, song, ending) => stage.moment(&name, song, ending),
                    RefCmd::Duck(db) => stage.duck.depth = db,
                    RefCmd::Load(f) => {
                        r.frames = Some(f);
                        r.pos = 0.0;
                    }
                    RefCmd::Unload => {
                        r.frames = None;
                        r.playing = false;
                        r.audible = false;
                    }
                    RefCmd::Play(p) => r.playing = p,
                    RefCmd::Seek(p) => r.pos = p.max(0.0),
                    RefCmd::Audible(a) => r.audible = a,
                    RefCmd::Gain(g) => r.gain = g,
                    RefCmd::Span(s) => r.span = s,
                }
            }
            stage.render(buf);
            mix_reference(r, buf, rate);
        }));
        if ok.is_err() {
            self.buf.fill(0.0);
        }
        self.publish();
    }

    fn publish(&mut self) {
        let Ok(mut p) = self.shared.try_lock() else {
            // Keep the peaks for the next block instead of losing them.
            return;
        };
        p.status = self.stage.song().status();
        p.song_level = self.stage.song_level();
        p.moment = self.stage.moment_playing().map(String::from);
        p.ref_pos = self.reference.pos;
        p.ref_playing = self.reference.playing && self.reference.frames.is_some();
        let m = self.stage.song().meters();
        let out = &mut p.meters;
        if out.tracks.len() != m.tracks.len() || out.buses.len() != m.buses.len() {
            out.tracks.clone_from(&m.tracks);
            out.buses.clone_from(&m.buses);
        } else {
            for (o, i) in out
                .tracks
                .iter_mut()
                .zip(&m.tracks)
                .chain(out.buses.iter_mut().zip(&m.buses))
            {
                o.peak[0] = o.peak[0].max(i.peak[0]);
                o.peak[1] = o.peak[1].max(i.peak[1]);
                o.reduction = o.reduction.min(i.reduction);
            }
        }
        out.master.peak[0] = out.master.peak[0].max(m.master.peak[0]);
        out.master.peak[1] = out.master.peak[1].max(m.master.peak[1]);
        out.master.reduction = out.master.reduction.min(m.master.reduction);
        out.loudness = m.loudness;
        out.scope.clone_from(&m.scope);
        out.scope_at = m.scope_at;
        out.layers.clone_from(&m.layers);
        drop(p);
        self.stage.song_mut().meters_mut().decay();
    }
}

/// Plays the reference into `buf` (interleaved stereo), crossfading with the
/// song over about 30 ms when the audible side flips.
fn mix_reference(r: &mut RefPlayer, buf: &mut [f32], rate: f32) {
    let Some(frames) = r.frames.clone() else {
        r.mix = 0.0;
        return;
    };
    let target = if r.audible { 1.0 } else { 0.0 };
    let step = 1.0 / (0.03 * rate);
    let n = buf.len() / 2;
    let len = frames.len();
    for i in 0..n {
        r.mix += (target - r.mix).clamp(-step, step);
        let mut s = [0.0f32; 2];
        if r.playing && len > 0 {
            let at = r.pos as usize;
            if at < len {
                s = frames[at];
            }
            r.pos += 1.0;
            let (a, b) = r.span.unwrap_or((0, len));
            if r.pos as usize >= b.min(len) {
                if r.span.is_some() {
                    r.pos = a as f64;
                } else {
                    r.playing = false;
                }
            }
        }
        if r.mix > 0.0 || target > 0.0 {
            let k = r.mix;
            buf[i * 2] = buf[i * 2] * (1.0 - k) + s[0] * r.gain * k;
            buf[i * 2 + 1] = buf[i * 2 + 1] * (1.0 - k) + s[1] * r.gain * k;
        }
    }
}

impl Audio {
    pub fn start(song: Arc<Song>) -> Audio {
        let blank = Engine::new(48000.0, song.clone());
        let shared = Arc::new(Mutex::new(Published {
            status: blank.status(),
            meters: blank.meters().clone(),
            ref_pos: 0.0,
            ref_playing: false,
            song_level: 1.0,
            moment: None,
        }));
        drop(blank);
        let stop = Arc::new(AtomicBool::new(false));

        #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
        let why = {
            let (tx, rx) = channel();
            let (ref_tx, ref_rx) = channel();
            match device::probe() {
                Ok((dev, config)) => {
                    let rate = config.sample_rate().0;
                    let worker = Worker::new(rate, song.clone(), rx, ref_rx, shared.clone());
                    match device::build(&dev, config, worker) {
                        Ok(stream) => {
                            let name = cpal::traits::DeviceTrait::name(&dev).unwrap_or_default();
                            return Audio {
                                tx,
                                ref_tx,
                                shared,
                                rate,
                                device: name,
                                silent: false,
                                stop,
                                stream: Some(stream),
                            };
                        }
                        Err(e) => e,
                    }
                }
                Err(e) => e,
            }
        };
        #[cfg(not(any(not(target_os = "linux"), feature = "alsa")))]
        let why = "built without audio".to_string();

        eprintln!("mc-studio: no audio output ({why}); running silent");
        let (tx, rx) = channel();
        let (ref_tx, ref_rx) = channel();
        let rate = 48000;
        spawn_silent(
            Worker::new(rate, song, rx, ref_rx, shared.clone()),
            rate,
            stop.clone(),
        );
        Audio {
            tx,
            ref_tx,
            shared,
            rate,
            device: format!("silent: {why}"),
            silent: true,
            stop,
            #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
            stream: None,
        }
    }

    pub fn send(&self, c: Command) {
        let _ = self.tx.send(c);
    }

    pub fn send_ref(&self, c: RefCmd) {
        let _ = self.ref_tx.send(c);
    }

    /// A handle for other threads (MIDI) to talk to the engine directly.
    pub fn sender(&self) -> Sender<Command> {
        self.tx.clone()
    }

    /// The latest status and the meters since the last call (peaks reset).
    /// Returns the reference player's position and whether it is playing.
    pub fn poll(&self, status: &mut Status, meters: &mut Meters) -> Side {
        let mut p = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        status.clone_from(&p.status);
        meters.clone_from(&p.meters);
        p.meters.decay();
        Side {
            ref_pos: p.ref_pos,
            ref_playing: p.ref_playing,
            song_level: p.song_level,
            moment: p.moment.clone(),
        }
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
        if let Some(stream) = self.stream.take() {
            // cpal 0.15 WASAPI unwraps IAudioClient::Stop(); keep a failing
            // driver from turning quitting into a crash.
            let _ = std::panic::catch_unwind(AssertUnwindSafe(|| drop(stream)));
        }
    }
}

/// Renders in 256-frame blocks at real-time pace, into nothing.
fn spawn_silent(mut w: Worker, rate: u32, stop: Arc<AtomicBool>) {
    let _ = std::thread::Builder::new()
        .name("mc-studio-silent".into())
        .spawn(move || {
            let start = std::time::Instant::now();
            let mut rendered: u64 = 0;
            let block = 256;
            while !stop.load(Ordering::Relaxed) {
                let due = (start.elapsed().as_secs_f64() * rate as f64) as u64;
                if rendered + block as u64 > due {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                    continue;
                }
                w.run(block);
                rendered += block as u64;
            }
        });
}

#[cfg(any(not(target_os = "linux"), feature = "alsa"))]
mod device {
    use super::Worker;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{FromSample, SampleFormat, SizedSample};

    pub fn probe() -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("no output device")?;
        let config = device.default_output_config().map_err(|e| e.to_string())?;
        Ok((device, config))
    }

    pub fn build(
        device: &cpal::Device,
        config: cpal::SupportedStreamConfig,
        worker: Worker,
    ) -> Result<cpal::Stream, String> {
        let format = config.sample_format();
        let config: cpal::StreamConfig = config.into();
        let stream = match format {
            SampleFormat::F32 => build_typed::<f32>(device, &config, worker),
            SampleFormat::I16 => build_typed::<i16>(device, &config, worker),
            SampleFormat::U16 => build_typed::<u16>(device, &config, worker),
            SampleFormat::I32 => build_typed::<i32>(device, &config, worker),
            other => Err(format!("unsupported sample format {other:?}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(stream)
    }

    fn build_typed<T: SizedSample + FromSample<f32>>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        mut worker: Worker,
    ) -> Result<cpal::Stream, String> {
        let channels = (config.channels as usize).max(1);
        device
            .build_output_stream(
                config,
                move |out: &mut [T], _| {
                    let frames = out.len() / channels;
                    worker.run(frames);
                    let buf = &worker.buf;
                    for (f, frame) in out.chunks_mut(channels).enumerate() {
                        let (l, r) = (buf[f * 2], buf[f * 2 + 1]);
                        match frame.len() {
                            1 => frame[0] = T::from_sample((l + r) * 0.5),
                            _ => {
                                frame[0] = T::from_sample(l);
                                frame[1] = T::from_sample(r);
                                for s in frame.iter_mut().skip(2) {
                                    *s = T::from_sample(0.0f32);
                                }
                            }
                        }
                    }
                },
                |e| eprintln!("mc-studio: audio stream error: {e}"),
                None,
            )
            .map_err(|e| e.to_string())
    }
}
