//! Instruments at play: voice allocation, the synth voice and the drum voice.

use crate::dsp::filter::{ModeFilter, OnePole, Svf};
use crate::dsp::osc::Phase;
use crate::dsp::{midi_hz, pan_gains, soft_clip, Adsr, Rng};
use crate::patch::{Drum, FilterMode, Instrument, Kit, LfoShape, LfoTo, Synth};

/// Most unison copies per oscillator.
pub const MAX_UNISON: usize = 7;
const MAX_OSCS: usize = 3;

/// What the track feeds its instrument each block.
#[derive(Clone, Copy, Debug)]
pub struct Mods {
    /// Octaves added to the filter cutoff (automation, intensity).
    pub cutoff: f32,
    /// Samples per beat, for synced LFOs.
    pub beat: f32,
}

pub struct Player {
    voices: Vec<Voice>,
    rate: f32,
    rng: Rng,
    /// Free-running LFO phases, one per LFO slot.
    lfo_phase: [f32; 4],
    lfo_hold: [f32; 4],
    /// Last note played, for glide.
    last_key: f32,
    /// Keys held for a mono synth, most recent last.
    held: Vec<(u8, u8)>,
    age: u64,
}

struct Voice {
    key: u8,
    /// Which key was pressed (a drum's pitch may differ).
    pressed: u8,
    vel: f32,
    age: u64,
    released: bool,
    body: Body,
}

enum Body {
    Synth(Box<SynthVoice>),
    Drum(Box<DrumVoice>),
}

impl Player {
    pub fn new(rate: f32, seed: u32) -> Player {
        Player {
            voices: Vec::with_capacity(32),
            rate,
            rng: Rng::new(seed.wrapping_mul(2654435761) | 1),
            lfo_phase: [0.0; 4],
            lfo_hold: [0.0; 4],
            last_key: -1.0,
            held: Vec::new(),
            age: 0,
        }
    }

    pub fn active(&self) -> usize {
        self.voices.len()
    }

    pub fn note_on(&mut self, inst: &Instrument, key: u8, vel: u8) {
        self.age += 1;
        let vel = vel.clamp(1, 127) as f32 / 127.0;
        match inst {
            Instrument::Synth(s) => {
                if s.mono {
                    self.held.retain(|h| h.0 != key);
                    self.held.push((key, (vel * 127.0) as u8));
                    let legato = self.voices.iter().any(|v| !v.released);
                    if let Some(v) = self.voices.iter_mut().find(|v| matches!(v.body, Body::Synth(_))) {
                        v.key = key;
                        v.pressed = key;
                        v.vel = vel;
                        v.age = self.age;
                        v.released = false;
                        if let Body::Synth(sv) = &mut v.body {
                            sv.retarget(s, key as f32, vel, !legato, self.rate);
                        }
                        self.last_key = key as f32;
                        return;
                    }
                }
                let limit = if s.mono { 1 } else { s.voices.clamp(1, 32) as usize };
                self.make_room(limit, key);
                let from = if s.glide > 0.0 && self.last_key >= 0.0 { self.last_key } else { key as f32 };
                let sv = SynthVoice::new(s, key as f32, from, vel, self.rate, &mut self.rng, &self.lfo_phase);
                self.voices.push(Voice { key, pressed: key, vel, age: self.age, released: false, body: Body::Synth(Box::new(sv)) });
                self.last_key = key as f32;
            }
            Instrument::Kit(kit) => {
                let Some(drum) = kit.drum_for(key) else { return };
                if drum.choke != 0 {
                    for v in self.voices.iter_mut() {
                        if let Body::Drum(d) = &mut v.body {
                            if kit.drum_for(v.pressed).map(|o| o.choke) == Some(drum.choke) {
                                d.choke(self.rate);
                            }
                        }
                    }
                }
                // A drum retriggered cuts its own last hit short, as a real one would.
                for v in self.voices.iter_mut() {
                    if v.pressed == key {
                        if let Body::Drum(d) = &mut v.body {
                            d.choke(self.rate);
                        }
                    }
                }
                self.voices.retain(|v| !matches!(&v.body, Body::Drum(d) if d.done()));
                if self.voices.len() >= 48 {
                    self.voices.remove(0);
                }
                let dv = DrumVoice::new(drum, key, vel, self.rate, self.rng.next_u32());
                self.voices.push(Voice { key, pressed: key, vel, age: self.age, released: true, body: Body::Drum(Box::new(dv)) });
            }
        }
    }

