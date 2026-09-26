//! The studio's shared desk: song history, Claude's proposals, what the studio
//! is showing right now, and messages from the studio to Claude.
//!
//! Everything lives in `data/music/.studio/` as small RON files so the studio
//! (mc-studio) and Claude (through the `mc-music` CLI, from a chat) can work on
//! one song at once without either stepping on the other:
//!
//! ```text
//! .studio/
//!   session.ron                  what the studio shows now (song, selection, playhead...)
//!   inbox/m<time>.ron            "Tell Claude" messages, each with the session at that moment
//!   <song>/log.ron               revisions and proposals, newest last
//!   <song>/r0007.ron             the song as it was at revision 7
//!   <song>/p0012.ron             a proposal (Claude's version of the song)
//!   taste.ron                    every verdict and reaction, for Claude to learn from
//! ```
//!
//! The working copy is still `data/music/<song>.ron`: the game plays it, the
//! studio edits it. A revision is a snapshot of it; accepting a proposal writes
//! the proposal over it and records a revision. Proposals are never written to
//! the working copy until they are accepted, so the user always hears one next
//! to the other before choosing.

use crate::song::{Pattern, Song};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    /// A saved state of the song.
    Revision,
    /// Waiting for the user.
    Proposed,
    Accepted,
    Rejected,
    /// Another proposal of its group was accepted instead.
    Superseded,
}

/// How a proposal landed, beyond yes or no: the quick reactions in the studio.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reaction {
    Closer,
    Further,
    TooMuch,
    TooLittle,
    RightIdeaWrongSound,
    Love,
}

impl Reaction {
    pub const ALL: [Reaction; 6] = [
        Reaction::Closer,
        Reaction::Further,
        Reaction::TooMuch,
        Reaction::TooLittle,
        Reaction::RightIdeaWrongSound,
        Reaction::Love,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Reaction::Closer => "Closer",
            Reaction::Further => "Further",
            Reaction::TooMuch => "Too much",
            Reaction::TooLittle => "Too little",
            Reaction::RightIdeaWrongSound => "Right idea, wrong sound",
            Reaction::Love => "Love it",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// "r0007" for a revision, "p0012" for a proposal.
    pub id: String,
    pub status: Status,
    /// "you" or "claude".
    pub author: String,
    /// Seconds since the Unix epoch.
    pub time: u64,
    pub message: String,
    /// The revision it was made from.
    #[serde(default)]
    pub base: Option<String>,
    /// Proposals offered together as alternatives share a group; accepting one supersedes the rest.
    #[serde(default)]
    pub group: Option<String>,
    /// "A", "B"... within a group, or a short name for the variant.
    #[serde(default)]
    pub label: Option<String>,
    /// What changed against the base, one line each (`diff::summarise`).
    #[serde(default)]
    pub changes: Vec<String>,
    /// For a revision made by accepting a proposal: which one.
    #[serde(default)]
    pub from_proposal: Option<String>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    /// What the user said about it.
    #[serde(default)]
    pub comment: String,
    /// A question Claude asks with the proposal ("is the horn too high now?").
    #[serde(default)]
    pub question: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Log {
    pub entries: Vec<Entry>,
}

/// What the studio is showing, rewritten whenever it changes. "This" in a chat
/// message means whatever this says.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub song: String,
    /// The revision the working copy was last saved as.
    #[serde(default)]
    pub head: Option<String>,
    /// There are edits in the studio not yet saved (autosaved to `.studio/<song>/working.ron`).
    #[serde(default)]
    pub unsaved: bool,
    pub playing: bool,
    /// "Song", "Section", "Pattern" or "Director".
    pub mode: String,
    pub view: String,
    #[serde(default)]
    pub section: Option<String>,
    /// Playhead as bar.beat within the section (1-based), and in the arrangement.
    #[serde(default)]
    pub position: String,
    #[serde(default)]
    pub loop_bars: Option<(u32, u32)>,
    pub intensity: f32,
    #[serde(default)]
    pub track: Option<String>,
    #[serde(default)]
    pub pattern: Option<String>,
    /// Selected notes of `pattern` as (tick, len, key, vel).
    #[serde(default)]
    pub notes: Vec<(u32, u32, u8, u8)>,
    #[serde(default)]
    pub soloed: Vec<String>,
    #[serde(default)]
    pub muted: Vec<String>,
    /// A proposal being auditioned (A/B) right now.
    #[serde(default)]
    pub auditioning: Option<String>,
    /// A reference being compared, and the span of it marked, in seconds.
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub reference_span: Option<(f32, f32)>,
    /// Clips selected in the arrangement, as (section, track, pattern).
    #[serde(default)]
    pub clips: Vec<(String, String, String)>,
    /// What the studio is playing, in words: "whole song", "section battle_a (loop)",
    /// "selection: 3 clips in battle_a".
    #[serde(default)]
    pub listening: String,
    pub time: u64,
}

/// A message at the desk: from the studio's "Tell Claude" box, or Claude's reply.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub time: u64,
    pub text: String,
    /// "you" (the studio) or "claude".
    #[serde(default = "you")]
    pub from: String,
    /// For a reply: the time of the message it answers.
    #[serde(default)]
    pub reply_to: Option<u64>,
    pub session: Session,
    /// Notes hummed, sung, tapped or played to go with it (ticks at the song's tempo).
    #[serde(default)]
    pub sketch: Option<Pattern>,
    #[serde(default)]
    pub read: bool,
}

