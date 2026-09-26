//! Moments: the short pieces the game plays over the song when something
//! happens (`data/music/moments/<name>.ron`). The song dips while one plays and
//! comes back after; an ending (listed in `score.ron`) stops the song.
//!
//! In the studio a moment plays when its chip is clicked, or when the playhead
//! crosses a marker dropped on the timeline: a way to hear "a nuke lands here".
//! Markers belong to the studio, not the song: they are kept per song in the
//! user's config folder.

use crate::app::{Studio, Tone};
use crate::audio::RefCmd;
use mc_music::Song;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

pub struct Moment {
    pub name: String,
    pub path: PathBuf,
    pub ending: bool,
    /// Loaded when first played, reloaded when the file changes.
    song: Option<(Arc<Song>, Option<SystemTime>)>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Marker {
    /// Arrangement tick.
    pub tick: u32,
    pub name: String,
}

#[derive(Default)]
pub struct Moments {
    pub list: Vec<Moment>,
    scanned_at: Option<f64>,
    pub duck_db: f32,
    duck_sent: Option<f32>,
    pub markers: Vec<Marker>,
    markers_for: Option<String>,
    /// Where the playhead was last frame (arrangement ticks).
    last_pos: Option<u32>,
    /// The moment chip being dragged onto the timeline.
    pub drag: Option<String>,
}

/// Moment files and the score's endings and dip depth.
pub fn scan(st: &mut Studio, now: f64) {
    if st.moments.scanned_at.is_some_and(|t| now - t < 3.0) {
        return;
    }
    st.moments.scanned_at = Some(now);
    let Some(dir) = st.music_dir.clone() else {
        return;
    };
    let score = mc_music::Score::load(&dir).unwrap_or_default();
    st.moments.duck_db = if score.duck_db < 0.0 {
        score.duck_db
    } else {
        -16.0
    };
    let mut found: Vec<(String, PathBuf)> = std::fs::read_dir(dir.join("moments"))
        .map(|r| {
            r.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "ron"))
                .filter_map(|p| Some((p.file_stem()?.to_string_lossy().into_owned(), p)))
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    let old = std::mem::take(&mut st.moments.list);
    st.moments.list = found
        .into_iter()
        .map(|(name, path)| {
            // A game event can name a moment file differently: an event is an
            // ending when it, or the file it plays, is listed as one.
            let ending = score
                .endings
                .iter()
                .any(|e| *e == name || score.moments.get(e).is_some_and(|f| *f == name));
            let song = old
                .iter()
                .find(|m| m.path == path)
                .and_then(|m| m.song.clone());
            Moment {
                name,
                path,
                ending,
                song,
            }
        })
        .collect();
    if st.moments.duck_sent != Some(st.moments.duck_db) {
        st.moments.duck_sent = Some(st.moments.duck_db);
        st.audio.send_ref(RefCmd::Duck(st.moments.duck_db));
    }
}

/// Plays a moment over the song now.
pub fn play(st: &mut Studio, name: &str) {
    let Some(m) = st.moments.list.iter_mut().find(|m| m.name == name) else {
        return;
    };
    let mtime = crate::files::mtime(&m.path);
    let fresh = m.song.as_ref().is_some_and(|(_, t)| *t == mtime);
    if !fresh {
        match Song::load(&m.path) {
            Ok(s) => m.song = Some((Arc::new(s), mtime)),
            Err(e) => {
                st.say(e, Tone::Bad);
                return;
            }
        }
    }
    let (song, ending) = (m.song.as_ref().unwrap().0.clone(), m.ending);
    st.audio
        .send_ref(RefCmd::Moment(name.to_string(), song, ending));
}

fn markers_file(stem: &str) -> String {
    format!("markers_{stem}.ron")
}

/// Loads the song's markers when the song changes.
pub fn load_markers(st: &mut Studio) {
    let key = st.collab.stem.clone();
    if st.moments.markers_for == key {
        return;
    }
    st.moments.markers_for = key.clone();
    st.moments.markers = key
        .and_then(|k| crate::files::read_config(&markers_file(&k)))
        .and_then(|t| ron::from_str(&t).ok())
        .unwrap_or_default();
}

fn save_markers(st: &Studio) {
    if let Some(k) = &st.moments.markers_for {
        if let Ok(text) = ron::to_string(&st.moments.markers) {
            crate::files::write_config(&markers_file(k), &text);
        }
    }
}

pub fn add_marker(st: &mut Studio, tick: u32, name: &str) {
    st.moments.markers.push(Marker {
        tick,
        name: name.to_string(),
    });
    st.moments.markers.sort_by_key(|m| m.tick);
    save_markers(st);
}

pub fn remove_marker(st: &mut Studio, i: usize) {
    if i < st.moments.markers.len() {
        st.moments.markers.remove(i);
        save_markers(st);
    }
}

/// Markers passed going from `prev` to `cur` (a jump back is a loop: from 0).
pub fn crossed(markers: &[Marker], prev: u32, cur: u32) -> Vec<usize> {
    let hit = |t: u32| {
        if cur >= prev {
            t > prev && t <= cur
        } else {
            t <= cur
        }
    };
    markers
        .iter()
        .enumerate()
        .filter(|(_, m)| hit(m.tick))
        .map(|(i, _)| i)
        .collect()
}

/// Fires markers as the playhead crosses them. `pos` is the playhead in
/// arrangement ticks, or `None` when stopped.
pub fn follow(st: &mut Studio, pos: Option<u32>) {
    let prev = st.moments.last_pos;
    st.moments.last_pos = pos;
    let (Some(p), Some(c)) = (prev, pos) else {
        return;
    };
    // A seek far away is not a crossing.
    let bar = st.song.bar_ticks();
    if c > p + 4 * bar {
        return;
    }
    for i in crossed(&st.moments.markers, p, c) {
        let name = st.moments.markers[i].name.clone();
        play(st, &name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_fire_once_as_the_playhead_passes() {
        let m = vec![
            Marker {
                tick: 100,
                name: "a".into(),
            },
            Marker {
                tick: 400,
                name: "b".into(),
            },
        ];
        assert_eq!(crossed(&m, 90, 100), vec![0]);
        assert!(
            crossed(&m, 100, 150).is_empty(),
            "not again on the next frame"
        );
        assert_eq!(crossed(&m, 390, 410), vec![1]);
        // Looping back to the start of the part passes the first again.
        assert_eq!(crossed(&m, 900, 120), vec![0]);
    }
}
