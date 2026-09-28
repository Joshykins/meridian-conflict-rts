//! The Sounds screen: every sound the game makes, to play and pick apart.
//!
//! A sound plays exactly as the game makes it (`mc_sfx`, the game's own
//! synthesiser): whole, or one of its layers alone at its level in the whole.
//! Each shows the words written above it in its file, what it is built on, and
//! which units, buildings and parts of the game use it.
//!
//! The screen watches the files. When a recipe changes on disk (Claude edits
//! it while you talk), the sound is made again at once and a strip at the top
//! says so, with the version from before it changed to play against it.

mod catalogue;

use crate::app::{Screen, Studio};
use crate::audio::RefCmd;
use crate::theme::{self, ACCENT, BAD, BG0, BG1, BG2, BG3, DIM, FAINT, GOOD, TEXT};
use crate::widgets::{self, Icon};
use crate::workbench::{big_toggle, pretty};
use catalogue::{Catalogue, Entry, User};
use eframe::egui::{self, pos2, vec2, Align2, CornerRadius, Rect, Sense, Stroke, StrokeKind};
use mc_data::sounds::{Layer, Sound};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::SystemTime;

/// Which rendering of a sound.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Take {
    Whole,
    /// The recipe as it was before its last change.
    Before,
    Layer(usize),
}

type Key = (String, Take);

struct Clip {
    frames: Arc<Vec<[f32; 2]>>,
    /// Low and high of each of `WAVE` slices, for drawing.
    wave: Vec<(f32, f32)>,
}

const WAVE: usize = 400;

/// A finished rendering: its key, the version of the sound it was made from, the samples.
type Done = (Key, u32, Vec<[f32; 2]>);

pub struct Sounds {
    data: Option<PathBuf>,
    catalogue: Catalogue,
    stamp: Vec<(PathBuf, Option<SystemTime>)>,
    checked: Option<f64>,
    /// Why the files could not be read, when they could not (the last good read stays).
    pub error: Option<String>,
    pub selected: Option<String>,
    search: String,
    clips: HashMap<Key, Clip>,
    making: HashSet<Key>,
    /// Bumped when a sound's recipe changes, so a rendering of the old one is dropped.
    version: HashMap<String, u32>,
    done_tx: Sender<Done>,
    done_rx: Receiver<Done>,
    /// Played as soon as it is made.
    play_when_made: Option<Key>,
    /// Recipes as they were before they last changed.
    before: HashMap<String, Sound>,
    /// Sounds that changed on disk, and when (studio time).
    pub changed: Vec<(String, f64)>,
    serial: u64,
    /// The last thing played and its serial.
    playing: Option<(u64, Key)>,
    pub scroll_to_selected: bool,
}

impl Default for Sounds {
    fn default() -> Sounds {
        let (done_tx, done_rx) = channel();
        Sounds {
            data: None,
            catalogue: Catalogue::default(),
            stamp: Vec::new(),
            checked: None,
            error: None,
            selected: None,
            search: String::new(),
            clips: HashMap::new(),
            making: HashSet::new(),
            version: HashMap::new(),
            done_tx,
            done_rx,
            play_when_made: None,
            before: HashMap::new(),
            changed: Vec::new(),
            serial: 0,
            playing: None,
            scroll_to_selected: false,
        }
    }
}

impl Sounds {
    /// How many sounds there are (read once for the library screen).
    pub fn count(&mut self, music_dir: Option<&PathBuf>, now: f64) -> usize {
        self.refresh(music_dir, now);
        self.catalogue.entries.len()
    }

    /// Something is being made or played: keep drawing.
    pub fn busy(&self) -> bool {
        !self.making.is_empty()
    }

