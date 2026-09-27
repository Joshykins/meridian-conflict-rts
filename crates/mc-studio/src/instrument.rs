//! The instrument editor for the selected track: a synth's oscillators,
//! filter (with its drawn response), envelopes (drawn, with draggable
//! handles), LFOs and voice settings; a kit's drums, each built from a
//! pitched body, filtered noise and a metallic ring; or a recorded
//! instrument's shaping (swell, release, tone). Presets are the library,
//! `data/music/instruments/`. A live scope and spectrum sit beside it.

use crate::app::Studio;
use crate::fft;
use crate::files;
use crate::songops::unique_name;
use crate::theme::{self, ACCENT, BG0, BG2, DIM, FAINT, TEXT, WARN};
use crate::widgets::{self, group, knob, knob_i32, knob_u32, tiny_icon, Icon, Knob};
use eframe::egui::{self, pos2, vec2, Align2, CornerRadius, Rect, Sense, Shape, Stroke};
use mc_music::library;
use mc_music::patch::{
    Body, Drum, Env, Filter, FilterMode, Hiss, Lfo, LfoShape, LfoTo, Osc, Ring, Wave,
};
use mc_music::song::key_name;
use mc_music::{Command, Instrument};

#[derive(Default)]
pub struct State {
    pub catalogue: crate::swap::Catalogue,
    preset_name: String,
    /// Smoothed spectrum, dB per displayed column.
    spectrum: Vec<f32>,
    sounding: Option<(usize, u8, f64)>,
}

pub fn show(ui: &mut egui::Ui, st: &mut Studio) {
    let t = st.sel.track;
    if t >= st.song.tracks.len() {
        ui.label(egui::RichText::new("No track").color(FAINT));
        return;
    }
    header(ui, st, t);
    let avail = ui.available_rect_before_wrap();
    let scope_w = 280.0f32.min(avail.width() * 0.3);
    let left = Rect::from_min_max(
        avail.min,
        pos2(avail.right() - scope_w - 8.0, avail.bottom()),
    );
    let right = Rect::from_min_max(
        pos2(left.right() + 8.0, avail.top() + 4.0),
        pos2(avail.right() - 6.0, avail.bottom() - 6.0),
    );
    ui.allocate_rect(avail, Sense::hover());
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(left.shrink2(vec2(6.0, 4.0)))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    egui::ScrollArea::both()
        .id_salt("instrument-scroll")
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            let mut inst = st.song.tracks[t].instrument.clone();
            match &mut inst {
                Instrument::Synth(s) => synth(ui, st, s),
                Instrument::Kit(k) => kit(ui, st, t, k),
                Instrument::Sampler(s) => sampler(ui, s),
                Instrument::Use(name) => {
                    if let Some(copy) = library_instrument(ui, st, name) {
                        inst = copy;
                    }
                }
            }
            if inst != st.song.tracks[t].instrument {
                let relink = matches!(inst, Instrument::Sampler(_) | Instrument::Use(_));
                st.song.tracks[t].instrument = inst;
                if relink {
                    crate::swap::relink(st);
                }
            }
        });
    analyser(ui, st, right);
    let now = ui.input(|i| i.time);
    if let Some((tr, k, until)) = st.instrument.sounding {
        if now > until && !ui.input(|i| i.pointer.any_down()) {
            st.send(Command::NoteOff { track: tr, key: k });
            st.instrument.sounding = None;
        }
    }
}

fn header(ui: &mut egui::Ui, st: &mut Studio, t: usize) {
    let now = ui.input(|i| i.time);
    let music = st.music_dir.clone();
    let presets: Vec<(String, String)> = st
        .instrument
        .catalogue
        .entries(music.as_deref(), now)
        .iter()
        .map(|l| (l.name.clone(), l.title.clone()))
        .collect();
    egui::Frame::new()
        .fill(BG2)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let names: Vec<String> = st.song.tracks.iter().map(|t| t.name.clone()).collect();
                let mut sel = t;
                egui::ComboBox::from_id_salt("inst-track")
                    .width(120.0)
                    .selected_text(names[t].clone())
                    .show_ui(ui, |ui| {
                        for (i, n) in names.iter().enumerate() {
                            ui.selectable_value(&mut sel, i, n);
                        }
                    });
                st.sel.track = sel;
                ui.label(egui::RichText::new(st.song.tracks[t].instrument.kind_name()).color(DIM));
                ui.add_space(12.0);
                ui.label(egui::RichText::new("Preset").color(FAINT));
                let dir = st.music_dir.as_deref().map(library::instruments_dir);
                egui::ComboBox::from_id_salt("inst-preset")
                    .width(130.0)
                    .selected_text("Load...")
                    .show_ui(ui, |ui| {
                        for (name, title) in &presets {
                            if ui.selectable_label(false, title).clicked() {
                                if let Some(m) = &music {
                                    match library::load_instrument(m, name) {
                                        Ok(inst) => {
                                            st.song.tracks[t].instrument = inst;
                                            st.instrument.preset_name = name.clone();
                                            crate::swap::relink(st);
                                            st.say(
                                                format!("Loaded preset {title}"),
                                                crate::app::Tone::Info,
                                            );
                                        }
                                        Err(e) => st.say(e, crate::app::Tone::Bad),
                                    }
                                }
                            }
                        }
                    });
                ui.add(
                    egui::TextEdit::singleline(&mut st.instrument.preset_name)
                        .desired_width(110.0)
                        .hint_text("preset name"),
                );
                if ui.button("Save preset").clicked() {
                    if let Some(d) = &dir {
                        let name = files::file_stem(&st.instrument.preset_name);
                        match files::save_preset(d, &name, &st.song.tracks[t].instrument) {
                            Ok(p) => {
                                st.say(format!("Saved {}", p.display()), crate::app::Tone::Good);
                                st.instrument.catalogue.stale();
                            }
                            Err(e) => st.say(e, crate::app::Tone::Bad),
                        }
                    }
                }
                ui.add_space(12.0);
                let (r, resp) = ui.allocate_exact_size(vec2(60.0, 20.0), Sense::click_and_drag());
                let down = resp.is_pointer_button_down_on();
                ui.painter().rect(
                    r,
                    CornerRadius::same(2),
                    if down { ACCENT } else { theme::BG3 },
                    Stroke::new(1.0, theme::line(30)),
                    egui::StrokeKind::Inside,
                );
                ui.painter().text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    "Play C4",
                    theme::font_semi(12.0),
                    TEXT,
                );
                if resp.drag_started() || resp.clicked() {
                    let now = ui.input(|i| i.time);
                    play(st, t, 60, now, 0.6);
                }
            });
        });
}

