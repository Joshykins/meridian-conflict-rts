//! Swapping a part's instrument: the library (`data/music/instruments`) as a
//! menu, recorded instruments first, and the short name a row shows for what a
//! part plays.

use crate::app::{Studio, Tone};
use eframe::egui;
use mc_music::library::{self, Listing};
use mc_music::Instrument;

/// The library, read again every couple of seconds so new files show up.
#[derive(Default)]
pub struct Catalogue {
    entries: Vec<Listing>,
    at: f64,
}

impl Catalogue {
    pub fn entries(&mut self, st_music: Option<&std::path::Path>, now: f64) -> &[Listing] {
        if self.entries.is_empty() && self.at == 0.0 || now - self.at > 2.0 {
            self.at = now;
            self.entries = st_music.map(library::catalogue).unwrap_or_default();
        }
        &self.entries
    }

    /// Reads the folder again on the next look (after saving a preset).
    pub fn stale(&mut self) {
        self.at = -10.0;
    }
}

/// What a part plays, in a few words: a library instrument's title, else its kind.
pub fn label(st: &mut Studio, inst: &Instrument, now: f64) -> String {
    let name = match inst {
        Instrument::Use(n) => n.clone(),
        Instrument::Sampler(s) => s.set.clone(),
        Instrument::Synth(_) => return "Synth".into(),
        Instrument::Kit(_) => return "Drums".into(),
    };
    let dir = st.music_dir.clone();
    st.instrument
        .catalogue
        .entries(dir.as_deref(), now)
        .iter()
        .find(|l| l.name == name)
        .map(|l| l.title.clone())
        .unwrap_or_else(|| name.replace('_', " "))
}

/// A Workbench row's second line: what the part plays; click it to swap. `here`
/// is what the part plays in the looped part, when one is looped.
pub fn row_label(
    ui: &mut egui::Ui,
    st: &mut Studio,
    t: usize,
    at: egui::Pos2,
    width: f32,
    here: Option<&[String]>,
) {
    use crate::theme::{self, FAINT, TEXT};
    let now = ui.input(|i| i.time);
    let inst = st.song.tracks[t].instrument.clone();
    let text = crate::transport::truncate(&label(st, &inst, now), 19);
    let r = egui::Rect::from_min_size(at, egui::vec2(width.max(40.0), 18.0));
    let resp = ui.interact(r, ui.id().with(("swap", t)), egui::Sense::click());
    let away = here.is_some_and(|c| c.is_empty());
    let colour = if resp.hovered() {
        TEXT
    } else if away {
        theme::mix(FAINT, theme::BG2, 0.45)
    } else {
        FAINT
    };
    let galley = ui
        .painter()
        .layout_no_wrap(text, theme::font_body(13.0), colour);
    let w = galley.size().x;
    ui.painter().galley(at, galley, colour);
    if resp.hovered() {
        ui.painter().hline(
            at.x..=at.x + w,
            at.y + 16.0,
            egui::Stroke::new(1.0, theme::with_alpha(TEXT, 120)),
        );
    }
    let hover = match here {
        Some([]) => "Not in this part. Click to change the instrument".to_string(),
        Some(c) => format!(
            "Plays {} here. Click to change the instrument",
            c.join(", ")
        ),
        None => "Click to change the instrument".to_string(),
    };
    let resp = resp.on_hover_text(hover);
    egui::Popup::menu(&resp).show(|ui| menu(ui, st, t));
}

/// The library as a menu; picking one makes track `t` play it.
pub fn menu(ui: &mut egui::Ui, st: &mut Studio, t: usize) {
    let now = ui.input(|i| i.time);
    let dir = st.music_dir.clone();
    let entries = st
        .instrument
        .catalogue
        .entries(dir.as_deref(), now)
        .to_vec();
    let current = match &st.song.tracks[t].instrument {
        Instrument::Use(n) => Some(n.clone()),
        _ => None,
    };
    let mut picked = None;
    let mut heading = None;
    for l in &entries {
        let group = if l.recorded {
            "Orchestra"
        } else {
            "Synthesised"
        };
        if heading != Some(group) {
            if heading.is_some() {
                ui.separator();
            }
            ui.label(
                egui::RichText::new(group)
                    .color(crate::theme::FAINT)
                    .small(),
            );
            heading = Some(group);
        }
        let on = current.as_deref() == Some(l.name.as_str());
        if ui.selectable_label(on, &l.title).clicked() {
            picked = Some(l.clone());
        }
    }
    if entries.is_empty() {
        ui.label(egui::RichText::new("No instruments in the library").color(crate::theme::FAINT));
    }
    if let Some(l) = picked {
        use_instrument(st, t, &l.name);
        st.say(
            format!("{} now plays {}", st.song.tracks[t].name, l.title),
            Tone::Info,
        );
        ui.close();
    }
}

/// Makes track `t` play the library instrument `name`, loading its recordings.
pub fn use_instrument(st: &mut Studio, t: usize, name: &str) {
    st.song.tracks[t].instrument = Instrument::Use(name.to_string());
    relink(st);
}

/// Loads what the song's instruments now need (library files, recordings).
pub fn relink(st: &mut Studio) {
    match st.song.library.dir().map(std::path::Path::to_path_buf) {
        Some(_) => st.song.relink(),
        None => {
            if let Some(dir) = st.music_dir.clone() {
                st.song.link(&dir);
            }
        }
    }
    for p in st.song.library.problems().to_vec() {
        st.say(p, Tone::Bad);
    }
}
