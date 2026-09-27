//! A song as it is written down: `data/music/*.ron`.
//!
//! A song is tracks (each one instrument, its mixer strip and its effects),
//! patterns (note lists any track may play), and sections (a few bars placing
//! patterns on tracks). Played straight through, the `arrangement` lists the
//! sections in order, the way a DAW timeline does. In the game the director
//! (`director.rs`) picks sections instead, by how intense the battle is, and
//! every track fades in and out by its `layer`.
//!
//! Time in patterns is in ticks, `PPQ` to the beat, so a note never drifts;
//! sections are in bars; nothing is in seconds except envelopes and effects.

use crate::library::Library;
use crate::patch::{Effect, Instrument};
use serde::{Deserialize, Serialize};

/// Ticks per beat (a quarter note). Divides by 2, 3, 4, 6, 8, 12, 16, 24, 32.
pub const PPQ: u32 = 96;

/// The key the piano roll highlights. Nothing is forced into it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Scale {
    #[default]
    Minor,
    Major,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    HarmonicMinor,
    Chromatic,
}

impl Scale {
    pub const ALL: [Scale; 8] = [
        Scale::Minor,
        Scale::Major,
        Scale::Dorian,
        Scale::Phrygian,
        Scale::Lydian,
        Scale::Mixolydian,
        Scale::HarmonicMinor,
        Scale::Chromatic,
    ];

    /// Semitones above the root that belong to the scale.
    pub fn steps(self) -> &'static [u8] {
        match self {
            Scale::Minor => &[0, 2, 3, 5, 7, 8, 10],
            Scale::Major => &[0, 2, 4, 5, 7, 9, 11],
            Scale::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Scale::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            Scale::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            Scale::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            Scale::HarmonicMinor => &[0, 2, 3, 5, 7, 8, 11],
            Scale::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        }
    }

    pub fn contains(self, root: u8, key: u8) -> bool {
        let d = (key as i32 - root as i32).rem_euclid(12) as u8;
        self.steps().contains(&d)
    }

    pub fn name(self) -> &'static str {
        match self {
            Scale::Minor => "Minor",
            Scale::Major => "Major",
            Scale::Dorian => "Dorian",
            Scale::Phrygian => "Phrygian",
            Scale::Lydian => "Lydian",
            Scale::Mixolydian => "Mixolydian",
            Scale::HarmonicMinor => "Harmonic minor",
            Scale::Chromatic => "Chromatic",
        }
    }
}

pub const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// "C4" for 60.
pub fn key_name(key: u8) -> String {
    format!("{}{}", NOTE_NAMES[key as usize % 12], key as i32 / 12 - 1)
}

/// One note: start and length in ticks, MIDI key, velocity 1..=127.
/// Written as a tuple, `(0, 96, 60, 100)`, so a pattern reads as a list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note(pub u32, pub u32, pub u8, pub u8);

impl Note {
    pub fn at(&self) -> u32 {
        self.0
    }
    #[expect(
        clippy::len_without_is_empty,
        reason = "len is the note's duration in ticks, not a collection length"
    )]
    pub fn len(&self) -> u32 {
        self.1
    }
    pub fn key(&self) -> u8 {
        self.2
    }
    pub fn vel(&self) -> u8 {
        self.3
    }
    pub fn end(&self) -> u32 {
        self.0 + self.1
    }
}

/// A parameter moved over a pattern: points of (tick, value), straight lines
/// between them, the first value before the first point and the last after.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Automation {
    pub target: Target,
    pub points: Vec<(u32, f32)>,
}

impl Automation {
    pub fn value_at(&self, tick: f32) -> Option<f32> {
        let p = &self.points;
        let first = p.first()?;
        if tick <= first.0 as f32 {
            return Some(first.1);
        }
        for w in p.windows(2) {
            let (a, b) = (w[0], w[1]);
            if tick < b.0 as f32 {
                let span = (b.0 - a.0).max(1) as f32;
                return Some(a.1 + (b.1 - a.1) * (tick - a.0 as f32) / span);
            }
        }
        Some(p.last()?.1)
    }
}

