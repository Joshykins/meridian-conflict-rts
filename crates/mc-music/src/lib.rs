//! Meridian Conflict's music: songs written as text (`data/music/*.ron`), an
//! engine that plays and mixes them in real time, and a director that follows
//! the battle.
//!
//! The orchestra is recorded: real notes from VSCO 2 Community Edition (CC0),
//! in `data/music/samples`. Synthesiser patches and drum recipes remain for
//! electronic parts. The score stays diffable text that the studio
//! (`mc-studio`) edits and the game plays through this same engine. What you
//! hear in the studio is what the game plays.
//!
//! - `song`: the data: tracks, patterns, sections, arrangement, layers.
//! - `patch`: instruments and effects.
//! - `library`: the music folder: named instruments and sample sets.
//! - `samples`: recorded notes, decoded and shared.
//! - `engine`: real-time playback, mixing, the director.
//! - `render`: offline rendering, WAV export and measurement.
//! - `score`: which song the game plays when.
//! - `history`: revisions, Claude's proposals, the studio's session and inbox.
//! - `stage`: a song playing straight through, and moments played over it (the song dips).

#![cfg_attr(
    any(target_arch = "x86_64", target_arch = "x86"),
    expect(unsafe_code, reason = "x86 audio denormal control in engine.rs")
)]

pub mod dsp;
pub mod engine;
pub mod history;
pub mod library;
pub mod patch;
pub mod render;
mod sampler;
pub mod samples;
pub mod score;
pub mod song;
pub mod stage;
pub mod voice;

pub use engine::{Command, Engine, Meters, Mode, Status};
pub use patch::{Effect, Instrument, Kit, Sampler, Synth};
pub use score::Score;
pub use song::{Clip, Note, Pattern, Section, SectionKind, Song, Track, PPQ};
