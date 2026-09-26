//! The studio's state and frame loop.
//!
//! The working copy of the song lives here, and every view edits it directly.
//! Once a frame the app compares it with the copy the engine last received;
//! when they differ it sends the whole song again (the engine keeps voices and
//! tails by name) and lets the history fold the change into an undo step.
//! Selection, zoom and scroll belong to the views and never enter the `Song`.

use crate::audio::Audio;
use crate::history::History;
use crate::keys::{KeyEvent, KeyPiano};
use crate::midi::Midi;
use crate::theme::{self, ACCENT, BAD, DIM, FAINT, TEXT, WARN};
use crate::{
    arrange, browser, collab, director, drums, export, files, instrument, mixer, piano, reference,
    sketch, transport,
};
use eframe::egui::{self, Key, Modifiers};
use mc_music::{Command, Instrument, Meters, Mode, Song, Status};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Arrange,
    Mixer,
    Director,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Detail {
    Piano,
    Drums,
    Instrument,
    Effect,
    Reference,
}

/// Which pane last took a click: keyboard shortcuts go there.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pane {
    Arrange,
    Detail,
    Other,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayMode {
    Song,
    Section,
    Pattern,
    Director,
}

impl PlayMode {
    pub const ALL: [PlayMode; 4] = [
        PlayMode::Song,
        PlayMode::Section,
        PlayMode::Pattern,
        PlayMode::Director,
    ];
    pub fn name(self) -> &'static str {
        match self {
            PlayMode::Song => "Song",
            PlayMode::Section => "Section",
            PlayMode::Pattern => "Pattern",
            PlayMode::Director => "Director",
        }
    }
}

/// A mixer strip: its insert chain lives on a track, a bus or the master.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Strip {
    Track(usize),
    Bus(usize),
    Master,
}

/// What fills the window.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    /// The library: songs, moments and audio to pick from.
    Picker,
    Song,
    Audio,
}

pub fn chain_mut(song: &mut Song, strip: Strip) -> Option<&mut Vec<mc_music::Effect>> {
    match strip {
        Strip::Track(i) => song.tracks.get_mut(i).map(|t| &mut t.effects),
        Strip::Bus(i) => song.buses.get_mut(i).map(|b| &mut b.effects),
        Strip::Master => Some(&mut song.master.effects),
    }
}

pub fn chain(song: &Song, strip: Strip) -> Option<&Vec<mc_music::Effect>> {
    match strip {
        Strip::Track(i) => song.tracks.get(i).map(|t| &t.effects),
        Strip::Bus(i) => song.buses.get(i).map(|b| &b.effects),
        Strip::Master => Some(&song.master.effects),
    }
}

pub fn strip_name(song: &Song, strip: Strip) -> String {
    match strip {
        Strip::Track(i) => song
            .tracks
            .get(i)
            .map(|t| t.name.clone())
            .unwrap_or_default(),
        Strip::Bus(i) => song
            .buses
            .get(i)
            .map(|b| b.name.clone())
            .unwrap_or_default(),
        Strip::Master => "Master".into(),
    }
}

#[derive(Clone, Debug, Default)]
pub struct Selection {
    pub track: usize,
    pub section: Option<usize>,
    /// Arrangement entry (index into `song.arrangement`).
    pub entry: Option<usize>,
    /// (section, clip index).
    pub clip: Option<(usize, usize)>,
    /// The pattern open in the piano roll / drum grid.
    pub pattern: Option<usize>,
    /// The track whose instrument plays that pattern.
    pub pattern_track: usize,
    pub fx: Option<(Strip, usize)>,
    /// Drum selected in the kit editor.
    pub drum: usize,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tone {
    Info,
    Good,
    Warn,
    Bad,
}

/// A scripted intensity move for hearing the director's transitions.
#[derive(Clone, Debug)]
pub struct Ramp {
    pub points: Vec<(f64, f32)>,
    pub started: f64,
}

impl Ramp {
    pub fn value(&self, now: f64) -> Option<f32> {
        let t = now - self.started;
        let last = self.points.last()?;
        if t >= last.0 {
            return None;
        }
        for w in self.points.windows(2) {
            if t < w[1].0 {
                let k = ((t - w[0].0) / (w[1].0 - w[0].0).max(1e-6)) as f32;
                return Some(w[0].1 + (w[1].1 - w[0].1) * k);
            }
        }
        Some(last.1)
    }
}

pub enum Dialog {
    SaveAs {
        name: String,
    },
    /// Unsaved edits and another song was asked for (None = a new song).
    Discard {
        then: Option<PathBuf>,
    },
}

/// Command-line options for verification.
#[derive(Default, Clone)]
pub struct Cli {
    pub song: Option<PathBuf>,
    pub screenshot: Option<PathBuf>,
    pub view: Option<String>,
    pub smoke: bool,
    pub play: bool,
    pub size: Option<(f32, f32)>,
    /// Hear this log entry from the start (checking the A/B path).
    pub audition: Option<String>,
    /// A reference recording to load at start.
    pub reference: Option<PathBuf>,
    /// Workbench: open the first instrument's editor.
    pub edit: bool,
    /// The music folder, instead of finding `data/music`.
    pub music: Option<PathBuf>,
    /// Workbench: loop this part / play the whole song / play this moment at start.
    pub part: Option<String>,
    pub whole: bool,
    pub moment: Option<String>,
    /// Open this recording in the player.
    pub audio: Option<PathBuf>,
    /// Workbench: show the notice for the newest Claude revision.
    pub notice: bool,
}

pub struct Studio {
    pub song: Song,
    pub path: Option<PathBuf>,
    pub music_dir: Option<PathBuf>,
    /// What is on disk (as last loaded or saved), for the dirty marker.
    pub saved: Song,
    pub dirty: bool,
    disk_mtime: Option<SystemTime>,
    pub disk_changed: bool,
    last_disk_poll: f64,
    pub history: History<Song>,
    sent: Arc<Song>,
    pub audio: Audio,
    pub midi: Midi,
    pub keys: KeyPiano,
    pub status: Status,
    pub meters: Meters,
    pub page: Page,
    pub detail: Detail,
    pub detail_open: bool,
    pub focus: Pane,
    pub sel: Selection,
    pub mode: PlayMode,
    sent_mode: Option<Mode>,
    pub loop_on: bool,
    /// Loop range over the arrangement, ticks.
    pub loop_range: (u32, u32),
    sent_loop: Option<Option<(u32, u32)>>,
    pub metronome: bool,
    pub intensity: f32,
    pub ramp: Option<Ramp>,
    pub message: Option<(String, Tone, f64)>,
    pub dialog: Option<Dialog>,
    pub arrange: arrange::State,
    pub piano: piano::State,
    pub drums: drums::State,
    pub instrument: instrument::State,
    pub mixer: mixer::State,
    pub director: director::State,
    pub export: export::State,
    pub browser: browser::State,
    pub collab: collab::Collab,
    pub reference: reference::Reference,
    /// The full tool set (true) or the Workbench (false).
    pub advanced: bool,
    pub wb: crate::workbench::State,
    /// A seek to send after the next mode change.
    pub pending_seek: Option<u32>,
    /// The stage's moment and duck, and the reference player, as last polled.
    pub side: crate::audio::Side,
    quiet_save_due: Option<f64>,
    pub sketch: sketch::Sketch,
    pub screen: Screen,
    screen_at: f64,
    pub picker: crate::picker::State,
    pub player: crate::player::State,
    pub moments: crate::moments::Moments,
    cli: Cli,
    frame: u64,
    time: f64,
    shot_sent: bool,
    smoke_stopped: bool,
    style_set: bool,
}

impl Studio {
    pub fn new(ctx: &egui::Context, cli: Cli) -> Studio {
        theme::install(ctx);
        let music_dir = cli.music.clone().or_else(files::find_music_dir);
        // A song on the command line opens straight away; otherwise the
        // library comes first and nothing opens by itself.
        let candidate = cli.song.clone();
        let mut message = None;
        let (song, path) = match candidate {
            Some(p) => match Song::load(&p) {
                Ok(s) => (s, Some(p)),
                Err(e) => {
                    message = Some((e, Tone::Bad, 0.0));
                    (crate::songops::starter_song("Untitled"), None)
                }
            },
            None => (crate::songops::starter_song("Untitled"), None),
        };
        if let Some(p) = &path {
            files::remember_song(p);
        }
        let shared = Arc::new(song.clone());
        let audio = Audio::start(shared.clone());
        let midi = Midi::connect(audio.sender());
        let mut st = Studio {
            saved: song.clone(),
            dirty: path.is_none(),
            disk_mtime: path.as_deref().and_then(files::mtime),
            disk_changed: false,
            last_disk_poll: 0.0,
            history: History::new(&song),
            sent: shared,
            song,
            path,
            music_dir,
            audio,
            midi,
            keys: KeyPiano::new(),
            status: Status::default(),
            meters: Meters {
                tracks: Vec::new(),
                buses: Vec::new(),
                master: Default::default(),
                loudness: -70.0,
                scope: vec![[0.0; 2]; mc_music::engine::SCOPE_LEN],
                scope_at: 0,
                layers: Vec::new(),
            },
            page: Page::Arrange,
            detail: Detail::Piano,
            detail_open: true,
            focus: Pane::Arrange,
            sel: Selection::default(),
            mode: PlayMode::Song,
            sent_mode: None,
            loop_on: false,
            loop_range: (0, 0),
            sent_loop: None,
            metronome: false,
            intensity: 0.5,
            ramp: None,
            message,
            dialog: None,
            arrange: arrange::State::default(),
            piano: piano::State::default(),
            drums: drums::State::default(),
            instrument: instrument::State::default(),
            mixer: mixer::State::default(),
            director: director::State::default(),
            export: export::State::default(),
            browser: browser::State::default(),
            collab: collab::Collab::default(),
            reference: reference::Reference::default(),
            advanced: match cli.view.as_deref() {
                Some("workbench") => false,
                Some(_) => true,
                None => files::flag("advanced"),
            },
            wb: crate::workbench::State::default(),
            pending_seek: None,
            side: crate::audio::Side {
                song_level: 1.0,
                ..Default::default()
            },
            quiet_save_due: None,
            sketch: sketch::Sketch::new(),
            cli: cli.clone(),
            frame: 0,
            time: 0.0,
            shot_sent: false,
            smoke_stopped: false,
            style_set: false,
            screen: if cli.song.is_some() {
                Screen::Song
            } else {
                Screen::Picker
            },
            screen_at: -10.0,
            picker: Default::default(),
            player: Default::default(),
            moments: Default::default(),
        };
        st.select_first_clip();
        st.collab.attach(st.path.as_deref());
        st.audio.send(Command::ForceIntensity(st.intensity));
        match cli.view.as_deref() {
            Some("mixer") => {
                st.page = Page::Mixer;
                st.detail = Detail::Effect;
                st.sel.fx = st.song.master.effects.first().map(|_| (Strip::Master, 0));
            }
            Some("director") => {
                st.page = Page::Director;
                st.detail_open = false;
            }
            Some("reference") => st.detail = Detail::Reference,
            Some("export") => st.export.open = true,
            Some("drums") => st.open_first_of_kind(true),
            Some("piano") => st.open_first_of_kind(false),
            Some("instrument") => {
                st.detail = Detail::Instrument;
                st.sel.track = st
                    .song
                    .tracks
                    .iter()
                    .position(|t| matches!(t.instrument, Instrument::Synth(_)))
                    .unwrap_or(0);
            }
            _ => {}
        }
        if let Some(r) = cli.reference.clone() {
            reference::load(&mut st, r);
            st.detail = Detail::Reference;
        }
        if cli.smoke || cli.play {
            st.audio.send(Command::Play);
        }
        st
    }