    fn make_room(&mut self, limit: usize, key: u8) {
        // The same key sounding again takes over that voice's slot.
        for v in self.voices.iter_mut() {
            if v.key == key && !v.released {
                v.released = true;
                if let Body::Synth(s) = &mut v.body {
                    s.release();
                }
            }
        }
        let live = |vs: &Vec<Voice>| vs.iter().filter(|v| !matches!(&v.body, Body::Synth(s) if s.stealing)).count();
        while live(&self.voices) >= limit {
            // Steal: released voices first, oldest first.
            let pick = self
                .voices
                .iter()
                .enumerate()
                .filter(|(_, v)| !matches!(&v.body, Body::Synth(s) if s.stealing))
                .min_by_key(|(_, v)| (!v.released, v.age))
                .map(|(i, _)| i);
            match pick {
                Some(i) => {
                    if let Body::Synth(s) = &mut self.voices[i].body {
                        s.steal(self.rate);
                    } else {
                        self.voices.remove(i);
                    }
                }
                None => break,
            }
        }
    }

    pub fn note_off(&mut self, inst: &Instrument, key: u8) {
        if let Instrument::Synth(s) = inst {
            if s.mono {
                self.held.retain(|h| h.0 != key);
                if let Some(&(back, vel)) = self.held.last() {
                    // Fall back to the key still held, legato.
                    if let Some(v) = self.voices.iter_mut().find(|v| !v.released) {
                        if v.key == key {
                            v.key = back;
                            v.pressed = back;
                            if let Body::Synth(sv) = &mut v.body {
                                sv.retarget(s, back as f32, vel as f32 / 127.0, false, self.rate);
                            }
                            self.last_key = back as f32;
                        }
                    }
                    return;
                }
            }
        }
        for v in self.voices.iter_mut() {
            if v.key == key && !v.released {
                v.released = true;
                if let Body::Synth(s) = &mut v.body {
                    s.release();
                }
            }
        }
    }

    pub fn all_off(&mut self) {
        self.held.clear();
        for v in self.voices.iter_mut() {
            v.released = true;
            if let Body::Synth(s) = &mut v.body {
                s.release();
            }
        }
    }

    /// Silence at once (a few ms of fade): stop pressed twice, or a song swap.
    pub fn panic(&mut self) {
        self.held.clear();
        for v in self.voices.iter_mut() {
            v.released = true;
            match &mut v.body {
                Body::Synth(s) => s.steal(self.rate),
                Body::Drum(d) => d.choke(self.rate),
            }
        }
    }

    /// Adds this instrument's output for the block into `out`.
    pub fn render(&mut self, inst: &Instrument, out: &mut [[f32; 2]], mods: Mods) {
        let n = out.len();
        match inst {
            Instrument::Synth(s) => {
                // Advance the shared LFOs by the block and pass their per-block values.
                let mut lfo_vals = [0.0f32; 4];
                for (i, l) in s.lfos.iter().take(4).enumerate() {
                    let hz = if l.sync { l.rate * self.rate / mods.beat.max(1.0) } else { l.rate };
                    let before = self.lfo_phase[i];
                    self.lfo_phase[i] = (before + hz * n as f32 / self.rate).fract();
                    if self.lfo_phase[i] < before {
                        self.lfo_hold[i] = self.rng.bipolar();
                    }
                    lfo_vals[i] = lfo_value(l.shape, self.lfo_phase[i], self.lfo_hold[i]);
                }
                for v in self.voices.iter_mut() {
                    if let Body::Synth(sv) = &mut v.body {
                        sv.render(s, out, mods, &lfo_vals, self.rate, &mut self.rng);
                    }
                }
                self.voices.retain(|v| match &v.body {
                    Body::Synth(s) => !s.done(),
                    Body::Drum(_) => false,
                });
            }
            Instrument::Kit(kit) => {
                for v in self.voices.iter_mut() {
                    if let Body::Drum(d) = &mut v.body {
                        let drum = kit.drum_for(v.pressed);
                        match drum {
                            Some(drum) => d.render(drum, out, self.rate),
                            None => d.kill(),
                        }
                    }
                }
                self.voices.retain(|v| match &v.body {
                    Body::Drum(d) => !d.done(),
                    Body::Synth(_) => false,
                });
                let _ = v_unused(kit);
            }
        }
    }
}