/// What a pattern's automation, or the battle's intensity, moves on a track.
/// Values are normalised 0..=1 and mapped by the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    /// The instrument's filter cutoff, 0 = its own setting four octaves down, 1 = four up, 0.5 = as set.
    Cutoff,
    /// The track's fader, 0 = silent, 1 = as set.
    Volume,
    /// -1..1 written as 0..1.
    Pan,
    /// Every send of the track, 0 = none, 1 = as set.
    Sends,
}

impl Target {
    pub const ALL: [Target; 4] = [Target::Cutoff, Target::Volume, Target::Pan, Target::Sends];
    pub fn name(self) -> &'static str {
        match self {
            Target::Cutoff => "Cutoff",
            Target::Volume => "Volume",
            Target::Pan => "Pan",
            Target::Sends => "Sends",
        }
    }
    /// The value a lane with no points means.
    pub fn neutral(self) -> f32 {
        match self {
            Target::Cutoff | Target::Pan => 0.5,
            Target::Volume | Target::Sends => 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    pub name: String,
    /// Length in beats. Notes past the end are not played.
    pub beats: u32,
    #[serde(default)]
    pub notes: Vec<Note>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub automation: Vec<Automation>,
}

impl Pattern {
    pub fn ticks(&self) -> u32 {
        self.beats * PPQ
    }
}

/// A pattern placed on a track inside a section.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub track: String,
    pub pattern: String,
    /// Where it starts in the section, in beats.
    #[serde(default)]
    pub at: u32,
    /// Played this many times back to back; 0 fills to the section's end.
    #[serde(default = "one")]
    pub times: u32,
    /// Semitones added to every note.
    #[serde(default, skip_serializing_if = "is_zero_i8")]
    pub transpose: i8,
}

impl Clip {
    /// Ticks it covers in a section of `section_ticks`, for a pattern of `pattern_ticks`.
    pub fn span(&self, pattern_ticks: u32, section_ticks: u32) -> u32 {
        let start = self.at * PPQ;
        let room = section_ticks.saturating_sub(start);
        if self.times == 0 {
            room
        } else {
            (pattern_ticks * self.times).min(room)
        }
    }
}

fn one() -> u32 {
    1
}
fn is_zero_i8(v: &i8) -> bool {
    *v == 0
}
fn is_false(v: &bool) -> bool {
    !*v
}
fn full_range() -> (f32, f32) {
    (0.0, 1.0)
}
fn is_full_range(v: &(f32, f32)) -> bool {
    *v == (0.0, 1.0)
}

/// What a section is for, to the director.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SectionKind {
    /// Played once at the start, then the director moves on.
    Intro,
    /// Repeats while the intensity stays in its range.
    #[default]
    Loop,
    /// Only reached through another section's `next`: a bridge.
    Bridge,
    /// Played over whatever is playing when cued (`Director::cue`), never chosen.
    Stinger,
    /// Played when the music is told to finish; the song stops after it.
    Ending,
}

impl SectionKind {
    pub const ALL: [SectionKind; 5] = [
        SectionKind::Intro,
        SectionKind::Loop,
        SectionKind::Bridge,
        SectionKind::Stinger,
        SectionKind::Ending,
    ];
    pub fn name(self) -> &'static str {
        match self {
            SectionKind::Intro => "Intro",
            SectionKind::Loop => "Loop",
            SectionKind::Bridge => "Bridge",
            SectionKind::Stinger => "Stinger",
            SectionKind::Ending => "Ending",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub name: String,
    pub bars: u32,
    #[serde(default)]
    pub kind: SectionKind,
    /// The intensities the director may pick it at, low..high.
    #[serde(default = "full_range", skip_serializing_if = "is_full_range")]
    pub intensity: (f32, f32),
    /// Sections the director may go to after this one, by name; empty = any loop that fits.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub next: Vec<String>,
    /// The director may leave at every this many bars when the intensity leaves
    /// the range, instead of waiting for the end; 0 = only at the end.
    #[serde(default)]
    pub exit_every: u32,
    #[serde(default)]
    pub clips: Vec<Clip>,
}

/// How a track follows the battle's intensity: silent below `from`, rising to
/// full at `full`, and (when `until` < 1) fading out again above `until`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    #[serde(default)]
    pub from: f32,
    #[serde(default)]
    pub full: f32,
    #[serde(default = "one_f")]
    pub until: f32,
}