    /// Selects the first clip of the first arrangement section, so a song opens
    /// with something in the piano roll.
    fn select_first_clip(&mut self) {
        let song = &self.song;
        let first = song.arrangement.first().and_then(|n| song.section(n)).or(
            if song.sections.is_empty() {
                None
            } else {
                Some(0)
            },
        );
        self.sel.section = first;
        self.sel.entry = if song.arrangement.is_empty() {
            None
        } else {
            Some(0)
        };
        if let Some(s) = first {
            if let Some(c) = song.sections[s]
                .clips
                .iter()
                .position(|c| song.track(&c.track).is_some() && song.pattern(&c.pattern).is_some())
            {
                self.select_clip(s, c);
            }
        }
    }

    fn open_first_of_kind(&mut self, kit: bool) {
        let song = &self.song;
        for (si, s) in song.sections.iter().enumerate() {
            for (ci, c) in s.clips.iter().enumerate() {
                let Some(t) = song.track(&c.track) else {
                    continue;
                };
                if matches!(song.tracks[t].instrument, Instrument::Kit(_)) == kit
                    && song.pattern(&c.pattern).is_some()
                {
                    self.select_clip(si, ci);
                    return;
                }
            }
        }
    }

    pub fn select_clip(&mut self, section: usize, clip: usize) {
        let Some(c) = self
            .song
            .sections
            .get(section)
            .and_then(|s| s.clips.get(clip))
        else {
            return;
        };
        self.sel.clip = Some((section, clip));
        self.sel.section = Some(section);
        if let Some(t) = self.song.track(&c.track) {
            self.sel.track = t;
            self.sel.pattern_track = t;
            self.detail = if matches!(self.song.tracks[t].instrument, Instrument::Kit(_)) {
                Detail::Drums
            } else {
                Detail::Piano
            };
        }
        if let Some(p) = self.song.pattern(&c.pattern) {
            self.sel.pattern = Some(p);
        }
    }

    pub fn say(&mut self, text: impl Into<String>, tone: Tone) {
        self.message = Some((text.into(), tone, self.now()));
    }

    /// Seconds since the studio started (egui's clock, so tests can drive it).
    fn now(&self) -> f64 {
        self.time
    }

    /// The song the engine has (the working copy, or a copy made for listening).
    pub fn sent_song(&self) -> &Song {
        &self.sent
    }

    pub fn time_now(&self) -> f64 {
        self.time
    }

    pub fn go(&mut self, screen: Screen) {
        if self.screen != screen {
            self.screen = screen;
            self.screen_at = self.time;
        }
    }

    /// Leaves the song for the library, saving first.
    pub fn to_library(&mut self) {
        if self.dirty && self.path.is_some() {
            self.save_quiet();
        }
        self.send(Command::Stop);
        self.status.playing = false;
        self.wb.edit = None;
        self.go(Screen::Picker);
    }

    pub fn sent_mode_is_song(&self) -> bool {
        matches!(self.sent_mode, Some(Mode::Song) | None)
    }

    pub fn set_advanced(&mut self, on: bool) {
        self.advanced = on;
        self.sent_mode = None;
        files::set_flag_to("advanced", on);
    }

    /// Saves without recording a revision: the Workbench's autosave.
    fn save_quiet(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        self.history.settle(&self.song);
        match self.song.save(&path) {
            Ok(()) => {
                self.saved = self.song.clone();
                self.dirty = false;
                self.disk_mtime = files::mtime(&path);
            }
            Err(e) => self.say(e, Tone::Bad),
        }
    }

    pub fn send(&self, c: Command) {
        self.audio.send(c);
    }

    /// Tracks the proposal being heard (or the newest one) changes.
    pub fn collab_changed_tracks(&self) -> std::collections::HashSet<String> {
        self.collab.changed_tracks.clone()
    }

    pub fn is_kit(&self, track: usize) -> bool {
        self.song
            .tracks
            .get(track)
            .is_some_and(|t| matches!(t.instrument, Instrument::Kit(_)))
    }

    /// The track the computer keyboard and MIDI play.
    pub fn play_track(&self) -> usize {
        if matches!(self.detail, Detail::Piano | Detail::Drums)
            && self.detail_open
            && self.sel.pattern.is_some()
        {
            self.sel.pattern_track
        } else {
            self.sel.track
        }
    }

    pub fn toggle_play(&mut self) {
        if self.status.playing {
            self.send(Command::Stop);
        } else {
            self.sync_engine();
            self.send(Command::Play);
        }
        self.status.playing = !self.status.playing;
    }

    // -- files -------------------------------------------------------------------

    pub fn open(&mut self, path: PathBuf) {
        match Song::load(&path) {
            Ok(song) => {
                self.send(Command::Panic);
                self.status.playing = false;
                self.song = song.clone();
                self.saved = song;
                self.history.reset(&self.song);
                self.disk_mtime = files::mtime(&path);
                self.disk_changed = false;
                files::remember_song(&path);
                self.say(format!("Opened {}", path.display()), Tone::Info);
                self.collab.attach(Some(&path));
                self.path = Some(path);
                self.dirty = false;
                self.sel = Selection::default();
                self.select_first_clip();
                self.piano.fit_next = true;
                self.arrange.fit_next = true;
            }
            Err(e) => self.say(e, Tone::Bad),
        }
    }

