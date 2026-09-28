//! mc-listen: ears for an AI that cannot hear.
//!
//! Turns audio into facts precise enough to write music from, and compares
//! our renders with a reference. Everything is measured, nothing is played.
//!
//! - [`decode::load`]: WAV, MP3, FLAC, Ogg Vorbis to stereo frames.
//! - [`pitch::transcribe`]: humming, singing or a bass line to `mc_music::Note`s.
//! - [`analyse::reference`]: tempo and grid, key, chords per bar, bassline,
//!   drum grid, melody guess, arrangement, mix measurements and a lead sheet.
//! - [`leadsheet::from_song`]: the same lead sheet straight from a song's notes.
//! - [`compare()`]: ranked plain-English differences, ours vs a reference.
//!
//! Times in reports are seconds from the start of the analysed span; notes
//! are in ticks (`mc_music::PPQ` = 96 to the beat) from the first downbeat.

pub mod analyse;
pub mod compare;
pub mod decode;
pub mod drums;
pub mod dsp;
pub mod features;
pub mod harmony;
pub mod leadsheet;
pub mod melody;
pub mod pitch;
pub mod sound;
pub mod structure;
pub mod tempo;
pub mod theory;

pub use analyse::{reference, Options, Report};
pub use compare::{compare, compare_charts, compare_ranked, Finding};
pub use decode::{load, Audio};
pub use leadsheet::LeadSheet;
pub use pitch::{transcribe, transcribe_with, TranscribeOpts, Transcription};
pub use theory::Key;
