//! Working on a song with Claude, who is in a chat while the studio is open.
//!
//! The shared state is `mc_music::history::Desk` (`data/music/.studio/`):
//! Claude proposes whole versions of the song there, and the studio reviews
//! them live. Auditioning a proposal swaps the engine's song with
//! `Command::SetSong` and nothing else, so the playhead, voices and reverb
//! tails carry on and the user hears the two versions against each other at
//! the same moment of the music.
//!
//! The studio also writes what it is showing (`Session`) so "this part" in a
//! chat message resolves to a section, a pattern, selected notes or a loop,
//! autosaves unsaved work where Claude can read it, and posts the "Tell
//! Claude" box's messages to the inbox.

use crate::app::{Studio, Tone};
use crate::theme::{self, ACCENT, BG0, BG2, DIM, FAINT, GOOD, TEXT, WARN};
use eframe::egui::{self, vec2, CornerRadius, Stroke};
use mc_music::history::{
    self, Desk, Entry, Log, Message, Reaction, Session, Status as EntryStatus,
};
use mc_music::{Pattern, Song, PPQ};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DockTab {
    Review,
    History,
    Messages,
}

pub struct Collab {
    pub desk: Option<Desk>,
    /// The song's key on the desk: its file stem.
    pub stem: Option<String>,
    pub log: Log,
    log_mtime: Option<SystemTime>,
    last_poll: f64,
    /// Songs of log entries, loaded on demand.
    cache: HashMap<String, Song>,
    /// The entry the engine is playing instead of the working copy.
    pub audition: Option<String>,
    /// The last entry auditioned, for Tab to flip back to.
    pub last_audition: Option<String>,
    /// Where each proposal card's Hear button was drawn last frame, for tests.
    pub hear_at: HashMap<String, egui::Rect>,
    pub changed_tracks: HashSet<String>,
    pub changed_patterns: HashSet<String>,
    changes_for: Option<String>,
    changes_at: f64,
    picked: HashMap<String, Vec<Reaction>>,
    pub comment: String,
    pub tell: String,
    pub focus_tell: bool,
    pub inbox: Vec<(PathBuf, Message)>,
    last_inbox: f64,
    last_session: Option<Session>,
    last_session_write: f64,
    autosave_due: Option<f64>,
    autosaved: Option<Song>,
    pub commit_msg: String,
    pub open: bool,
    pub tab: DockTab,
    seen: HashSet<String>,
    pub flash: f64,
    /// Opened history entry.
    expanded: Option<String>,
    /// A sketch waiting to go with the next message.
    pub sketch: Option<Pattern>,
}

impl Default for Collab {
    fn default() -> Collab {
        Collab {
            desk: None,
            stem: None,
            log: Log::default(),
            log_mtime: None,
            last_poll: -10.0,
            cache: HashMap::new(),
            audition: None,
            last_audition: None,
            hear_at: HashMap::new(),
            changed_tracks: HashSet::new(),
            changed_patterns: HashSet::new(),
            changes_for: None,
            changes_at: -10.0,
            picked: HashMap::new(),
            comment: String::new(),
            tell: String::new(),
            focus_tell: false,
            inbox: Vec::new(),
            last_inbox: -10.0,
            last_session: None,
            last_session_write: -10.0,
            autosave_due: None,
            autosaved: None,
            commit_msg: String::new(),
            open: true,
            tab: DockTab::Review,
            seen: HashSet::new(),
            flash: -10.0,
            expanded: None,
            sketch: None,
        }
    }
}

fn log_path(desk: &Desk, stem: &str) -> PathBuf {
    desk.root.join(stem).join("log.ron")
}

impl Collab {
    /// Points the desk at the song file's folder (songs outside a music folder
    /// still get history, next to them).
    pub fn attach(&mut self, path: Option<&Path>) {
        let keep_open = self.open;
        *self = Collab::default();
        self.open = keep_open;
        let Some(path) = path else { return };
        let (Some(dir), Some(stem)) = (path.parent(), path.file_stem()) else {
            return;
        };
        self.desk = Some(Desk::open(dir));
        self.stem = Some(stem.to_string_lossy().into_owned());
    }

    /// Re-read the log on the next frame.
    pub fn force_poll(&mut self) {
        self.last_poll = -10.0;
        self.log_mtime = None;
    }

    pub fn pending(&self) -> Vec<&Entry> {
        self.log
            .entries
            .iter()
            .filter(|e| e.status == EntryStatus::Proposed)
            .collect()
    }

    pub fn head(&self) -> Option<String> {
        self.log
            .entries
            .iter()
            .rev()
            .find(|e| e.status == EntryStatus::Revision)
            .map(|e| e.id.clone())
    }

    fn entry(&self, id: &str) -> Option<&Entry> {
        self.log.entries.iter().find(|e| e.id == id)
    }