    pub fn new_song(&mut self) {
        self.send(Command::Panic);
        self.status.playing = false;
        self.song = crate::songops::starter_song("Untitled");
        self.saved = Song::empty("");
        self.history.reset(&self.song);
        self.path = None;
        self.collab.attach(None);
        self.dirty = true;
        self.disk_mtime = None;
        self.disk_changed = false;
        self.sel = Selection::default();
        self.select_first_clip();
        self.arrange.fit_next = true;
        self.piano.fit_next = true;
    }

    /// Opens `path` (or a new song), asking first when there are unsaved edits.
    pub fn request_open(&mut self, path: Option<PathBuf>) {
        if self.dirty {
            self.dialog = Some(Dialog::Discard { then: path });
        } else {
            match path {
                Some(p) => self.open(p),
                None => self.new_song(),
            }
        }
    }

    pub fn save(&mut self) {
        match self.path.clone() {
            Some(p) => self.save_to(p),
            None => {
                self.dialog = Some(Dialog::SaveAs {
                    name: self.song.name.clone(),
                })
            }
        }
    }

    pub fn save_to(&mut self, path: PathBuf) {
        self.history.settle(&self.song);
        match self.song.save(&path) {
            Ok(()) => {
                self.saved = self.song.clone();
                self.dirty = false;
                self.disk_mtime = files::mtime(&path);
                self.disk_changed = false;
                files::remember_song(&path);
                self.say(format!("Saved {}", path.display()), Tone::Good);
                if self.path.as_ref() != Some(&path) {
                    self.collab.attach(Some(&path));
                }
                self.path = Some(path);
                self.browser.rescan = true;
                collab::commit(self);
            }
            Err(e) => self.say(e, Tone::Bad),
        }
    }