    /// Reads the files again if any changed (at most once a second).
    fn refresh(&mut self, music_dir: Option<&PathBuf>, now: f64) {
        if self.checked.is_some_and(|t| now - t < 1.0) {
            return;
        }
        self.checked = Some(now);
        if self.data.is_none() {
            self.data = music_dir.and_then(|m| m.parent()).map(PathBuf::from);
        }
        let Some(data) = self.data.clone() else {
            self.error = Some("The data folder was not found.".into());
            return;
        };
        let stamp = catalogue::watched(&data);
        if stamp == self.stamp {
            return;
        }
        let first = self.stamp.is_empty();
        self.stamp = stamp;
        match catalogue::load(&data) {
            Ok(c) => {
                self.error = None;
                if !first {
                    self.note_changes(&c, now);
                }
                self.catalogue = c;
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Keeps the old recipe of every sound that changed, forgets its renderings,
    /// and selects the first one changed.
    fn note_changes(&mut self, new: &Catalogue, now: f64) {
        let mut changed = Vec::new();
        for e in &new.entries {
            let (Some(s), Some(old)) = (
                &e.sound,
                self.catalogue.get(&e.name).and_then(|o| o.sound.as_ref()),
            ) else {
                continue;
            };
            if s.layers != old.layers
                || s.length != old.length
                || s.peak != old.peak
                || s.room != old.room
                || s.looped != old.looped
            {
                changed.push(e.name.clone());
                self.before.insert(e.name.clone(), old.clone());
            }
        }
        for name in &changed {
            *self.version.entry(name.clone()).or_default() += 1;
            self.clips
                .retain(|(n, t), _| n != name || *t == Take::Before);
            self.clips.remove(&(name.clone(), Take::Before));
            self.changed.retain(|(n, _)| n != name);
            self.changed.push((name.clone(), now));
        }
        if let Some(first) = changed.first() {
            self.selected = Some(first.clone());
            self.scroll_to_selected = true;
        }
    }

    fn version_of(&self, name: &str) -> u32 {
        self.version.get(name).copied().unwrap_or(0)
    }

    /// Starts making `key` on a thread of its own, unless it is made or on its way.
    fn make(&mut self, key: &Key, rate: u32) {
        if self.clips.contains_key(key) || self.making.contains(key) {
            return;
        }
        let Some(e) = self.catalogue.get(&key.0) else {
            return;
        };
        let job: Box<dyn FnOnce() -> Vec<[f32; 2]> + Send> = match (&key.1, &e.sound, e.sfx) {
            (_, None, Some(sfx)) => Box::new(move || mc_sfx::interface(sfx, rate)),
            (Take::Whole, Some(s), _) => {
                let s = s.clone();
                Box::new(move || mc_sfx::synth(&s, rate))
            }
            (Take::Layer(i), Some(s), _) => {
                let (s, i) = (s.clone(), *i);
                Box::new(move || mc_sfx::synth_layer(&s, rate, i))
            }
            (Take::Before, Some(_), _) => match self.before.get(&key.0) {
                Some(old) => {
                    let s = old.clone();
                    Box::new(move || mc_sfx::synth(&s, rate))
                }
                None => return,
            },
            _ => return,
        };
        self.making.insert(key.clone());
        let (tx, key, version) = (self.done_tx.clone(), key.clone(), self.version_of(&key.0));
        let _ = std::thread::Builder::new()
            .name("mc-studio-sound".into())
            .spawn(move || {
                let frames = job();
                let _ = tx.send((key, version, frames));
            });
    }

    /// Takes in what the threads have made; plays what was waiting for it.
    fn collect(&mut self, st_audio: &crate::audio::Audio) {
        while let Ok((key, version, frames)) = self.done_rx.try_recv() {
            self.making.remove(&key);
            if version != self.version_of(&key.0) {
                continue;
            }
            let wave = overview(&frames);
            self.clips.insert(
                key.clone(),
                Clip {
                    frames: Arc::new(frames),
                    wave,
                },
            );
            if self.play_when_made.as_ref() == Some(&key) {
                self.play_when_made = None;
                self.start(&key, st_audio);
            }
        }
    }

    fn looped(&self, name: &str) -> bool {
        self.catalogue
            .get(name)
            .and_then(|e| e.sound.as_ref())
            .is_some_and(|s| s.looped)
    }

    fn start(&mut self, key: &Key, audio: &crate::audio::Audio) {
        let Some(clip) = self.clips.get(key) else {
            return;
        };
        self.serial += 1;
        // One loop at a time: a new loop replaces whatever was going round.
        let looped = self.looped(&key.0);
        if looped
            || self
                .playing
                .as_ref()
                .is_some_and(|(_, k)| self.looped(&k.0))
        {
            audio.send_ref(RefCmd::StopShots);
        }
        audio.send_ref(RefCmd::Shot {
            frames: clip.frames.clone(),
            looped,
            serial: self.serial,
        });
        self.playing = Some((self.serial, key.clone()));
    }

    /// Plays `key`, making it first if need be.
    fn play(&mut self, key: Key, audio: &crate::audio::Audio) {
        if self.clips.contains_key(&key) {
            self.start(&key, audio);
        } else {
            self.make(&key, audio.rate);
            self.play_when_made = Some(key);
        }
    }

    fn stop(&mut self, audio: &crate::audio::Audio) {
        audio.send_ref(RefCmd::StopShots);
        self.playing = None;
        self.play_when_made = None;
    }
}

/// Low and high of each slice of a sound, for drawing its shape.
fn overview(frames: &[[f32; 2]]) -> Vec<(f32, f32)> {
    let n = frames.len().max(1);
    (0..WAVE)
        .map(|i| {
            let (a, b) = (i * n / WAVE, ((i + 1) * n / WAVE).min(frames.len()));
            frames[a.min(b)..b]
                .iter()
                .fold((0.0f32, 0.0f32), |(lo, hi), f| {
                    let m = (f[0] + f[1]) * 0.5;
                    (lo.min(m), hi.max(m))
                })
        })
        .collect()
}

pub fn enter(st: &mut Studio) {
    st.send(mc_music::Command::Stop);
    st.status.playing = false;
    st.go(Screen::Sounds);
}

fn leave(st: &mut Studio) {
    st.sounds.stop(&st.audio);
    st.go(Screen::Picker);
}

/// Space: play the selected sound again (or stop a loop).
pub fn toggle_play(st: &mut Studio) {
    let Some(name) = st.sounds.selected.clone() else {
        return;
    };
    let looping = st.side.shot.is_some()
        && st
            .sounds
            .playing
            .as_ref()
            .is_some_and(|(_, k)| st.sounds.looped(&k.0));
    if looping {
        st.sounds.stop(&st.audio);
    } else {
        st.sounds.play((name, Take::Whole), &st.audio);
    }
}

/// Where the latest played sound is, 0..1 of its length, if it is `key`.
fn playhead(st: &Studio, key: &Key) -> Option<f32> {
    let (serial, k) = st.sounds.playing.as_ref()?;
    let (at_serial, pos) = st.side.shot?;
    if k != key || at_serial != *serial {
        return None;
    }
    let len = st.sounds.clips.get(key)?.frames.len().max(1);
    Some((pos as f32 / len as f32).clamp(0.0, 1.0))
}

pub fn show(ui: &mut egui::Ui, st: &mut Studio) {
    let now = st.time_now();
    let music_dir = st.music_dir.clone();
    st.sounds.refresh(music_dir.as_ref(), now);
    st.sounds.collect(&st.audio);
    if st.sounds.selected.is_none() {
        st.sounds.selected = st.sounds.catalogue.entries.first().map(|e| e.name.clone());
    }
    keys(ui, st);
    let mut back = false;
    egui::Panel::top("sounds-top")
        .frame(
            egui::Frame::new()
                .fill(BG0)
                .inner_margin(egui::Margin::symmetric(16, 12)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                if big_toggle(ui, false, "Library", DIM).clicked() {
                    back = true;
                }
                ui.label(
                    egui::RichText::new("Game sounds")
                        .font(theme::font_light(26.0))
                        .color(TEXT),
                );
                ui.label(
                    egui::RichText::new(format!("{}", st.sounds.catalogue.entries.len()))
                        .size(16.0)
                        .color(FAINT),
                );
            });
            changed_strip(ui, st, now);
        });
    if back {
        leave(st);
        return;
    }
    egui::Panel::left("sounds-list")
        .resizable(false)
        .exact_size(360.0)
        .frame(
            egui::Frame::new()
                .fill(BG1)
                .inner_margin(egui::Margin::symmetric(12, 12)),
        )
        .show(ui, |ui| list(ui, st));
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(BG1)
                .inner_margin(egui::Margin::symmetric(24, 18)),
        )
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("sound-detail")
                .auto_shrink([false, false])
                .show(ui, |ui| detail(ui, st));
        });
}