    /// The song of a log entry, loading it the first time.
    pub fn song_of(&mut self, id: &str) -> Option<&Song> {
        if !self.cache.contains_key(id) {
            let (desk, stem) = (self.desk.as_ref()?, self.stem.as_ref()?);
            let s = desk.load(stem, id).ok()?;
            self.cache.insert(id.to_string(), s);
        }
        self.cache.get(id)
    }

    /// The song the engine should play: the auditioned entry, if any.
    pub fn audition_song(&self) -> Option<&Song> {
        self.audition.as_ref().and_then(|id| self.cache.get(id))
    }

    pub fn label(&self, id: &str) -> String {
        match self.entry(id) {
            Some(e) => match &e.label {
                Some(l) => format!("{} ({l})", e.id),
                None => e.id.clone(),
            },
            None => id.to_string(),
        }
    }

    /// "A" for a labelled proposal, else its id.
    pub fn entry_label(&self, id: &str) -> String {
        self.entry(id)
            .and_then(|e| e.label.clone())
            .unwrap_or_else(|| id.to_string())
    }

    pub fn set_audition(&mut self, id: Option<String>) {
        if let Some(i) = &id {
            if self.song_of(i).is_none() {
                return;
            }
            self.last_audition = Some(i.clone());
        }
        self.audition = id;
    }
}

/// Seconds since an entry's time, as "12 s", "4 min", "3 h", "2 d".
fn ago(t: u64) -> String {
    let d = history::now().saturating_sub(t);
    match d {
        0..=59 => format!("{d} s ago"),
        60..=3599 => format!("{} min ago", d / 60),
        3600..=86399 => format!("{} h ago", d / 3600),
        _ => format!("{} d ago", d / 86400),
    }
}

// -- the frame's bookkeeping ---------------------------------------------------------

/// Polls the log and inbox, autosaves, writes the session. Once a frame.
pub fn tick(st: &mut Studio, now: f64) {
    if st.collab.desk.is_none() {
        return;
    }
    // The log: about once a second, and only re-read when its file changed.
    if now - st.collab.last_poll > 1.0 {
        st.collab.last_poll = now;
        let (desk, stem) = (
            st.collab.desk.as_ref().unwrap(),
            st.collab.stem.clone().unwrap_or_default(),
        );
        let m = crate::files::mtime(&log_path(desk, &stem));
        if m != st.collab.log_mtime {
            st.collab.log_mtime = m;
            st.collab.log = desk.log(&stem);
            let fresh: Vec<String> = st
                .collab
                .log
                .entries
                .iter()
                .filter(|e| e.status == EntryStatus::Proposed && !st.collab.seen.contains(&e.id))
                .map(|e| e.id.clone())
                .collect();
            for id in &st
                .collab
                .log
                .entries
                .iter()
                .map(|e| e.id.clone())
                .collect::<Vec<_>>()
            {
                st.collab.seen.insert(id.clone());
            }
            if !fresh.is_empty() && st.advanced {
                st.collab.open = true;
                st.collab.tab = DockTab::Review;
                st.collab.flash = now;
                st.say(format!("Claude proposed: {}", fresh.join(", ")), Tone::Warn);
            }
            // A decided or vanished proposal stops sounding.
            if let Some(a) = st.collab.audition.clone() {
                if st.collab.entry(&a).is_none() {
                    st.collab.audition = None;
                }
            }
            st.collab.changes_at = -10.0;
        }
    }
    if now - st.collab.last_inbox > 2.0 {
        st.collab.last_inbox = now;
        st.collab.inbox = read_inbox(st.collab.desk.as_ref().unwrap());
    }
    // What changed, for highlighting: against the auditioned entry, else the
    // newest proposal. Recomputed when the target changes and once a second.
    let target = st
        .collab
        .audition
        .clone()
        .or_else(|| st.collab.pending().last().map(|e| e.id.clone()));
    let due = now - st.collab.changes_at > 1.0;
    if target != st.collab.changes_for || (due && target.is_some()) {
        st.collab.changes_for = target.clone();
        st.collab.changes_at = now;
        st.collab.changed_tracks.clear();
        st.collab.changed_patterns.clear();
        if let Some(t) = target {
            if let Some(other) = st.collab.song_of(&t).cloned() {
                let (tracks, patterns) = changed_parts(&st.song, &other);
                st.collab.changed_tracks = tracks;
                st.collab.changed_patterns = patterns;
            }
        }
    }
    autosave(st, now);
    session(st, now);
}