    /// Takes the version on disk, keeping the selection where it still fits.
    pub fn reload(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        match Song::load(&path) {
            Ok(song) => {
                self.song = song.clone();
                self.saved = song;
                self.dirty = false;
                self.disk_changed = false;
                self.disk_mtime = files::mtime(&path);
                self.say(
                    format!(
                        "Reloaded {} from disk",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    Tone::Info,
                );
            }
            Err(e) => self.say(format!("The file on disk does not load: {e}"), Tone::Bad),
        }
    }

    /// Twice a second: has someone else written the file?
    fn watch_file(&mut self, now: f64) {
        if now - self.last_disk_poll < 0.5 {
            return;
        }
        self.last_disk_poll = now;
        let Some(path) = self.path.clone() else {
            return;
        };
        let m = files::mtime(&path);
        if m.is_none() || m == self.disk_mtime {
            return;
        }
        self.disk_mtime = m;
        match Song::load(&path) {
            Ok(disk) => {
                if disk == self.saved {
                    return;
                }
                if !self.dirty {
                    self.song = disk.clone();
                    self.saved = disk;
                    crate::workbench::external_change(self, now);
                    self.say(
                        format!(
                            "{} changed on disk: reloaded",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ),
                        Tone::Info,
                    );
                } else {
                    self.disk_changed = true;
                }
            }
            Err(e) => self.say(
                format!("The file on disk does not load (yet?): {e}"),
                Tone::Warn,
            ),
        }
    }

    // -- engine sync -------------------------------------------------------------

    fn wanted_mode(&self) -> Mode {
        match self.mode {
            PlayMode::Song => Mode::Song,
            PlayMode::Section => Mode::Section(
                self.sel
                    .section
                    .unwrap_or(0)
                    .min(self.song.sections.len().saturating_sub(1)),
            ),
            PlayMode::Pattern => Mode::Pattern {
                pattern: self
                    .sel
                    .pattern
                    .unwrap_or(0)
                    .min(self.song.patterns.len().saturating_sub(1)),
                track: self
                    .sel
                    .pattern_track
                    .min(self.song.tracks.len().saturating_sub(1)),
            },
            PlayMode::Director => Mode::Director,
        }
    }

    /// Sends the song, mode and loop when they changed.
    fn sync_engine(&mut self) {
        // While a proposal or an old revision is auditioned the engine plays it
        // instead; the working copy stays what the views edit.
        let base = self.collab.audition_song().unwrap_or(&self.song);
        let mode = if self.advanced {
            if *self.sent != *base {
                self.sent = Arc::new(base.clone());
                self.send(Command::SetSong(self.sent.clone()));
            }
            self.wanted_mode()
        } else {
            // The Workbench plays a copy with its listening mutes applied.
            let (song, mode) = crate::workbench::engine_plan(self, base);
            if *self.sent != song {
                self.sent = Arc::new(song);
                self.send(Command::SetSong(self.sent.clone()));
            }
            mode
        };
        let mode_ok = match &mode {
            Mode::Section(_) => !self.song.sections.is_empty(),
            Mode::Pattern { .. } => !self.song.patterns.is_empty() && !self.song.tracks.is_empty(),
            _ => true,
        };
        if mode_ok && self.sent_mode.as_ref() != Some(&mode) {
            self.sent_mode = Some(mode.clone());
            self.send(Command::SetMode(mode));
        }
        if let Some(t) = self.pending_seek.take() {
            self.send(Command::Seek(t));
        }
        let lp = if self.advanced && self.loop_on && self.loop_range.1 > self.loop_range.0 {
            Some(self.loop_range)
        } else {
            None
        };
        if self.sent_loop != Some(lp) {
            self.sent_loop = Some(lp);
            self.send(Command::SetLoop(lp));
        }
        self.midi.track.store(self.play_track(), Ordering::Relaxed);
    }

    /// Keeps indices valid after anything changed the song under the views.
    fn validate(&mut self) {
        let s = &self.song;
        let nt = s.tracks.len();
        self.sel.track = self.sel.track.min(nt.saturating_sub(1));
        self.sel.pattern_track = self.sel.pattern_track.min(nt.saturating_sub(1));
        if self.sel.section.is_some_and(|i| i >= s.sections.len()) {
            self.sel.section = None;
        }
        if self.sel.entry.is_some_and(|i| i >= s.arrangement.len()) {
            self.sel.entry = None;
        }
        if let Some((si, ci)) = self.sel.clip {
            if s.sections.get(si).is_none_or(|x| ci >= x.clips.len()) {
                self.sel.clip = None;
            }
        }
        if self.sel.pattern.is_some_and(|i| i >= s.patterns.len()) {
            self.sel.pattern = None;
        }
        if let Some((strip, i)) = self.sel.fx {
            if chain(s, strip).is_none_or(|c| i >= c.len()) {
                self.sel.fx = None;
            }
        }
    }

    // -- input -------------------------------------------------------------------

    pub fn text_focus(ctx: &egui::Context) -> bool {
        ctx.memory(|m| m.focused())
            .is_some_and(|id| egui::text_edit::TextEditState::load(ctx, id).is_some())
    }

    fn global_keys(&mut self, ctx: &egui::Context) {
        let typing = Self::text_focus(ctx);
        let track = self.play_track();
        if typing {
            for n in self.keys.release_all() {
                self.send(Command::NoteOff { track, key: n });
            }
            return;
        }
        let pressed = |k: Key, m: Modifiers| ctx.input_mut(|i| i.consume_key(m, k));
        if pressed(Key::Space, Modifiers::NONE) {
            if self.screen == Screen::Audio {
                crate::player::toggle_play(self);
            } else if self.screen == Screen::Picker {
            } else if self.advanced {
                self.toggle_play();
            } else {
                crate::workbench::toggle_play(self);
            }
        }
        if pressed(Key::Home, Modifiers::NONE) {
            self.send(Command::Seek(0));
        }
        if pressed(Key::S, Modifiers::COMMAND) {
            self.save();
        }
        if pressed(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)
            || pressed(Key::Y, Modifiers::COMMAND)
        {
            self.redo();
        }
        if pressed(Key::Z, Modifiers::COMMAND) {
            self.undo();
        }
        collab::keys(self, ctx);
        if pressed(Key::Backtick, Modifiers::NONE) {
            reference::flip(self);
        }
        if pressed(Key::Escape, Modifiers::NONE) {
            if !self.advanced && self.wb.edit.is_some() {
                self.wb.edit = None;
            } else {
                self.send(Command::Panic);
                self.status.playing = false;
            }
        }
        // The keyboard piano: only plain keys, so shortcuts still work.
        let events = ctx.input(|i| i.events.clone());
        let focused = ctx.input(|i| i.focused);
        if !focused {
            for n in self.keys.release_all() {
                self.send(Command::NoteOff { track, key: n });
            }
            return;
        }
        for e in events {
            if let egui::Event::Key {
                key,
                pressed,
                repeat,
                modifiers,
                ..
            } = e
            {
                if modifiers.ctrl || modifiers.alt || modifiers.command {
                    continue;
                }
                match self.keys.key(key, pressed, repeat) {
                    Some(KeyEvent::On(n)) => {
                        self.send(Command::NoteOn {
                            track,
                            key: n,
                            vel: self.keys.velocity,
                        });
                        self.sketch.note(n, self.keys.velocity);
                    }
                    Some(KeyEvent::Off(n)) => {
                        self.send(Command::NoteOff { track, key: n });
                        self.sketch.note(n, 0);
                    }
                    None => {}
                }
            }
        }
    }

    pub fn undo(&mut self) {
        if let Some(s) = self.history.undo(&self.song) {
            self.song = s;
            self.say("Undo", Tone::Info);
        }
    }

    pub fn redo(&mut self) {
        if let Some(s) = self.history.redo(&self.song) {
            self.song = s;
            self.say("Redo", Tone::Info);
        }
    }

    // -- verification hooks ------------------------------------------------------

    fn verification(&mut self, ctx: &egui::Context, now: f64) {
        if self.frame == 2 {
            if let Some(p) = self.cli.part.clone() {
                crate::workbench::focus(self, &p);
            }
            if self.cli.whole {
                crate::workbench::set_loop(self, false);
            }
            if let Some(a) = self.cli.audio.clone() {
                let dir = a
                    .parent()
                    .map(|d| d.join("reference.ron"))
                    .filter(|p| p.is_file());
                let row = crate::picker::Row {
                    name: a
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    path: a,
                    seconds: None,
                    changed: None,
                    transcription: dir,
                };
                crate::player::enter(self, row);
            }
        }
        if self.frame == 20 {
            if let Some(m) = self.cli.moment.clone() {
                crate::moments::play(self, &m);
            }
        }
        if self.cli.edit && self.frame > 2 && self.wb.edit.is_none() {
            self.cli.edit = false;
            if let Some(sec) = self.wb.focus.as_ref().and_then(|f| self.song.section(f)) {
                let rows = crate::workbench::row_tracks(&self.song, sec, false);
                // A synth if there is one: the piano roll shows more.
                let pick = rows
                    .iter()
                    .copied()
                    .find(|&t| !self.is_kit(t))
                    .or(rows.first().copied());
                if let Some(t) = pick {
                    let name = self.song.tracks[t].name.clone();
                    if let Some(c) = self.song.sections[sec]
                        .clips
                        .iter()
                        .find(|c| c.track == name)
                    {
                        self.wb.edit = Some((name, c.pattern.clone()));
                    }
                }
            }
        }
        if self.cli.notice && !self.collab.log.entries.is_empty() {
            self.cli.notice = false;
            self.wb.external_at = Some(now);
            self.wb.forget_announced();
        }
        if let Some(id) = self.cli.audition.clone() {
            if !self.collab.log.entries.is_empty() {
                self.cli.audition = None;
                self.collab.set_audition(Some(id));
            }
        }
        if let Some(out) = self.cli.screenshot.clone() {
            let waiting = self.reference.is_loading() && now < 15.0;
            if !self.shot_sent && self.frame > 30 && now > 1.2 && !waiting {
                self.shot_sent = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            let shots: Vec<Arc<egui::ColorImage>> = ctx.input(|i| {
                i.raw
                    .events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::Screenshot { image, .. } => Some(image.clone()),
                        _ => None,
                    })
                    .collect()
            });
            if let Some(img) = shots.first() {
                match write_png(&out, img) {
                    Ok(()) => println!(
                        "mc-studio: wrote {} ({}x{})",
                        out.display(),
                        img.size[0],
                        img.size[1]
                    ),
                    Err(e) => eprintln!("mc-studio: screenshot failed: {e}"),
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if now > 20.0 {
                eprintln!("mc-studio: no screenshot arrived");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if self.cli.smoke && now > 2.2 && !self.smoke_stopped {
            self.smoke_stopped = true;
            let s = &self.status;
            println!(
                "mc-studio smoke: device {:?} at {} Hz; playing {} section {:?} song tick {} voices {} load {:.3} peak {:.3}/{:.3} loudness {:.1} LUFS; problems {}",
                self.audio.device,
                self.audio.rate,
                s.playing,
                s.section.and_then(|i| self.song.sections.get(i)).map(|x| x.name.clone()),
                s.song_tick,
                s.voices,
                s.load,
                self.meters.master.peak[0],
                self.meters.master.peak[1],
                self.meters.loudness,
                self.song.problems().len()
            );
            self.send(Command::Stop);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    // -- frame -------------------------------------------------------------------

    fn frame_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        self.time = now;
        self.frame += 1;
        if !self.style_set {
            self.style_set = true;
            ctx.set_theme(egui::Theme::Dark);
        }
        let side = self.audio.poll(&mut self.status, &mut self.meters);
        reference::tick(self, side.ref_pos, side.ref_playing);
        self.side = side;
        if let Some(r) = &self.ramp {
            match r.value(now) {
                Some(v) => {
                    self.intensity = v;
                    self.send(Command::SetIntensity(v));
                }
                None => self.ramp = None,
            }
        }
        if self.dialog.is_none() {
            self.global_keys(&ctx);
        }
        self.validate();

        // Screens fade in when they change.
        let fade = ((now - self.screen_at) / 0.25).clamp(0.0, 1.0) as f32;
        if fade < 1.0 {
            ui.set_opacity(1.0 - (1.0 - fade).powi(3));
            ctx.request_repaint();
        }
        match self.screen {
            Screen::Picker => {
                crate::picker::show(ui, self);
                self.after_frame(&ctx, now);
                return;
            }
            Screen::Audio => {
                crate::player::show(ui, self);
                self.after_frame(&ctx, now);
                return;
            }
            Screen::Song => {}
        }
        if !self.advanced {
            crate::workbench::show(ui, self);
            self.after_frame(&ctx, now);
            return;
        }
        egui::Panel::top("transport")
            .frame(
                egui::Frame::new()
                    .fill(theme::BG0)
                    .inner_margin(egui::Margin::symmetric(10, 6)),
            )
            .show(ui, |ui| transport::show(ui, self));
        egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(theme::BG0)
                    .inner_margin(egui::Margin::symmetric(10, 3)),
            )
            .show(ui, |ui| self.status_line(ui, now));
        egui::Panel::left("browser")
            .resizable(true)
            .default_size(230.0)
            .size_range(170.0..=420.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::BG1)
                    .inner_margin(egui::Margin::same(8)),
            )
            .show(ui, |ui| browser::show(ui, self));
        if self.collab.open {
            egui::Panel::right("collab")
                .resizable(true)
                .default_size(360.0)
                .size_range(280.0..=640.0)
                .frame(
                    egui::Frame::new()
                        .fill(theme::BG1)
                        .inner_margin(egui::Margin::same(8))
                        .stroke(egui::Stroke::new(1.0, theme::line(20))),
                )
                .show(ui, |ui| collab::dock(ui, self));
        }
        if self.detail_open {
            let h = ui.ctx().content_rect().height().max(600.0) - 90.0;
            let big = matches!(
                self.cli.view.as_deref(),
                Some("piano" | "drums" | "instrument")
            );
            egui::Panel::bottom("detail")
                .resizable(true)
                .default_size(if big { h * 0.62 } else { h * 0.45 })
                .size_range(160.0..=(h - 120.0).max(200.0))
                .frame(
                    egui::Frame::new()
                        .fill(theme::BG1)
                        .stroke(egui::Stroke::new(1.0, theme::line(20))),
                )
                .show(ui, |ui| {
                    let r = ui.max_rect();
                    self.detail_ui(ui);
                    if ui.input(|i| i.pointer.any_pressed()) && ui.rect_contains_pointer(r) {
                        self.focus = Pane::Detail;
                    }
                });
        }
        if !self.detail_open {
            egui::Panel::bottom("detail-closed")
                .frame(
                    egui::Frame::new()
                        .fill(theme::BG0)
                        .inner_margin(egui::Margin::symmetric(8, 3)),
                )
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (d, name) in [
                            (Detail::Piano, "Piano roll"),
                            (Detail::Drums, "Drum grid"),
                            (Detail::Instrument, "Instrument"),
                            (Detail::Effect, "Effect"),
                            (Detail::Reference, "Reference"),
                        ] {
                            if tab(ui, false, name).clicked() {
                                self.detail = d;
                                self.detail_open = true;
                            }
                        }
                    });
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BG1))
            .show(ui, |ui| {
                let r = ui.max_rect();
                if self.disk_changed {
                    self.disk_banner(ui);
                }
                match self.page {
                    Page::Arrange => arrange::show(ui, self),
                    Page::Mixer => mixer::show(ui, self),
                    Page::Director => director::show(ui, self),
                }
                if ui.input(|i| i.pointer.any_pressed()) && ui.rect_contains_pointer(r) {
                    self.focus = if self.page == Page::Arrange {
                        Pane::Arrange
                    } else {
                        Pane::Other
                    };
                }
            });
        self.dialogs(&ctx);
        export::window(&ctx, self);

        self.after_frame(&ctx, now);
    }