fn one_f() -> f32 {
    1.0
}

impl Default for Layer {
    fn default() -> Layer {
        Layer {
            from: 0.0,
            full: 0.0,
            until: 1.0,
        }
    }
}

impl Layer {
    /// Linear gain at `intensity`.
    pub fn gain(&self, intensity: f32) -> f32 {
        let rise = if intensity >= self.full {
            1.0
        } else if intensity <= self.from {
            if self.from <= 0.0 && self.full <= 0.0 {
                1.0
            } else {
                0.0
            }
        } else {
            (intensity - self.from) / (self.full - self.from).max(1e-4)
        };
        let fall = if self.until >= 1.0 || intensity <= self.until {
            1.0
        } else {
            (1.0 - (intensity - self.until) / 0.15).max(0.0)
        };
        let g = rise.clamp(0.0, 1.0) * fall;
        g * g * (3.0 - 2.0 * g)
    }

    pub fn is_always(&self) -> bool {
        self.from <= 0.0 && self.full <= 0.0 && self.until >= 1.0
    }
}

/// An effect send from a track to a bus.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Send {
    pub bus: String,
    /// Level in dB.
    pub db: f32,
}

/// Intensity moving a track's parameter, on top of any automation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Follow {
    pub target: Target,
    /// Value at intensity 0 and at intensity 1 (normalised as `Target`).
    pub from: f32,
    pub to: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub name: String,
    pub instrument: Instrument,
    #[serde(default)]
    pub db: f32,
    #[serde(default)]
    pub pan: f32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub mute: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub solo: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sends: Vec<Send>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<Effect>,
    #[serde(default, skip_serializing_if = "Layer::is_always")]
    pub layer: Layer,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub follow: Vec<Follow>,
    /// A colour for the studio, 0xRRGGBB.
    #[serde(default = "track_colour")]
    pub colour: u32,
}

fn track_colour() -> u32 {
    0xE0603A
}