fn play(st: &mut Studio, track: usize, key: u8, now: f64, hold: f64) {
    if let Some((t, k, _)) = st.instrument.sounding.take() {
        st.send(Command::NoteOff { track: t, key: k });
    }
    st.send(Command::NoteOn {
        track,
        key,
        vel: 110,
    });
    st.instrument.sounding = Some((track, key, now + hold));
}

// -- recorded ---------------------------------------------------------------------

fn sampler(ui: &mut egui::Ui, s: &mut mc_music::Sampler) {
    let title = s
        .bank
        .0
        .as_ref()
        .map_or_else(|| format!("{} (not found)", s.set), |b| b.title.clone());
    group(ui, "Recording", |ui| {
        ui.label(egui::RichText::new(title).color(TEXT));
        ui.horizontal(|ui| {
            ui.add(
                Knob::new(&mut s.offset, 0.0, 0.5, 0.0, "Skip start")
                    .unit("s")
                    .decimals(3),
            );
            knob_i32(ui, &mut s.transpose, -24, 24, 0, "Transpose");
        });
    });
    group(ui, "Shape", |ui| {
        ui.horizontal(|ui| {
            ui.add(
                Knob::new(&mut s.amp.a, 0.001, 4.0, 0.004, "Swell in")
                    .unit("s")
                    .decimals(3)
                    .log(),
            );
            ui.add(
                Knob::new(&mut s.amp.r, 0.01, 6.0, 0.35, "Release")
                    .unit("s")
                    .decimals(2)
                    .log(),
            );
        });
    });
    group(ui, "Tone and level", |ui| {
        ui.horizontal(|ui| {
            ui.add(
                Knob::new(&mut s.cutoff, 0.0, 16000.0, 0.0, "Darken")
                    .unit("Hz")
                    .decimals(0),
            );
            ui.add(
                Knob::new(&mut s.soften, 0.0, 4.0, 0.0, "Soft = dark")
                    .unit("oct")
                    .decimals(1),
            );
            knob(ui, &mut s.velocity, 0.0, 1.0, 0.5, "Velocity");
            ui.add(Knob::new(&mut s.gain, 0.0, 2.0, 1.0, "Gain"));
            knob_u32(ui, &mut s.voices, 1, 48, 12, "Voices");
        });
    });
}

/// A library instrument: shown by name; editing makes the part its own copy.
fn library_instrument(ui: &mut egui::Ui, st: &mut Studio, name: &str) -> Option<Instrument> {
    let now = ui.input(|i| i.time);
    let title = crate::swap::label(st, &Instrument::Use(name.to_string()), now);
    let mut copy = None;
    group(ui, "From the library", |ui| {
        ui.label(egui::RichText::new(title).color(TEXT));
        ui.label(
            egui::RichText::new(format!("data/music/instruments/{name}.ron"))
                .color(FAINT)
                .small(),
        );
        if ui
            .button("Edit a copy for this part")
            .on_hover_text("The part gets its own settings; the library is not changed")
            .clicked()
        {
            copy = st
                .music_dir
                .as_deref()
                .and_then(|m| library::load_instrument(m, name).ok());
        }
    });
    copy
}

// -- synth ------------------------------------------------------------------------