    /// After every view had its say: validate, send, record, save, watch.
    fn after_frame(&mut self, ctx: &egui::Context, now: f64) {
        let ctx = ctx.clone();
        self.validate();
        self.sync_engine();
        let gesture = ctx.input(|i| i.pointer.any_down()) || Self::text_focus(&ctx);
        self.history.observe(&self.song, gesture);
        self.dirty = self.song != self.saved;
        // The Workbench saves by itself a moment after an edit (no revision).
        if !self.advanced && self.dirty && self.path.is_some() && !self.disk_changed {
            let due = *self.quiet_save_due.get_or_insert(now + 1.5);
            if now >= due && !gesture {
                self.quiet_save_due = None;
                self.save_quiet();
            }
        } else {
            self.quiet_save_due = None;
        }
        self.watch_file(now);
        collab::tick(self, now);
        self.verification(&ctx, now);
        let title = format!(
            "{}{} - mc-studio",
            if self.dirty { "* " } else { "" },
            self.path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "untitled".into())
        );
        if self.frame % 30 == 1 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
        // Draw continuously only while something moves.
        let moving = self.status.playing
            || self.side.moment.is_some()
            || self.side.ref_playing
            || self.wb.animating
            || self.ramp.is_some()
            || self.export.busy();
        if moving {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    fn detail_ui(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(theme::BG0)
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    for (d, name) in [
                        (Detail::Piano, "Piano roll"),
                        (Detail::Drums, "Drum grid"),
                        (Detail::Instrument, "Instrument"),
                        (Detail::Effect, "Effect"),
                        (Detail::Reference, "Reference"),
                    ] {
                        if tab(ui, self.detail == d, name).clicked() {
                            self.detail = d;
                        }
                    }
                    ui.add_space(12.0);
                    if ui
                        .small_button("Hide")
                        .on_hover_text("Hide the editor pane")
                        .clicked()
                    {
                        self.detail_open = false;
                    }
                    let t = self.play_track();
                    if let Some(track) = self.song.tracks.get(t) {
                        ui.label(
                            egui::RichText::new(format!("Keys play {}", track.name))
                                .color(FAINT)
                                .small(),
                        );
                        ui.label(
                            egui::RichText::new(format!("Octave {}", self.keys.base / 12 - 1))
                                .color(FAINT)
                                .small(),
                        )
                        .on_hover_text("Z..M and Q..P play; minus and equals shift the octave");
                    }
                });
            });
        match self.detail {
            Detail::Piano => piano::show(ui, self),
            Detail::Drums => drums::show(ui, self),
            Detail::Instrument => instrument::show(ui, self),
            Detail::Effect => crate::effects::detail(ui, self),
            Detail::Reference => reference::show(ui, self),
        }
    }

    fn disk_banner(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(theme::with_alpha(WARN, 30))
            .stroke(egui::Stroke::new(1.0, theme::with_alpha(WARN, 120)))
            .inner_margin(egui::Margin::symmetric(10, 5))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(
                            "The file changed on disk while you have unsaved edits.",
                        )
                        .color(WARN),
                    );
                    if ui.button("Reload from disk").clicked() {
                        self.reload();
                    }
                    if ui.button("Keep mine").clicked() {
                        self.disk_changed = false;
                    }
                });
            });
    }

    fn status_line(&mut self, ui: &mut egui::Ui, now: f64) {
        ui.horizontal(|ui| {
            let problems = self.song.problems();
            if problems.is_empty() {
                ui.label(egui::RichText::new("No problems").color(FAINT).small());
            } else {
                ui.label(
                    egui::RichText::new(format!("{} problems: {}", problems.len(), problems[0]))
                        .color(WARN)
                        .small(),
                )
                .on_hover_text(problems.join("\n"));
            }
            ui.separator();
            if let Some((text, tone, at)) = &self.message {
                let age = now - at;
                if age < 8.0 {
                    let c = match tone {
                        Tone::Info => DIM,
                        Tone::Good => theme::GOOD,
                        Tone::Warn => WARN,
                        Tone::Bad => BAD,
                    };
                    ui.label(egui::RichText::new(text).color(c).small());
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let midi = if self.midi.ports.is_empty() {
                    self.midi
                        .error
                        .clone()
                        .unwrap_or_else(|| "No MIDI inputs".into())
                } else {
                    format!("MIDI: {}", self.midi.ports.join(", "))
                };
                let last = self.midi.last.load(Ordering::Relaxed);
                let lit = last != 0 && self.browser.midi_seen != last;
                if lit {
                    self.browser.midi_seen = last;
                    self.browser.midi_flash = now;
                }
                let flash = now - self.browser.midi_flash < 0.15;
                if ui
                    .small_button("Rescan")
                    .on_hover_text("Look for MIDI inputs again")
                    .clicked()
                {
                    self.midi.rescan(self.audio.sender());
                }
                ui.label(
                    egui::RichText::new(midi)
                        .color(if flash { ACCENT } else { FAINT })
                        .small(),
                );
                ui.separator();
                ui.label(
                    egui::RichText::new(format!("{} at {} Hz", self.audio.device, self.audio.rate))
                        .color(if self.audio.silent { WARN } else { FAINT })
                        .small(),
                );
            });
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        let mut keep = true;
        let mut next: Option<Dialog> = None;
        let modal = egui::Modal::new(egui::Id::new("dialog")).show(ctx, |ui| {
            ui.set_width(360.0);
            match &dialog {
                Dialog::SaveAs { name } => {
                    let mut name = name.clone();
                    ui.label(
                        egui::RichText::new("Save song as")
                            .font(theme::font_light(22.0))
                            .color(TEXT),
                    );
                    ui.add_space(6.0);
                    let dir = self.music_dir.clone().unwrap_or_else(|| PathBuf::from("."));
                    let r =
                        ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY));
                    r.request_focus();
                    let stem = files::file_stem(&name);
                    ui.label(
                        egui::RichText::new(format!(
                            "{}",
                            dir.join(format!("{stem}.ron")).display()
                        ))
                        .color(FAINT)
                        .small(),
                    );
                    ui.add_space(6.0);
                    let enter = ui.input(|i| i.key_pressed(Key::Enter));
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() || enter {
                            if self.song.name.is_empty() || self.song.name == "Untitled" {
                                self.song.name = name.trim().to_string();
                            }
                            self.save_to(dir.join(format!("{stem}.ron")));
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                    });
                    if keep {
                        next = Some(Dialog::SaveAs { name });
                    }
                }
                Dialog::Discard { then } => {
                    ui.label(
                        egui::RichText::new("Unsaved changes")
                            .font(theme::font_light(22.0))
                            .color(TEXT),
                    );
                    ui.label("This song has edits that are not saved.");
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Save first").clicked() {
                            self.save();
                            if !self.dirty {
                                match then.clone() {
                                    Some(p) => self.open(p),
                                    None => self.new_song(),
                                }
                            }
                            keep = false;
                        }
                        if ui.button("Discard").clicked() {
                            self.dirty = false;
                            match then.clone() {
                                Some(p) => self.open(p),
                                None => self.new_song(),
                            }
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                    });
                }
            }
        });
        if modal.should_close() {
            keep = false;
        }
        // "Save first" on an untitled song opens Save as in `self.dialog`; keep it.
        if keep {
            self.dialog = next.or(Some(dialog));
        }
    }
}

/// A tab header: accent underline when selected.
pub fn tab(ui: &mut egui::Ui, on: bool, text: &str) -> egui::Response {
    let font = theme::font_semi(14.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, TEXT);
    let (rect, resp) =
        ui.allocate_exact_size(galley.size() + egui::vec2(16.0, 10.0), egui::Sense::click());
    let colour = if on {
        TEXT
    } else if resp.hovered() {
        theme::mix(DIM, TEXT, 0.5)
    } else {
        DIM
    };
    let p = ui.painter();
    p.galley(rect.center() - galley.size() * 0.5, galley, colour);
    if on {
        p.hline(
            rect.x_range().shrink(6.0),
            rect.bottom() - 1.0,
            egui::Stroke::new(2.0, ACCENT),
        );
    }
    resp
}

