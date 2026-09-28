//! Names and numbers of music: keys, chords, Roman numerals, scale degrees,
//! positions in a bar. Shared by the audio analysis and the song charts so
//! both speak the same vocabulary.

use serde::{Deserialize, Serialize};

pub const SHARPS: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
pub const FLATS: [&str; 12] = [
    "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
];

/// Scale degrees by semitones above the tonic, spelled against the major scale.
pub const DEGREES: [&str; 12] = [
    "1", "b2", "2", "b3", "3", "4", "#4", "5", "b6", "6", "b7", "7",
];

pub fn pc_name(pc: u8, flats: bool) -> &'static str {
    if flats {
        FLATS[pc as usize % 12]
    } else {
        SHARPS[pc as usize % 12]
    }
}

/// "Eb2" for 39 in a flat key.
pub fn note_name(key: u8, flats: bool) -> String {
    format!("{}{}", pc_name(key % 12, flats), key as i32 / 12 - 1)
}

/// A key: tonic pitch class and mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Key {
    pub root: u8,
    pub minor: bool,
}

impl Key {
    /// Flat keys spell with flats (F, Bb, Eb... and their relative minors).
    pub fn flats(&self) -> bool {
        let rel = if self.minor {
            (self.root + 3) % 12
        } else {
            self.root
        };
        matches!(rel, 1 | 3 | 5 | 6 | 8 | 10)
    }
    pub fn name(&self) -> String {
        format!(
            "{} {}",
            pc_name(self.root, self.flats()),
            if self.minor { "minor" } else { "major" }
        )
    }
    /// The seven diatonic steps (natural minor for minor keys).
    pub fn steps(&self) -> [u8; 7] {
        if self.minor {
            [0, 2, 3, 5, 7, 8, 10]
        } else {
            [0, 2, 4, 5, 7, 9, 11]
        }
    }
    /// The degree of a MIDI key or pitch class relative to the tonic ("b7").
    pub fn degree(&self, key: u8) -> &'static str {
        DEGREES[((key as i32 - self.root as i32).rem_euclid(12)) as usize]
    }
    /// Parses "C:minor", "Eb major", "F#m", "a" (lower case = minor).
    pub fn parse(s: &str) -> Option<Key> {
        let s = s.trim();
        let mut chars = s.chars();
        let l = chars.next()?;
        let base = match l.to_ascii_uppercase() {
            'C' => 0,
            'D' => 2,
            'E' => 4,
            'F' => 5,
            'G' => 7,
            'A' => 9,
            'B' => 11,
            _ => return None,
        };
        let rest: String = chars.collect();
        let mut acc = 0i32;
        let mut tail = rest.as_str();
        while let Some(c) = tail.chars().next() {
            match c {
                '#' => acc += 1,
                'b' => acc -= 1,
                _ => break,
            }
            tail = &tail[1..];
        }
        let tail = tail
            .trim_start_matches([':', ' ', '_', '-'])
            .to_ascii_lowercase();
        let minor = if tail.is_empty() {
            l.is_ascii_lowercase()
        } else {
            tail.starts_with("min")
                || tail == "m"
                || tail.starts_with("aeol")
                || tail.starts_with("dor")
                || tail.starts_with("phr")
        };
        Some(Key {
            root: ((base + acc).rem_euclid(12)) as u8,
            minor,
        })
    }
    pub fn scale(&self) -> mc_music::song::Scale {
        if self.minor {
            mc_music::song::Scale::Minor
        } else {
            mc_music::song::Scale::Major
        }
    }
}

