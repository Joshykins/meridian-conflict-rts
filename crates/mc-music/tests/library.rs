//! The instrument library and its recordings, as checked in under data/music.

use mc_music::library::{self, Library};
use mc_music::render::render;
use mc_music::song::{Clip, Note, Pattern, Section, SectionKind, Track};
use mc_music::{Instrument, Mode, Song};
use std::path::PathBuf;

fn music_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/music")
}

/// One track playing `inst` one note: `key` at `vel`, `beats` long, in a 4-bar loop.
fn one_note(inst: Instrument, key: u8, vel: u8, beats: u32) -> Song {
    let mut s = Song::empty("t");
    s.tempo = 60.0;
    s.tracks.push(Track {
        name: "a".into(),
        instrument: inst,
        db: 0.0,
        pan: 0.0,
        mute: false,
        solo: false,
        sends: vec![],
        effects: vec![],
        layer: Default::default(),
        follow: vec![],
        colour: 0,
    });
    s.patterns.push(Pattern {
        name: "p".into(),
        beats: 16,
        notes: vec![Note(0, beats * mc_music::PPQ, key, vel)],
        automation: vec![],
    });
    s.sections.push(Section {
        name: "s".into(),
        bars: 4,
        kind: SectionKind::Loop,
        intensity: (0.0, 1.0),
        next: vec![],
        exit_every: 0,
        clips: vec![Clip {
            track: "a".into(),
            pattern: "p".into(),
            at: 0,
            times: 0,
            transpose: 0,
        }],
    });
    s.arrangement = vec!["s".into()];
    s.link(&music_dir());
    s
}

/// RMS in dB of each 0.2 s window.
fn windows_db(frames: &[[f32; 2]], rate: usize) -> Vec<f32> {
    frames
        .chunks(rate / 5)
        .map(|c| {
            let p: f32 = c.iter().map(|f| f[0] * f[0] + f[1] * f[1]).sum::<f32>() / c.len() as f32;
            10.0 * (p + 1e-12).log10()
        })
        .collect()
}

#[test]
fn every_library_instrument_and_recording_loads() {
    let dir = music_dir();
    let names = library::instrument_names(&dir);
    assert!(
        names.iter().any(|n| n == "violins"),
        "no violins in {names:?}"
    );
    let insts: Vec<Instrument> = names.iter().map(|n| Instrument::Use(n.clone())).collect();
    let lib = Library::link(&dir, insts.iter());
    assert!(lib.problems().is_empty(), "{:?}", lib.problems());
    for set in library::set_names(&dir) {
        let bank = mc_music::samples::open(&library::samples_dir(&dir).join(&set)).unwrap();
        assert!(!bank.zones.is_empty(), "{set}: no notes");
        for z in &bank.zones {
            assert!(z.len() > 1000, "{set}: key {} is {} frames", z.key, z.len());
        }
    }
}

#[test]
fn the_songs_and_moments_find_their_instruments() {
    let dir = music_dir();
    let files = [dir.clone(), dir.join("moments")]
        .into_iter()
        .flat_map(|d| std::fs::read_dir(d).unwrap().flatten().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "ron") && !p.ends_with("score.ron"));
    let mut n = 0;
    for path in files {
        let song = Song::load(&path).unwrap();
        assert!(
            song.problems().is_empty(),
            "{}: {:?}",
            path.display(),
            song.problems()
        );
        assert!(song.library.dir().is_some());
        n += 1;
    }
    assert!(n > 1);
}

#[test]
fn a_held_recorded_note_sustains_past_its_recording() {
    // Eight seconds of cello, recorded as four and a half: the loop carries it.
    let rate = 48000;
    let song = one_note(Instrument::Use("cellos".into()), 48, 100, 8);
    let out = render(&song, rate as u32, 9.0, Mode::Song, (1.0, 1.0));
    let db = windows_db(&out, rate);
    let held = &db[5..39];
    let (lo, hi) = held
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), &x| (a.min(x), b.max(x)));
    assert!(lo > -45.0, "the note dropped out: {db:?}");
    assert!(
        hi - lo < 8.0,
        "the level pumps by {:.1} dB: {db:?}",
        hi - lo
    );
    // And it stops once let go.
    assert!(db[44] < lo - 20.0, "no release: {db:?}");
}

#[test]
fn an_unpitched_set_sounds_only_on_its_keys() {
    let rate = 48000;
    let hit = render(
        &one_note(Instrument::Use("percussion".into()), 34, 110, 1),
        rate,
        1.0,
        Mode::Song,
        (1.0, 1.0),
    );
    let miss = render(
        &one_note(Instrument::Use("percussion".into()), 60, 110, 1),
        rate,
        1.0,
        Mode::Song,
        (1.0, 1.0),
    );
    let peak = |f: &[[f32; 2]]| {
        f.iter()
            .map(|x| x[0].abs().max(x[1].abs()))
            .fold(0.0, f32::max)
    };
    assert!(peak(&hit) > 0.05);
    assert_eq!(peak(&miss), 0.0);
}

#[test]
fn held_strings_and_brass_speak_on_the_beat() {
    // A bowed or blown recording swells for up to half a second; the sampler starts
    // each note where it speaks, so every note of a held set is near full level
    // within a tenth of a second of its beat.
    let rate = 48000;
    for (set, keys) in [
        ("violins", [60, 67, 74, 79]),
        ("violas", [55, 60, 67, 72]),
        ("cellos", [36, 43, 48, 55]),
        ("horns", [48, 55, 60, 67]),
    ] {
        for key in keys {
            let song = one_note(Instrument::Use(set.into()), key, 100, 4);
            let out = render(&song, rate as u32, 1.5, Mode::Song, (1.0, 1.0));
            let ms5: Vec<f32> = out
                .chunks(rate / 200)
                .map(|c| c.iter().map(|f| f[0] * f[0] + f[1] * f[1]).sum::<f32>() / c.len() as f32)
                .collect();
            let peak = ms5[..200].iter().copied().fold(0.0, f32::max);
            let speaks = ms5.iter().position(|&p| p >= peak * 0.1).unwrap() * 5;
            assert!(speaks <= 100, "{set} key {key}: -10 dB after {speaks} ms");
        }
    }
}