fn keys(ui: &mut egui::Ui, st: &mut Studio) {
    if Studio::text_focus(ui.ctx()) {
        return;
    }
    let (up, down) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
        )
    });
    if !(up || down) {
        return;
    }
    let names: Vec<String> = visible(st).into_iter().map(|e| e.name.clone()).collect();
    let at = st
        .sounds
        .selected
        .as_ref()
        .and_then(|s| names.iter().position(|n| n == s));
    let next = match at {
        Some(i) if up => i.saturating_sub(1),
        Some(i) => (i + 1).min(names.len().saturating_sub(1)),
        None => 0,
    };
    if let Some(n) = names.get(next) {
        st.sounds.selected = Some(n.clone());
        st.sounds.scroll_to_selected = true;
    }
}

/// The sounds that match the search: by name, group, words or users.
fn visible(st: &Studio) -> Vec<&Entry> {
    let q = st.sounds.search.trim().to_lowercase();
    st.sounds
        .catalogue
        .entries
        .iter()
        .filter(|e| {
            q.is_empty()
                || e.name.to_lowercase().contains(&q)
                || pretty(&e.name).to_lowercase().contains(&q)
                || e.group.to_lowercase().contains(&q)
                || e.about.to_lowercase().contains(&q)
                || e.users.iter().any(|u| {
                    u.who.to_lowercase().contains(&q) || u.when.to_lowercase().contains(&q)
                })
        })
        .collect()
}