/// One verdict, kept so taste accumulates across sessions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    pub time: u64,
    pub song: String,
    pub proposal: String,
    pub message: String,
    pub changes: Vec<String>,
    pub status: Status,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    #[serde(default)]
    pub comment: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Taste {
    pub verdicts: Vec<Verdict>,
}

fn you() -> String {
    "you".into()
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn read_ron<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(&text)
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn write_ron<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let config = ron::ser::PrettyConfig::new()
        .depth_limit(5)
        .indentor("    ".to_string())
        .extensions(ron::extensions::Extensions::IMPLICIT_SOME);
    let text = ron::ser::to_string_pretty(value, config).map_err(|e| e.to_string())?;
    // Write then rename, so a reader never sees half a file.
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, text + "\n").map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The desk for one music directory.
pub struct Desk {
    /// `data/music`.
    pub music: PathBuf,
    /// `data/music/.studio`.
    pub root: PathBuf,
}

impl Desk {
    pub fn open(music: &Path) -> Desk {
        Desk { music: music.to_path_buf(), root: music.join(".studio") }
    }

    pub fn song_path(&self, song: &str) -> PathBuf {
        self.music.join(format!("{song}.ron"))
    }
    fn dir(&self, song: &str) -> PathBuf {
        self.root.join(song)
    }
    pub fn entry_path(&self, song: &str, id: &str) -> PathBuf {
        self.dir(song).join(format!("{id}.ron"))
    }
    pub fn working_path(&self, song: &str) -> PathBuf {
        self.dir(song).join("working.ron")
    }
    pub fn session_path(&self) -> PathBuf {
        self.root.join("session.ron")
    }
    pub fn inbox_dir(&self) -> PathBuf {
        self.root.join("inbox")
    }
    pub fn taste_path(&self) -> PathBuf {
        self.root.join("taste.ron")
    }

    pub fn log(&self, song: &str) -> Log {
        read_ron(&self.dir(song).join("log.ron")).unwrap_or_default()
    }
    fn save_log(&self, song: &str, log: &Log) -> Result<(), String> {
        write_ron(&self.dir(song).join("log.ron"), log)
    }

    pub fn load(&self, song: &str, id: &str) -> Result<Song, String> {
        Song::load(&self.entry_path(song, id))
    }

    /// The last revision (not proposal).
    pub fn head(&self, song: &str) -> Option<String> {
        self.log(song).entries.iter().rev().find(|e| e.status == Status::Revision).map(|e| e.id.clone())
    }

