//! Meridian Conflict's music: songs written as text (`data/music/*.ron`), an
//! engine that synthesises and mixes them in real time, and a director that
//! follows the battle.
//!
//! Like the sound library, nothing here is a recording: every instrument is a
//! synthesiser patch and every drum a recipe, so the whole score is diffable
//! text that the studio (`mc-studio`) edits and the game plays through this
//! same engine. What you hear in the studio is what the game plays.
//!
//! - `song`: the data: tracks, patterns, sections, arrangement, layers.
//! - `patch`: instruments and effects.
//! - `engine`: real-time playback, mixing, the director.
//! - `render`: offline rendering, WAV export and measurement.
//! - `score`: which song the game plays when.
//! - `history`: revisions, Claude's proposals, the studio's session and inbox.
//! - `stage`: a song playing straight through, and moments played over it (the song dips).

#![expect(unsafe_code, reason = "lock-free parameter hand-off to the audio thread in engine.rs")]

pub mod dsp;
pub mod engine;
pub mod history;
pub mod patch;
pub mod render;
pub mod score;
pub mod song;
pub mod stage;
pub mod voice;

pub use engine::{Command, Engine, Meters, Mode, Status};
pub use patch::{Effect, Instrument, Kit, Synth};
pub use score::Score;
pub use song::{Clip, Note, Pattern, Section, SectionKind, Song, Track, PPQ};