/// Tracks and patterns that differ between two versions of a song. A track
/// counts as changed when anything it plays changed too.
pub fn changed_parts(a: &Song, b: &Song) -> (HashSet<String>, HashSet<String>) {
    let mut tracks = HashSet::new();
    let mut patterns = HashSet::new();
    for p in &b.patterns {
        if a.patterns.iter().find(|x| x.name == p.name) != Some(p) {
            patterns.insert(p.name.clone());
        }
    }
    for t in &b.tracks {
        if a.tracks.iter().find(|x| x.name == t.name) != Some(t) {
            tracks.insert(t.name.clone());
        }
    }
    for s in &b.sections {
        let old = a.sections.iter().find(|x| x.name == s.name);
        for c in &s.clips {
            let clip_new = old.is_none_or(|o| !o.clips.contains(c));
            if clip_new || patterns.contains(&c.pattern) {
                tracks.insert(c.track.clone());
            }
        }
    }
    (tracks, patterns)
}

fn autosave(st: &mut Studio, now: f64) {
    if !st.dirty {
        st.collab.autosave_due = None;
        return;
    }
    if st.collab.autosaved.as_ref() != Some(&st.song) {
        if st.collab.autosave_due.is_none() {
            st.collab.autosave_due = Some(now + 1.5);
        }
    }
    if st.collab.autosave_due.is_some_and(|t| now >= t) {
        st.collab.autosave_due = None;
        let (Some(desk), Some(stem)) = (&st.collab.desk, &st.collab.stem) else {
            return;
        };
        let path = desk.working_path(stem);
        if let Some(d) = path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        match st.song.save(&path) {
            Ok(()) => st.collab.autosaved = Some(st.song.clone()),
            Err(e) => st.say(format!("Autosave failed: {e}"), Tone::Warn),
        }
    }
}

/// The session as the studio shows it now (time left at 0).
pub fn current_session(st: &Studio) -> Session {
    let song = &st.song;
    let s = &st.status;
    let bar = song.bar_ticks().max(1);
    let bpb = song.beats_per_bar.max(1);
    let playing_section = s.section.and_then(|i| song.sections.get(i));
    let section = if s.playing {
        playing_section
    } else {
        st.sel.section.and_then(|i| song.sections.get(i))
    };
    let position = match (s.playing, playing_section) {
        (true, Some(sec)) => {
            let b = s.section_tick / bar + 1;
            let beat = (s.section_tick / PPQ) % bpb + 1;
            if st.mode == crate::app::PlayMode::Song {
                format!(
                    "bar {b} beat {beat} of {}; arrangement bar {}",
                    sec.name,
                    s.song_tick / bar + 1
                )
            } else {
                format!("bar {b} beat {beat} of {}", sec.name)
            }
        }
        _ if st.mode == crate::app::PlayMode::Pattern && s.playing => {
            format!("beat {} of the pattern", s.pattern_tick / PPQ + 1)
        }
        _ => format!("stopped at arrangement bar {}", s.song_tick / bar + 1),
    };
    let view = match st.page {
        crate::app::Page::Arrange => "Arrange",
        crate::app::Page::Mixer => "Mixer",
        crate::app::Page::Director => "Director",
    };
    let detail = match st.detail {
        crate::app::Detail::Piano => "piano roll",
        crate::app::Detail::Drums => "drum grid",
        crate::app::Detail::Instrument => "instrument",
        crate::app::Detail::Effect => "effect",
        crate::app::Detail::Reference => "reference",
    };
    let pattern = st.sel.pattern.and_then(|p| song.patterns.get(p));
    let notes = match pattern {
        Some(p) => st
            .piano
            .selected()
            .iter()
            .filter_map(|&i| p.notes.get(i))
            .map(|n| (n.0, n.1, n.2, n.3))
            .collect(),
        None => Vec::new(),
    };
    let clip_of = |s: usize, c: usize| -> Option<(String, String, String)> {
        let sec = song.sections.get(s)?;
        let clip = sec.clips.get(c)?;
        Some((sec.name.clone(), clip.track.clone(), clip.pattern.clone()))
    };
    if !st.advanced {
        // The Workbench: the part in view, what is being edited, the listening mutes.
        let wb = &st.wb;
        let edit = wb.edit.clone();
        let focus = wb.focus.clone();
        let clips = match (&edit, &focus) {
            (Some((t, p)), Some(f)) => vec![(f.clone(), t.clone(), p.clone())],
            _ => Vec::new(),
        };
        let mut muted: Vec<String> = song
            .tracks
            .iter()
            .filter(|t| t.mute)
            .map(|t| t.name.clone())
            .collect();
        muted.extend(wb.muted.iter().cloned());
        return Session {
            song: st.collab.stem.clone().unwrap_or_else(|| song.name.clone()),
            head: st.collab.head(),
            unsaved: st.dirty,
            playing: s.playing,
            mode: if wb.loop_part {
                "Loop this part".into()
            } else {
                "Whole song".into()
            },
            view: if edit.is_some() {
                "Workbench, editing".into()
            } else {
                "Workbench".into()
            },
            section: focus,
            position,
            loop_bars: None,
            intensity: (s.intensity * 100.0).round() / 100.0,
            track: edit.as_ref().map(|e| e.0.clone()).or(wb.solo.clone()),
            pattern: edit.as_ref().map(|e| e.1.clone()),
            notes: Vec::new(),
            soloed: wb.solo.iter().cloned().collect(),
            muted,
            auditioning: st.collab.audition.clone(),
            reference: None,
            reference_span: None,
            clips,
            listening: crate::workbench::describe(st),
            time: 0,
        };
    }
    Session {
        song: st.collab.stem.clone().unwrap_or_else(|| song.name.clone()),
        head: st.collab.head(),
        unsaved: st.dirty,
        playing: s.playing,
        mode: st.mode.name().to_string(),
        view: if st.detail_open {
            format!("{view}, {detail}")
        } else {
            view.to_string()
        },
        section: section.map(|x| x.name.clone()),
        position,
        loop_bars: if st.loop_on && st.loop_range.1 > st.loop_range.0 {
            Some((st.loop_range.0 / bar + 1, st.loop_range.1 / bar))
        } else {
            None
        },
        intensity: (s.intensity * 100.0).round() / 100.0,
        track: song.tracks.get(st.sel.track).map(|t| t.name.clone()),
        pattern: pattern.map(|p| p.name.clone()),
        notes,
        soloed: song
            .tracks
            .iter()
            .filter(|t| t.solo)
            .map(|t| t.name.clone())
            .collect(),
        muted: song
            .tracks
            .iter()
            .filter(|t| t.mute)
            .map(|t| t.name.clone())
            .collect(),
        auditioning: st.collab.audition.clone(),
        reference: st.reference.current_name(),
        reference_span: st.reference.span,
        clips: st
            .sel
            .clip
            .and_then(|(s, c)| clip_of(s, c))
            .into_iter()
            .collect(),
        listening: st.mode.name().to_lowercase(),
        time: 0,
    }
}