fn synth(ui: &mut egui::Ui, st: &mut Studio, s: &mut mc_music::Synth) {
    group(ui, "Oscillators", |ui| {
        let mut remove = None;
        for (i, o) in s.oscs.iter_mut().enumerate() {
            ui.push_id(("osc", i), |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{}", i + 1))
                            .font(theme::font_light(20.0))
                            .color(DIM),
                    );
                    osc(ui, o);
                    if tiny_icon(ui, Icon::Close, "Remove this oscillator").clicked() {
                        remove = Some(i);
                    }
                });
            });
        }
        if let Some(i) = remove {
            if s.oscs.len() > 1 {
                s.oscs.remove(i);
            }
        }
        if s.oscs.len() < 3 && ui.small_button("+ Oscillator").clicked() {
            s.oscs.push(Osc::new(Wave::Saw, 0.4));
        }
    });
    ui.add_space(6.0);
    ui.horizontal_top(|ui| {
        group(ui, "Filter", |ui| filter(ui, &mut s.filter));
        group(ui, "Amp envelope", |ui| {
            env_editor(ui, "amp-env", &mut s.amp, ACCENT)
        });
        group(ui, "Mod envelope", |ui| {
            env_editor(ui, "mod-env", &mut s.mod_env, WARN);
            ui.label(
                egui::RichText::new("Opens the filter and sets FM depth")
                    .color(FAINT)
                    .small(),
            );
        });
    });
    ui.add_space(6.0);
    ui.horizontal_top(|ui| {
        group(ui, "LFOs", |ui| {
            let mut remove = None;
            for (i, l) in s.lfos.iter_mut().enumerate() {
                ui.push_id(("lfo", i), |ui| {
                    ui.horizontal(|ui| {
                        lfo(ui, l);
                        if tiny_icon(ui, Icon::Close, "Remove this LFO").clicked() {
                            remove = Some(i);
                        }
                    });
                });
            }
            if let Some(i) = remove {
                s.lfos.remove(i);
            }
            if s.lfos.len() < 2 && ui.small_button("+ LFO").clicked() {
                s.lfos.push(Lfo {
                    shape: LfoShape::Sine,
                    to: LfoTo::Cutoff,
                    rate: 2.0,
                    sync: false,
                    amount: 0.5,
                    delay: 0.0,
                    retrigger: false,
                });
            }
            if s.lfos.is_empty() {
                ui.label(egui::RichText::new("None").color(FAINT).small());
            }
        });
        group(ui, "Voice", |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    Knob::new(&mut s.glide, 0.0, 1.0, 0.0, "Glide")
                        .unit("s")
                        .decimals(3),
                );
                knob_u32(ui, &mut s.voices, 1, 32, 12, "Voices");
                knob(ui, &mut s.velocity, 0.0, 1.0, 0.5, "Velocity");
                ui.add(Knob::new(&mut s.gain, 0.0, 2.0, 1.0, "Gain"));
                ui.add(
                    Knob::new(&mut s.punch, 0.0, 24.0, 0.0, "Punch")
                        .unit("st")
                        .decimals(1),
                );
                ui.add(
                    Knob::new(&mut s.punch_time, 0.001, 0.5, 0.0, "Punch time")
                        .unit("s")
                        .decimals(3)
                        .log(),
                );
            });
            if widgets::toggle(ui, s.mono, "Mono (legato)", ACCENT).clicked() {
                s.mono = !s.mono;
            }
        });
    });
    let _ = st;
}

fn osc(ui: &mut egui::Ui, o: &mut Osc) {
    egui::ComboBox::from_id_salt("wave")
        .width(76.0)
        .selected_text(o.wave.name())
        .show_ui(ui, |ui| {
            for w in Wave::ALL {
                ui.selectable_value(&mut o.wave, w, w.name());
            }
        });
    knob(ui, &mut o.gain, 0.0, 1.0, 0.5, "Gain");
    knob_i32(ui, &mut o.octave, -4, 4, 0, "Octave");
    knob_i32(ui, &mut o.semi, -12, 12, 0, "Semi");
    ui.add(
        Knob::new(&mut o.fine, -100.0, 100.0, 0.0, "Fine")
            .unit("ct")
            .decimals(0),
    );
    let shape_label = match o.wave {
        Wave::Pulse => "Width",
        Wave::Fm => "Depth",
        Wave::Fold => "Fold",
        _ => "Shape",
    };
    knob(ui, &mut o.shape, 0.0, 1.0, 0.5, shape_label);
    ui.add(
        Knob::new(&mut o.ratio, 0.25, 16.0, 1.0, "Ratio")
            .decimals(2)
            .log(),
    );
    knob_u32(ui, &mut o.unison, 1, 16, 1, "Unison");
    ui.add(
        Knob::new(&mut o.detune, 0.0, 100.0, 0.0, "Detune")
            .unit("ct")
            .decimals(0),
    );
    knob(ui, &mut o.width, 0.0, 1.0, 0.0, "Width");
    if widgets::toggle(ui, o.retrigger, "Retrig", ACCENT)
        .on_hover_text("Start every note at the same phase")
        .clicked()
    {
        o.retrigger = !o.retrigger;
    }
}