fn write_png(path: &std::path::Path, img: &egui::ColorImage) -> Result<(), String> {
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(
        std::io::BufWriter::new(file),
        img.size[0] as u32,
        img.size[1] as u32,
    );
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(|e| e.to_string())?;
    let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
    w.write_image_data(&bytes).map_err(|e| e.to_string())
}

impl eframe::App for Studio {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frame_ui(ui);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let c = theme::BG0;
        [
            c.r() as f32 / 255.0,
            c.g() as f32 / 255.0,
            c.b() as f32 / 255.0,
            1.0,
        ]
    }
}

#[cfg(test)]
mod ui_tests {
    //! The studio driven through egui with synthetic input: the gestures that
    //! screenshots cannot show.
    use super::*;
    use egui::{pos2, Event, PointerButton, Pos2, RawInput, Rect};
    use mc_music::PPQ;

    struct Harness {
        ctx: egui::Context,
        st: Studio,
        time: f64,
        _dir: PathBuf,
    }

    impl Harness {
        fn new(view: &str) -> Harness {
            Harness::build(view, true)
        }

        /// A studio started without a song: the library.
        fn library() -> Harness {
            Harness::build("library", false)
        }

        fn build(view: &str, open: bool) -> Harness {
            let dir = {
                static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
                let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                std::env::temp_dir().join(format!("mc-studio-ui-{}-{view}-{n}", std::process::id()))
            };
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("moments")).unwrap();
            std::fs::create_dir_all(dir.join("references")).unwrap();
            let path = dir.join("t.ron");
            crate::songops::starter_song("T").save(&path).unwrap();
            let mut hit = crate::songops::starter_song("Hit");
            hit.sections[0].bars = 1;
            hit.save(&dir.join("moments").join("hit.ron")).unwrap();
            let tone: Vec<[f32; 2]> = (0..4800)
                .map(|i| [((i as f32) * 0.06).sin() * 0.3; 2])
                .collect();
            mc_music::render::write_wav(&dir.join("references").join("a.wav"), &tone, 48000)
                .unwrap();
            let ctx = egui::Context::default();
            let cli = Cli {
                song: open.then_some(path),
                view: if open { Some(view.into()) } else { None },
                music: Some(dir.clone()),
                ..Default::default()
            };
            let st = Studio::new(&ctx, cli);
            let mut h = Harness {
                ctx,
                st,
                time: 0.0,
                _dir: dir,
            };
            for _ in 0..4 {
                h.frame(vec![]);
            }
            h
        }

