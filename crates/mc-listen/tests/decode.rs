//! Decoding a WAV that mc-music wrote, and the lead sheet of a song file.

mod common;
use common::*;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mc-listen-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

#[test]
fn decodes_mc_music_wav() {
    let frames: Vec<[f32; 2]> = (0..48000)
        .map(|i| {
            let t = i as f32 / 48000.0;
            [
                0.5 * (2.0 * std::f32::consts::PI * 220.0 * t).sin(),
                0.25 * (2.0 * std::f32::consts::PI * 330.0 * t).sin(),
            ]
        })
        .collect();
    let path = scratch("tone.wav");
    mc_music::render::write_wav(&path, &frames, 48000).unwrap();
    let a = mc_listen::load(&path).unwrap();
    assert_eq!(a.rate, 48000);
    assert_eq!(a.frames.len(), frames.len());
    let err = a
        .frames
        .iter()
        .zip(&frames)
        .map(|(x, y)| (x[0] - y[0]).abs().max((x[1] - y[1]).abs()))
        .fold(0.0f32, f32::max);
    assert!(err < 2.0 / 32767.0, "max error {err}");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn missing_and_bad_files_are_errors() {
    assert!(mc_listen::load(std::path::Path::new("/nonexistent/x.wav")).is_err());
    let path = scratch("junk.mp3");
    std::fs::write(&path, b"this is not audio at all").unwrap();
    assert!(mc_listen::load(&path).is_err());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn song_lead_sheet_is_exact() {
    let case = Case {
        name: "sheet",
        tempo: 120.0,
        key_root: 0,
        minor: true,
        prog: vec![ch(0, true), ch(8, false), ch(3, false), ch(10, false)],
        bass_rhythm: vec![(0, 3, 0), (3, 3, 0), (6, 2, 0), (8, 4, 7), (12, 4, 10)],
        drums: ["x.......x.x.....", "....x.......x...", "x.x.x.x.x.x.x.x."],
        bars: 8,
        lead: true,
        sidechain: false,
    };
    let t = build(&case);
    let sheet = mc_listen::leadsheet::from_song(&t.song, None, 1.0).unwrap();
    println!("{sheet}");
    assert_eq!(sheet.key_name, "C minor");
    let s = &sheet.sections[0];
    assert_eq!(
        mc_listen::leadsheet::progression(&s.numerals),
        "(i | VI | III | VII) x2"
    );
    assert_eq!(
        mc_listen::leadsheet::progression(&s.chords),
        "(Cm | Ab | Eb | Bb) x2"
    );
    assert!((s.harmonic_rhythm - 1.0).abs() < 1e-3);
    assert_eq!(s.bass_rhythm, "x--x--x-x---x---");
    assert_eq!(s.bass_degrees, "1 . . 1 . . 1 . 5 . . . b7 . . .");
    assert_eq!(
        s.drums,
        [
            "x.......x.x.....".to_string(),
            "....x.......x...".into(),
            "x.x.x.x.x.x.x.x.".into()
        ]
    );
    assert!(s.melody.is_some());
    // Transposing the song changes names, never numerals or degrees.
    let mut up = t.song.clone();
    for c in up.sections[0].clips.iter_mut() {
        if c.track != "drums" {
            c.transpose = 5;
        }
    }
    let s2 = mc_listen::leadsheet::from_song(&up, None, 1.0).unwrap();
    assert_eq!(s2.key.unwrap().name(), "F minor");
    assert_eq!(s2.sections[0].numerals, s.numerals);
    assert_eq!(s2.sections[0].bass_degrees, s.bass_degrees);
    let diffs = mc_listen::compare_charts(&s2, &sheet);
    assert!(
        diffs
            .iter()
            .all(|d| d.topic == "harmony" && d.text.starts_with("transposed")),
        "{:?}",
        diffs.iter().map(|d| &d.text).collect::<Vec<_>>()
    );
}

#[test]
fn numerals() {
    use mc_listen::theory::{Chord, Key, Quality};
    let c_major = Key {
        root: 0,
        minor: false,
    };
    let a_minor = Key {
        root: 9,
        minor: true,
    };
    let n = |root: u8, q: Quality, k: &Key| Chord { root, quality: q }.numeral(k);
    assert_eq!(n(7, Quality::Maj, &c_major), "V");
    assert_eq!(n(7, Quality::Dom7, &c_major), "V7");
    assert_eq!(n(2, Quality::Maj, &c_major), "V/V");
    assert_eq!(n(4, Quality::Dom7, &c_major), "V7/vi");
    assert_eq!(n(10, Quality::Maj, &c_major), "bVII");
    assert_eq!(n(8, Quality::Maj, &c_major), "bVI");
    assert_eq!(n(11, Quality::Dim, &c_major), "vii°");
    assert_eq!(n(9, Quality::Min, &a_minor), "i");
    assert_eq!(n(5, Quality::Maj, &a_minor), "VI");
    assert_eq!(n(4, Quality::Maj, &a_minor), "V");
    assert_eq!(n(7, Quality::Maj, &a_minor), "VII");
    assert_eq!(
        Key::parse("C:minor"),
        Some(Key {
            root: 0,
            minor: true
        })
    );
    assert_eq!(
        Key::parse("Bb major"),
        Some(Key {
            root: 10,
            minor: false
        })
    );
    assert_eq!(
        Key::parse("f#m"),
        Some(Key {
            root: 6,
            minor: true
        })
    );
}