fn session(st: &mut Studio, now: f64) {
    if now - st.collab.last_session_write < 0.5 {
        return;
    }
    st.collab.last_session_write = now;
    let s = current_session(st);
    if st.collab.last_session.as_ref() == Some(&s) {
        return;
    }
    let mut out = s.clone();
    out.time = history::now();
    if let Some(desk) = &st.collab.desk {
        if desk.write_session(&out).is_ok() {
            st.collab.last_session = Some(s);
        }
    }
}

fn read_inbox(desk: &Desk) -> Vec<(PathBuf, Message)> {
    let mut v: Vec<(PathBuf, Message)> = std::fs::read_dir(desk.inbox_dir())
        .map(|r| {
            r.filter_map(|e| e.ok().map(|e| e.path()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.extension().is_some_and(|x| x == "ron"))
        .filter_map(|p| {
            let text = std::fs::read_to_string(&p).ok()?;
            let m: Message = ron::Options::default()
                .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
                .from_str(&text)
                .ok()?;
            Some((p, m))
        })
        .collect();
    v.sort_by_key(|(_, m)| std::cmp::Reverse(m.time));
    v.truncate(30);
    v
}

/// Sends the "Tell Claude" box.
pub fn post(st: &mut Studio) {
    let text = st.collab.tell.trim().to_string();
    if text.is_empty() && st.collab.sketch.is_none() {
        return;
    }
    let Some(desk) = &st.collab.desk else {
        st.say(
            "Save the song into a folder first: messages live beside it",
            Tone::Warn,
        );
        return;
    };
    let mut time = history::now();
    // One file per second: never overwrite a message sent a moment ago.
    while desk.inbox_dir().join(format!("m{time}.ron")).exists() {
        time += 1;
    }
    let mut session = current_session(st);
    session.time = time;
    let m = Message {
        time,
        text,
        from: "you".into(),
        reply_to: None,
        session,
        sketch: st.collab.sketch.clone(),
        read: false,
    };
    match desk.post(&m) {
        Ok(_) => {
            st.collab.tell.clear();
            st.collab.sketch = None;
            st.collab.last_inbox = -10.0;
            st.collab.tab = DockTab::Messages;
            st.say("Sent to Claude", Tone::Good);
        }
        Err(e) => st.say(format!("Could not post: {e}"), Tone::Bad),
    }
}

/// Records the saved song as a revision.
pub fn commit(st: &mut Studio) {
    let (Some(desk), Some(stem)) = (&st.collab.desk, &st.collab.stem) else {
        return;
    };
    let msg = st.collab.commit_msg.trim().to_string();
    match desk.commit(stem, &st.song, "you", &msg) {
        Ok(Some(e)) => {
            st.collab.commit_msg.clear();
            st.collab.last_poll = -10.0;
            st.say(format!("Saved as {}: {}", e.id, e.message), Tone::Good);
        }
        Ok(None) => {}
        Err(e) => st.say(
            format!("Saved, but the revision was not recorded: {e}"),
            Tone::Warn,
        ),
    }
}

pub fn accept(st: &mut Studio, id: &str) {
    let (Some(desk), Some(stem)) = (&st.collab.desk, st.collab.stem.clone()) else {
        return;
    };
    let reactions = st.collab.picked.get(id).cloned().unwrap_or_default();
    match desk.accept(&stem, id, &reactions, &st.collab.comment) {
        Ok(rev) => {
            st.collab.audition = None;
            st.collab.comment.clear();
            st.collab.last_poll = -10.0;
            st.reload();
            st.say(format!("Accepted {id} as {}", rev.id), Tone::Good);
        }
        Err(e) => st.say(format!("Accept failed: {e}"), Tone::Bad),
    }
}

pub fn reject(st: &mut Studio, id: &str) {
    let (Some(desk), Some(stem)) = (&st.collab.desk, st.collab.stem.clone()) else {
        return;
    };
    let reactions = st.collab.picked.get(id).cloned().unwrap_or_default();
    match desk.reject(&stem, id, &reactions, &st.collab.comment) {
        Ok(()) => {
            if st.collab.audition.as_deref() == Some(id) {
                st.collab.audition = None;
            }
            st.collab.comment.clear();
            st.collab.last_poll = -10.0;
            st.say(format!("Rejected {id}"), Tone::Info);
        }
        Err(e) => st.say(format!("Reject failed: {e}"), Tone::Bad),
    }
}

fn react(st: &mut Studio, id: &str) {
    let (Some(desk), Some(stem)) = (&st.collab.desk, st.collab.stem.clone()) else {
        return;
    };
    let reactions = st.collab.picked.get(id).cloned().unwrap_or_default();
    match desk.react(&stem, id, &reactions, &st.collab.comment) {
        Ok(()) => {
            st.collab.last_poll = -10.0;
            st.say(format!("Told Claude what you think of {id}"), Tone::Good);
        }
        Err(e) => st.say(format!("Could not send: {e}"), Tone::Bad),
    }
}

fn restore(st: &mut Studio, id: &str) {
    let (Some(desk), Some(stem)) = (&st.collab.desk, st.collab.stem.clone()) else {
        return;
    };
    match desk.revert(&stem, id, "you") {
        Ok(_) => {
            st.collab.audition = None;
            st.collab.last_poll = -10.0;
            st.reload();
            st.say(format!("Restored {id}"), Tone::Good);
        }
        Err(e) => st.say(format!("Restore failed: {e}"), Tone::Bad),
    }
}

// -- keys -------------------------------------------------------------------------

/// F1 hears the working copy, F2.. the waiting proposals in order, Tab flips
/// between the working copy and the last one heard, / focuses "Tell Claude".
pub fn keys(st: &mut Studio, ctx: &egui::Context) {
    let pressed = |k: egui::Key| ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, k));
    if pressed(egui::Key::Slash) {
        st.collab.open = true;
        st.collab.focus_tell = true;
    }
    if st.collab.desk.is_none() {
        return;
    }
    if pressed(egui::Key::F1) {
        st.collab.set_audition(None);
    }
    let pending: Vec<String> = st.collab.pending().iter().map(|e| e.id.clone()).collect();
    let fkeys = [
        egui::Key::F2,
        egui::Key::F3,
        egui::Key::F4,
        egui::Key::F5,
        egui::Key::F6,
        egui::Key::F7,
        egui::Key::F8,
    ];
    for (i, k) in fkeys.iter().enumerate() {
        if pressed(*k) {
            if let Some(id) = pending.get(i) {
                st.collab.set_audition(Some(id.clone()));
            }
        }
    }
    if pressed(egui::Key::Tab) {
        if st.collab.audition.is_some() {
            st.collab.set_audition(None);
        } else if let Some(last) = st
            .collab
            .last_audition
            .clone()
            .or_else(|| pending.first().cloned())
        {
            st.collab.set_audition(Some(last));
        }
    }
}

// -- the dock ---------------------------------------------------------------------

pub fn dock(ui: &mut egui::Ui, st: &mut Studio) {
    let now = ui.input(|i| i.time);
    ui.horizontal(|ui| {
        let n = st.collab.pending().len();
        let review = if n > 0 {
            format!("Review ({n})")
        } else {
            "Review".into()
        };
        for (t, name) in [
            (DockTab::Review, review.as_str()),
            (DockTab::History, "History"),
            (DockTab::Messages, "Messages"),
        ] {
            if crate::app::tab(ui, st.collab.tab == t, name).clicked() {
                st.collab.tab = t;
            }
        }
    });
    hearing(ui, st);
    ui.add_space(4.0);
    let tell_h = if st.sketch.take.is_some() {
        250.0
    } else {
        150.0
    };
    let body_h = (ui.available_height() - tell_h).max(80.0);
    egui::ScrollArea::vertical().id_salt("dock-body").max_height(body_h).auto_shrink([false, false]).show(ui, |ui| {
        if st.collab.desk.is_none() {
            ui.label(egui::RichText::new("Save the song first: its history and Claude's proposals live beside the file.").color(FAINT));
            return;
        }
        match st.collab.tab {
            DockTab::Review => review(ui, st, now),
            DockTab::History => history_tab(ui, st),
            DockTab::Messages => messages(ui, st),
        }
    });
    tell(ui, st);
}

/// Which version is sounding, always visible at the top of the dock.
fn hearing(ui: &mut egui::Ui, st: &mut Studio) {
    let (text, colour) = match &st.collab.audition {
        Some(id) => (format!("Hearing {}", st.collab.label(id)), WARN),
        None => ("Hearing your working copy".to_string(), DIM),
    };
    egui::Frame::new()
        .fill(if st.collab.audition.is_some() {
            theme::with_alpha(WARN, 28)
        } else {
            BG0
        })
        .stroke(Stroke::new(1.0, theme::with_alpha(colour, 90)))
        .corner_radius(CornerRadius::same(3))
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(text)
                        .font(theme::font_semi(13.5))
                        .color(colour),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if st.collab.audition.is_some()
                        && ui
                            .small_button("Back to mine")
                            .on_hover_text("F1")
                            .clicked()
                    {
                        st.collab.set_audition(None);
                    }
                });
            });
            ui.label(
                egui::RichText::new(
                    "F1 yours, F2.. proposals, Tab flips; the playhead keeps going",
                )
                .color(FAINT)
                .small(),
            );
        });
}