fn changed_strip(ui: &mut egui::Ui, st: &mut Studio, now: f64) {
    if let Some(e) = st.sounds.error.clone() {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(format!("Can't read the sound files: {e}"))
                .size(14.0)
                .color(BAD),
        );
    }
    // A change stays up for two minutes.
    st.sounds.changed.retain(|(_, t)| now - t < 120.0);
    let changed = st.sounds.changed.clone();
    if changed.is_empty() {
        return;
    }
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
        ui.label(egui::RichText::new("Changed").size(15.0).color(GOOD));
        for (name, _) in changed.iter().rev() {
            if big_toggle(ui, false, &format!("{}  now", pretty(name)), GOOD).clicked() {
                st.sounds.selected = Some(name.clone());
                st.sounds.scroll_to_selected = true;
                st.sounds.play((name.clone(), Take::Whole), &st.audio);
            }
            if big_toggle(ui, false, "before", DIM).clicked() {
                st.sounds.selected = Some(name.clone());
                st.sounds.play((name.clone(), Take::Before), &st.audio);
            }
            ui.add_space(8.0);
        }
    });
}

fn list(ui: &mut egui::Ui, st: &mut Studio) {
    ui.add(
        egui::TextEdit::singleline(&mut st.sounds.search)
            .desired_width(f32::INFINITY)
            .hint_text("Find a sound, a unit or a word"),
    );
    ui.add_space(8.0);
    let rows: Vec<(String, String, String)> = visible(st)
        .into_iter()
        .map(|e| {
            (
                e.name.clone(),
                e.group.clone(),
                users_short(&e.users, &e.bases_of),
            )
        })
        .collect();
    let mut pick = None;
    let mut play = None;
    let scroll = std::mem::take(&mut st.sounds.scroll_to_selected);
    egui::ScrollArea::vertical()
        .id_salt("sound-list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut group = "";
            for (name, g, users) in &rows {
                if g != group {
                    group = g;
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(g)
                            .font(theme::font_semi(13.0))
                            .color(FAINT),
                    );
                    ui.add_space(2.0);
                }
                let on = st.sounds.selected.as_deref() == Some(name.as_str());
                let w = ui.available_width();
                let (rect, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::click());
                if on && scroll {
                    ui.scroll_to_rect(rect, Some(egui::Align::Center));
                }
                let hover = ui
                    .ctx()
                    .animate_bool_with_time(resp.id, resp.hovered(), 0.1);
                let p = ui.painter();
                let fill = if on { BG3 } else { theme::mix(BG1, BG2, hover) };
                p.rect(
                    rect,
                    CornerRadius::same(6),
                    fill,
                    Stroke::new(
                        1.0,
                        if on {
                            theme::with_alpha(ACCENT, 140)
                        } else {
                            theme::line(0)
                        },
                    ),
                    StrokeKind::Inside,
                );
                let icon = Rect::from_center_size(
                    pos2(rect.left() + 18.0, rect.center().y),
                    vec2(22.0, 22.0),
                );
                let icon_hover = resp.hover_pos().is_some_and(|p| p.x < rect.left() + 34.0);
                widgets::draw_icon(p, icon, Icon::Play, if icon_hover { ACCENT } else { FAINT });
                p.text(
                    pos2(rect.left() + 36.0, rect.top() + 12.0),
                    Align2::LEFT_CENTER,
                    pretty(name),
                    theme::font_semi(14.5),
                    TEXT,
                );
                p.text(
                    pos2(rect.left() + 36.0, rect.top() + 29.0),
                    Align2::LEFT_CENTER,
                    users,
                    theme::font_body(12.0),
                    FAINT,
                );
                if resp.clicked() {
                    pick = Some(name.clone());
                    if icon_hover {
                        play = Some(name.clone());
                    }
                }
                if resp.double_clicked() {
                    play = Some(name.clone());
                }
            }
            if rows.is_empty() {
                ui.label(
                    egui::RichText::new("Nothing matches.")
                        .size(14.0)
                        .color(FAINT),
                );
            }
        });
    if let Some(n) = pick {
        st.sounds.selected = Some(n);
    }
    if let Some(n) = play {
        st.sounds.play((n, Take::Whole), &st.audio);
    }
}