    fn next_id(log: &Log, prefix: char) -> String {
        let n = log
            .entries
            .iter()
            .filter(|e| e.id.starts_with(prefix))
            .filter_map(|e| e.id[1..].parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        format!("{prefix}{:04}", n + 1)
    }

    /// Records `current` as a revision. Nothing is recorded when it equals the head.
    pub fn commit(&self, song: &str, current: &Song, author: &str, message: &str) -> Result<Option<Entry>, String> {
        let mut log = self.log(song);
        let head = log.entries.iter().rev().find(|e| e.status == Status::Revision).cloned();
        let base = match &head {
            Some(h) => self.load(song, &h.id).ok(),
            None => None,
        };
        if base.as_ref() == Some(current) {
            return Ok(None);
        }
        let changes = match &base {
            Some(b) => summarise(b, current),
            None => vec!["first revision".into()],
        };
        let id = Self::next_id(&log, 'r');
        current.save(&self.entry_path(song, &id))?;
        let e = Entry {
            id,
            status: Status::Revision,
            author: author.into(),
            time: now(),
            message: if message.is_empty() { changes.first().cloned().unwrap_or_default() } else { message.into() },
            base: head.map(|h| h.id),
            group: None,
            label: None,
            changes,
            from_proposal: None,
            reactions: Vec::new(),
            comment: String::new(),
            question: String::new(),
        };
        log.entries.push(e.clone());
        self.save_log(song, &log)?;
        Ok(Some(e))
    }

    /// Records the working copy as a revision if it differs from the head (so a
    /// proposal always has a revision to be compared against).
    pub fn snapshot_working(&self, song: &str, author: &str, message: &str) -> Result<Option<String>, String> {
        let current = Song::load(&self.song_path(song))?;
        self.commit(song, &current, author, message)?;
        Ok(self.head(song))
    }

    /// Offers `proposed` as a change to the song. It is not applied.
    pub fn propose(
        &self,
        song: &str,
        proposed: &Song,
        message: &str,
        group: Option<&str>,
        label: Option<&str>,
        question: &str,
    ) -> Result<Entry, String> {
        let base_id = self.snapshot_working(song, "you", "")?;
        let base = Song::load(&self.song_path(song))?;
        let mut log = self.log(song);
        let id = Self::next_id(&log, 'p');
        proposed.save(&self.entry_path(song, &id))?;
        let e = Entry {
            id,
            status: Status::Proposed,
            author: "claude".into(),
            time: now(),
            message: message.into(),
            base: base_id,
            group: group.map(String::from),
            label: label.map(String::from),
            changes: summarise(&base, proposed),
            from_proposal: None,
            reactions: Vec::new(),
            comment: String::new(),
            question: question.into(),
        };
        log.entries.push(e.clone());
        self.save_log(song, &log)?;
        Ok(e)
    }

    pub fn pending(&self, song: &str) -> Vec<Entry> {
        self.log(song).entries.into_iter().filter(|e| e.status == Status::Proposed).collect()
    }

    /// Makes a proposal the working copy and records it as a revision; others in its group are superseded.
    pub fn accept(&self, song: &str, id: &str, reactions: &[Reaction], comment: &str) -> Result<Entry, String> {
        let proposed = self.load(song, id)?;
        proposed.save(&self.song_path(song))?;
        let mut log = self.log(song);
        let group = log.entries.iter().find(|e| e.id == id).and_then(|e| e.group.clone());
        let mut verdicts = Vec::new();
        for e in log.entries.iter_mut() {
            if e.id == id {
                e.status = Status::Accepted;
                e.reactions = reactions.to_vec();
                e.comment = comment.into();
                verdicts.push(e.clone());
            } else if e.status == Status::Proposed && group.is_some() && e.group == group {
                e.status = Status::Superseded;
                verdicts.push(e.clone());
            }
        }
        let message = log.entries.iter().find(|e| e.id == id).map(|e| e.message.clone()).unwrap_or_default();
        self.save_log(song, &log)?;
        self.record(song, &verdicts);
        let rev = self.commit(song, &proposed, "claude", &message)?;
        let mut log = self.log(song);
        let rev_id = rev.as_ref().map(|r| r.id.clone());
        if let Some(r) = log.entries.iter_mut().rev().find(|e| Some(&e.id) == rev_id.as_ref()) {
            r.from_proposal = Some(id.into());
        }
        self.save_log(song, &log)?;
        Ok(rev.unwrap_or_else(|| log.entries.last().cloned().expect("a revision exists")))
    }

    pub fn reject(&self, song: &str, id: &str, reactions: &[Reaction], comment: &str) -> Result<(), String> {
        let mut log = self.log(song);
        let mut verdicts = Vec::new();
        for e in log.entries.iter_mut().filter(|e| e.id == id) {
            e.status = Status::Rejected;
            e.reactions = reactions.to_vec();
            e.comment = comment.into();
            verdicts.push(e.clone());
        }
        self.save_log(song, &log)?;
        self.record(song, &verdicts);
        Ok(())
    }

    /// Adds reactions or a comment to an entry without deciding it (auditioning a proposal).
    pub fn react(&self, song: &str, id: &str, reactions: &[Reaction], comment: &str) -> Result<(), String> {
        let mut log = self.log(song);
        for e in log.entries.iter_mut().filter(|e| e.id == id) {
            for r in reactions {
                if !e.reactions.contains(r) {
                    e.reactions.push(*r);
                }
            }
            if !comment.is_empty() {
                e.comment = comment.into();
            }
        }
        self.save_log(song, &log)
    }

    /// Puts an old revision back as the working copy (and records that as a new revision).
    pub fn revert(&self, song: &str, id: &str, author: &str) -> Result<Option<Entry>, String> {
        let old = self.load(song, id)?;
        old.save(&self.song_path(song))?;
        self.commit(song, &old, author, &format!("Back to {id}"))
    }

    fn record(&self, song: &str, entries: &[Entry]) {
        let mut taste: Taste = read_ron(&self.taste_path()).unwrap_or_default();
        for e in entries {
            taste.verdicts.push(Verdict {
                time: now(),
                song: song.into(),
                proposal: e.id.clone(),
                message: e.message.clone(),
                changes: e.changes.clone(),
                status: e.status,
                reactions: e.reactions.clone(),
                comment: e.comment.clone(),
            });
        }
        let _ = write_ron(&self.taste_path(), &taste);
    }

    pub fn taste(&self) -> Taste {
        read_ron(&self.taste_path()).unwrap_or_default()
    }

    pub fn write_session(&self, s: &Session) -> Result<(), String> {
        write_ron(&self.session_path(), s)
    }
    pub fn session(&self) -> Option<Session> {
        read_ron(&self.session_path()).ok()
    }

    pub fn post(&self, m: &Message) -> Result<PathBuf, String> {
        let path = self.inbox_dir().join(format!("m{}.ron", m.time));
        write_ron(&path, m)?;
        Ok(path)
    }

    /// Claude's answer, shown in the studio's conversation. `session` is left as it was
    /// when the message it answers was sent, so the studio can point at the same place.
    pub fn reply(&self, text: &str, reply_to: Option<u64>) -> Result<PathBuf, String> {
        let session = self.session().unwrap_or_default();
        let mut time = now();
        // Two messages in one second would share a file name.
        while self.inbox_dir().join(format!("m{time}.ron")).exists() {
            time += 1;
        }
        let m = Message { time, text: text.into(), from: "claude".into(), reply_to, session, sketch: None, read: false };
        self.post(&m)
    }

    /// Every message, both ways, oldest first: the conversation.
    pub fn conversation(&self) -> Vec<Message> {
        let mut out: Vec<Message> = std::fs::read_dir(self.inbox_dir())
            .map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.extension().is_some_and(|x| x == "ron"))
            .filter_map(|p| read_ron::<Message>(&p).ok())
            .collect();
        out.sort_by_key(|m| m.time);
        out
    }

