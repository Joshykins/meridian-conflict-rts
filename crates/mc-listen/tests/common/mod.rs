//! Songs with known answers, built in code and rendered by mc-music.
#![expect(
    dead_code,
    reason = "shared by several test binaries; each uses only some of the helpers"
)]

use mc_music::patch::{Env, FilterMode, Instrument, Osc, Synth, Wave};
use mc_music::song::{Clip, Master, Scale, Section, SectionKind, Track};
use mc_music::{Note, Pattern, Song, PPQ};

pub(crate) const RATE: u32 = 44100;

pub(crate) fn kit() -> Instrument {
    // Parsed from text so new optional fields in mc-music keep their defaults.
    let text = r#"Kit((drums: [
        (name: "Kick", key: 36, body: (from: 150.0, to: 46.0, sweep: 0.03, decay: 0.3, gain: 1.0), click: 0.35, drive: 0.25, gain: 0.95, velocity: 0.5),
        (name: "Snare", key: 38, body: (from: 235.0, to: 175.0, sweep: 0.02, decay: 0.1, gain: 0.55, overtone: 1.7),
            hiss: (mode: BandPass, cutoff: 3200.0, resonance: 0.15, decay: 0.17, gain: 1.0), click: 0.2, drive: 0.15, gain: 1.0, velocity: 0.6),
        (name: "Hat", key: 42, hiss: (mode: HighPass, cutoff: 8500.0, resonance: 0.2, decay: 0.03, gain: 0.6),
            ring: (freq: 520.0, decay: 0.03, gain: 0.3, highpass: 8000.0), gain: 0.5, pan: 0.2, choke: 1, velocity: 0.5),
    ]))"#;
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(text)
        .expect("kit parses")
}

pub(crate) fn bass_synth() -> Instrument {
    let mut s = Synth {
        oscs: vec![Osc::new(Wave::Saw, 0.6), Osc::new(Wave::Sine, 0.6)],
        amp: Env::new(0.005, 0.2, 0.8, 0.06),
        mod_env: Env::new(0.002, 0.15, 0.2, 0.1),
        mono: true,
        voices: 1,
        ..Synth::default()
    };
    s.filter.mode = FilterMode::LowPass4;
    s.filter.cutoff = 900.0;
    s.filter.resonance = 0.1;
    s.filter.env = 1.0;
    s.filter.keytrack = 0.3;
    s.filter.velocity = 0.3;
    s.filter.drive = 0.0;
    Instrument::Synth(s)
}

pub(crate) fn pad_synth() -> Instrument {
    let mut s = Synth {
        oscs: vec![
            Osc::new(Wave::Saw, 0.5),
            Osc {
                fine: 7.0,
                ..Osc::new(Wave::Saw, 0.3)
            },
        ],
        amp: Env::new(0.02, 0.4, 0.8, 0.2),
        voices: 12,
        ..Synth::default()
    };
    s.filter.mode = FilterMode::LowPass;
    s.filter.cutoff = 2200.0;
    s.filter.resonance = 0.1;
    s.filter.env = 0.5;
    s.filter.keytrack = 0.4;
    s.filter.velocity = 0.3;
    s.filter.drive = 0.0;
    Instrument::Synth(s)
}

pub(crate) fn lead_synth() -> Instrument {
    let mut s = Synth {
        oscs: vec![Osc::new(Wave::Square, 0.5)],
        amp: Env::new(0.01, 0.2, 0.8, 0.08),
        mono: true,
        voices: 1,
        ..Synth::default()
    };
    s.filter.mode = FilterMode::LowPass;
    s.filter.cutoff = 3000.0;
    s.filter.resonance = 0.1;
    s.filter.env = 0.5;
    s.filter.keytrack = 0.5;
    s.filter.velocity = 0.3;
    s.filter.drive = 0.0;
    Instrument::Synth(s)
}

pub(crate) fn track(name: &str, instrument: Instrument, db: f32) -> Track {
    Track {
        name: name.into(),
        instrument,
        db,
        pan: 0.0,
        mute: false,
        solo: false,
        sends: vec![],
        effects: vec![],
        layer: Default::default(),
        follow: vec![],
        colour: 0,
    }
}