fn filter(ui: &mut egui::Ui, f: &mut Filter) {
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("filter-mode")
            .width(96.0)
            .selected_text(f.mode.name())
            .show_ui(ui, |ui| {
                for m in FilterMode::ALL {
                    ui.selectable_value(&mut f.mode, m, m.name());
                }
            });
    });
    response_graph(ui, f, vec2(250.0, 84.0));
    ui.horizontal(|ui| {
        ui.add(
            Knob::new(&mut f.cutoff, 20.0, 20000.0, 2400.0, "Cutoff")
                .unit("Hz")
                .decimals(0)
                .log(),
        );
        knob(ui, &mut f.resonance, 0.0, 1.0, 0.0, "Reso");
        ui.add(
            Knob::new(&mut f.env, -6.0, 6.0, 0.0, "Env")
                .unit("oct")
                .decimals(1),
        );
        knob(ui, &mut f.keytrack, 0.0, 1.0, 0.0, "Key");
        ui.add(
            Knob::new(&mut f.velocity, 0.0, 4.0, 0.0, "Vel")
                .unit("oct")
                .decimals(1),
        );
        knob(ui, &mut f.drive, 0.0, 1.0, 0.0, "Drive");
        if f.mode == FilterMode::Formant {
            let vowel = ["ah", "eh", "ee", "oh", "oo"][((f.vowel * 4.0).round() as usize).min(4)];
            let label = format!("Vowel {vowel}");
            ui.add(
                Knob::new(&mut f.vowel, 0.0, 1.0, 0.0, &label)
                    .decimals(2)
                    .colour(WARN),
            );
        }
    });
}

/// A 2-pole state-variable filter's magnitude, analog prototype, as the synth
/// uses it (damping from resonance as in `Svf::set`).
fn svf_mag(mode: FilterMode, f: f32, fc: f32, res: f32) -> f32 {
    let k = 2.0 - 1.95 * res.clamp(0.0, 1.0);
    let w = f / fc.max(1.0);
    let den = ((1.0 - w * w).powi(2) + (k * w).powi(2)).sqrt().max(1e-9);
    match mode {
        FilterMode::LowPass => 1.0 / den,
        FilterMode::LowPass4 => {
            let flat = 1.0 / ((1.0 - w * w).powi(2) + (2.0 * w).powi(2)).sqrt().max(1e-9);
            flat / den
        }
        FilterMode::HighPass => w * w / den,
        FilterMode::BandPass => k * w / den,
        FilterMode::Notch => (1.0 - w * w).abs() / den,
        // Drawn by `formant_mag`.
        FilterMode::Formant => 1.0,
    }
}

/// Roughly what the voice filter does: three resonances at a man's formants
/// for the vowel, blended between vowels and scaled by cutoff / 1000. For
/// drawing only.
fn formant_mag(f: f32, filter: &Filter) -> f32 {
    let table = [
        [700.0f32, 1220.0, 2600.0],
        [530.0, 1840.0, 2480.0],
        [270.0, 2290.0, 3010.0],
        [570.0, 840.0, 2410.0],
        [300.0, 870.0, 2240.0],
    ];
    let x = filter.vowel.clamp(0.0, 1.0) * 4.0;
    let i = (x.floor() as usize).min(3);
    let t = x - i as f32;
    let scale = filter.cutoff / 1000.0;
    let mut m = 0.0f32;
    for b in 0..3 {
        let fc = (table[i][b] + (table[i + 1][b] - table[i][b]) * t) * scale;
        let k = 0.25 - 0.2 * filter.resonance.clamp(0.0, 1.0) + 0.05;
        let w = f / fc.max(1.0);
        let den = ((1.0 - w * w).powi(2) + (k * w).powi(2)).sqrt().max(1e-9);
        m += (k * w / den) * [1.0, 0.6, 0.35][b];
    }
    m
}

fn response_graph(ui: &mut egui::Ui, f: &Filter, size: egui::Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(3), BG0);
    let g = rect.shrink(4.0);
    let (lo, hi) = (20.0f32.ln(), 20000.0f32.ln());
    let x_of = |hz: f32| g.left() + (hz.ln() - lo) / (hi - lo) * g.width();
    let y_of = |db: f32| g.top() + (18.0 - db.clamp(-36.0, 18.0)) / 54.0 * g.height();
    for hz in [100.0, 1000.0, 10000.0] {
        p.vline(x_of(hz), g.y_range(), Stroke::new(1.0, theme::line(12)));
    }
    p.hline(g.x_range(), y_of(0.0), Stroke::new(1.0, theme::line(22)));
    let n = 96;
    let mut pts = Vec::with_capacity(n);
    for i in 0..n {
        let hz = (lo + (hi - lo) * i as f32 / (n - 1) as f32).exp();
        let m = if f.mode == FilterMode::Formant {
            formant_mag(hz, f)
        } else {
            svf_mag(f.mode, hz, f.cutoff, f.resonance)
        };
        let db = 20.0 * m.max(1e-6).log10();
        pts.push(pos2(x_of(hz), y_of(db)));
    }
    let mut fill = pts.clone();
    fill.push(pos2(g.right(), g.bottom()));
    fill.push(pos2(g.left(), g.bottom()));
    let mut mesh = egui::Mesh::default();
    let c = theme::with_alpha(ACCENT, 30);
    for w in pts.windows(2) {
        let b = mesh.vertices.len() as u32;
        mesh.colored_vertex(w[0], c);
        mesh.colored_vertex(w[1], c);
        mesh.colored_vertex(pos2(w[1].x, g.bottom()), c);
        mesh.colored_vertex(pos2(w[0].x, g.bottom()), c);
        mesh.add_triangle(b, b + 1, b + 2);
        mesh.add_triangle(b, b + 2, b + 3);
    }
    let clip = p.with_clip_rect(g);
    clip.add(Shape::mesh(mesh));
    clip.add(Shape::line(pts, Stroke::new(1.5, ACCENT)));
    if f.mode != FilterMode::Formant {
        let x = x_of(f.cutoff.clamp(20.0, 20000.0));
        clip.vline(
            x,
            g.y_range(),
            Stroke::new(1.0, theme::with_alpha(ACCENT, 90)),
        );
    }
    p.text(
        g.left_top(),
        Align2::LEFT_TOP,
        "response",
        theme::font_body(9.5),
        FAINT,
    );
}