    /// Unread messages from the studio (not Claude's own replies), oldest first.
    pub fn unread(&self) -> Vec<(PathBuf, Message)> {
        let mut out: Vec<(PathBuf, Message)> = std::fs::read_dir(self.inbox_dir())
            .map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.extension().is_some_and(|x| x == "ron"))
            .filter_map(|p| read_ron::<Message>(&p).ok().map(|m| (p, m)))
            .filter(|(_, m)| !m.read && m.from != "claude")
            .collect();
        out.sort_by_key(|(_, m)| m.time);
        out
    }

    pub fn mark_read(&self, path: &Path) -> Result<(), String> {
        let mut m: Message = read_ron(path)?;
        m.read = true;
        write_ron(path, &m)
    }
}

/// What changed from `a` to `b`, in words, most important first.
pub fn summarise(a: &Song, b: &Song) -> Vec<String> {
    let mut out = Vec::new();
    if a.tempo != b.tempo {
        out.push(format!("tempo {} -> {}", a.tempo, b.tempo));
    }
    if (a.root, a.scale) != (b.root, b.scale) {
        out.push(format!(
            "key {} {} -> {} {}",
            crate::song::NOTE_NAMES[a.root as usize % 12],
            a.scale.name(),
            crate::song::NOTE_NAMES[b.root as usize % 12],
            b.scale.name()
        ));
    }
    for t in &b.tracks {
        match a.tracks.iter().find(|x| x.name == t.name) {
            None => out.push(format!("new track {} ({})", t.name, t.instrument.kind_name())),
            Some(old) => {
                if old.db != t.db {
                    out.push(format!("{}: level {:+.1} dB ({:.1} -> {:.1})", t.name, t.db - old.db, old.db, t.db));
                }
                if old.pan != t.pan {
                    out.push(format!("{}: pan {:.2} -> {:.2}", t.name, old.pan, t.pan));
                }
                if old.mute != t.mute {
                    out.push(format!("{}: {}", t.name, if t.mute { "muted" } else { "unmuted" }));
                }
                if old.layer != t.layer {
                    out.push(format!(
                        "{}: layer {:.2}/{:.2}/{:.2} -> {:.2}/{:.2}/{:.2}",
                        t.name, old.layer.from, old.layer.full, old.layer.until, t.layer.from, t.layer.full, t.layer.until
                    ));
                }
                if old.sends != t.sends {
                    out.push(format!("{}: sends changed", t.name));
                }
                if old.follow != t.follow {
                    out.push(format!("{}: intensity follow changed", t.name));
                }
                if old.effects != t.effects {
                    let names = |fx: &[crate::patch::Effect]| fx.iter().map(|e| e.name()).collect::<Vec<_>>().join(", ");
                    if names(&old.effects) != names(&t.effects) {
                        out.push(format!("{}: effects [{}] -> [{}]", t.name, names(&old.effects), names(&t.effects)));
                    } else {
                        for line in field_changes(&old.effects, &t.effects) {
                            out.push(format!("{}: effect {line}", t.name));
                        }
                    }
                }
                if old.instrument != t.instrument {
                    for line in field_changes(&old.instrument, &t.instrument) {
                        out.push(format!("{}: {line}", t.name));
                    }
                }
            }
        }
    }
    for t in &a.tracks {
        if !b.tracks.iter().any(|x| x.name == t.name) {
            out.push(format!("removed track {}", t.name));
        }
    }
    for p in &b.patterns {
        match a.patterns.iter().find(|x| x.name == p.name) {
            None => out.push(format!("new pattern {} ({} notes, {} beats)", p.name, p.notes.len(), p.beats)),
            Some(old) if old != p => {
                let added = p.notes.iter().filter(|n| !old.notes.contains(n)).count();
                let removed = old.notes.iter().filter(|n| !p.notes.contains(n)).count();
                let mut parts = Vec::new();
                if old.beats != p.beats {
                    parts.push(format!("length {} -> {} beats", old.beats, p.beats));
                }
                let changed = added.min(removed);
                if changed > 0 {
                    parts.push(format!("{changed} notes changed"));
                }
                if added > changed {
                    parts.push(format!("{} notes added", added - changed));
                }
                if removed > changed {
                    parts.push(format!("{} notes removed", removed - changed));
                }
                if old.automation != p.automation {
                    parts.push("automation changed".into());
                }
                out.push(format!("pattern {}: {}", p.name, parts.join(", ")));
            }
            _ => {}
        }
    }
    for p in &a.patterns {
        if !b.patterns.iter().any(|x| x.name == p.name) {
            out.push(format!("removed pattern {}", p.name));
        }
    }
    for s in &b.sections {
        match a.sections.iter().find(|x| x.name == s.name) {
            None => out.push(format!("new section {} ({} bars)", s.name, s.bars)),
            Some(old) if old != s => {
                let mut parts = Vec::new();
                if old.bars != s.bars {
                    parts.push(format!("{} -> {} bars", old.bars, s.bars));
                }
                if old.intensity != s.intensity {
                    parts.push(format!(
                        "intensity {:.2}-{:.2} -> {:.2}-{:.2}",
                        old.intensity.0, old.intensity.1, s.intensity.0, s.intensity.1
                    ));
                }
                if old.clips != s.clips {
                    parts.push("clips changed".into());
                }
                if old.next != s.next || old.kind != s.kind || old.exit_every != s.exit_every {
                    parts.push("flow changed".into());
                }
                out.push(format!("section {}: {}", s.name, parts.join(", ")));
            }
            _ => {}
        }
    }
    for s in &a.sections {
        if !b.sections.iter().any(|x| x.name == s.name) {
            out.push(format!("removed section {}", s.name));
        }
    }
    if a.arrangement != b.arrangement {
        out.push("arrangement changed".into());
    }
    if a.buses != b.buses {
        out.push("buses changed".into());
    }
    if a.master != b.master {
        out.push("master changed".into());
    }
    if out.is_empty() && a != b {
        out.push("small changes".into());
    }
    out
}