/// Drum rows as the song scripts write them: {key: "x...x..."}, 16 steps a bar.
pub(crate) fn grid_pattern(name: &str, rows: &[(u8, &str)]) -> Pattern {
    let mut notes = Vec::new();
    let vel = |c: char| match c {
        'X' => Some(124),
        'x' => Some(100),
        'o' => Some(76),
        'g' => Some(48),
        _ => None,
    };
    let mut len = 0;
    for (key, s) in rows {
        let s: String = s.chars().filter(|c| *c != ' ' && *c != '|').collect();
        len = len.max(s.len());
        for (i, c) in s.chars().enumerate() {
            if let Some(v) = vel(c) {
                notes.push(Note(i as u32 * PPQ / 4, PPQ / 4, *key, v));
            }
        }
    }
    notes.sort_by_key(|n| (n.0, n.2));
    Pattern {
        name: name.into(),
        beats: (len as u32).div_ceil(4),
        notes,
        automation: vec![],
    }
}

/// A chord as (root pitch class, minor?, seventh?).
#[derive(Clone, Copy, Debug)]
pub(crate) struct ChordSpec {
    pub root: u8,
    pub minor: bool,
    pub seventh: bool,
}

pub(crate) fn ch(root: u8, minor: bool) -> ChordSpec {
    ChordSpec {
        root,
        minor,
        seventh: false,
    }
}