fn env_editor(ui: &mut egui::Ui, id: &str, env: &mut Env, colour: egui::Color32) {
    widgets::envelope(ui, egui::Id::new(id), env, vec2(220.0, 84.0), colour);
    ui.horizontal(|ui| {
        ui.add(
            Knob::new(&mut env.a, 0.0005, 10.0, 0.005, "Attack")
                .unit("s")
                .decimals(3)
                .log()
                .size(26.0),
        );
        ui.add(
            Knob::new(&mut env.d, 0.0005, 10.0, 0.3, "Decay")
                .unit("s")
                .decimals(3)
                .log()
                .size(26.0),
        );
        ui.add(
            Knob::new(&mut env.s, 0.0, 1.0, 0.7, "Sustain")
                .decimals(2)
                .size(26.0),
        );
        ui.add(
            Knob::new(&mut env.r, 0.0005, 10.0, 0.25, "Release")
                .unit("s")
                .decimals(3)
                .log()
                .size(26.0),
        );
    });
}

fn lfo(ui: &mut egui::Ui, l: &mut Lfo) {
    egui::ComboBox::from_id_salt("lfo-shape")
        .width(96.0)
        .selected_text(l.shape.name())
        .show_ui(ui, |ui| {
            for s in LfoShape::ALL {
                ui.selectable_value(&mut l.shape, s, s.name());
            }
        });
    ui.label(egui::RichText::new("to").color(FAINT));
    egui::ComboBox::from_id_salt("lfo-to")
        .width(70.0)
        .selected_text(l.to.name())
        .show_ui(ui, |ui| {
            for t in LfoTo::ALL {
                ui.selectable_value(&mut l.to, t, t.name());
            }
        });
    if l.sync {
        ui.add(
            Knob::new(&mut l.rate, 0.0625, 8.0, 1.0, "Per beat")
                .decimals(3)
                .log(),
        );
    } else {
        ui.add(
            Knob::new(&mut l.rate, 0.01, 40.0, 2.0, "Rate")
                .unit("Hz")
                .decimals(2)
                .log(),
        );
    }
    if widgets::toggle(ui, l.sync, "Sync", ACCENT)
        .on_hover_text("Rate in cycles per beat")
        .clicked()
    {
        l.sync = !l.sync;
    }
    let (min, max, unit) = match l.to {
        LfoTo::Pitch => (-12.0, 12.0, "st"),
        LfoTo::Cutoff => (-4.0, 4.0, "oct"),
        _ => (-1.0, 1.0, ""),
    };
    ui.add(
        Knob::new(&mut l.amount, min, max, 0.0, "Amount")
            .unit(unit)
            .decimals(2),
    );
    ui.add(
        Knob::new(&mut l.delay, 0.0, 4.0, 0.0, "Delay")
            .unit("s")
            .decimals(2),
    );
    if widgets::toggle(ui, l.retrigger, "Retrig", ACCENT).clicked() {
        l.retrigger = !l.retrigger;
    }
}

// -- kit --------------------------------------------------------------------------