fn review(ui: &mut egui::Ui, st: &mut Studio, now: f64) {
    let pending: Vec<Entry> = st.collab.pending().into_iter().cloned().collect();
    if pending.is_empty() {
        ui.label(egui::RichText::new("No proposals waiting.").color(DIM));
        ui.label(
            egui::RichText::new("When Claude proposes a change (mc-music propose), it appears here to hear against yours before anything is written.")
                .color(FAINT)
                .small(),
        );
        return;
    }
    // Group alternatives together; a proposal without a group stands alone.
    let mut groups: Vec<(Option<String>, Vec<Entry>)> = Vec::new();
    for e in pending {
        match groups
            .iter_mut()
            .find(|(g, _)| g.is_some() && *g == e.group)
        {
            Some((_, v)) => v.push(e),
            None => groups.push((e.group.clone(), vec![e])),
        }
    }
    let fresh = now - st.collab.flash < 3.0;
    for (gi, (group, entries)) in groups.iter().enumerate() {
        let frame = egui::Frame::new()
            .fill(BG2)
            .stroke(Stroke::new(
                1.0,
                if fresh { ACCENT } else { theme::line(26) },
            ))
            .corner_radius(CornerRadius::same(4))
            .inner_margin(egui::Margin::same(8));
        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            if entries.len() > 1 {
                ui.label(
                    egui::RichText::new(format!(
                        "{} alternatives{}",
                        entries.len(),
                        group
                            .as_ref()
                            .map(|g| format!(" ({g})"))
                            .unwrap_or_default()
                    ))
                    .color(DIM)
                    .small(),
                );
            }
            let n = entries.len().min(4);
            ui.columns(n, |cols| {
                for (i, e) in entries.iter().take(n).enumerate() {
                    card(&mut cols[i], st, e, gi * 10 + i);
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Comment").color(FAINT).small());
                ui.add(
                    egui::TextEdit::singleline(&mut st.collab.comment)
                        .desired_width(f32::INFINITY)
                        .hint_text("Say what works and what does not"),
                );
            });
            if st.dirty {
                ui.label(
                    egui::RichText::new("Accepting replaces your unsaved edits.")
                        .color(WARN)
                        .small(),
                );
            }
        });
        ui.add_space(6.0);
    }
}