/// "Warden, Sentinel +3" for a list row.
fn users_short(users: &[User], bases_of: &[String]) -> String {
    let mut names: Vec<String> = Vec::new();
    for u in users {
        let who = u.who.split(" (").next().unwrap_or(&u.who).to_owned();
        if !names.contains(&who) {
            names.push(who);
        }
    }
    if names.is_empty() {
        return if bases_of.is_empty() {
            "Not used".into()
        } else {
            format!("Base of {}", pretty(&bases_of[0]))
        };
    }
    let more = names.len().saturating_sub(3);
    let mut s = names.into_iter().take(3).collect::<Vec<_>>().join(", ");
    if more > 0 {
        s.push_str(&format!(" +{more}"));
    }
    s
}

fn detail(ui: &mut egui::Ui, st: &mut Studio) {
    let Some(e) = st
        .sounds
        .selected
        .as_ref()
        .and_then(|n| st.sounds.catalogue.get(n))
        .cloned()
    else {
        ui.label(
            egui::RichText::new("No sounds found.")
                .size(16.0)
                .color(FAINT),
        );
        return;
    };
    let rate = st.audio.rate;
    let whole = (e.name.clone(), Take::Whole);
    st.sounds.make(&whole, rate);

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(pretty(&e.name))
                .font(theme::font_light(34.0))
                .color(TEXT),
        );
        ui.add_space(8.0);
        let key = ui
            .add(
                egui::Label::new(
                    egui::RichText::new(&e.name)
                        .monospace()
                        .size(14.0)
                        .color(DIM),
                )
                .sense(Sense::click()),
            )
            .on_hover_text("Click to copy the name, to tell Claude which sound");
        if key.clicked() {
            ui.ctx().copy_text(e.name.clone());
        }
    });
    let mut facts = Vec::new();
    if !e.file.is_empty() {
        facts.push(e.file.clone());
    } else {
        facts.push("Written in the game's code (mc-sfx)".into());
    }
    if let Some(s) = &e.sound {
        facts.push(format!("{:.2} s", s.length));
        if s.looped {
            facts.push("loops".into());
        }
    }
    ui.label(
        egui::RichText::new(facts.join("   ·   "))
            .size(13.0)
            .color(FAINT),
    );
    if !e.about.is_empty() {
        ui.add_space(6.0);
        ui.label(egui::RichText::new(&e.about).size(15.5).color(DIM));
    }
    ui.add_space(14.0);

    // The whole sound: a big play button and its shape.
    let has_before = st.sounds.before.contains_key(&e.name);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let looping = st.sounds.looped(&e.name)
            && st.side.shot.is_some()
            && st
                .sounds
                .playing
                .as_ref()
                .is_some_and(|(_, k)| k.0 == e.name);
        let (r, resp) = ui.allocate_exact_size(vec2(64.0, 52.0), Sense::click());
        let p = ui.painter();
        p.rect_filled(r, CornerRadius::same(8), ACCENT);
        widgets::draw_icon(
            p,
            r.shrink(6.0),
            if looping { Icon::Stop } else { Icon::Play },
            BG0,
        );
        if resp.on_hover_text("Play (Space)").clicked() {
            if looping {
                st.sounds.stop(&st.audio);
            } else {
                st.sounds.play(whole.clone(), &st.audio);
            }
        }
        if has_before {
            let before = (e.name.clone(), Take::Before);
            if big_toggle(ui, false, "Before the change", DIM)
                .on_hover_text("The recipe as it was before it last changed")
                .clicked()
            {
                st.sounds.play(before, &st.audio);
            }
        }
    });
    ui.add_space(8.0);
    let w = ui.available_width();
    wave(ui, st, &whole, vec2(w, 120.0), ACCENT);
    if has_before {
        let before = (e.name.clone(), Take::Before);
        st.sounds.make(&before, rate);
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Before").size(12.0).color(FAINT));
        wave(ui, st, &before, vec2(w, 48.0), DIM);
    }

    if let Some(s) = &e.sound {
        if let Some((base, size)) = &s.like {
            ui.add_space(18.0);
            heading(ui, "Built on");
            ui.horizontal(|ui| {
                if big_toggle(ui, false, &pretty(base), ACCENT).clicked() {
                    st.sounds.selected = Some(base.clone());
                    st.sounds.scroll_to_selected = true;
                }
                ui.label(egui::RichText::new(size_words(*size)).size(14.0).color(DIM));
            });
        }
        ui.add_space(18.0);
        heading(ui, "Parts");
        ui.label(
            egui::RichText::new("Each layer alone, as loud as it is in the whole sound.")
                .size(13.0)
                .color(FAINT),
        );
        ui.add_space(6.0);
        // Short sounds have every part made at once, for their shapes.
        let cheap = s.length * (s.layers.len() as f32) < 120.0;
        for (i, layer) in s.layers.iter().enumerate() {
            let key = (e.name.clone(), Take::Layer(i));
            let process = matches!(layer, Layer::Drive(_));
            if cheap && !process {
                st.sounds.make(&key, rate);
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                if process {
                    ui.add_space(30.0);
                } else if widgets::icon_button(ui, Icon::Play, false, 30.0, "Play this part alone")
                    .clicked()
                {
                    st.sounds.play(key.clone(), &st.audio);
                }
                ui.vertical(|ui| {
                    ui.set_width(300.0);
                    ui.label(egui::RichText::new(describe(layer)).size(14.5).color(TEXT));
                    if let Some(n) = e.layer_notes.get(i).filter(|n| !n.is_empty()) {
                        ui.label(egui::RichText::new(n).size(12.5).color(FAINT));
                    }
                });
                if !process {
                    let w = ui.available_width().max(60.0);
                    wave(ui, st, &key, vec2(w, 34.0), theme::with_alpha(TEXT, 150));
                }
            });
            ui.add_space(4.0);
        }
    }

    ui.add_space(18.0);
    heading(ui, "Used by");
    if e.users.is_empty() {
        ui.label(
            egui::RichText::new("Nothing plays this sound directly.")
                .size(14.0)
                .color(FAINT),
        );
    }
    let mut sides: Vec<&str> = Vec::new();
    for u in &e.users {
        if !sides.contains(&u.side.as_str()) {
            sides.push(&u.side);
        }
    }
    for side in sides {
        if !side.is_empty() {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(side)
                    .font(theme::font_semi(13.0))
                    .color(FAINT),
            );
        }
        // One line per user, its uses joined.
        let mut lines: Vec<(String, Vec<String>, bool)> = Vec::new();
        for u in e.users.iter().filter(|u| u.side == side) {
            match lines.iter_mut().find(|l| l.0 == u.who) {
                Some(l) => {
                    l.1.push(u.when.clone());
                    l.2 &= u.fallback;
                }
                None => lines.push((u.who.clone(), vec![u.when.clone()], u.fallback)),
            }
        }
        for (who, whens, fallback) in lines {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(who).size(14.5).color(TEXT));
                ui.label(egui::RichText::new(whens.join(", ")).size(13.5).color(DIM));
                if fallback {
                    ui.label(
                        egui::RichText::new("(it names no sound of its own)")
                            .size(12.5)
                            .color(FAINT),
                    );
                }
            });
        }
    }
    if !e.bases_of.is_empty() {
        ui.add_space(12.0);
        heading(ui, "Sounds built on this one");
        ui.horizontal_wrapped(|ui| {
            for b in &e.bases_of {
                if big_toggle(ui, false, &pretty(b), DIM).clicked() {
                    st.sounds.selected = Some(b.clone());
                    st.sounds.scroll_to_selected = true;
                }
            }
        });
    }
    ui.add_space(30.0);
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(theme::font_semi(18.0))
            .color(TEXT),
    );
    ui.add_space(2.0);
}

