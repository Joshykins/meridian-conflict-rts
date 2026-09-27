//! Recorded instruments: sets of notes on disk, decoded once and shared.
//!
//! A set is a folder `data/music/samples/<set>/` holding `set.ron` and one Ogg
//! Vorbis file per recorded note (`scripts/music/import_vsco.py` makes them from
//! VSCO 2 Community Edition, CC0). Each note is a zone: the key it was played
//! at, its tuning in cents, its loudness layer and, for held sets, a loop in its
//! steady part so a note can last as long as the score asks.

use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug)]
pub enum SampleError {
    Io(PathBuf, std::io::Error),
    Parse(PathBuf, String),
    Decode(PathBuf, String),
}

impl fmt::Display for SampleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SampleError::Io(p, e) => write!(f, "{}: {e}", p.display()),
            SampleError::Parse(p, e) => write!(f, "{}: {e}", p.display()),
            SampleError::Decode(p, e) => write!(f, "{}: {e}", p.display()),
        }
    }
}

impl std::error::Error for SampleError {}

/// `set.ron` as written.
#[derive(Deserialize)]
struct SetFile {
    title: String,
    rate: u32,
    pitched: bool,
    held: bool,
    zones: Vec<ZoneFile>,
}

#[derive(Deserialize)]
struct ZoneFile {
    file: String,
    key: u8,
    #[serde(default)]
    cents: f32,
    #[serde(default)]
    layer: u8,
    #[serde(default, rename = "loop")]
    looped: Option<(u32, u32)>,
}

/// One recorded note, decoded.
pub struct Zone {
    pub key: u8,
    pub cents: f32,
    pub layer: u8,
    /// Frames `[start, end)` played round and round while the note is held.
    pub looped: Option<(u32, u32)>,
    pub channels: usize,
    /// Interleaved 16-bit frames.
    pub frames: Vec<i16>,
}

impl Zone {
    pub fn len(&self) -> usize {
        self.frames.len() / self.channels
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

pub struct SampleSet {
    pub name: String,
    pub title: String,
    pub rate: f32,
    /// Notes between recorded keys are played by repitching the nearest; an
    /// unpitched set (a percussion kit) sounds only on its recorded keys.
    pub pitched: bool,
    /// Held notes loop until released; otherwise every note rings to its end.
    pub held: bool,
    /// Sorted by key, then layer.
    pub zones: Vec<Zone>,
    pub layers: u8,
}

impl SampleSet {
    /// The zones to choose from for `key` at `vel` (0..1): its loudness layer (or the
    /// nearest one recorded near this key), then the nearest recorded key in it. Several
    /// takes of one note come back together, for round-robin.
    pub fn pick(&self, key: u8, vel: f32) -> &[Zone] {
        let want = ((vel.clamp(0.0, 1.0) * self.layers as f32) as u8).min(self.layers - 1);
        let near = |z: &Zone| (z.key as i32 - key as i32).unsigned_abs();
        let best = self
            .zones
            .iter()
            .filter(|z| self.pitched || z.key == key)
            // The right loudness layer unless it is only recorded far off (a wrong
            // layer costs as much as five semitones of repitching), then the zone above
            // (pitched down sounds more natural than pitched up).
            .min_by_key(|z| {
                (
                    near(z).min(24) + (z.layer as i32 - want as i32).unsigned_abs() * 5,
                    z.key < key,
                )
            });
        let Some(b) = best else {
            return &[];
        };
        let from = self
            .zones
            .iter()
            .position(|z| z.key == b.key && z.layer == b.layer)
            .unwrap_or(0);
        let n = self.zones[from..]
            .iter()
            .take_while(|z| z.key == b.key && z.layer == b.layer)
            .count();
        &self.zones[from..from + n]
    }

    fn load(dir: &Path) -> Result<SampleSet, SampleError> {
        let path = dir.join("set.ron");
        let text = std::fs::read_to_string(&path).map_err(|e| SampleError::Io(path.clone(), e))?;
        let file: SetFile = ron::Options::default()
            .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
            .from_str(&text)
            .map_err(|e| SampleError::Parse(path.clone(), e.to_string()))?;
        let mut zones = Vec::with_capacity(file.zones.len());
        for z in &file.zones {
            let p = dir.join(&z.file);
            let (channels, frames) = decode(&p)?;
            let len = frames.len() / channels.max(1);
            // A decoder may drop a few frames at the end: a loop ending just past the
            // audio ends at the audio instead.
            let looped = z
                .looped
                .map(|(s, e)| (s, e.min(len as u32)))
                .filter(|&(s, e)| e > s && e - s >= 64);
            zones.push(Zone {
                key: z.key,
                cents: z.cents,
                layer: z.layer,
                looped,
                channels,
                frames,
            });
        }
        zones.sort_by_key(|z| (z.key, z.layer));
        let layers = zones.iter().map(|z| z.layer + 1).max().unwrap_or(1);
        Ok(SampleSet {
            name: dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            title: file.title,
            rate: file.rate as f32,
            pitched: file.pitched,
            held: file.held,
            zones,
            layers,
        })
    }
}

/// Decodes an Ogg Vorbis file to interleaved 16-bit frames: (channels, frames).
fn decode(path: &Path) -> Result<(usize, Vec<i16>), SampleError> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let bad =
        |e: symphonia::core::errors::Error| SampleError::Decode(path.to_path_buf(), e.to_string());
    let file = std::fs::File::open(path).map_err(|e| SampleError::Io(path.to_path_buf(), e))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("ogg");
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(bad)?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| SampleError::Decode(path.to_path_buf(), "no audio track".into()))?;
    let id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(bad)?;
    let mut channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(1);
    let mut out = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
            }
            Err(e) => return Err(bad(e)),
        };
        if packet.track_id() != id {
            continue;
        }
        let audio = decoder.decode(&packet).map_err(bad)?;
        channels = audio.spec().channels.count();
        let mut buf = SampleBuffer::<i16>::new(audio.capacity() as u64, *audio.spec());
        buf.copy_interleaved_ref(audio);
        out.extend_from_slice(buf.samples());
    }
    Ok((channels.max(1), out))
}

/// Every set decoded so far, by folder, with its `set.ron` time, so reloading a
/// song (the studio does it on every save) does not decode the recordings again.
type Cache = Mutex<HashMap<PathBuf, (Option<std::time::SystemTime>, Arc<SampleSet>)>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// The set in `dir`, decoded once and shared (decoded again if its `set.ron` changed).
pub fn open(dir: &Path) -> Result<Arc<SampleSet>, SampleError> {
    let stamp = std::fs::metadata(dir.join("set.ron"))
        .and_then(|m| m.modified())
        .ok();
    let mut c = cache().lock().unwrap_or_else(|p| p.into_inner());
    if let Some((s, set)) = c.get(dir) {
        if *s == stamp {
            return Ok(set.clone());
        }
    }
    let set = Arc::new(SampleSet::load(dir)?);
    c.insert(dir.to_path_buf(), (stamp, set.clone()));
    Ok(set)
}