fn card(ui: &mut egui::Ui, st: &mut Studio, e: &Entry, salt: usize) {
    let hearing = st.collab.audition.as_deref() == Some(e.id.as_str());
    ui.push_id(salt, |ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                if let Some(l) = &e.label {
                    ui.label(
                        egui::RichText::new(l)
                            .font(theme::font_light(24.0))
                            .color(if hearing { WARN } else { TEXT }),
                    );
                }
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(&e.id)
                            .font(theme::font_semi(12.0))
                            .color(DIM),
                    );
                    ui.label(
                        egui::RichText::new(format!("{}, {}", e.author, ago(e.time)))
                            .color(FAINT)
                            .small(),
                    );
                });
            });
            ui.label(egui::RichText::new(&e.message).color(TEXT));
            for c in e.changes.iter().take(10) {
                ui.label(egui::RichText::new(format!("- {c}")).color(DIM).small());
            }
            if e.changes.len() > 10 {
                ui.label(
                    egui::RichText::new(format!("and {} more", e.changes.len() - 10))
                        .color(FAINT)
                        .small(),
                )
                .on_hover_text(e.changes[10..].join("\n"));
            }
            if !e.question.is_empty() {
                ui.label(egui::RichText::new(&e.question).color(ACCENT).italics());
            }
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                let hear = crate::widgets::toggle(
                    ui,
                    hearing,
                    if hearing { "Hearing" } else { "Hear" },
                    WARN,
                );
                st.collab.hear_at.insert(e.id.clone(), hear.rect);
                if hear.clicked() {
                    st.collab
                        .set_audition(if hearing { None } else { Some(e.id.clone()) });
                }
                if crate::widgets::toggle(ui, false, "Accept", GOOD).clicked() {
                    accept(st, &e.id);
                }
                if crate::widgets::toggle(ui, false, "Reject", theme::BAD).clicked() {
                    reject(st, &e.id);
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                for r in Reaction::ALL {
                    let on = st.collab.picked.get(&e.id).is_some_and(|v| v.contains(&r));
                    if crate::widgets::toggle(ui, on, r.name(), ACCENT).clicked() {
                        let v = st.collab.picked.entry(e.id.clone()).or_default();
                        if on {
                            v.retain(|x| *x != r);
                        } else {
                            v.push(r);
                        }
                    }
                }
                if ui
                    .small_button("Send")
                    .on_hover_text("Send the reactions and comment without deciding")
                    .clicked()
                {
                    react(st, &e.id);
                }
            });
        })
    });
}

