//! Instruments and effects as they are written down.
//!
//! Every sound is synthesised, like the game's sound recipes: there are no
//! samples. A `Synth` is a polyphonic subtractive/FM voice (three oscillators,
//! a filter, two envelopes, two LFOs); a `Kit` is drums, one synthesised pad
//! per key, each made of a pitched body, filtered noise and a metallic ring.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Instrument {
    Synth(Synth),
    Kit(Kit),
}

impl Instrument {
    pub fn kind_name(&self) -> &'static str {
        match self {
            Instrument::Synth(_) => "Synth",
            Instrument::Kit(_) => "Drum kit",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Wave {
    Sine,
    Triangle,
    #[default]
    Saw,
    Square,
    /// A square whose width `Osc::shape` sets (0.5 = square), and the PWM LFO moves.
    Pulse,
    /// A sine phase-modulated by a sine at `Osc::ratio` times its pitch, `Osc::shape` deep
    /// (0..1 = index 0..8), the depth following the filter envelope: bells, metal, reeds.
    Fm,
    /// A sine folded back on itself `shape` hard: hollow, then buzzing.
    Fold,
    /// White noise (pitch ignored).
    Noise,
}

impl Wave {
    pub const ALL: [Wave; 8] = [
        Wave::Sine,
        Wave::Triangle,
        Wave::Saw,
        Wave::Square,
        Wave::Pulse,
        Wave::Fm,
        Wave::Fold,
        Wave::Noise,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Wave::Sine => "Sine",
            Wave::Triangle => "Triangle",
            Wave::Saw => "Saw",
            Wave::Square => "Square",
            Wave::Pulse => "Pulse",
            Wave::Fm => "FM",
            Wave::Fold => "Fold",
            Wave::Noise => "Noise",
        }
    }
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}
fn is_zero_i(v: &i32) -> bool {
    *v == 0
}
fn is_one_u(v: &u32) -> bool {
    *v == 1
}
fn one_u() -> u32 {
    1
}
fn half() -> f32 {
    0.5
}
fn one() -> f32 {
    1.0
}
fn is_false(v: &bool) -> bool {
    !*v
}
fn yes() -> bool {
    true
}
fn is_true(v: &bool) -> bool {
    *v
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Osc {
    pub wave: Wave,
    /// Linear level 0..1.
    pub gain: f32,
    #[serde(default, skip_serializing_if = "is_zero_i")]
    pub octave: i32,
    #[serde(default, skip_serializing_if = "is_zero_i")]
    pub semi: i32,
    /// Cents.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fine: f32,
    /// Pulse width, FM depth or fold, by wave.
    #[serde(default = "half")]
    pub shape: f32,
    /// FM modulator ratio.
    #[serde(default = "one")]
    pub ratio: f32,
    /// Copies of the oscillator, spread in pitch by `detune` cents and across the
    /// stereo field by `width` (0..1): one saw becomes a supersaw.
    #[serde(default = "one_u", skip_serializing_if = "is_one_u")]
    pub unison: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub detune: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub width: f32,
    /// Starts every note at the same phase (tight attacks) instead of free-running.
    #[serde(default, skip_serializing_if = "is_false")]
    pub retrigger: bool,
}

impl Osc {
    pub fn new(wave: Wave, gain: f32) -> Osc {
        Osc {
            wave,
            gain,
            octave: 0,
            semi: 0,
            fine: 0.0,
            shape: 0.5,
            ratio: 1.0,
            unison: 1,
            detune: 0.0,
            width: 0.0,
            retrigger: false,
        }
    }
}

/// Attack, decay and release in seconds, sustain as a level 0..1.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Env {
    pub a: f32,
    pub d: f32,
    pub s: f32,
    pub r: f32,
}

impl Env {
    pub const fn new(a: f32, d: f32, s: f32, r: f32) -> Env {
        Env { a, d, s, r }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FilterMode {
    #[default]
    LowPass,
    BandPass,
    HighPass,
    Notch,
    /// Two low-pass stages: 24 dB/oct, darker and rounder.
    LowPass4,
    /// Three resonances where a voice has them, so a saw sings a vowel (`Filter::vowel`).
    /// `cutoff` sizes the throat: 1000 is a man's voice, higher is smaller and brighter.
    Formant,
}

impl FilterMode {
    pub const ALL: [FilterMode; 6] = [
        FilterMode::LowPass,
        FilterMode::LowPass4,
        FilterMode::BandPass,
        FilterMode::HighPass,
        FilterMode::Notch,
        FilterMode::Formant,
    ];
    pub fn name(self) -> &'static str {
        match self {
            FilterMode::LowPass => "Low pass 12",
            FilterMode::LowPass4 => "Low pass 24",
            FilterMode::BandPass => "Band pass",
            FilterMode::HighPass => "High pass",
            FilterMode::Notch => "Notch",
            FilterMode::Formant => "Voice",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Filter {
    #[serde(default)]
    pub mode: FilterMode,
    /// Hz.
    pub cutoff: f32,
    /// 0..1; near 1 it sings.
    #[serde(default)]
    pub resonance: f32,
    /// Octaves the filter envelope opens it by (may be negative).
    #[serde(default)]
    pub env: f32,
    /// 0..1: how far the cutoff follows the note (1 = an octave per octave).
    #[serde(default)]
    pub keytrack: f32,
    /// Octaves added at full velocity.
    #[serde(default)]
    pub velocity: f32,
    /// Saturation into the filter, 0..1.
    #[serde(default)]
    pub drive: f32,
    /// For `Formant`: 0 "ah", 0.25 "eh", 0.5 "ee", 0.75 "oh", 1 "oo", blended between.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub vowel: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LfoShape {
    #[default]
    Sine,
    Triangle,
    Saw,
    Square,
    /// A new random value each cycle.
    Hold,
}

impl LfoShape {
    pub const ALL: [LfoShape; 5] = [
        LfoShape::Sine,
        LfoShape::Triangle,
        LfoShape::Saw,
        LfoShape::Square,
        LfoShape::Hold,
    ];
    pub fn name(self) -> &'static str {
        match self {
            LfoShape::Sine => "Sine",
            LfoShape::Triangle => "Triangle",
            LfoShape::Saw => "Saw",
            LfoShape::Square => "Square",
            LfoShape::Hold => "Sample & hold",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LfoTo {
    /// Semitones.
    #[default]
    Pitch,
    /// Octaves.
    Cutoff,
    /// 0..1 of the level.
    Amp,
    Pan,
    /// Oscillator `shape`.
    Shape,
}

impl LfoTo {
    pub const ALL: [LfoTo; 5] = [
        LfoTo::Pitch,
        LfoTo::Cutoff,
        LfoTo::Amp,
        LfoTo::Pan,
        LfoTo::Shape,
    ];
    pub fn name(self) -> &'static str {
        match self {
            LfoTo::Pitch => "Pitch",
            LfoTo::Cutoff => "Cutoff",
            LfoTo::Amp => "Amp",
            LfoTo::Pan => "Pan",
            LfoTo::Shape => "Shape",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lfo {
    #[serde(default)]
    pub shape: LfoShape,
    pub to: LfoTo,
    /// Hz, or when `sync` is set, cycles per beat.
    pub rate: f32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub sync: bool,
    pub amount: f32,
    /// Seconds for the LFO to fade in after a note starts.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub delay: f32,
    /// Restarts with every note instead of running freely.
    #[serde(default, skip_serializing_if = "is_false")]
    pub retrigger: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Synth {
    pub oscs: Vec<Osc>,
    pub filter: Filter,
    pub amp: Env,
    /// The filter envelope; FM oscillators take their depth from it too.
    pub mod_env: Env,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lfos: Vec<Lfo>,
    /// Seconds a new note's pitch slides from the last one; 0 = none.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub glide: f32,
    /// One note at a time (legato: a note held into the next does not restart the envelopes).
    #[serde(default, skip_serializing_if = "is_false")]
    pub mono: bool,
    /// Voices at once; the oldest is stolen past this.
    #[serde(default = "voices")]
    pub voices: u32,
    /// 0..1: how much velocity sets the level.
    #[serde(default = "half")]
    pub velocity: f32,
    /// Linear output gain.
    #[serde(default = "one")]
    pub gain: f32,
    /// A pitch drop at the start of each note, semitones, falling away over `punch_time`.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub punch: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub punch_time: f32,
}

fn voices() -> u32 {
    12
}

impl Default for Synth {
    fn default() -> Synth {
        Synth {
            oscs: vec![Osc::new(Wave::Saw, 0.6)],
            filter: Filter {
                mode: FilterMode::LowPass,
                cutoff: 2400.0,
                resonance: 0.2,
                env: 1.5,
                keytrack: 0.4,
                velocity: 0.5,
                drive: 0.0,
                vowel: 0.0,
            },
            amp: Env::new(0.005, 0.3, 0.7, 0.25),
            mod_env: Env::new(0.002, 0.4, 0.2, 0.3),
            lfos: Vec::new(),
            glide: 0.0,
            mono: false,
            voices: 12,
            velocity: 0.5,
            gain: 1.0,
            punch: 0.0,
            punch_time: 0.0,
        }
    }
}

/// The pitched part of a drum: a sine (or `Fm` metal) falling from `from` to `to` Hz.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Body {
    pub from: f32,
    pub to: f32,
    /// Seconds for the pitch to fall about two thirds of the way.
    pub sweep: f32,
    /// Seconds to fall to a third.
    pub decay: f32,
    pub gain: f32,
    /// A second sine at this multiple, for toms and struck plates; 0 = none.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub overtone: f32,
}

/// The noisy part: noise through a filter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hiss {
    #[serde(default)]
    pub mode: FilterMode,
    pub cutoff: f32,
    #[serde(default)]
    pub resonance: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub attack: f32,
    pub decay: f32,
    pub gain: f32,
    /// Retriggers this many times `spread` seconds apart: a clap.
    #[serde(default = "one_u", skip_serializing_if = "is_one_u")]
    pub bursts: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub spread: f32,
}

/// Six square waves at inharmonic ratios, as the classic machines made cymbals and bells.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ring {
    pub freq: f32,
    pub decay: f32,
    pub gain: f32,
    /// High-pass after the squares, Hz.
    #[serde(default = "ring_hp")]
    pub highpass: f32,
}

fn ring_hp() -> f32 {
    6000.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Drum {
    pub name: String,
    /// The MIDI key that plays it.
    pub key: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Body>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hiss: Option<Hiss>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ring: Option<Ring>,
    /// A 1-2 ms transient at the start, 0..1.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub click: f32,
    /// Saturation of the whole hit, 0..1.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub drive: f32,
    #[serde(default = "one")]
    pub gain: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pan: f32,
    /// Pads sharing a group cut each other off (open and closed hat); 0 = none.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub choke: u8,
    /// 0..1: how much velocity sets level (the rest brightens it).
    #[serde(default = "half")]
    pub velocity: f32,
    /// Plays at the note's pitch, relative to `key` (a tuned 808 or tom); otherwise fixed.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub fixed: bool,
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Kit {
    pub drums: Vec<Drum>,
}

impl Kit {
    pub fn drum_for(&self, key: u8) -> Option<&Drum> {
        self.drums.iter().find(|d| d.key == key)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Curve {
    /// Warm and even-ordered.
    #[default]
    Tape,
    /// Hard and odd-ordered.
    Clip,
    /// Folds back: metallic.
    Fold,
}

impl Curve {
    pub const ALL: [Curve; 3] = [Curve::Tape, Curve::Clip, Curve::Fold];
    pub fn name(self) -> &'static str {
        match self {
            Curve::Tape => "Tape",
            Curve::Clip => "Clip",
            Curve::Fold => "Fold",
        }
    }
}

/// An insert effect on a track, bus or the master, in chain order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    /// Shelves and a bell, dB and Hz.
    Eq {
        #[serde(default)]
        low_db: f32,
        #[serde(default = "eq_low")]
        low_hz: f32,
        #[serde(default)]
        mid_db: f32,
        #[serde(default = "eq_mid")]
        mid_hz: f32,
        #[serde(default = "eq_q")]
        mid_q: f32,
        #[serde(default)]
        high_db: f32,
        #[serde(default = "eq_high")]
        high_hz: f32,
        /// A 12 dB/oct high-pass under everything, Hz; 0 = off.
        #[serde(default)]
        cut_hz: f32,
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        on: bool,
    },
    Filter {
        mode: FilterMode,
        cutoff: f32,
        #[serde(default)]
        resonance: f32,
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        on: bool,
    },
    Drive {
        /// 0..1.
        amount: f32,
        #[serde(default)]
        curve: Curve,
        /// Low-pass after the curve, Hz.
        #[serde(default = "drive_tone")]
        tone: f32,
        #[serde(default = "one")]
        mix: f32,
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        on: bool,
    },
    Chorus {
        /// Hz.
        rate: f32,
        /// Milliseconds of sweep.
        depth: f32,
        mix: f32,
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        on: bool,
    },
    Delay {
        /// In beats (0.75 = dotted eighth).
        beats: f32,
        feedback: f32,
        mix: f32,
        /// Left and right repeats alternate.
        #[serde(default)]
        pingpong: bool,
        /// Low-pass in the feedback path, Hz: each repeat darker.
        #[serde(default = "delay_tone")]
        tone: f32,
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        on: bool,
    },
    /// A modulated feedback-delay-network hall.
    Reverb {
        /// 0..1: room size.
        size: f32,
        /// Seconds to fall 60 dB.
        decay: f32,
        /// Hz: how quickly the tail darkens.
        #[serde(default = "reverb_damp")]
        damp: f32,
        /// Milliseconds before the tail.
        #[serde(default)]
        predelay: f32,
        mix: f32,
        /// Low cut into the tail, Hz, so it does not muddy the bass.
        #[serde(default = "reverb_lowcut")]
        lowcut: f32,
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        on: bool,
    },
    Compressor {
        threshold: f32,
        ratio: f32,
        /// Milliseconds.
        attack: f32,
        release: f32,
        #[serde(default)]
        makeup: f32,
        /// Duck to another track's level (kick pumping a pad) instead of its own.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sidechain: Option<String>,
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        on: bool,
    },
    /// A brick-wall peak limiter with lookahead: nothing passes `ceiling` dB.
    Limiter {
        ceiling: f32,
        #[serde(default)]
        gain: f32,
        #[serde(default = "limiter_release")]
        release: f32,
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        on: bool,
    },
    /// Mid/side: 0 = mono, 1 = as is, 2 = twice as wide.
    Width {
        amount: f32,
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        on: bool,
    },
    Crush {
        bits: f32,
        /// Output rate, Hz.
        rate: f32,
        #[serde(default = "one")]
        mix: f32,
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        on: bool,
    },
}

fn eq_low() -> f32 {
    120.0
}
fn eq_mid() -> f32 {
    1000.0
}
fn eq_q() -> f32 {
    0.8
}
fn eq_high() -> f32 {
    6000.0
}
fn drive_tone() -> f32 {
    9000.0
}
fn delay_tone() -> f32 {
    4500.0
}
fn reverb_damp() -> f32 {
    5000.0
}
fn reverb_lowcut() -> f32 {
    180.0
}
fn limiter_release() -> f32 {
    80.0
}

impl Effect {
    pub fn name(&self) -> &'static str {
        match self {
            Effect::Eq { .. } => "EQ",
            Effect::Filter { .. } => "Filter",
            Effect::Drive { .. } => "Drive",
            Effect::Chorus { .. } => "Chorus",
            Effect::Delay { .. } => "Delay",
            Effect::Reverb { .. } => "Reverb",
            Effect::Compressor { .. } => "Compressor",
            Effect::Limiter { .. } => "Limiter",
            Effect::Width { .. } => "Width",
            Effect::Crush { .. } => "Crush",
        }
    }

    pub fn is_on(&self) -> bool {
        match self {
            Effect::Eq { on, .. }
            | Effect::Filter { on, .. }
            | Effect::Drive { on, .. }
            | Effect::Chorus { on, .. }
            | Effect::Delay { on, .. }
            | Effect::Reverb { on, .. }
            | Effect::Compressor { on, .. }
            | Effect::Limiter { on, .. }
            | Effect::Width { on, .. }
            | Effect::Crush { on, .. } => *on,
        }
    }

    pub fn set_on(&mut self, value: bool) {
        match self {
            Effect::Eq { on, .. }
            | Effect::Filter { on, .. }
            | Effect::Drive { on, .. }
            | Effect::Chorus { on, .. }
            | Effect::Delay { on, .. }
            | Effect::Reverb { on, .. }
            | Effect::Compressor { on, .. }
            | Effect::Limiter { on, .. }
            | Effect::Width { on, .. }
            | Effect::Crush { on, .. } => *on = value,
        }
    }

    /// A fresh effect of each kind with sensible settings, for the studio's add menu.
    pub fn defaults() -> Vec<Effect> {
        vec![
            Effect::Eq {
                low_db: 0.0,
                low_hz: 120.0,
                mid_db: 0.0,
                mid_hz: 1000.0,
                mid_q: 0.8,
                high_db: 0.0,
                high_hz: 6000.0,
                cut_hz: 0.0,
                on: true,
            },
            Effect::Filter {
                mode: FilterMode::LowPass,
                cutoff: 3000.0,
                resonance: 0.2,
                on: true,
            },
            Effect::Drive {
                amount: 0.3,
                curve: Curve::Tape,
                tone: 9000.0,
                mix: 1.0,
                on: true,
            },
            Effect::Chorus {
                rate: 0.4,
                depth: 4.0,
                mix: 0.4,
                on: true,
            },
            Effect::Delay {
                beats: 0.75,
                feedback: 0.35,
                mix: 0.25,
                pingpong: true,
                tone: 4500.0,
                on: true,
            },
            Effect::Reverb {
                size: 0.7,
                decay: 2.8,
                damp: 5000.0,
                predelay: 20.0,
                mix: 0.3,
                lowcut: 180.0,
                on: true,
            },
            Effect::Compressor {
                threshold: -18.0,
                ratio: 4.0,
                attack: 10.0,
                release: 120.0,
                makeup: 3.0,
                sidechain: None,
                on: true,
            },
            Effect::Limiter {
                ceiling: -1.0,
                gain: 0.0,
                release: 80.0,
                on: true,
            },
            Effect::Width {
                amount: 1.3,
                on: true,
            },
            Effect::Crush {
                bits: 10.0,
                rate: 16000.0,
                mix: 1.0,
                on: true,
            },
        ]
    }
}