fn kit(ui: &mut egui::Ui, st: &mut Studio, track: usize, k: &mut mc_music::Kit) {
    let now = ui.input(|i| i.time);
    let sel = st.sel.drum.min(k.drums.len().saturating_sub(1));
    ui.horizontal_top(|ui| {
        // The drum list.
        ui.vertical(|ui| {
            ui.set_width(180.0);
            ui.horizontal(|ui| {
                widgets::caption(ui, "Drums");
                if ui.small_button("Add").clicked() {
                    let key = k
                        .drums
                        .iter()
                        .map(|d| d.key)
                        .max()
                        .map(|m| m.saturating_add(1))
                        .unwrap_or(36);
                    let name = unique_name("Drum", |n| k.drums.iter().any(|d| d.name == n));
                    k.drums.push(blank_drum(&name, key));
                    st.sel.drum = k.drums.len() - 1;
                }
                if !k.drums.is_empty() && ui.small_button("Duplicate").clicked() {
                    let mut d = k.drums[sel].clone();
                    d.name = unique_name(&d.name, |n| k.drums.iter().any(|x| x.name == n));
                    d.key = d.key.saturating_add(1);
                    k.drums.insert(sel + 1, d);
                    st.sel.drum = sel + 1;
                }
            });
            for (i, d) in k.drums.iter().enumerate() {
                let w = 180.0;
                let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
                let p = ui.painter();
                if i == sel {
                    p.rect_filled(r, CornerRadius::same(2), theme::with_alpha(ACCENT, 40));
                    p.vline(r.left() + 1.0, r.y_range(), Stroke::new(2.0, ACCENT));
                } else if resp.hovered() {
                    p.rect_filled(r, CornerRadius::same(2), theme::line(8));
                }
                p.text(
                    r.left_center() + vec2(8.0, 0.0),
                    Align2::LEFT_CENTER,
                    &d.name,
                    theme::font_semi(12.5),
                    if i == sel { TEXT } else { DIM },
                );
                p.text(
                    r.right_center() - vec2(6.0, 0.0),
                    Align2::RIGHT_CENTER,
                    key_name(d.key),
                    theme::font_body(10.0),
                    FAINT,
                );
                if resp.clicked() {
                    st.sel.drum = i;
                    play(st, track, d.key, now, 0.3);
                }
            }
        });
        ui.add_space(8.0);
        if let Some(d) = k.drums.get_mut(sel) {
            let mut delete = false;
            ui.vertical(|ui| {
                drum(ui, st, track, d, &mut delete, now);
            });
            if delete {
                k.drums.remove(sel);
                st.sel.drum = sel.saturating_sub(1);
            }
        }
    });
}

fn blank_drum(name: &str, key: u8) -> Drum {
    Drum {
        name: name.into(),
        key,
        body: Some(Body {
            from: 200.0,
            to: 60.0,
            sweep: 0.03,
            decay: 0.3,
            gain: 1.0,
            overtone: 0.0,
        }),
        hiss: None,
        ring: None,
        click: 0.0,
        drive: 0.0,
        gain: 1.0,
        pan: 0.0,
        choke: 0,
        velocity: 0.5,
        fixed: true,
    }
}

fn drum(
    ui: &mut egui::Ui,
    st: &mut Studio,
    track: usize,
    d: &mut Drum,
    delete: &mut bool,
    now: f64,
) {
    group(ui, "", |ui| {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(120.0));
            ui.label(egui::RichText::new("key").color(FAINT));
            ui.add(
                egui::DragValue::new(&mut d.key)
                    .range(0..=127)
                    .custom_formatter(|v, _| format!("{} ({})", v, key_name(v as u8))),
            );
            if ui.button("Play").clicked() {
                play(st, track, d.key, now, 0.3);
            }
            if ui.small_button("Delete").clicked() {
                *delete = true;
            }
        });
        ui.horizontal(|ui| {
            knob(ui, &mut d.click, 0.0, 1.0, 0.0, "Click");
            knob(ui, &mut d.drive, 0.0, 1.0, 0.0, "Drive");
            ui.add(Knob::new(&mut d.gain, 0.0, 2.0, 1.0, "Gain"));
            knob(ui, &mut d.pan, -1.0, 1.0, 0.0, "Pan");
            let mut choke = d.choke as u32;
            knob_u32(ui, &mut choke, 0, 8, 0, "Choke");
            d.choke = choke as u8;
            knob(ui, &mut d.velocity, 0.0, 1.0, 0.5, "Velocity");
            if widgets::toggle(ui, !d.fixed, "Tuned", ACCENT)
                .on_hover_text("Plays at the note's pitch relative to its key (a tuned 808 or tom)")
                .clicked()
            {
                d.fixed = !d.fixed;
            }
        });
    });
    ui.add_space(4.0);
    ui.horizontal_top(|ui| {
        part(
            ui,
            "Body",
            "A sine falling in pitch",
            &mut d.body,
            || Body {
                from: 200.0,
                to: 60.0,
                sweep: 0.03,
                decay: 0.3,
                gain: 1.0,
                overtone: 0.0,
            },
            |ui, b| {
                ui.horizontal(|ui| {
                    ui.add(
                        Knob::new(&mut b.from, 20.0, 8000.0, 200.0, "From")
                            .unit("Hz")
                            .decimals(0)
                            .log(),
                    );
                    ui.add(
                        Knob::new(&mut b.to, 20.0, 8000.0, 60.0, "To")
                            .unit("Hz")
                            .decimals(0)
                            .log(),
                    );
                    ui.add(
                        Knob::new(&mut b.sweep, 0.001, 1.0, 0.03, "Sweep")
                            .unit("s")
                            .decimals(3)
                            .log(),
                    );
                });
                ui.horizontal(|ui| {
                    ui.add(
                        Knob::new(&mut b.decay, 0.005, 4.0, 0.3, "Decay")
                            .unit("s")
                            .decimals(3)
                            .log(),
                    );
                    ui.add(Knob::new(&mut b.gain, 0.0, 2.0, 1.0, "Gain"));
                    ui.add(Knob::new(&mut b.overtone, 0.0, 8.0, 0.0, "Overtone").decimals(2));
                });
            },
        );
        part(
            ui,
            "Hiss",
            "Filtered noise; bursts make a clap",
            &mut d.hiss,
            || Hiss {
                mode: FilterMode::BandPass,
                cutoff: 3000.0,
                resonance: 0.2,
                attack: 0.0,
                decay: 0.15,
                gain: 0.7,
                bursts: 1,
                spread: 0.0,
            },
            |ui, h| {
                egui::ComboBox::from_id_salt("hiss-mode")
                    .width(96.0)
                    .selected_text(h.mode.name())
                    .show_ui(ui, |ui| {
                        for m in FilterMode::ALL {
                            ui.selectable_value(&mut h.mode, m, m.name());
                        }
                    });
                ui.horizontal(|ui| {
                    ui.add(
                        Knob::new(&mut h.cutoff, 20.0, 20000.0, 3000.0, "Cutoff")
                            .unit("Hz")
                            .decimals(0)
                            .log(),
                    );
                    knob(ui, &mut h.resonance, 0.0, 1.0, 0.2, "Reso");
                    ui.add(
                        Knob::new(&mut h.attack, 0.0, 0.2, 0.0, "Attack")
                            .unit("s")
                            .decimals(3),
                    );
                    ui.add(
                        Knob::new(&mut h.decay, 0.005, 4.0, 0.15, "Decay")
                            .unit("s")
                            .decimals(3)
                            .log(),
                    );
                });
                ui.horizontal(|ui| {
                    ui.add(Knob::new(&mut h.gain, 0.0, 2.0, 0.7, "Gain"));
                    knob_u32(ui, &mut h.bursts, 1, 8, 1, "Bursts");
                    ui.add(
                        Knob::new(&mut h.spread, 0.0, 0.05, 0.0, "Spread")
                            .unit("s")
                            .decimals(3),
                    );
                });
            },
        );
        part(
            ui,
            "Ring",
            "Six detuned squares: cymbals, bells",
            &mut d.ring,
            || Ring {
                freq: 420.0,
                decay: 0.2,
                gain: 0.5,
                highpass: 6000.0,
            },
            |ui, r| {
                ui.horizontal(|ui| {
                    ui.add(
                        Knob::new(&mut r.freq, 40.0, 4000.0, 420.0, "Pitch")
                            .unit("Hz")
                            .decimals(0)
                            .log(),
                    );
                    ui.add(
                        Knob::new(&mut r.decay, 0.005, 4.0, 0.2, "Decay")
                            .unit("s")
                            .decimals(3)
                            .log(),
                    );
                });
                ui.horizontal(|ui| {
                    ui.add(Knob::new(&mut r.gain, 0.0, 2.0, 0.5, "Gain"));
                    ui.add(
                        Knob::new(&mut r.highpass, 200.0, 16000.0, 6000.0, "High-pass")
                            .unit("Hz")
                            .decimals(0)
                            .log(),
                    );
                });
            },
        );
    });
}