fn status_colour(s: EntryStatus) -> egui::Color32 {
    match s {
        EntryStatus::Revision => DIM,
        EntryStatus::Proposed => WARN,
        EntryStatus::Accepted => GOOD,
        EntryStatus::Rejected => theme::BAD,
        EntryStatus::Superseded => FAINT,
    }
}

fn status_name(s: EntryStatus) -> &'static str {
    match s {
        EntryStatus::Revision => "revision",
        EntryStatus::Proposed => "waiting",
        EntryStatus::Accepted => "accepted",
        EntryStatus::Rejected => "rejected",
        EntryStatus::Superseded => "superseded",
    }
}

fn history_tab(ui: &mut egui::Ui, st: &mut Studio) {
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut st.collab.commit_msg)
                .desired_width(ui.available_width() - 70.0)
                .hint_text("Message for the next save (optional)"),
        );
        if ui
            .button("Save")
            .on_hover_text("Save and record a revision (Ctrl+S)")
            .clicked()
        {
            st.save();
        }
    });
    ui.add_space(4.0);
    let entries: Vec<Entry> = st.collab.log.entries.iter().rev().cloned().collect();
    if entries.is_empty() {
        ui.label(
            egui::RichText::new("No history yet. Saving records the first revision.").color(FAINT),
        );
    }
    let head = st.collab.head();
    for e in entries {
        let open = st.collab.expanded.as_deref() == Some(e.id.as_str());
        let hearing = st.collab.audition.as_deref() == Some(e.id.as_str());
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 36.0), egui::Sense::click());
        let p = ui.painter();
        if hearing {
            p.rect_filled(rect, CornerRadius::same(2), theme::with_alpha(WARN, 30));
        } else if open || resp.hovered() {
            p.rect_filled(rect, CornerRadius::same(2), theme::line(8));
        }
        let c = status_colour(e.status);
        p.circle_filled(rect.left_center() + vec2(7.0, 0.0), 3.5, c);
        let is_head = head.as_deref() == Some(e.id.as_str());
        p.text(
            rect.left_top() + vec2(16.0, 3.0),
            egui::Align2::LEFT_TOP,
            format!(
                "{}{}  {}  {}",
                e.id,
                if is_head { " (head)" } else { "" },
                status_name(e.status),
                e.author
            ),
            theme::font_semi(12.0),
            c,
        );
        p.text(
            rect.right_top() + vec2(-4.0, 3.0),
            egui::Align2::RIGHT_TOP,
            ago(e.time),
            theme::font_body(10.5),
            FAINT,
        );
        p.text(
            rect.left_top() + vec2(16.0, 19.0),
            egui::Align2::LEFT_TOP,
            crate::transport::truncate(&e.message, ((w - 30.0) / 6.0) as usize),
            theme::font_body(12.0),
            TEXT,
        );
        if resp.clicked() {
            st.collab.expanded = if open { None } else { Some(e.id.clone()) };
        }
        if open {
            egui::Frame::new()
                .inner_margin(egui::Margin {
                    left: 16,
                    right: 4,
                    top: 2,
                    bottom: 6,
                })
                .show(ui, |ui| {
                    for c in &e.changes {
                        ui.label(egui::RichText::new(format!("- {c}")).color(DIM).small());
                    }
                    if !e.reactions.is_empty() {
                        let r: Vec<&str> = e.reactions.iter().map(|r| r.name()).collect();
                        ui.label(egui::RichText::new(r.join(", ")).color(ACCENT).small());
                    }
                    if !e.comment.is_empty() {
                        ui.label(
                            egui::RichText::new(format!("\u{201C}{}\u{201D}", e.comment))
                                .color(DIM)
                                .italics(),
                        );
                    }
                    if let Some(p) = &e.from_proposal {
                        ui.label(
                            egui::RichText::new(format!("from proposal {p}"))
                                .color(FAINT)
                                .small(),
                        );
                    }
                    ui.horizontal(|ui| {
                        if crate::widgets::toggle(
                            ui,
                            hearing,
                            if hearing { "Hearing" } else { "Hear" },
                            WARN,
                        )
                        .clicked()
                        {
                            st.collab
                                .set_audition(if hearing { None } else { Some(e.id.clone()) });
                        }
                        if matches!(
                            e.status,
                            EntryStatus::Revision
                                | EntryStatus::Accepted
                                | EntryStatus::Rejected
                                | EntryStatus::Superseded
                        ) && ui
                            .small_button("Restore")
                            .on_hover_text(
                                "Make this the working copy (recorded as a new revision)",
                            )
                            .clicked()
                        {
                            restore(st, &e.id);
                        }
                        if e.status == EntryStatus::Proposed {
                            if ui.small_button("Accept").clicked() {
                                accept(st, &e.id);
                            }
                            if ui.small_button("Reject").clicked() {
                                reject(st, &e.id);
                            }
                        }
                    });
                });
        }
    }
}