const KK_MAJOR: [f32; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const KK_MINOR: [f32; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];
const TP_MAJOR: [f32; 12] = [
    0.748, 0.060, 0.488, 0.082, 0.670, 0.460, 0.096, 0.715, 0.104, 0.366, 0.057, 0.400,
];
const TP_MINOR: [f32; 12] = [
    0.712, 0.084, 0.474, 0.618, 0.049, 0.460, 0.105, 0.747, 0.404, 0.067, 0.133, 0.330,
];

/// All 24 keys scored against a pitch-class histogram: the mean of the
/// Krumhansl-Kessler and Temperley profile correlations, best first.
pub fn key_scores(hist: &[f32; 12]) -> Vec<(Key, f32)> {
    let mut out = Vec::with_capacity(24);
    for minor in [false, true] {
        for root in 0..12u8 {
            let rot: Vec<f32> = (0..12).map(|i| hist[(i + root as usize) % 12]).collect();
            let (kk, tp) = if minor {
                (&KK_MINOR, &TP_MINOR)
            } else {
                (&KK_MAJOR, &TP_MAJOR)
            };
            let s = 0.5 * (crate::dsp::pearson(&rot, kk) + crate::dsp::pearson(&rot, tp));
            out.push((Key { root, minor }, s));
        }
    }
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Quality {
    Maj,
    Min,
    Dom7,
    Min7,
    Maj7,
    Sus2,
    Sus4,
    Dim,
    Five,
}

impl Quality {
    pub const ALL: [Quality; 9] = [
        Quality::Maj,
        Quality::Min,
        Quality::Dom7,
        Quality::Min7,
        Quality::Maj7,
        Quality::Sus2,
        Quality::Sus4,
        Quality::Dim,
        Quality::Five,
    ];
    pub fn intervals(self) -> &'static [u8] {
        match self {
            Quality::Maj => &[0, 4, 7],
            Quality::Min => &[0, 3, 7],
            Quality::Dom7 => &[0, 4, 7, 10],
            Quality::Min7 => &[0, 3, 7, 10],
            Quality::Maj7 => &[0, 4, 7, 11],
            Quality::Sus2 => &[0, 2, 7],
            Quality::Sus4 => &[0, 5, 7],
            Quality::Dim => &[0, 3, 6],
            Quality::Five => &[0, 7],
        }
    }
    pub fn suffix(self) -> &'static str {
        match self {
            Quality::Maj => "",
            Quality::Min => "m",
            Quality::Dom7 => "7",
            Quality::Min7 => "m7",
            Quality::Maj7 => "maj7",
            Quality::Sus2 => "sus2",
            Quality::Sus4 => "sus4",
            Quality::Dim => "dim",
            Quality::Five => "5",
        }
    }
    /// A small handicap for richer chords, so a plain triad wins a near tie.
    fn penalty(self) -> f32 {
        match self {
            Quality::Maj | Quality::Min => 0.0,
            Quality::Five => 0.04,
            Quality::Dom7 | Quality::Min7 | Quality::Maj7 => 0.03,
            Quality::Sus2 | Quality::Sus4 => 0.035,
            Quality::Dim => 0.04,
        }
    }
    fn minorish(self) -> bool {
        matches!(self, Quality::Min | Quality::Min7 | Quality::Dim)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Chord {
    pub root: u8,
    pub quality: Quality,
}

impl Chord {
    pub fn name(&self, flats: bool) -> String {
        format!("{}{}", pc_name(self.root, flats), self.quality.suffix())
    }

    /// Roman numeral in `key`: minor-key numerals are relative to natural minor
    /// ("i VI III VII"); chromatic roots take b/# ("bVII" in major); a major or
    /// dominant chord on a non-diatonic degree that resolves down a fifth to a
    /// diatonic chord is a secondary dominant ("V/V", "V7/vi").
    pub fn numeral(&self, key: &Key) -> String {
        let iv = (self.root as i32 - key.root as i32).rem_euclid(12) as u8;
        let steps = key.steps();
        let diatonic_q: [Quality; 7] = if key.minor {
            [
                Quality::Min,
                Quality::Dim,
                Quality::Maj,
                Quality::Min,
                Quality::Min,
                Quality::Maj,
                Quality::Maj,
            ]
        } else {
            [
                Quality::Maj,
                Quality::Min,
                Quality::Min,
                Quality::Maj,
                Quality::Maj,
                Quality::Min,
                Quality::Dim,
            ]
        };
        let q = self.quality;
        let dominantish = matches!(q, Quality::Maj | Quality::Dom7);
        let deg = steps.iter().position(|&s| s == iv);
        let fits = |d: usize| -> bool {
            let dq = diatonic_q[d];
            match q {
                Quality::Maj | Quality::Maj7 => dq == Quality::Maj,
                Quality::Min | Quality::Min7 => dq == Quality::Min,
                Quality::Dim => dq == Quality::Dim,
                Quality::Dom7 => {
                    dq == Quality::Maj && (!key.minor && d == 4 || key.minor && d == 6)
                }
                Quality::Sus2 | Quality::Sus4 | Quality::Five => true,
            }
        };
        let in_key = deg.map(fits).unwrap_or(false);
        // The harmonic-minor V (and V7) are at home in a minor key.
        let minor_v = key.minor && iv == 7 && dominantish;
        if !in_key && !minor_v && dominantish {
            let target = (iv + 5) % 12;
            if target != 0 || key.minor {
                if let Some(td) = steps.iter().position(|&s| s == target) {
                    if diatonic_q[td] != Quality::Dim && target != iv {
                        let t = Chord {
                            root: (key.root + target) % 12,
                            quality: diatonic_q[td],
                        };
                        let v = if q == Quality::Dom7 { "V7" } else { "V" };
                        return format!("{v}/{}", t.numeral(key));
                    }
                }
            }
        }
        const ROMAN: [&str; 7] = ["I", "II", "III", "IV", "V", "VI", "VII"];
        let (d, acc) = match deg {
            Some(d) => (d, ""),
            None => {
                if key.minor && iv == 11 {
                    (6, if q == Quality::Dim { "" } else { "#" })
                } else if key.minor && (iv == 4 || iv == 9) {
                    (steps.iter().position(|&s| s == iv - 1).unwrap(), "#")
                } else {
                    (
                        steps.iter().position(|&s| s == (iv + 1) % 12).unwrap_or(0),
                        "b",
                    )
                }
            }
        };
        let base = if q.minorish() {
            ROMAN[d].to_lowercase()
        } else {
            ROMAN[d].to_string()
        };
        let suf = match q {
            Quality::Maj | Quality::Min => "",
            Quality::Dom7 | Quality::Min7 => "7",
            Quality::Maj7 => "maj7",
            Quality::Sus2 => "sus2",
            Quality::Sus4 => "sus4",
            Quality::Dim => "°",
            Quality::Five => "5",
        };
        format!("{acc}{base}{suf}")
    }
}

/// Chord templates over 12 pitch classes. With `harmonics`, each chord tone
/// also lights its upper partials (as a synth or a voice does), so a saw's
/// strong fifth partial does not read as an extra chord tone.
pub fn templates(harmonics: bool) -> Vec<(Chord, [f32; 12])> {
    let partials: &[(f32, f32)] = if harmonics {
        // (semitones above, weight) for harmonics 1..6.
        &[
            (0.0, 1.0),
            (12.0, 0.5),
            (19.02, 0.33),
            (24.0, 0.25),
            (27.86, 0.2),
            (31.02, 0.16),
        ]
    } else {
        &[(0.0, 1.0)]
    };
    let mut out = Vec::new();
    for q in Quality::ALL {
        for root in 0..12u8 {
            let mut t = [0.0f32; 12];
            for &iv in q.intervals() {
                for &(p, w) in partials {
                    let pc = ((root as f32 + iv as f32 + p).round() as i32).rem_euclid(12) as usize;
                    t[pc] += w;
                }
            }
            out.push((Chord { root, quality: q }, t));
        }
    }
    out
}

/// The best chords for a chroma vector (with an optional bass chroma that
/// favours roots in the bass), best first, as (chord, score 0..1).
pub fn match_chords(
    chroma: &[f32; 12],
    bass: Option<&[f32; 12]>,
    tpl: &[(Chord, [f32; 12])],
) -> Vec<(Chord, f32)> {
    let bass_n = bass.map(|b| {
        let m = b.iter().cloned().fold(0.0f32, f32::max);
        if m > 0.0 {
            b.map(|x| x / m)
        } else {
            [0.0; 12]
        }
    });
    let mut out: Vec<(Chord, f32)> = tpl
        .iter()
        .map(|(c, t)| {
            let mut s = crate::dsp::cosine(chroma, t) - c.quality.penalty();
            if let Some(b) = &bass_n {
                s += 0.08 * b[c.root as usize]
                    - 0.03
                        * b.iter()
                            .enumerate()
                            .filter(|(i, _)| {
                                !c.quality
                                    .intervals()
                                    .iter()
                                    .any(|iv| (c.root + iv) % 12 == *i as u8)
                            })
                            .map(|(_, v)| v)
                            .sum::<f32>()
                        / 4.0;
            }
            (*c, s)
        })
        .collect();
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    out
}

/// "1", "1e", "1&", "1a" for the sixteenths of a bar (4 steps to the beat).
pub fn step_name(step: usize) -> String {
    const SUB: [&str; 4] = ["", "e", "&", "a"];
    format!("{}{}", step / 4 + 1, SUB[step % 4])
}

/// Spoken: "1", "the e-of-1", "the and-of-2", "the a-of-3".
pub fn step_words(step: usize) -> String {
    match step % 4 {
        0 => format!("{}", step / 4 + 1),
        1 => format!("the e-of-{}", step / 4 + 1),
        2 => format!("the and-of-{}", step / 4 + 1),
        _ => format!("the a-of-{}", step / 4 + 1),
    }
}

/// "1, the and-of-2 and 3" for the hits in a 16-step grid string.
pub fn hits_in_words(grid: &str) -> String {
    let hits: Vec<String> = grid
        .chars()
        .enumerate()
        .filter(|(_, c)| *c != '.' && *c != '-')
        .map(|(i, _)| step_words(i))
        .collect();
    match hits.len() {
        0 => "nothing".to_string(),
        1 => hits[0].clone(),
        16 => "every sixteenth".to_string(),
        _ => {
            let (last, rest) = hits.split_last().unwrap();
            format!("{} and {}", rest.join(", "), last)
        }
    }
}