/// A drum part that may be switched off (`None`).
fn part<T>(
    ui: &mut egui::Ui,
    title: &str,
    tip: &str,
    slot: &mut Option<T>,
    make: impl FnOnce() -> T,
    edit: impl FnOnce(&mut egui::Ui, &mut T),
) {
    group(ui, "", |ui| {
        ui.set_min_width(170.0);
        ui.horizontal(|ui| {
            widgets::caption(ui, title);
            let on = slot.is_some();
            if widgets::toggle(ui, on, if on { "On" } else { "Off" }, ACCENT)
                .on_hover_text(tip)
                .clicked()
            {
                *slot = if on { None } else { Some(make()) };
            }
        });
        match slot {
            Some(v) => ui.push_id(title, |ui| edit(ui, v)).inner,
            None => {
                ui.label(egui::RichText::new(tip).color(FAINT).small());
            }
        }
    });
}

// -- scope and spectrum -----------------------------------------------------------

fn analyser(ui: &mut egui::Ui, st: &mut Studio, rect: Rect) {
    let p = ui.painter();
    let scope_r = Rect::from_min_size(
        rect.min,
        vec2(rect.width(), (rect.height() * 0.42).min(150.0)),
    );
    let spec_r = Rect::from_min_max(pos2(rect.left(), scope_r.bottom() + 8.0), rect.max);
    let frames = st.meters.scope_ordered();
    draw_scope(p, scope_r, &frames);
    let mono: Vec<f32> = frames[frames.len().saturating_sub(4096)..]
        .iter()
        .map(|f| (f[0] + f[1]) * 0.5)
        .collect();
    let spec = fft::spectrum_db(&mono);
    let cols = spec_r.width().max(8.0) as usize / 2;
    let rate = st.audio.rate as f32;
    let (lo, hi) = (20.0f32.ln(), (rate * 0.5).min(20000.0).ln());
    let bins = spec.len();
    let mut cur = vec![-120.0f32; cols];
    for (c, v) in cur.iter_mut().enumerate() {
        let f0 = (lo + (hi - lo) * c as f32 / cols as f32).exp();
        let f1 = (lo + (hi - lo) * (c + 1) as f32 / cols as f32).exp();
        let b0 = ((f0 / (rate * 0.5)) * bins as f32) as usize;
        let b1 = (((f1 / (rate * 0.5)) * bins as f32) as usize)
            .max(b0 + 1)
            .min(bins);
        *v = spec[b0.min(bins - 1)..b1]
            .iter()
            .copied()
            .fold(-140.0, f32::max);
    }
    let sm = &mut st.instrument.spectrum;
    if sm.len() != cols {
        *sm = cur.clone();
    }
    for (s, c) in sm.iter_mut().zip(&cur) {
        *s = if *c > *s { *c } else { *s + (*c - *s) * 0.15 };
    }
    draw_spectrum(p, spec_r, &st.instrument.spectrum, None, "Spectrum, master");
}