fn messages(ui: &mut egui::Ui, st: &mut Studio) {
    if st.collab.inbox.is_empty() {
        ui.label(
            egui::RichText::new("Nothing sent yet. Type below, or press / anywhere.").color(FAINT),
        );
    }
    for (_, m) in &st.collab.inbox {
        egui::Frame::new()
            .fill(BG2)
            .corner_radius(CornerRadius::same(3))
            .inner_margin(egui::Margin::symmetric(8, 5))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(ago(m.time)).color(FAINT).small());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if m.read {
                            ui.label(egui::RichText::new("Read").color(GOOD).small());
                        } else {
                            ui.label(egui::RichText::new("Not read yet").color(WARN).small());
                        }
                    });
                });
                ui.label(egui::RichText::new(&m.text).color(TEXT));
                let s = &m.session;
                let mut ctx = vec![s.position.clone()];
                if let Some(p) = &s.pattern {
                    ctx.push(format!("pattern {p}"));
                }
                if !s.notes.is_empty() {
                    ctx.push(format!("{} notes selected", s.notes.len()));
                }
                if let Some(sk) = &m.sketch {
                    ctx.push(format!("sketch of {} notes", sk.notes.len()));
                }
                ui.label(egui::RichText::new(ctx.join(", ")).color(FAINT).small());
            });
        ui.add_space(4.0);
    }
}

/// The "Tell Claude" box at the bottom of the dock.
fn tell(ui: &mut egui::Ui, st: &mut Studio) {
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Tell Claude")
                .font(theme::font_semi(13.5))
                .color(TEXT),
        );
        ui.label(
            egui::RichText::new("/ to type, Enter sends")
                .color(FAINT)
                .small(),
        );
    });
    let r = ui.add(
        egui::TextEdit::multiline(&mut st.collab.tell)
            .desired_rows(2)
            .desired_width(f32::INFINITY)
            .hint_text("\"This part\" means what the studio shows: the playhead, selection, loop"),
    );
    if st.collab.focus_tell {
        st.collab.focus_tell = false;
        r.request_focus();
    }
    // Enter sends; Shift+Enter makes a new line.
    let send_key =
        r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
    if send_key {
        while st.collab.tell.ends_with('\n') {
            st.collab.tell.pop();
        }
    }
    crate::sketch::controls(ui, st);
    ui.horizontal(|ui| {
        if let Some(sk) = &st.collab.sketch {
            ui.label(
                egui::RichText::new(format!("Sketch attached ({} notes)", sk.notes.len()))
                    .color(ACCENT)
                    .small(),
            );
            if ui.small_button("Drop").clicked() {
                st.collab.sketch = None;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Send").clicked() || send_key {
                post(st);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_music::Note;

    #[test]
    fn changed_parts_follow_patterns_to_tracks() {
        let a = crate::songops::starter_song("t");
        let mut b = a.clone();
        b.patterns[0].notes.push(Note(0, 24, 40, 100));
        b.tracks[1].db -= 3.0;
        let (tracks, patterns) = changed_parts(&a, &b);
        assert!(patterns.contains("Beat"));
        assert!(tracks.contains(&a.tracks[0].name), "the kit plays Beat");
        assert!(
            tracks.contains(&a.tracks[1].name),
            "the synth's level moved"
        );
        let (t2, p2) = changed_parts(&a, &a);
        assert!(t2.is_empty() && p2.is_empty());
    }
}
