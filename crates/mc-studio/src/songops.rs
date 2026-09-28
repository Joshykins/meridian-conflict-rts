//! Edits that touch more than one place in a song. Tracks, patterns, sections
//! and buses refer to each other by name, so renaming or deleting one has to
//! follow every reference or the song quietly loses parts.

use crate::theme::TRACK_COLOURS;
use mc_music::patch::{Body, Drum, Hiss, Kit, Ring};
use mc_music::song::{Layer, Master};
use mc_music::{
    Clip, Effect, Instrument, Note, Pattern, Section, SectionKind, Song, Synth, Track, PPQ,
};

/// `base`, or `base 2`, `base 3`... whichever is free.
pub fn unique_name(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_string();
    }
    // "Verse 2" duplicates to "Verse 3", not "Verse 2 2".
    let (stem, mut n) = match base.rsplit_once(' ') {
        Some((s, d)) if d.parse::<u32>().is_ok() => {
            (s.to_string(), d.parse::<u32>().unwrap_or(1) + 1)
        }
        _ => (base.to_string(), 2),
    };
    loop {
        let name = format!("{stem} {n}");
        if !taken(&name) {
            return name;
        }
        n += 1;
    }
}

pub fn rename_track(song: &mut Song, index: usize, name: &str) {
    let Some(old) = song.tracks.get(index).map(|t| t.name.clone()) else {
        return;
    };
    if old == name || name.is_empty() || song.track(name).is_some() {
        return;
    }
    song.tracks[index].name = name.to_string();
    for s in &mut song.sections {
        for c in &mut s.clips {
            if c.track == old {
                c.track = name.to_string();
            }
        }
    }
    for t in &mut song.tracks {
        for fx in &mut t.effects {
            if let Effect::Compressor {
                sidechain: Some(sc),
                ..
            } = fx
            {
                if *sc == old {
                    *sc = name.to_string();
                }
            }
        }
    }
}

pub fn rename_pattern(song: &mut Song, index: usize, name: &str) {
    let Some(old) = song.patterns.get(index).map(|p| p.name.clone()) else {
        return;
    };
    if old == name || name.is_empty() || song.pattern(name).is_some() {
        return;
    }
    song.patterns[index].name = name.to_string();
    for s in &mut song.sections {
        for c in &mut s.clips {
            if c.pattern == old {
                c.pattern = name.to_string();
            }
        }
    }
}

pub fn rename_section(song: &mut Song, index: usize, name: &str) {
    let Some(old) = song.sections.get(index).map(|s| s.name.clone()) else {
        return;
    };
    if old == name || name.is_empty() || song.section(name).is_some() {
        return;
    }
    song.sections[index].name = name.to_string();
    for a in &mut song.arrangement {
        if *a == old {
            *a = name.to_string();
        }
    }
    for s in &mut song.sections {
        for n in &mut s.next {
            if *n == old {
                *n = name.to_string();
            }
        }
    }
}

pub fn rename_bus(song: &mut Song, index: usize, name: &str) {
    let Some(old) = song.buses.get(index).map(|b| b.name.clone()) else {
        return;
    };
    if old == name || name.is_empty() || song.bus(name).is_some() {
        return;
    }
    song.buses[index].name = name.to_string();
    for t in &mut song.tracks {
        for s in &mut t.sends {
            if s.bus == old {
                s.bus = name.to_string();
            }
        }
    }
}

/// Removes a track and every clip that played on it.
pub fn delete_track(song: &mut Song, index: usize) {
    if index >= song.tracks.len() {
        return;
    }
    let name = song.tracks.remove(index).name;
    for s in &mut song.sections {
        s.clips.retain(|c| c.track != name);
    }
    for t in &mut song.tracks {
        for fx in &mut t.effects {
            if let Effect::Compressor { sidechain, .. } = fx {
                if sidechain.as_deref() == Some(name.as_str()) {
                    *sidechain = None;
                }
            }
        }
    }
}

pub fn delete_pattern(song: &mut Song, index: usize) {
    if index >= song.patterns.len() {
        return;
    }
    let name = song.patterns.remove(index).name;
    for s in &mut song.sections {
        s.clips.retain(|c| c.pattern != name);
    }
}

pub fn delete_section(song: &mut Song, index: usize) {
    if index >= song.sections.len() {
        return;
    }
    let name = song.sections.remove(index).name;
    song.arrangement.retain(|a| *a != name);
    for s in &mut song.sections {
        s.next.retain(|n| *n != name);
    }
}