/// Lines of two values' RON that differ, as "field: old -> new".
fn field_changes<T: Serialize>(a: &T, b: &T) -> Vec<String> {
    let flat = |v: &T| -> Vec<String> {
        let config = ron::ser::PrettyConfig::new().depth_limit(8).indentor(" ".to_string());
        let text = ron::ser::to_string_pretty(v, config).unwrap_or_default();
        text.lines().map(|l| l.trim().trim_end_matches(',').to_string()).collect()
    };
    let (la, lb) = (flat(a), flat(b));
    let mut out = Vec::new();
    if la.len() == lb.len() {
        for (x, y) in la.iter().zip(&lb) {
            if x != y {
                let key = y.split(':').next().unwrap_or("").trim();
                let old = x.split_once(':').map(|v| v.1.trim()).unwrap_or(x);
                let new = y.split_once(':').map(|v| v.1.trim()).unwrap_or(y);
                out.push(format!("{key} {old} -> {new}"));
            }
        }
    } else {
        out.push("structure changed".into());
    }
    out.truncate(8);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("mc-music-desk-{}-{}", std::process::id(), now()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_proposal_is_heard_before_it_lands_and_accepting_supersedes_its_group() {
        let dir = tmp();
        let desk = Desk::open(&dir);
        let mut song = Song::empty("t");
        song.patterns.push(Pattern { name: "p".into(), beats: 4, notes: vec![], automation: vec![] });
        song.save(&desk.song_path("t")).unwrap();

        let mut a = song.clone();
        a.tempo = 130.0;
        let mut b = song.clone();
        b.tempo = 90.0;
        let pa = desk.propose("t", &a, "faster", Some("g1"), Some("A"), "").unwrap();
        let pb = desk.propose("t", &b, "slower", Some("g1"), Some("B"), "").unwrap();
        assert_eq!(desk.pending("t").len(), 2);
        // Proposing never touches the working copy.
        assert_eq!(Song::load(&desk.song_path("t")).unwrap().tempo, 110.0);

        desk.accept("t", &pb.id, &[Reaction::Closer], "better").unwrap();
        assert_eq!(Song::load(&desk.song_path("t")).unwrap().tempo, 90.0);
        let log = desk.log("t");
        assert_eq!(log.entries.iter().find(|e| e.id == pa.id).unwrap().status, Status::Superseded);
        assert!(desk.pending("t").is_empty());
        assert_eq!(desk.taste().verdicts.len(), 2);

        // Back to the first revision.
        let first = log.entries.iter().find(|e| e.status == Status::Revision).unwrap().id.clone();
        desk.revert("t", &first, "you").unwrap();
        assert_eq!(Song::load(&desk.song_path("t")).unwrap().tempo, 110.0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn summaries_name_what_changed() {
        let a = Song::empty("x");
        let mut b = a.clone();
        b.tempo = 128.0;
        b.patterns.push(Pattern { name: "bass".into(), beats: 4, notes: vec![crate::Note(0, 96, 36, 100)], automation: vec![] });
        let s = summarise(&a, &b);
        assert!(s.iter().any(|l| l.contains("tempo")));
        assert!(s.iter().any(|l| l.contains("new pattern bass")));
    }
}
