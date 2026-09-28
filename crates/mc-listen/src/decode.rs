//! Reading audio files: WAV, MP3, FLAC and Ogg Vorbis through symphonia (pure
//! Rust, no system libraries). The rate is reported, never changed.

use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decoded audio: stereo frames at `rate` (mono files are doubled, extra channels dropped).
#[derive(Clone, Debug, Default)]
pub struct Audio {
    pub rate: u32,
    pub frames: Vec<[f32; 2]>,
}

impl Audio {
    pub fn new(rate: u32, frames: Vec<[f32; 2]>) -> Audio {
        Audio { rate, frames }
    }

    pub fn seconds(&self) -> f32 {
        self.frames.len() as f32 / self.rate.max(1) as f32
    }

    /// (L+R)/2.
    pub fn mono(&self) -> Vec<f32> {
        self.frames.iter().map(|f| (f[0] + f[1]) * 0.5).collect()
    }

    /// The part from `from` to `to` seconds (clamped); `None` = the start / the end.
    pub fn span(&self, from: Option<f32>, to: Option<f32>) -> Audio {
        let r = self.rate as f32;
        let a = (from.unwrap_or(0.0).max(0.0) * r) as usize;
        let b = to.map(|t| (t * r) as usize).unwrap_or(self.frames.len());
        let a = a.min(self.frames.len());
        let b = b.clamp(a, self.frames.len());
        Audio {
            rate: self.rate,
            frames: self.frames[a..b].to_vec(),
        }
    }
}

/// Decodes a whole file. Errors are one line naming the file.
pub fn load(path: &Path) -> Result<Audio, String> {
    let name = path.display().to_string();
    let file = std::fs::File::open(path).map_err(|e| format!("{name}: {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("{name}: not a readable audio file ({e})"))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| format!("{name}: no audio track"))?;
    let track_id = track.id;
    let mut rate = track.codec_params.sample_rate.unwrap_or(0);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("{name}: unsupported codec ({e})"))?;
    let mut frames: Vec<[f32; 2]> = Vec::new();
    let mut sb: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(e) => return Err(format!("{name}: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(Error::DecodeError(_)) => continue,
            Err(Error::IoError(_)) => continue,
            Err(e) => return Err(format!("{name}: {e}")),
        };
        let spec = *decoded.spec();
        if rate == 0 {
            rate = spec.rate;
        }
        let ch = spec.channels.count().max(1);
        let need = decoded.capacity() as u64;
        if sb
            .as_ref()
            .map(|b| b.capacity() < need as usize * ch)
            .unwrap_or(true)
        {
            sb = Some(SampleBuffer::<f32>::new(need, spec));
        }
        let buf = sb.as_mut().unwrap();
        buf.copy_interleaved_ref(decoded);
        for f in buf.samples().chunks(ch) {
            if ch == 1 {
                frames.push([f[0], f[0]]);
            } else {
                frames.push([f[0], f[1]]);
            }
        }
    }
    if rate == 0 {
        return Err(format!("{name}: unknown sample rate"));
    }
    if frames.is_empty() {
        return Err(format!("{name}: no audio decoded"));
    }
    Ok(Audio { rate, frames })
}