pub fn delete_bus(song: &mut Song, index: usize) {
    if index >= song.buses.len() {
        return;
    }
    let name = song.buses.remove(index).name;
    for t in &mut song.tracks {
        t.sends.retain(|s| s.bus != name);
    }
}

/// Copies a section under a free name and returns the copy's index.
pub fn duplicate_section(song: &mut Song, index: usize) -> usize {
    let mut s = song.sections[index].clone();
    s.name = unique_name(&s.name, |n| song.section(n).is_some());
    song.sections.push(s);
    song.sections.len() - 1
}

pub fn duplicate_pattern(song: &mut Song, index: usize) -> usize {
    let mut p = song.patterns[index].clone();
    p.name = unique_name(&p.name, |n| song.pattern(n).is_some());
    song.patterns.push(p);
    song.patterns.len() - 1
}

pub fn duplicate_track(song: &mut Song, index: usize) -> usize {
    let mut t = song.tracks[index].clone();
    t.name = unique_name(&t.name, |n| song.track(n).is_some());
    t.solo = false;
    song.tracks.insert(index + 1, t);
    index + 1
}

pub fn new_pattern(song: &mut Song, base: &str, beats: u32) -> usize {
    let name = unique_name(base, |n| song.pattern(n).is_some());
    song.patterns.push(Pattern {
        name,
        beats: beats.max(1),
        notes: Vec::new(),
        automation: Vec::new(),
    });
    song.patterns.len() - 1
}

pub fn new_section(song: &mut Song, base: &str, bars: u32) -> usize {
    let name = unique_name(base, |n| song.section(n).is_some());
    song.sections.push(Section {
        name,
        bars: bars.max(1),
        kind: SectionKind::Loop,
        intensity: (0.0, 1.0),
        next: Vec::new(),
        exit_every: 0,
        clips: Vec::new(),
    });
    song.sections.len() - 1
}

/// A plain four-piece kit to start a drum track from.
pub fn starter_kit() -> Kit {
    Kit {
        drums: vec![
            Drum {
                name: "Kick".into(),
                key: 36,
                body: Some(Body {
                    from: 180.0,
                    to: 48.0,
                    sweep: 0.025,
                    decay: 0.35,
                    gain: 1.0,
                    overtone: 0.0,
                }),
                hiss: None,
                ring: None,
                click: 0.3,
                drive: 0.2,
                gain: 1.0,
                pan: 0.0,
                choke: 0,
                velocity: 0.5,
                fixed: true,
            },
            Drum {
                name: "Snare".into(),
                key: 38,
                body: Some(Body {
                    from: 240.0,
                    to: 180.0,
                    sweep: 0.02,
                    decay: 0.08,
                    gain: 0.5,
                    overtone: 0.0,
                }),
                hiss: Some(Hiss {
                    mode: mc_music::patch::FilterMode::BandPass,
                    cutoff: 3500.0,
                    resonance: 0.2,
                    attack: 0.0,
                    decay: 0.16,
                    gain: 0.8,
                    bursts: 1,
                    spread: 0.0,
                }),
                ring: None,
                click: 0.0,
                drive: 0.0,
                gain: 1.0,
                pan: 0.0,
                choke: 0,
                velocity: 0.5,
                fixed: true,
            },
            Drum {
                name: "Closed hat".into(),
                key: 42,
                body: None,
                hiss: None,
                ring: Some(Ring {
                    freq: 420.0,
                    decay: 0.05,
                    gain: 0.6,
                    highpass: 6000.0,
                }),
                click: 0.0,
                drive: 0.0,
                gain: 1.0,
                pan: 0.1,
                choke: 1,
                velocity: 0.5,
                fixed: true,
            },
            Drum {
                name: "Open hat".into(),
                key: 46,
                body: None,
                hiss: None,
                ring: Some(Ring {
                    freq: 420.0,
                    decay: 0.35,
                    gain: 0.5,
                    highpass: 6000.0,
                }),
                click: 0.0,
                drive: 0.0,
                gain: 1.0,
                pan: 0.1,
                choke: 1,
                velocity: 0.5,
                fixed: true,
            },
        ],
    }
}