fn v_unused(_: &Kit) {}

#[inline]
fn lfo_value(shape: LfoShape, phase: f32, hold: f32) -> f32 {
    match shape {
        LfoShape::Sine => crate::dsp::sin_cycles(phase),
        LfoShape::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
        LfoShape::Saw => 1.0 - 2.0 * phase,
        LfoShape::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        LfoShape::Hold => hold,
    }
}

struct SynthVoice {
    key: f32,
    /// The pitch now, gliding to `key`.
    pitch: f32,
    glide_coef: f32,
    vel: f32,
    amp: Adsr,
    menv: Adsr,
    oscs: [[Phase; MAX_UNISON]; MAX_OSCS],
    filter: [ModeFilter; 2],
    /// A gentle fixed high-pass removing sub-sonic rumble from detuned stacks.
    dc: [Svf; 2],
    age_samples: u32,
    /// Own LFO phases for retriggered LFOs.
    lfo_phase: [f32; 4],
    stealing: bool,
    released: bool,
    /// Smoothed cutoff in Hz, so jumps in modulation do not click.
    cut_smooth: f32,
}

impl SynthVoice {
    fn new(s: &Synth, key: f32, from: f32, vel: f32, rate: f32, rng: &mut Rng, lfo: &[f32; 4]) -> SynthVoice {
        let mut amp = Adsr::default();
        amp.set(&s.amp, rate);
        amp.gate_on(true);
        let mut menv = Adsr::default();
        menv.set(&s.mod_env, rate);
        menv.gate_on(true);
        let mut oscs = [[Phase::default(); MAX_UNISON]; MAX_OSCS];
        for (o, row) in s.oscs.iter().zip(oscs.iter_mut()) {
            for p in row.iter_mut() {
                *p = Phase::with(if o.retrigger { 0.0 } else { rng.unit() });
            }
        }
        let mut dc = [Svf::default(); 2];
        for d in dc.iter_mut() {
            d.set(18.0, 0.0, rate);
        }
        let mut lfo_phase = *lfo;
        for (i, l) in s.lfos.iter().take(4).enumerate() {
            if l.retrigger {
                lfo_phase[i] = 0.0;
            }
        }
        SynthVoice {
            key,
            pitch: from,
            glide_coef: crate::dsp::one_pole(s.glide * 0.4, rate),
            vel,
            amp,
            menv,
            oscs,
            filter: Default::default(),
            dc,
            age_samples: 0,
            lfo_phase,
            stealing: false,
            released: false,
            cut_smooth: 0.0,
        }
    }

    fn retarget(&mut self, s: &Synth, key: f32, vel: f32, restart: bool, rate: f32) {
        self.key = key;
        self.vel = vel;
        self.released = false;
        self.stealing = false;
        self.glide_coef = crate::dsp::one_pole(s.glide * 0.4, rate);
        if s.glide <= 0.0 {
            self.pitch = key;
        }
        self.amp.set(&s.amp, rate);
        self.menv.set(&s.mod_env, rate);
        if restart {
            self.amp.gate_on(false);
            self.menv.gate_on(false);
            self.age_samples = 0;
        }
    }

    fn release(&mut self) {
        self.released = true;
        self.amp.gate_off();
        self.menv.gate_off();
    }

    fn steal(&mut self, rate: f32) {
        self.stealing = true;
        self.released = true;
        self.amp.kill(rate);
        self.menv.gate_off();
    }

