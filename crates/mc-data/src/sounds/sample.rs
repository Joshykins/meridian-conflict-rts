//! Bounded PCM WAV loading for recipe sample layers. No runtime audio-thread IO.
use crate::DataError;
use std::{
    io::Read,
    path::{Component, Path},
    sync::Arc,
};

/// Decoded mono/stereo 16-bit PCM, shared by sounds derived with `like`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SampleClip {
    pub rate: u32,
    pub frames: Arc<[[f32; 2]]>,
}

const MAX_BYTES: usize = 16 * 1024 * 1024;

pub(super) fn load(root: &Path, file: &str) -> Result<SampleClip, DataError> {
    let relative = Path::new(file);
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
        || file.contains('\\')
        || file.contains(':')
        || file.is_empty()
    {
        return Err(DataError::Invalid(format!(
            "sample path must stay inside data/: {file}"
        )));
    }
    let path = root.join(relative);
    let canonical_root = root
        .canonicalize()
        .map_err(|e| DataError::Io(root.to_owned(), e))?;
    let canonical = path
        .canonicalize()
        .map_err(|e| DataError::Io(path.clone(), e))?;
    if !canonical.starts_with(canonical_root) {
        return Err(DataError::Invalid(format!("sample escapes data/: {file}")));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&path)
        .and_then(|f| f.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes))
        .map_err(|e| DataError::Io(path.clone(), e))?;
    decode(&bytes).map_err(|e| DataError::Invalid(format!("{}: {e}", path.display())))
}

fn decode(bytes: &[u8]) -> Result<SampleClip, &'static str> {
    if bytes.len() > MAX_BYTES
        || bytes.len() < 12
        || &bytes[..4] != b"RIFF"
        || &bytes[8..12] != b"WAVE"
    {
        return Err("expected a bounded RIFF/WAVE file");
    }
    let u16_at = |b: &[u8], i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_at = |b: &[u8], i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let end = (u32_at(bytes, 4) as usize)
        .checked_add(8)
        .ok_or("RIFF size overflow")?;
    if end != bytes.len() {
        return Err("RIFF length does not match file");
    }
    let (mut format, mut data) = (None, None);
    let mut pos = 12;
    while pos < end {
        if end - pos < 8 {
            return Err("truncated WAV chunk header");
        }
        let len = u32_at(bytes, pos + 4) as usize;
        let start = pos + 8;
        let stop = start
            .checked_add(len)
            .filter(|&n| n <= end)
            .ok_or("truncated WAV chunk")?;
        let chunk = &bytes[start..stop];
        match &bytes[pos..pos + 4] {
            b"fmt " => {
                if format.is_some() || len < 16 {
                    return Err("invalid WAV format chunk");
                }
                let (kind, channels, rate, align, bits) = (
                    u16_at(chunk, 0),
                    u16_at(chunk, 2),
                    u32_at(chunk, 4),
                    u16_at(chunk, 12),
                    u16_at(chunk, 14),
                );
                if kind != 1
                    || !(1..=2).contains(&channels)
                    || bits != 16
                    || !(8000..=192000).contains(&rate)
                    || align != channels * 2
                    || u32_at(chunk, 8) != rate * u32::from(align)
                {
                    return Err("sample must be mono/stereo 16-bit PCM at 8-192 kHz");
                }
                format = Some((channels, rate, align));
            }
            b"data" => {
                let previous = data.replace(chunk);
                if previous.is_some() {
                    return Err("duplicate WAV data chunk");
                }
            }
            _ => {}
        }
        pos = stop
            .checked_add(len % 2)
            .filter(|&n| n <= end)
            .ok_or("missing WAV chunk padding")?;
    }
    let (channels, rate, align) = format.ok_or("missing WAV format")?;
    let data = data.ok_or("missing WAV samples")?;
    if data.is_empty() || data.len() % usize::from(align) != 0 {
        return Err("incomplete PCM frame");
    }
    if data.len() / usize::from(align) > rate as usize * 40 {
        return Err("sample exceeds 40 seconds");
    }
    let frames: Vec<_> = data
        .chunks_exact(usize::from(align))
        .map(|f| {
            let left = i16::from_le_bytes([f[0], f[1]]) as f32 / 32768.0;
            let right = if channels == 1 {
                left
            } else {
                i16::from_le_bytes([f[2], f[3]]) as f32 / 32768.0
            };
            [left, right]
        })
        .collect();
    Ok(SampleClip {
        rate,
        frames: frames.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const F: &[u8] = include_bytes!("../../../../data/sounds/samples/regency-repeater-f.wav");
    #[test]
    fn chosen_sample_loads_and_malformed_chunks_fail() {
        let clip = decode(F).unwrap();
        assert_eq!(clip.rate, 48000);
        assert_eq!(clip.frames.len(), 29760);
        for n in [0, 11, 20, 43, F.len() - 1] {
            assert!(decode(&F[..n]).is_err());
        }
        let mut corrupt = F.to_vec();
        corrupt[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode(&corrupt).is_err());
        corrupt = F.to_vec();
        corrupt[22] = 3;
        assert!(decode(&corrupt).is_err());
        let root = Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../data");
        for path in [
            "../Cargo.toml",
            "/etc/passwd",
            "C:/sample.wav",
            "..\\sample.wav",
        ] {
            assert!(load(&root, path).is_err());
        }
    }
}