pub fn draw_scope(p: &egui::Painter, r: Rect, frames: &[[f32; 2]]) {
    p.rect_filled(r, CornerRadius::same(3), BG0);
    p.hline(r.x_range(), r.center().y, Stroke::new(1.0, theme::line(14)));
    let n = frames.len();
    let window = 1024.min(n);
    // Trigger on a rising zero crossing so a steady tone stands still.
    let mut start = n.saturating_sub(window * 2);
    for i in (n.saturating_sub(window * 2)..n - window).rev() {
        let a = frames[i][0] + frames[i][1];
        let b = frames[i + 1][0] + frames[i + 1][1];
        if a <= 0.0 && b > 0.0 {
            start = i;
            break;
        }
    }
    let pts: Vec<egui::Pos2> = (0..window)
        .step_by(2)
        .map(|k| {
            let f = frames[(start + k).min(n - 1)];
            let v = ((f[0] + f[1]) * 0.5).clamp(-1.0, 1.0);
            pos2(
                r.left() + r.width() * k as f32 / window as f32,
                r.center().y - v * r.height() * 0.45,
            )
        })
        .collect();
    p.with_clip_rect(r)
        .add(Shape::line(pts, Stroke::new(1.2, ACCENT)));
    p.text(
        r.left_top() + vec2(5.0, 3.0),
        Align2::LEFT_TOP,
        "Scope",
        theme::font_body(9.5),
        FAINT,
    );
}

/// A spectrum as a filled curve; `other` is drawn as a line over it (a reference).
pub fn draw_spectrum(p: &egui::Painter, r: Rect, cols: &[f32], other: Option<&[f32]>, label: &str) {
    p.rect_filled(r, CornerRadius::same(3), BG0);
    let g = r.shrink(4.0);
    let y_of = |db: f32| g.top() + (-db.clamp(-96.0, 0.0) / 96.0) * g.height();
    for db in [-24.0, -48.0, -72.0] {
        p.hline(g.x_range(), y_of(db), Stroke::new(1.0, theme::line(10)));
    }
    let n = cols.len().max(2);
    let pts: Vec<egui::Pos2> = cols
        .iter()
        .enumerate()
        .map(|(i, v)| pos2(g.left() + g.width() * i as f32 / (n - 1) as f32, y_of(*v)))
        .collect();
    let mut mesh = egui::Mesh::default();
    for w in pts.windows(2) {
        let b = mesh.vertices.len() as u32;
        let top = theme::with_alpha(ACCENT, 150);
        let bottom = theme::with_alpha(theme::ACCENT_DEEP, 30);
        mesh.colored_vertex(w[0], top);
        mesh.colored_vertex(w[1], top);
        mesh.colored_vertex(pos2(w[1].x, g.bottom()), bottom);
        mesh.colored_vertex(pos2(w[0].x, g.bottom()), bottom);
        mesh.add_triangle(b, b + 1, b + 2);
        mesh.add_triangle(b, b + 2, b + 3);
    }
    let clip = p.with_clip_rect(g);
    clip.add(Shape::mesh(mesh));
    if let Some(o) = other {
        let m = o.len().max(2);
        let opts: Vec<egui::Pos2> = o
            .iter()
            .enumerate()
            .map(|(i, v)| pos2(g.left() + g.width() * i as f32 / (m - 1) as f32, y_of(*v)))
            .collect();
        clip.add(Shape::line(
            opts,
            Stroke::new(
                1.5,
                theme::with_alpha(egui::Color32::from_rgb(0x9C, 0xC8, 0xFF), 220),
            ),
        ));
    }
    for (hz, name) in [(100.0f32, "100"), (1000.0, "1k"), (10000.0, "10k")] {
        let x = g.left() + (hz.ln() - 20.0f32.ln()) / (20000.0f32.ln() - 20.0f32.ln()) * g.width();
        p.vline(x, g.y_range(), Stroke::new(1.0, theme::line(12)));
        p.text(
            pos2(x + 2.0, g.bottom() - 2.0),
            Align2::LEFT_BOTTOM,
            name,
            theme::font_body(9.0),
            FAINT,
        );
    }
    p.text(
        r.left_top() + vec2(5.0, 3.0),
        Align2::LEFT_TOP,
        label,
        theme::font_body(9.5),
        FAINT,
    );
}