/// Draws a clip's shape with the playhead; a click plays it.
fn wave(ui: &mut egui::Ui, st: &mut Studio, key: &Key, size: egui::Vec2, colour: egui::Color32) {
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::same(6), BG0);
    let Some(clip) = st.sounds.clips.get(key) else {
        p.text(
            r.center(),
            Align2::CENTER_CENTER,
            "making...",
            theme::font_body(12.0),
            FAINT,
        );
        return;
    };
    let top = clip
        .wave
        .iter()
        .fold(1e-4f32, |m, (lo, hi)| m.max(-lo).max(*hi));
    // Parts are drawn to the same scale as the whole, so their sizes compare.
    let scale = match key.1 {
        Take::Layer(_) => st
            .sounds
            .clips
            .get(&(key.0.clone(), Take::Whole))
            .map(|w| {
                w.wave
                    .iter()
                    .fold(1e-4f32, |m, (lo, hi)| m.max(-lo).max(*hi))
            })
            .unwrap_or(top),
        _ => top,
    };
    let at = playhead(st, key);
    let c = r.center().y;
    let n = clip.wave.len().max(1);
    for (i, (lo, hi)) in clip.wave.iter().enumerate() {
        let x = r.left() + 2.0 + (r.width() - 4.0) * (i as f32 + 0.5) / n as f32;
        let played = at.is_some_and(|a| (i as f32 / n as f32) < a);
        let col = if played { ACCENT } else { colour };
        let h = r.height() * 0.46 / scale;
        p.vline(
            x,
            (c - hi * h - 0.5)..=(c - lo * h + 0.5),
            Stroke::new(1.5, col),
        );
    }
    if let Some(a) = at {
        p.vline(
            r.left() + r.width() * a,
            r.y_range(),
            Stroke::new(1.5, TEXT),
        );
    }
    if resp.on_hover_text("Play").clicked() {
        st.sounds.play(key.clone(), &st.audio);
    }
}