    fn done(&self) -> bool {
        self.amp.is_idle() && self.age_samples > 0
    }

    fn render(&mut self, s: &Synth, out: &mut [[f32; 2]], mods: Mods, shared_lfo: &[f32; 4], rate: f32, rng: &mut Rng) {
        if !self.stealing {
            // Settings may have changed under a held note.
            self.amp.set(&s.amp, rate);
            self.menv.set(&s.mod_env, rate);
        }
        let n = out.len();
        // Per-block LFO values: shared or this voice's own when retriggered.
        let mut lv = [0.0f32; 4];
        let fade_in = |l: &crate::patch::Lfo, age: u32| -> f32 {
            if l.delay <= 0.0 { 1.0 } else { (age as f32 / (l.delay * rate)).min(1.0) }
        };
        for (i, l) in s.lfos.iter().take(4).enumerate() {
            let v = if l.retrigger {
                let hz = if l.sync { l.rate * rate / mods.beat.max(1.0) } else { l.rate };
                self.lfo_phase[i] = (self.lfo_phase[i] + hz * n as f32 / rate).fract();
                lfo_value(l.shape, self.lfo_phase[i], rng.bipolar())
            } else {
                shared_lfo[i]
            };
            lv[i] = v * l.amount * fade_in(l, self.age_samples);
        }
        let mut pitch_mod = 0.0;
        let mut cut_mod = 0.0;
        let mut amp_mod = 0.0f32;
        let mut pan_mod = 0.0;
        let mut shape_mod = 0.0;
        for (i, l) in s.lfos.iter().take(4).enumerate() {
            match l.to {
                LfoTo::Pitch => pitch_mod += lv[i],
                LfoTo::Cutoff => cut_mod += lv[i],
                LfoTo::Amp => amp_mod += lv[i].abs().min(1.0) * 0.5 + lv[i] * 0.5,
                LfoTo::Pan => pan_mod += lv[i],
                LfoTo::Shape => shape_mod += lv[i],
            }
        }
        let vel_gain = 1.0 - s.velocity.clamp(0.0, 1.0) * (1.0 - self.vel);
        let (pl, pr) = pan_gains(pan_mod.clamp(-1.0, 1.0));
        let (pl, pr) = (pl * std::f32::consts::SQRT_2, pr * std::f32::consts::SQRT_2);
        let f = &s.filter;
        let base_cut = f.cutoff.max(20.0);
        let key_oct = (self.key - 60.0) / 12.0 * f.keytrack;
        let vel_oct = f.velocity * (self.vel - 0.5) * 2.0;
        let drive = 1.0 + f.drive.clamp(0.0, 1.0) * 5.0;
        let punch_coef = if s.punch_time > 0.0 { (-1.0 / (s.punch_time * rate)).exp() } else { 0.0 };
        let osc_count = s.oscs.len().min(MAX_OSCS);
        let total_gain = s.gain * 0.35;
        // Each copy's pitch as a ratio of the note's, and its place in the field, once a block.
        let mut ratio = [[1.0f32; MAX_UNISON]; MAX_OSCS];
        let mut place = [[(1.0f32, 1.0f32); MAX_UNISON]; MAX_OSCS];
        for oi in 0..osc_count {
            let o = &s.oscs[oi];
            let u = (o.unison as usize).clamp(1, MAX_UNISON);
            let norm = o.gain / (u as f32).sqrt();
            for k in 0..u {
                // Spread copies evenly across ±detune, and across the field.
                let off = if u == 1 { 0.0 } else { k as f32 / (u - 1) as f32 * 2.0 - 1.0 };
                let semis = o.octave as f32 * 12.0 + o.semi as f32 + (o.fine + off * o.detune) * 0.01;
                ratio[oi][k] = 2f32.powf(semis / 12.0) / rate;
                let pan = off * o.width;
                place[oi][k] = ((1.0 - pan).min(1.0) * norm, (1.0 + pan).min(1.0) * norm);
            }
        }
        let mut punch = if s.punch != 0.0 && s.punch_time > 0.0 {
            -s.punch * punch_coef.powf(self.age_samples as f32)
        } else {
            0.0
        };
        let mut last_pitch = f32::NAN;
        let mut hz_now = 0.0;
        let amp_gain = vel_gain * total_gain * (1.0 - amp_mod.clamp(0.0, 1.0));
        let (dl, dr) = (drive, drive.sqrt());
        // In blocks of up to 64: envelopes and pitch first, then each oscillator copy in its
        // own tight loop, then the filter.
        for block in out.chunks_mut(64) {
            let bn = block.len();
            let mut amp = [0.0f32; 64];
            let mut menv = [0.0f32; 64];
            let mut hz = [0.0f32; 64];
            for i in 0..bn {
                amp[i] = self.amp.next();
                menv[i] = self.menv.next();
                self.pitch += (self.key - self.pitch) * self.glide_coef;
                punch *= punch_coef;
                let pitch = self.pitch + pitch_mod + punch;
                if pitch != last_pitch {
                    hz_now = midi_hz(pitch);
                    last_pitch = pitch;
                }
                hz[i] = hz_now;
            }
            let mut l = [0.0f32; 64];
            let mut r = [0.0f32; 64];
            for oi in 0..osc_count {
                let o = &s.oscs[oi];
                if o.gain <= 0.0 {
                    continue;
                }
                let u = (o.unison as usize).clamp(1, MAX_UNISON);
                let shape = (o.shape + shape_mod * 0.5).clamp(0.0, 1.0);
                for k in 0..u {
                    self.oscs[oi][k].run(
                        o.wave,
                        &hz[..bn],
                        ratio[oi][k],
                        shape,
                        o.ratio,
                        &menv[..bn],
                        rng,
                        &mut l[..bn],
                        &mut r[..bn],
                        place[oi][k],
                    );
                }
            }
            for i in 0..bn {
                self.age_samples = self.age_samples.saturating_add(1);
                // Filter: cutoff in octaves from every source, smoothed in Hz, set every 8 samples.
                if self.age_samples % 8 == 1 {
                    let oct = key_oct + vel_oct + f.env * menv[i] + cut_mod + mods.cutoff;
                    let target = (base_cut * 2f32.powf(oct)).clamp(20.0, rate * 0.45);
                    if self.cut_smooth <= 0.0 {
                        self.cut_smooth = target;
                    }
                    self.cut_smooth += (target - self.cut_smooth) * 0.15;
                    for fl in self.filter.iter_mut() {
                        fl.vowel = f.vowel;
                        fl.set(f.mode, self.cut_smooth, f.resonance, rate);
                    }
                }
                let (mut fl, mut fr) = if f.drive > 0.0 {
                    (soft_clip(l[i] * dl) / dr, soft_clip(r[i] * dl) / dr)
                } else {
                    (l[i], r[i])
                };
                fl = self.filter[0].process(fl, f.mode);
                fr = self.filter[1].process(fr, f.mode);
                if f.mode != FilterMode::HighPass {
                    fl = self.dc[0].tick(fl).2;
                    fr = self.dc[1].tick(fr).2;
                }
                let g = amp[i] * amp_gain;
                block[i][0] += fl * g * pl;
                block[i][1] += fr * g * pr;
            }
        }
        self.filter[0].sanitise();
        self.filter[1].sanitise();
        self.dc[0].sanitise();
        self.dc[1].sanitise();
    }
}