pub fn new_track(song: &mut Song, kit: bool) -> usize {
    let base = if kit { "Drums" } else { "Synth" };
    let name = unique_name(base, |n| song.track(n).is_some());
    let colour = TRACK_COLOURS[song.tracks.len() % TRACK_COLOURS.len()];
    song.tracks.push(Track {
        name,
        instrument: if kit {
            Instrument::Kit(starter_kit())
        } else {
            Instrument::Synth(Synth::default())
        },
        db: -6.0,
        pan: 0.0,
        mute: false,
        solo: false,
        sends: Vec::new(),
        effects: Vec::new(),
        layer: Layer::default(),
        follow: Vec::new(),
        colour,
    });
    song.tracks.len() - 1
}

/// A new song with a drum track, a synth, one section and a limiter: something
/// that plays the moment it is made.
pub fn starter_song(name: &str) -> Song {
    let mut s = Song::empty(name);
    s.tempo = 120.0;
    new_track(&mut s, true);
    new_track(&mut s, false);
    s.master = Master {
        db: 0.0,
        effects: vec![Effect::Limiter {
            ceiling: -1.0,
            gain: 0.0,
            release: 80.0,
            on: true,
        }],
    };
    let beat: Vec<Note> = (0..4)
        .flat_map(|b| {
            let t = b * PPQ;
            let mut v = vec![Note(t, PPQ / 4, 42, 70)];
            v.push(Note(t, PPQ / 4, if b % 2 == 0 { 36 } else { 38 }, 110));
            v
        })
        .collect();
    s.patterns.push(Pattern {
        name: "Beat".into(),
        beats: 4,
        notes: beat,
        automation: Vec::new(),
    });
    s.patterns.push(Pattern {
        name: "Chords".into(),
        beats: 8,
        notes: vec![
            Note(0, 4 * PPQ, 57, 90),
            Note(0, 4 * PPQ, 60, 90),
            Note(0, 4 * PPQ, 64, 90),
            Note(4 * PPQ, 4 * PPQ, 55, 90),
            Note(4 * PPQ, 4 * PPQ, 59, 90),
            Note(4 * PPQ, 4 * PPQ, 62, 90),
        ],
        automation: Vec::new(),
    });
    let sec = new_section(&mut s, "Main", 4);
    s.sections[sec].clips = vec![
        Clip {
            track: s.tracks[0].name.clone(),
            pattern: "Beat".into(),
            at: 0,
            times: 0,
            transpose: 0,
        },
        Clip {
            track: s.tracks[1].name.clone(),
            pattern: "Chords".into(),
            at: 0,
            times: 0,
            transpose: 0,
        },
    ];
    s.arrangement.push(s.sections[sec].name.clone());
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_names_count_up() {
        let taken = ["Verse", "Verse 2"];
        assert_eq!(unique_name("Chorus", |n| taken.contains(&n)), "Chorus");
        assert_eq!(unique_name("Verse", |n| taken.contains(&n)), "Verse 3");
        assert_eq!(unique_name("Verse 2", |n| taken.contains(&n)), "Verse 3");
    }

    #[test]
    fn renames_follow_references() {
        let mut s = starter_song("t");
        let track = s.tracks[0].name.clone();
        rename_track(&mut s, 0, "Kit");
        assert!(s.sections[0].clips.iter().any(|c| c.track == "Kit"));
        assert!(!s.sections[0].clips.iter().any(|c| c.track == track));
        rename_pattern(&mut s, 0, "Groove");
        assert!(s.sections[0].clips.iter().any(|c| c.pattern == "Groove"));
        let d = duplicate_section(&mut s, 0);
        s.sections[d].next.push("Main".into());
        rename_section(&mut s, 0, "Calm");
        assert_eq!(s.arrangement, vec!["Calm".to_string()]);
        assert_eq!(s.sections[d].next, vec!["Calm".to_string()]);
        assert!(s.problems().is_empty(), "{:?}", s.problems());
    }

    #[test]
    fn a_rename_onto_a_taken_name_is_refused() {
        let mut s = starter_song("t");
        let other = s.tracks[1].name.clone();
        rename_track(&mut s, 0, &other);
        assert_ne!(s.tracks[0].name, other);
    }

    #[test]
    fn deletes_leave_no_dangling_names() {
        let mut s = starter_song("t");
        delete_pattern(&mut s, 0);
        assert_eq!(s.sections[0].clips.len(), 1);
        delete_track(&mut s, 1);
        assert!(s.sections[0].clips.is_empty());
        delete_section(&mut s, 0);
        assert!(s.arrangement.is_empty());
        assert!(s.problems().is_empty());
    }
}