fn size_words(size: f32) -> String {
    if (size - 1.0).abs() < 0.01 {
        "the same size".into()
    } else if size > 1.0 {
        format!("{size:.2}x bigger: lower and longer")
    } else {
        format!("{size:.2}x the size: higher and shorter")
    }
}

fn hz(f: f32) -> String {
    if f >= 1000.0 {
        format!("{:.1} kHz", f / 1000.0)
    } else {
        format!("{f:.0} Hz")
    }
}

fn pitch(from: f32, to: f32) -> String {
    if (from - to).abs() < 0.5 {
        hz(from)
    } else if to < from {
        format!("falling {} to {}", hz(from), hz(to))
    } else {
        format!("rising {} to {}", hz(from), hz(to))
    }
}

fn when(at: f32, decay: f32) -> String {
    let at = if at > 0.0005 {
        format!(", from {:.0} ms", at * 1000.0)
    } else {
        String::new()
    };
    // `decay` falls to a third; about five of them to silence.
    format!("{at}, rings ~{:.2} s", decay * 5.0)
}

/// A layer in plain words.
fn describe(layer: &Layer) -> String {
    match *layer {
        Layer::Tone {
            at,
            from,
            to,
            decay,
            ..
        } => format!("Tone, {}{}", pitch(from, to), when(at, decay)),
        Layer::Stack {
            at,
            from,
            to,
            decay,
            ref partials,
            ..
        } => {
            format!(
                "Buzz of {} tones, {}{}",
                partials.len() * 2,
                pitch(from, to),
                when(at, decay)
            )
        }
        Layer::Fm {
            at,
            from,
            to,
            ratio,
            decay,
            ..
        } => {
            let kind = if (ratio - ratio.round()).abs() < 0.01 {
                "Reedy tone"
            } else if (ratio * 2.0 - (ratio * 2.0).round()).abs() < 0.01 {
                "Growl"
            } else {
                "Metal ring"
            };
            format!("{kind}, {}{}", pitch(from, to), when(at, decay))
        }
        Layer::Burst {
            at,
            low,
            high,
            decay,
            ..
        } => {
            format!(
                "Crack, noise {} to {}{}",
                hz(low),
                hz(high),
                when(at, decay)
            )
        }
        Layer::Hiss {
            at, freq, decay, ..
        } => format!("Air, noise around {}{}", hz(freq), when(at, decay)),
        Layer::Sweep {
            at,
            from,
            to,
            decay,
            ..
        } => {
            format!("Rolling noise, {}{}", pitch(from, to), when(at, decay))
        }
        Layer::Roll {
            at,
            from,
            to,
            decay,
            ..
        } => {
            format!("Rumble under {}{}", pitch(from, to), when(at, decay))
        }
        Layer::Rumble { freq, .. } => format!("Steady noise around {}", hz(freq)),
        Layer::Drone { freq, .. } => format!("Steady hum at {}", hz(freq)),
        Layer::Wind { freq, .. } => format!("Wind around {}", hz(freq)),
        Layer::Chirp {
            at,
            from,
            to,
            decay,
            ..
        } => format!("Call, {}{}", pitch(from, to), when(at, decay)),
        Layer::Chorus { freq, voices, .. } => {
            format!("Chorus of {voices} callers around {}", hz(freq))
        }
        Layer::Drive(amount) => format!("Squashes the parts above it (drive {amount})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A recipe changed on disk is selected, listed as changed, and its old
    /// recipe kept to play against; a sound that did not change is left alone.
    #[test]
    fn a_changed_recipe_keeps_its_before() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let mut s = Sounds {
            catalogue: catalogue::load(&data).unwrap(),
            ..Default::default()
        };
        let mut new = catalogue::load(&data).unwrap();
        let tank = new
            .entries
            .iter_mut()
            .find(|e| e.name == "tank_gun")
            .and_then(|e| e.sound.as_mut())
            .unwrap();
        tank.length += 0.5;
        s.clips.insert(
            ("tank_gun".into(), Take::Whole),
            Clip {
                frames: Arc::new(Vec::new()),
                wave: Vec::new(),
            },
        );
        s.note_changes(&new, 3.0);
        assert_eq!(s.changed, vec![("tank_gun".to_owned(), 3.0)]);
        assert_eq!(s.selected.as_deref(), Some("tank_gun"));
        assert_eq!(s.before.len(), 1);
        assert!(s.clips.is_empty(), "the old rendering is dropped");
        assert_eq!(s.version_of("tank_gun"), 1);
    }

    #[test]
    fn every_layer_has_words() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        for e in catalogue::load(&data).unwrap().entries {
            for l in e.sound.iter().flat_map(|s| &s.layers) {
                assert!(!describe(l).is_empty());
            }
        }
    }
}