struct DrumVoice {
    t: u32,
    /// Pitch ratio (tuned drums) from the key played.
    tune: f32,
    vel: f32,
    phase: f32,
    phase2: f32,
    body_hz: f32,
    rng: Rng,
    hiss: ModeFilter,
    ring_phase: [f32; 6],
    ring_hp: [Svf; 2],
    ring_bp: Svf,
    tone: OnePole,
    /// Fade applied when choked; 1 = none.
    choke_gain: f32,
    choke_coef: f32,
    finished: bool,
    body_env: f32,
    hiss_env: f32,
    ring_env: f32,
    burst: u32,
}

impl DrumVoice {
    fn new(d: &Drum, key: u8, vel: f32, rate: f32, seed: u32) -> DrumVoice {
        let tune = if d.fixed { 1.0 } else { 2f32.powf((key as f32 - d.key as f32) / 12.0) };
        let mut hiss = ModeFilter::default();
        if let Some(h) = &d.hiss {
            // Brighter the harder it is hit.
            let bright = 1.0 + (1.0 - d.velocity) * (vel - 0.7) * 0.8;
            hiss.set(h.mode, h.cutoff * bright.max(0.4), h.resonance, rate);
        }
        let mut ring_hp = [Svf::default(); 2];
        let mut ring_bp = Svf::default();
        if let Some(r) = &d.ring {
            for h in ring_hp.iter_mut() {
                h.set(r.highpass, 0.1, rate);
            }
            ring_bp.set(r.freq * 11.0, 0.3, rate);
        }
        let mut tone = OnePole::default();
        tone.set(3000.0 + 15000.0 * vel, rate);
        DrumVoice {
            t: 0,
            tune,
            vel,
            phase: 0.0,
            phase2: 0.0,
            body_hz: d.body.as_ref().map(|b| b.from * tune).unwrap_or(0.0),
            rng: Rng::new(seed | 1),
            hiss,
            ring_phase: [0.0; 6],
            ring_hp,
            ring_bp,
            tone,
            choke_gain: 1.0,
            choke_coef: 1.0,
            finished: false,
            body_env: 1.0,
            hiss_env: 0.0,
            ring_env: 1.0,
            burst: 0,
        }
    }