        fn frame(&mut self, events: Vec<Event>) {
            self.time += 1.0 / 60.0;
            let raw = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1600.0, 960.0))),
                time: Some(self.time),
                focused: true,
                events,
                ..Default::default()
            };
            let st = &mut self.st;
            let mut out = self.ctx.run_ui(raw, |ui| st.frame_ui(ui));
            out.textures_delta.clear();
        }

        fn button(&mut self, at: Pos2, pressed: bool, modifiers: egui::Modifiers) {
            self.frame(vec![
                Event::PointerMoved(at),
                Event::PointerButton {
                    pos: at,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers,
                },
            ]);
        }

        fn click(&mut self, at: Pos2) {
            self.frame(vec![Event::PointerMoved(at)]);
            self.button(at, true, Default::default());
            self.button(at, false, Default::default());
            self.frame(vec![]);
        }

        fn drag(&mut self, from: Pos2, to: Pos2) {
            self.frame(vec![Event::PointerMoved(from)]);
            self.button(from, true, Default::default());
            for k in 1..=6 {
                let p = from + (to - from) * (k as f32 / 6.0);
                self.frame(vec![Event::PointerMoved(p)]);
            }
            self.button(to, false, Default::default());
            self.frame(vec![]);
        }

        fn key(&mut self, key: egui::Key, modifiers: egui::Modifiers) {
            self.frame(vec![Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }]);
            self.frame(vec![Event::Key {
                key,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers,
            }]);
        }

        fn notes(&self) -> Vec<mc_music::Note> {
            let p = self.st.sel.pattern.expect("a pattern is open");
            self.st.song.patterns[p].notes.clone()
        }
    }

    #[test]
    fn piano_roll_draws_moves_and_undoes() {
        let mut h = Harness::new("piano");
        assert_eq!(h.st.detail, Detail::Piano);
        let before = h.notes();
        // Chords: an empty spot a beat in, on D4 (the chord is A3 C4 E4).
        let at =
            h.st.piano
                .screen_pos(PPQ + 2, 62)
                .expect("the roll was drawn");
        h.click(at);
        let after = h.notes();
        assert_eq!(after.len(), before.len() + 1, "a click draws a note");
        let n = *after.iter().find(|n| n.2 == 62).expect("the note is on D4");
        assert_eq!(n.0, PPQ, "snapped to the 1/16 grid");
        assert_eq!(h.st.piano.selected().len(), 1);

        // Drag it a beat later and a tone up.
        let from = h.st.piano.screen_pos(n.0 + 4, 62).unwrap();
        let to = h.st.piano.screen_pos(n.0 + 4 + PPQ, 63).unwrap();
        h.drag(from, to);
        let moved = h.notes();
        assert!(
            moved.iter().any(|m| m.0 == 2 * PPQ && m.2 == 63),
            "moved: {moved:?}"
        );
        assert!(
            !moved.iter().any(|m| m.0 == PPQ && m.2 == 62),
            "the original moved"
        );

        // Undo twice: the move, then the drawing.
        h.key(egui::Key::Z, egui::Modifiers::COMMAND);
        assert!(
            h.notes().iter().any(|m| m.0 == PPQ && m.2 == 62),
            "undo puts it back"
        );
        h.key(egui::Key::Z, egui::Modifiers::COMMAND);
        assert_eq!(h.notes(), before, "and removes it");
        h.key(egui::Key::Y, egui::Modifiers::COMMAND);
        assert_eq!(h.notes().len(), before.len() + 1, "redo draws it again");
    }

    #[test]
    fn delete_and_duplicate_keys_work_on_the_selection() {
        let mut h = Harness::new("piano");
        let at = h.st.piano.screen_pos(2 * PPQ, 62).unwrap();
        h.click(at);
        let n = h.notes().len();
        h.key(egui::Key::ArrowUp, egui::Modifiers::NONE);
        assert!(
            h.notes().iter().any(|m| m.0 == 2 * PPQ && m.2 == 63),
            "Up raises the selection a semitone"
        );
        h.key(egui::Key::ArrowRight, egui::Modifiers::NONE);
        assert!(
            h.notes()
                .iter()
                .any(|m| m.0 == 2 * PPQ + PPQ / 4 && m.2 == 63),
            "Right nudges by the grid"
        );
        h.key(egui::Key::D, egui::Modifiers::COMMAND);
        assert_eq!(h.notes().len(), n + 1, "Ctrl+D duplicates");
        h.key(egui::Key::Delete, egui::Modifiers::NONE);
        assert_eq!(h.notes().len(), n, "Delete erases the selected copy");
    }

    #[test]
    fn saving_records_a_revision_and_the_session_is_written() {
        let mut h = Harness::new("arrange");
        h.st.song.tempo = 99.0;
        h.frame(vec![]);
        assert!(h.st.dirty);
        h.key(egui::Key::S, egui::Modifiers::COMMAND);
        assert!(!h.st.dirty, "Ctrl+S saves");
        let desk = h.st.collab.desk.as_ref().unwrap();
        let log = desk.log("t");
        assert_eq!(log.entries.len(), 1, "a revision was recorded");
        // The session is written within half a second of a change.
        for _ in 0..40 {
            h.frame(vec![]);
        }
        let s =
            h.st.collab
                .desk
                .as_ref()
                .unwrap()
                .session()
                .expect("a session file");
        assert_eq!(s.song, "t");
        assert_eq!(s.head.as_deref(), Some(log.entries[0].id.as_str()));
    }

    #[test]
    fn a_disk_change_reloads_when_there_are_no_edits() {
        let mut h = Harness::new("arrange");
        let path = h.st.path.clone().unwrap();
        let mut s = Song::load(&path).unwrap();
        s.tempo = 133.0;
        // Make sure the mtime moves even on coarse file systems.
        std::thread::sleep(std::time::Duration::from_millis(20));
        s.save(&path).unwrap();
        let t = std::time::SystemTime::now() + std::time::Duration::from_secs(2);
        let _ = std::fs::File::options()
            .write(true)
            .open(&path)
            .and_then(|f| f.set_modified(t));
        for _ in 0..60 {
            h.frame(vec![]);
        }
        assert_eq!(h.st.song.tempo, 133.0, "reloaded silently");
        // With an unsaved edit the banner shows instead.
        h.st.song.tempo = 90.0;
        h.frame(vec![]);
        s.tempo = 140.0;
        s.save(&path).unwrap();
        let _ = std::fs::File::options()
            .write(true)
            .open(&path)
            .and_then(|f| f.set_modified(t + std::time::Duration::from_secs(2)));
        for _ in 0..60 {
            h.frame(vec![]);
        }
        assert_eq!(h.st.song.tempo, 90.0);
        assert!(h.st.disk_changed);
    }

    #[allow(dead_code)]
    fn unused(_: Pos2) -> Pos2 {
        pos2(0.0, 0.0)
    }

    #[test]
    fn arrangement_clips_move_and_sections_reorder() {
        let mut h = Harness::new("arrange");
        h.st.page = Page::Arrange;
        // A second section so there is an order to change.
        let d = crate::songops::duplicate_section(&mut h.st.song, 0);
        let second = h.st.song.sections[d].name.clone();
        h.st.song.arrangement.push(second.clone());
        h.st.arrange.fit_next = true;
        for _ in 0..3 {
            h.frame(vec![]);
        }
        // Drag the Chords clip (track 1, at beat 0) four beats later.
        let from = h.st.arrange.screen_pos(PPQ, 1).unwrap();
        let to = h.st.arrange.screen_pos(5 * PPQ, 1).unwrap();
        h.drag(from, to);
        let c = h.st.song.sections[0]
            .clips
            .iter()
            .find(|c| c.pattern == "Chords")
            .unwrap();
        assert_eq!(c.at, 4, "moved by four beats");
        // Drag the first section block past the second.
        let bar = h.st.song.bar_ticks();
        let a = h.st.arrange.section_pos(bar).unwrap();
        let b = h.st.arrange.section_pos(4 * bar + 3 * bar).unwrap();
        h.drag(a, b);
        assert_eq!(
            h.st.song.arrangement[0], second,
            "reordered: {:?}",
            h.st.song.arrangement
        );
        // Double-clicking an empty lane offers patterns.
        // Chords now plays once (beats 4..12 of Main), leaving its last bar empty.
        for c in h.st.song.sections[0]
            .clips
            .iter_mut()
            .filter(|c| c.pattern == "Chords")
        {
            c.times = 1;
        }
        h.frame(vec![]);
        let empty = h.st.arrange.screen_pos(7 * bar + bar / 2, 1).unwrap();
        h.click(empty);
        h.click(empty);
        assert!(h.st.arrange.adding(), "the add-clip popup opened");
    }

    #[test]
    fn drum_grid_toggles_and_the_keyboard_plays() {
        let mut h = Harness::new("drums");
        assert_eq!(h.st.detail, Detail::Drums);
        let n = h.notes().len();
        // Row 0 is the kick; step 1 (a sixteenth in) is empty in the starter beat.
        let at = h.st.drums.cell_pos(0, 1).expect("the grid was drawn");
        h.click(at);
        assert_eq!(h.notes().len(), n + 1, "a click adds a hit");
        assert!(h.notes().iter().any(|x| x.0 == PPQ / 4 && x.2 == 36));
        h.click(at);
        assert_eq!(h.notes().len(), n, "a second click removes it");
    }

    #[test]
    fn every_variant_in_a_group_can_be_heard_by_clicking() {
        let mut h = Harness::new("arrange");
        let desk = h.st.collab.desk.as_ref().unwrap();
        let mut a = h.st.song.clone();
        a.tempo = 100.0;
        let mut b = h.st.song.clone();
        b.tempo = 90.0;
        let ea = desk
            .propose("t", &a, "slower", Some("g"), Some("A"), "")
            .unwrap();
        let eb = desk
            .propose("t", &b, "slower still", Some("g"), Some("B"), "")
            .unwrap();
        for _ in 0..80 {
            h.frame(vec![]);
        }
        assert_eq!(h.st.collab.pending().len(), 2);
        for (id, tempo) in [(&eb.id, 90.0), (&ea.id, 100.0), (&eb.id, 90.0)] {
            let at =
                h.st.collab
                    .hear_at
                    .get(id)
                    .copied()
                    .expect("the card was drawn")
                    .center();
            h.click(at);
            assert_eq!(
                h.st.collab.audition.as_deref(),
                Some(id.as_str()),
                "clicking Hear on {id} auditions it"
            );
            assert_eq!(h.st.sent.tempo, tempo, "the engine plays {id}");
        }
    }

    #[test]
    fn proposals_are_heard_in_place_and_accepted() {
        let mut h = Harness::new("arrange");
        let mut proposal = h.st.song.clone();
        proposal.tempo = 100.0;
        proposal.patterns[1]
            .notes
            .push(mc_music::Note(0, 96, 67, 90));
        let desk = h.st.collab.desk.as_ref().unwrap();
        let e = desk
            .propose(
                "t",
                &proposal,
                "slower, with a G on top",
                None,
                Some("A"),
                "",
            )
            .unwrap();
        for _ in 0..80 {
            h.frame(vec![]);
        }
        assert_eq!(h.st.collab.pending().len(), 1, "the proposal was picked up");
        assert!(h.st.collab.open);
        assert!(h.st.collab.changed_patterns.contains("Chords"));
        // F2 hears it: the engine gets the proposal, the working copy is untouched.
        h.key(egui::Key::F2, egui::Modifiers::NONE);
        assert_eq!(h.st.collab.audition.as_deref(), Some(e.id.as_str()));
        assert_eq!(h.st.sent.tempo, 100.0);
        assert_eq!(h.st.song.tempo, 120.0);
        // F1 back to ours, Tab flips again.
        h.key(egui::Key::F1, egui::Modifiers::NONE);
        assert_eq!(h.st.sent.tempo, 120.0);
        h.key(egui::Key::Tab, egui::Modifiers::NONE);
        assert_eq!(h.st.sent.tempo, 100.0);
        crate::collab::accept(&mut h.st, &e.id);
        h.frame(vec![]);
        assert_eq!(
            h.st.song.tempo, 100.0,
            "accepting loads it as the working copy"
        );
        assert!(!h.st.dirty);
        assert!(h.st.collab.audition.is_none());
        for _ in 0..80 {
            h.frame(vec![]);
        }
        assert!(h.st.collab.pending().is_empty());
    }

    /// A starter song with a second part that only has drums.
    fn two_parts(h: &mut Harness) {
        let mut s = h.st.song.clone();
        let d = crate::songops::duplicate_section(&mut s, 0);
        s.sections[d].clips.retain(|c| c.pattern == "Beat");
        let name = s.sections[d].name.clone();
        s.arrangement.push(name);
        h.st.song = s;
        h.st.save();
        for _ in 0..4 {
            h.frame(vec![]);
        }
    }

    fn row_button(h: &Harness, track: &str, what: &'static str) -> Pos2 {
        h.st.wb
            .row_at
            .get(&(track.to_string(), what))
            .copied()
            .unwrap_or_else(|| panic!("{track} {what} not drawn"))
            .center()
    }

    #[test]
    fn workbench_is_the_default_and_focuses_a_part() {
        let mut h = Harness::new("workbench");
        assert!(!h.st.advanced);
        two_parts(&mut h);
        assert_eq!(h.st.wb.focus.as_deref(), Some("Main"));
        assert!(h.st.wb.row_at.contains_key(&("Synth".to_string(), "edit")));
        let chip =
            h.st.wb
                .chip_at
                .get("Main 2")
                .copied()
                .expect("a chip per part")
                .center();
        h.click(chip);
        assert_eq!(h.st.wb.focus.as_deref(), Some("Main 2"));
        assert_eq!(h.st.sent_mode, Some(Mode::Section(1)), "the part loops");
        // The view glides in until the part fills the width.
        let bar = h.st.song.bar_ticks() as f32;
        for _ in 0..30 {
            h.frame(vec![]);
        }
        let v = h.st.wb.view;
        assert!(
            v.0 > 3.5 * bar && v.0 < 4.0 * bar && v.1 > 8.0 * bar && v.1 < 8.5 * bar,
            "zoomed on the part: {v:?}"
        );
        // Whole song plays the arrangement and glides back out.
        crate::workbench::set_loop(&mut h.st, false);
        for _ in 0..30 {
            h.frame(vec![]);
        }
        assert_eq!(h.st.sent_mode, Some(Mode::Song));
        let v = h.st.wb.view;
        assert!(
            v.0.abs() < 1.0 && (v.1 - 8.0 * bar).abs() < 1.0,
            "the whole song: {v:?}"
        );
    }

    #[test]
    fn workbench_mutes_and_solos_only_what_is_heard() {
        let mut h = Harness::new("workbench");
        h.frame(vec![]);
        let spk = row_button(&h, "Drums", "speaker");
        h.click(spk);
        assert!(h.st.wb.muted.contains("Drums"));
        assert!(h.st.sent.tracks[0].mute, "the engine hears it muted");
        assert!(!h.st.song.tracks[0].mute, "the song is not changed");
        assert!(!h.st.dirty);
        let on_disk = Song::load(h.st.path.as_ref().unwrap()).unwrap();
        assert!(!on_disk.tracks[0].mute);
        h.click(spk);
        assert!(!h.st.sent.tracks[0].mute);
        let solo = row_button(&h, "Synth", "solo");
        h.click(solo);
        assert!(h.st.sent.tracks[1].solo && !h.st.sent.tracks[0].solo);
        assert!(!h.st.song.tracks[1].solo);
        h.click(solo);
        assert!(!h.st.sent.tracks[1].solo, "a second click clears it");
    }

    #[test]
    fn workbench_editor_adds_removes_and_moves_notes() {
        let mut h = Harness::new("workbench");
        h.frame(vec![]);
        let e = row_button(&h, "Synth", "edit");
        h.click(e);
        assert_eq!(
            h.st.wb.edit,
            Some(("Synth".to_string(), "Chords".to_string()))
        );
        // The editor slides open.
        for _ in 0..30 {
            h.frame(vec![]);
        }
        let pi = h.st.song.pattern("Chords").unwrap();
        let n = h.st.song.patterns[pi].notes.len();
        let at =
            h.st.wb
                .editor
                .cell_pos(2 * PPQ, 62)
                .expect("the editor is drawn");
        h.click(at);
        let notes = h.st.song.patterns[pi].notes.clone();
        assert_eq!(notes.len(), n + 1, "a click adds a note");
        assert!(notes.iter().any(|x| x.0 == 2 * PPQ && x.2 == 62));
        // Drag it a beat later, two keys up.
        let from = h.st.wb.editor.cell_pos(2 * PPQ, 62).unwrap();
        let to = h.st.wb.editor.cell_pos(3 * PPQ, 64).unwrap();
        h.drag(from, to);
        let notes = h.st.song.patterns[pi].notes.clone();
        assert!(
            notes.iter().any(|x| x.0 == 3 * PPQ && x.2 == 64),
            "moved: {notes:?}"
        );
        let at = h.st.wb.editor.cell_pos(3 * PPQ, 64).unwrap();
        h.click(at);
        assert_eq!(
            h.st.song.patterns[pi].notes.len(),
            n,
            "a click on a note removes it"
        );
        // The drum grid.
        let e = row_button(&h, "Drums", "edit");
        h.click(e);
        for _ in 0..30 {
            h.frame(vec![]);
        }
        let bi = h.st.song.pattern("Beat").unwrap();
        let n = h.st.song.patterns[bi].notes.len();
        let at = h.st.wb.editor.cell_pos(PPQ / 4, 36).unwrap();
        h.click(at);
        assert_eq!(h.st.song.patterns[bi].notes.len(), n + 1);
        // The edit saves itself a moment later.
        for _ in 0..120 {
            h.frame(vec![]);
        }
        assert!(!h.st.dirty, "autosaved");
        let disk = Song::load(h.st.path.as_ref().unwrap()).unwrap();
        assert_eq!(disk.patterns[bi].notes.len(), n + 1);
    }

    #[test]
    fn claude_changing_the_file_keeps_the_part_and_mutes_and_can_be_undone() {
        let mut h = Harness::new("workbench");
        two_parts(&mut h);
        let chip = h.st.wb.chip_at.get("Main 2").copied().unwrap().center();
        h.click(chip);
        let spk = row_button(&h, "Drums", "speaker");
        h.click(spk);
        // Let the log load, with the user's first revision in it.
        for _ in 0..80 {
            h.frame(vec![]);
        }
        let path = h.st.path.clone().unwrap();
        let mut s = Song::load(&path).unwrap();
        s.tempo = 132.0;
        s.save(&path).unwrap();
        let t = std::time::SystemTime::now() + std::time::Duration::from_secs(3);
        let _ = std::fs::File::options()
            .write(true)
            .open(&path)
            .and_then(|f| f.set_modified(t));
        h.st.collab
            .desk
            .as_ref()
            .unwrap()
            .commit("t", &s, "claude", "Faster")
            .unwrap();
        for _ in 0..90 {
            h.frame(vec![]);
        }
        assert_eq!(h.st.song.tempo, 132.0, "reloaded");
        assert_eq!(
            h.st.wb.focus.as_deref(),
            Some("Main 2"),
            "still on the same part"
        );
        assert!(h.st.wb.loop_part);
        assert!(h.st.wb.muted.contains("Drums"), "mutes kept by name");
        assert!(h.st.sent.tracks[0].mute);
        let n = h.st.wb.notice.as_ref().expect("a notice");
        assert_eq!(n.text, "Claude changed: Faster");
        let to = n.undo_to.clone().expect("it can be undone");
        crate::workbench::undo_change(&mut h.st, &to);
        h.frame(vec![]);
        assert_eq!(h.st.song.tempo, 120.0, "undo goes back");
        assert_eq!(Song::load(&path).unwrap().tempo, 120.0);
    }

    #[test]
    fn the_advanced_switch_shows_everything_else() {
        let mut h = Harness::new("workbench");
        h.frame(vec![]);
        let at = h.st.wb.advanced_at.expect("drawn").center();
        h.click(at);
        assert!(h.st.advanced);
        h.frame(vec![]);
        h.st.set_advanced(false);
        h.frame(vec![]);
        assert!(!h.st.advanced);
        assert!(h.st.wb.advanced_at.is_some());
    }

    #[test]
    fn the_library_comes_first_and_opens_songs_and_audio() {
        let mut h = Harness::library();
        assert_eq!(h.st.screen, Screen::Picker);
        assert!(h.st.path.is_none(), "nothing opens by itself");
        h.frame(vec![]);
        for name in ["T", "Hit", "a"] {
            assert!(h.st.picker.row_at.contains_key(name), "{name} is listed");
        }
        let at = h.st.picker.row_at["T"].center();
        h.click(at);
        assert_eq!(h.st.screen, Screen::Song);
        assert!(h.st.path.is_some());
        h.frame(vec![]);
        let back =
            h.st.wb
                .library_at
                .expect("the song's name is drawn")
                .center();
        h.click(back);
        assert_eq!(h.st.screen, Screen::Picker);
        let at = h.st.picker.row_at["a"].center();
        h.click(at);
        assert_eq!(h.st.screen, Screen::Audio);
        assert!(h.st.player.row.is_some());
    }

    #[test]
    fn moments_play_now_and_can_be_dropped_on_the_timeline() {
        let mut h = Harness::new("workbench");
        for _ in 0..3 {
            h.frame(vec![]);
        }
        let chip =
            h.st.wb
                .moment_at
                .get("hit")
                .copied()
                .expect("a chip per moment")
                .center();
        h.click(chip);
        // The silent audio thread renders in real time.
        let mut heard = false;
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            h.frame(vec![]);
            if h.st.side.moment.as_deref() == Some("hit") {
                heard = true;
                break;
            }
        }
        assert!(heard, "the moment plays");
        // Drag the chip onto the ruler.
        let ruler = h.st.wb.ruler_at.expect("drawn");
        let to = pos2(ruler.left() + ruler.width() * 0.5, ruler.center().y);
        h.drag(chip, to);
        assert_eq!(h.st.moments.markers.len(), 1, "a marker was dropped");
        assert_eq!(h.st.moments.markers[0].name, "hit");
        h.frame(vec![]);
        let m = h.st.wb.marker_at[0].center();
        h.frame(vec![Event::PointerMoved(m)]);
        h.frame(vec![Event::PointerButton {
            pos: m,
            button: PointerButton::Secondary,
            pressed: true,
            modifiers: Default::default(),
        }]);
        h.frame(vec![Event::PointerButton {
            pos: m,
            button: PointerButton::Secondary,
            pressed: false,
            modifiers: Default::default(),
        }]);
        assert!(h.st.moments.markers.is_empty(), "right-click removes it");
    }
}