/// A return bus: sends from tracks go through its effects to the master.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bus {
    pub name: String,
    #[serde(default)]
    pub db: f32,
    #[serde(default)]
    pub effects: Vec<Effect>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub mute: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Master {
    #[serde(default)]
    pub db: f32,
    #[serde(default)]
    pub effects: Vec<Effect>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Song {
    pub name: String,
    pub tempo: f32,
    #[serde(default = "four")]
    pub beats_per_bar: u32,
    /// Root key (0 = C) and scale, for the piano roll.
    #[serde(default)]
    pub root: u8,
    #[serde(default)]
    pub scale: Scale,
    #[serde(default)]
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub buses: Vec<Bus>,
    #[serde(default)]
    pub master: Master,
    #[serde(default)]
    pub patterns: Vec<Pattern>,
    #[serde(default)]
    pub sections: Vec<Section>,
    /// Sections in order for straight-through playback and export.
    #[serde(default)]
    pub arrangement: Vec<String>,
    /// Notes about the song for whoever edits it next.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
    /// The folder it was loaded from: its library instruments and recordings.
    #[serde(skip)]
    pub library: Library,
}

fn four() -> u32 {
    4
}

impl Song {
    pub fn empty(name: &str) -> Song {
        Song {
            name: name.to_string(),
            tempo: 110.0,
            beats_per_bar: 4,
            root: 9,
            scale: Scale::Minor,
            tracks: Vec::new(),
            buses: Vec::new(),
            master: Master::default(),
            patterns: Vec::new(),
            sections: Vec::new(),
            arrangement: Vec::new(),
            notes: String::new(),
            library: Library::default(),
        }
    }

    pub fn bar_ticks(&self) -> u32 {
        self.beats_per_bar * PPQ
    }

    pub fn section_ticks(&self, s: &Section) -> u32 {
        s.bars * self.bar_ticks()
    }

    pub fn samples_per_tick(&self, rate: f32) -> f64 {
        rate as f64 * 60.0 / (self.tempo.max(1.0) as f64 * PPQ as f64)
    }

    pub fn track(&self, name: &str) -> Option<usize> {
        self.tracks.iter().position(|t| t.name == name)
    }
    pub fn pattern(&self, name: &str) -> Option<usize> {
        self.patterns.iter().position(|p| p.name == name)
    }
    pub fn section(&self, name: &str) -> Option<usize> {
        self.sections.iter().position(|s| s.name == name)
    }
    pub fn bus(&self, name: &str) -> Option<usize> {
        self.buses.iter().position(|b| b.name == name)
    }

    /// Ticks of the whole arrangement.
    pub fn arrangement_ticks(&self) -> u32 {
        self.arrangement
            .iter()
            .filter_map(|n| self.section(n))
            .map(|i| self.section_ticks(&self.sections[i]))
            .sum()
    }

    /// Where each arrangement entry starts, in ticks, with its section index.
    pub fn arrangement_starts(&self) -> Vec<(u32, usize)> {
        let mut at = 0;
        let mut out = Vec::new();
        for n in &self.arrangement {
            if let Some(i) = self.section(n) {
                out.push((at, i));
                at += self.section_ticks(&self.sections[i]);
            }
        }
        out
    }

    /// Names that are referred to but missing, and other mistakes worth a warning.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        for s in &self.sections {
            for c in &s.clips {
                if self.track(&c.track).is_none() {
                    out.push(format!("section {}: no track \"{}\"", s.name, c.track));
                }
                if self.pattern(&c.pattern).is_none() {
                    out.push(format!("section {}: no pattern \"{}\"", s.name, c.pattern));
                }
            }
            for n in &s.next {
                if self.section(n).is_none() {
                    out.push(format!("section {}: next \"{n}\" does not exist", s.name));
                }
            }
        }
        for n in &self.arrangement {
            if self.section(n).is_none() {
                out.push(format!("arrangement: no section \"{n}\""));
            }
        }
        for t in &self.tracks {
            for s in &t.sends {
                if self.bus(&s.bus).is_none() {
                    out.push(format!("track {}: no bus \"{}\"", t.name, s.bus));
                }
            }
        }
        out.extend(self.library.problems().iter().cloned());
        out
    }

    /// Reads a song and links it to its music folder (`library::music_dir`).
    pub fn load(path: &std::path::Path) -> Result<Song, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut song = Song::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        if let Some(dir) = crate::library::music_dir(path) {
            song.link(&dir);
        }
        Ok(song)
    }

    /// Links the song to the music folder `dir`: reads the library instruments its
    /// tracks name and decodes the recordings they play. Call again after changing
    /// a track's instrument.
    pub fn link(&mut self, dir: &std::path::Path) {
        self.library = Library::link(dir, self.tracks.iter().map(|t| &t.instrument));
    }

    /// Links again to the folder it was loaded from (after an instrument changed).
    pub fn relink(&mut self) {
        if let Some(dir) = self.library.dir().map(std::path::Path::to_path_buf) {
            self.link(&dir);
        }
    }

    pub fn parse(text: &str) -> Result<Song, String> {
        ron::Options::default()
            .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
            .from_str(text)
            .map_err(|e| e.to_string())
    }

    /// The song as the RON text it is saved as.
    pub fn to_ron(&self) -> String {
        let config = ron::ser::PrettyConfig::new()
            .depth_limit(6)
            .indentor("    ".to_string())
            .struct_names(false)
            .compact_arrays(true)
            .extensions(ron::extensions::Extensions::IMPLICIT_SOME);
        ron::ser::to_string_pretty(self, config).expect("song serialises")
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let mut text = self.to_ron();
        text.push('\n');
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let tmp = path.with_extension(format!("ron.tmp{}", std::process::id()));
        std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
    }
}