    fn choke(&mut self, rate: f32) {
        self.choke_coef = (-1.0 / (0.006 * rate)).exp();
    }
    fn kill(&mut self) {
        self.finished = true;
    }
    fn done(&self) -> bool {
        self.finished
    }

    fn render(&mut self, d: &Drum, out: &mut [[f32; 2]], rate: f32) {
        if self.finished {
            return;
        }
        let level = d.gain * (1.0 - d.velocity * (1.0 - self.vel)) * 0.8;
        let (pl, pr) = pan_gains(d.pan);
        let (pl, pr) = (pl * std::f32::consts::SQRT_2, pr * std::f32::consts::SQRT_2);
        let inv = 1.0 / rate;
        let drive = 1.0 + d.drive * 8.0;
        // Per-sample decay coefficients (to a third over `decay`).
        let k = |secs: f32| (-1.1 / (secs.max(0.001) * rate)).exp();
        let body = d.body.as_ref().map(|b| (b, k(b.decay), (-1.0 / (b.sweep.max(0.0005) * rate)).exp()));
        let hiss = d.hiss.as_ref().map(|h| (h, k(h.decay), (h.spread.max(0.0) * rate) as u32));
        let ring = d.ring.as_ref().map(|r| (r, k(r.decay)));
        const RATIOS: [f32; 6] = [1.0, 1.4471, 1.6170, 1.9265, 2.5028, 2.6637];
        let mut loudest = 0.0f32;
        for f in out.iter_mut() {
            if self.choke_coef < 1.0 {
                self.choke_gain *= self.choke_coef;
            }
            let secs = self.t as f32 * inv;
            let mut x = 0.0;
            if let Some((b, dk, sk)) = body {
                let to = b.to * self.tune;
                self.body_hz = to + (self.body_hz - to) * sk;
                self.phase = (self.phase + self.body_hz * inv).fract();
                let mut v = crate::dsp::sin_cycles(self.phase);
                if b.overtone > 0.0 {
                    self.phase2 = (self.phase2 + self.body_hz * b.overtone * inv).fract();
                    v = v * 0.75 + crate::dsp::sin_cycles(self.phase2) * 0.35;
                }
                // A 1 ms fade-in removes the start click unless one is asked for.
                let fade = (secs * 1000.0).min(1.0);
                x += v * self.body_env * b.gain * fade;
                self.body_env *= dk;
            }
            if let Some((h, dk, gap)) = hiss {
                if self.t == 0 || (gap > 0 && self.burst + 1 < h.bursts && self.t == (self.burst + 1) * gap) {
                    if self.t != 0 {
                        self.burst += 1;
                    }
                    self.hiss_env = 1.0;
                }
                let atk = if h.attack > 0.0 { (secs / h.attack).min(1.0) } else { 1.0 };
                let n = self.rng.bipolar();
                x += self.hiss.process(n, h.mode) * self.hiss_env * h.gain * atk * 1.6;
                self.hiss_env *= dk;
            }
            if let Some((r, dk)) = ring {
                let mut sq = 0.0;
                for (i, p) in self.ring_phase.iter_mut().enumerate() {
                    *p = (*p + r.freq * RATIOS[i] * self.tune * inv).fract();
                    sq += if *p < 0.5 { 1.0 } else { -1.0 };
                }
                let mut v = self.ring_hp[0].tick(sq / 6.0).2;
                v = self.ring_hp[1].tick(v).2;
                v += self.ring_bp.tick(sq / 6.0).1 * 0.3;
                x += v * self.ring_env * r.gain * 1.5;
                self.ring_env *= dk;
            }
            if d.click > 0.0 && self.t < (0.002 * rate) as u32 {
                let c = 1.0 - self.t as f32 / (0.002 * rate);
                x += self.rng.bipolar() * c * c * d.click;
            }
            if d.drive > 0.0 {
                x = soft_clip(x * drive) / drive.sqrt().max(1.0) * 1.3;
            }
            x = self.tone.process(x);
            let v = x * level * self.choke_gain;
            f[0] += v * pl;
            f[1] += v * pr;
            loudest = loudest.max(self.body_env.max(self.hiss_env).max(self.ring_env * ring.is_some() as u8 as f32));
            self.t += 1;
        }
        // Done at -60 dB: the rest is below anything the mix can show.
        let tails_done = body.is_none_or(|_| self.body_env < 1e-3)
            && hiss.is_none_or(|(h, _, _)| self.hiss_env < 1e-3 && self.burst + 1 >= h.bursts)
            && ring.is_none_or(|_| self.ring_env < 1e-3);
        if tails_done || self.choke_gain < 1e-4 || self.t > (rate * 12.0) as u32 {
            self.finished = true;
        }
        let _ = loudest;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::*;

    #[test]
    fn a_synth_note_sounds_and_dies_after_release() {
        let inst = Instrument::Synth(Synth::default());
        let mut p = Player::new(48000.0, 1);
        p.note_on(&inst, 57, 100);
        let mods = Mods { cutoff: 0.0, beat: 24000.0 };
        let mut buf = vec![[0.0f32; 2]; 4800];
        p.render(&inst, &mut buf, mods);
        let peak = buf.iter().map(|f| f[0].abs()).fold(0.0, f32::max);
        assert!(peak > 0.05, "peak {peak}");
        p.note_off(&inst, 57);
        for _ in 0..40 {
            buf.fill([0.0; 2]);
            p.render(&inst, &mut buf, mods);
        }
        assert_eq!(p.active(), 0);
    }

    #[test]
    fn a_kick_ends_by_itself() {
        let kit = Kit {
            drums: vec![Drum {
                name: "Kick".into(),
                key: 36,
                body: Some(crate::patch::Body { from: 160.0, to: 48.0, sweep: 0.03, decay: 0.3, gain: 1.0, overtone: 0.0 }),
                hiss: None,
                ring: None,
                click: 0.3,
                drive: 0.2,
                gain: 1.0,
                pan: 0.0,
                choke: 0,
                velocity: 0.5,
                fixed: true,
            }],
        };
        let inst = Instrument::Kit(kit);
        let mut p = Player::new(48000.0, 1);
        p.note_on(&inst, 36, 120);
        let mods = Mods { cutoff: 0.0, beat: 24000.0 };
        let mut buf = vec![[0.0f32; 2]; 96000];
        p.render(&inst, &mut buf, mods);
        let peak = buf.iter().map(|f| f[0].abs()).fold(0.0, f32::max);
        assert!(peak > 0.2);
        buf.fill([0.0; 2]);
        p.render(&inst, &mut buf, mods);
        assert_eq!(p.active(), 0);
    }
}