/// A song case with its answers.
pub(crate) struct Case {
    pub name: &'static str,
    pub tempo: f32,
    pub key_root: u8,
    pub minor: bool,
    pub prog: Vec<ChordSpec>,
    /// Bass rhythm over one bar: (step, length in steps, interval above the chord root in semitones).
    pub bass_rhythm: Vec<(u32, u32, i32)>,
    pub drums: [&'static str; 3],
    pub bars: u32,
    pub lead: bool,
    pub sidechain: bool,
}

pub(crate) struct Truth {
    pub song: Song,
    pub bass: Vec<Note>,
    /// kick/snare/hat hit masks per bar.
    pub drums: Vec<[Vec<bool>; 3]>,
    pub chords: Vec<ChordSpec>,
    pub lead: Vec<Note>,
}

pub(crate) fn build(c: &Case) -> Truth {
    let bar = 4 * PPQ;
    let n = c.prog.len() as u32;
    // Chord pad: triads (plus a 7th) voiced around C4, one per bar.
    let mut pad = Vec::new();
    let mut bass = Vec::new();
    for (i, chs) in c.prog.iter().enumerate() {
        let t = i as u32 * bar;
        let root = 48 + ((chs.root as i32).rem_euclid(12)) as u8; // C3..B3
        let third = if chs.minor { 3 } else { 4 };
        let mut keys = vec![root + 12, root + 12 + third, root + 12 + 7];
        if chs.seventh {
            keys.push(root + 12 + 10);
        }
        for k in keys {
            pad.push(Note(t, bar - 12, k, 90));
        }
        let broot = 36 + chs.root % 12; // C2..B2
        for &(s, l, iv) in &c.bass_rhythm {
            bass.push(Note(
                t + s * PPQ / 4,
                l * PPQ / 4 - 6,
                (broot as i32 + iv) as u8,
                105,
            ));
        }
    }
    let drums = grid_pattern(
        "beat",
        &[(36, c.drums[0]), (38, c.drums[1]), (42, c.drums[2])],
    );
    let mut lead_notes = Vec::new();
    if c.lead {
        // A simple line on chord tones, quarter and eighth notes.
        let scale: [u8; 7] = if c.minor {
            [0, 2, 3, 5, 7, 8, 10]
        } else {
            [0, 2, 4, 5, 7, 9, 11]
        };
        let mut deg = 4usize;
        for i in 0..n * 4 {
            let t = i * PPQ;
            let k = 72 + c.key_root % 12 + scale[deg % 7] + if deg >= 7 { 12 } else { 0 }
                - 12 * (c.key_root >= 6) as u8;
            lead_notes.push(Note(t, PPQ - 10, k, 100));
            deg = [2, 3, 4, 5, 6, 4, 3, 1][(i as usize * 3 + 1) % 8];
        }
    }
    let mut patterns = vec![
        drums,
        Pattern {
            name: "pad".into(),
            beats: 4 * n,
            notes: pad,
            automation: vec![],
        },
        Pattern {
            name: "bass".into(),
            beats: 4 * n,
            notes: bass.clone(),
            automation: vec![],
        },
    ];
    let mut clips = vec![
        Clip {
            track: "drums".into(),
            pattern: "beat".into(),
            at: 0,
            times: 0,
            transpose: 0,
        },
        Clip {
            track: "pad".into(),
            pattern: "pad".into(),
            at: 0,
            times: 0,
            transpose: 0,
        },
        Clip {
            track: "bass".into(),
            pattern: "bass".into(),
            at: 0,
            times: 0,
            transpose: 0,
        },
    ];
    let mut tracks = vec![
        track("drums", kit(), -6.0),
        track("pad", pad_synth(), -10.0),
        track("bass", bass_synth(), 2.0),
    ];
    if c.sidechain {
        tracks[1].effects.push(mc_music::Effect::Compressor {
            threshold: -24.0,
            ratio: 4.0,
            attack: 1.0,
            release: 150.0,
            makeup: 0.0,
            sidechain: Some("drums".into()),
            on: true,
        });
    }
    if c.lead {
        patterns.push(Pattern {
            name: "lead".into(),
            beats: 4 * n,
            notes: lead_notes.clone(),
            automation: vec![],
        });
        clips.push(Clip {
            track: "lead".into(),
            pattern: "lead".into(),
            at: 0,
            times: 0,
            transpose: 0,
        });
        tracks.push(track("lead", lead_synth(), -8.0));
    }
    let song = Song {
        name: c.name.into(),
        tempo: c.tempo,
        beats_per_bar: 4,
        root: c.key_root,
        scale: if c.minor { Scale::Minor } else { Scale::Major },
        tracks,
        buses: vec![],
        master: Master::default(),
        patterns,
        sections: vec![Section {
            name: "main".into(),
            bars: c.bars,
            kind: SectionKind::Loop,
            intensity: (0.0, 1.0),
            next: vec![],
            exit_every: 0,
            clips,
        }],
        arrangement: vec!["main".into()],
        notes: String::new(),
    };
    // Answers over the whole song.
    let mut bass_all = Vec::new();
    let mut lead_all = Vec::new();
    let mut chords = Vec::new();
    let mut drum_bars = Vec::new();
    let steps = |s: &str| -> Vec<bool> {
        s.chars()
            .filter(|c| *c != ' ' && *c != '|')
            .map(|c| c != '.')
            .collect()
    };
    for b in 0..c.bars {
        let cyc = b % n;
        chords.push(c.prog[cyc as usize]);
        let off = (b - cyc) * bar;
        for note in bass.iter().filter(|x| x.0 / bar == cyc) {
            bass_all.push(Note(note.0 + off, note.1, note.2, note.3));
        }
        for note in lead_notes.iter().filter(|x| x.0 / bar == cyc) {
            lead_all.push(Note(note.0 + off, note.1, note.2, note.3));
        }
        drum_bars.push([steps(c.drums[0]), steps(c.drums[1]), steps(c.drums[2])]);
    }
    Truth {
        song,
        bass: bass_all,
        drums: drum_bars,
        chords,
        lead: lead_all,
    }
}

pub(crate) fn render(song: &Song) -> mc_listen::Audio {
    let frames = mc_music::render::render_arrangement(song, RATE, 0.5);
    mc_listen::Audio::new(RATE, frames)
}

/// Share of `truth` notes matched by a detected note with the same key starting within `tol` ticks.
pub(crate) fn note_accuracy(truth: &[Note], got: &[Note], tol: u32) -> f32 {
    let hit = truth
        .iter()
        .filter(|t| {
            got.iter()
                .any(|g| g.2 == t.2 && (g.0 as i64 - t.0 as i64).unsigned_abs() as u32 <= tol)
        })
        .count();
    hit as f32 / truth.len().max(1) as f32
}
