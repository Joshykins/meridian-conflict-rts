//! Editors for insert effects: a knob for every parameter of every kind, the
//! EQ's curve drawn from the engine's own `eq_response`, and the compressor's
//! sidechain source. The detail pane shows the selected strip's chain as a row
//! of chips above the open effect.

use crate::app::{chain, chain_mut, strip_name, Strip, Studio};
use crate::theme::{self, ACCENT, BG0, BG2, DIM, FAINT, TEXT, WARN};
use crate::widgets::{self, knob, Knob};
use eframe::egui::{self, pos2, vec2, Align2, CornerRadius, Sense, Shape, Stroke};
use mc_music::dsp::effects::eq_response;
use mc_music::patch::{Curve, FilterMode};
use mc_music::Effect;

/// The strip whose chain the pane shows: the selected effect's, else the selected track's.
fn strip_of(st: &Studio) -> Strip {
    st.sel.fx.map(|f| f.0).unwrap_or(Strip::Track(st.sel.track))
}

pub fn detail(ui: &mut egui::Ui, st: &mut Studio) {
    let strip = strip_of(st);
    if chain(&st.song, strip).is_none() {
        ui.label(egui::RichText::new("No strip selected").color(FAINT));
        return;
    }
    egui::Frame::new()
        .fill(BG2)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(strip_name(&st.song, strip))
                        .font(theme::font_semi(14.0))
                        .color(TEXT),
                );
                ui.label(egui::RichText::new("inserts").color(FAINT));
                let n = chain(&st.song, strip).map(|c| c.len()).unwrap_or(0);
                for i in 0..n {
                    let fx = &chain(&st.song, strip).unwrap()[i];
                    let on = fx.is_on();
                    let selected = st.sel.fx == Some((strip, i));
                    let label = format!("{} {}", i + 1, fx.name());
                    let r = widgets::toggle(ui, selected, &label, if on { ACCENT } else { DIM });
                    if r.clicked() {
                        st.sel.fx = Some((strip, i));
                    }
                }
                add_menu(ui, st, strip);
            });
        });
    let Some((s, i)) = st.sel.fx.filter(|f| f.0 == strip) else {
        ui.add_space(12.0);
        ui.label(egui::RichText::new("  Pick an insert above, or add one.").color(FAINT));
        return;
    };
    let tracks: Vec<String> = st.song.tracks.iter().map(|t| t.name.clone()).collect();
    let reduction = match s {
        Strip::Track(t) => st.meters.tracks.get(t).map(|m| m.reduction),
        Strip::Bus(b) => st.meters.buses.get(b).map(|m| m.reduction),
        Strip::Master => Some(st.meters.master.reduction),
    }
    .unwrap_or(0.0);
    let rate = st.audio.rate as f32;
    let tempo = st.song.tempo;
    let mut remove = false;
    let mut moved: Option<isize> = None;
    egui::ScrollArea::both()
        .id_salt("fx-detail")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(6.0);
            let Some(fx) = chain_mut(&mut st.song, s).and_then(|c| c.get_mut(i)) else {
                return;
            };
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(fx.name())
                        .font(theme::font_light(24.0))
                        .color(TEXT),
                );
                let on = fx.is_on();
                if widgets::toggle(ui, on, if on { "On" } else { "Bypassed" }, ACCENT).clicked() {
                    fx.set_on(!on);
                }
                if ui.small_button("Move left").clicked() {
                    moved = Some(-1);
                }
                if ui.small_button("Move right").clicked() {
                    moved = Some(1);
                }
                if ui.small_button("Remove").clicked() {
                    remove = true;
                }
            });
            ui.add_space(4.0);
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(8, 0))
                .show(ui, |ui| {
                    editor(ui, fx, &tracks, reduction, rate, tempo);
                });
        });
    if let Some(c) = chain_mut(&mut st.song, s) {
        if remove && i < c.len() {
            c.remove(i);
            st.sel.fx = if c.is_empty() {
                None
            } else {
                Some((s, i.min(c.len() - 1)))
            };
        } else if let Some(d) = moved {
            let j = i as isize + d;
            if j >= 0 && (j as usize) < c.len() {
                c.swap(i, j as usize);
                st.sel.fx = Some((s, j as usize));
            }
        }
    }
}

pub fn add_menu(ui: &mut egui::Ui, st: &mut Studio, strip: Strip) {
    ui.menu_button("+ Insert", |ui| {
        for fx in Effect::defaults() {
            if ui.button(fx.name()).clicked() {
                if let Some(c) = chain_mut(&mut st.song, strip) {
                    c.push(fx);
                    st.sel.fx = Some((strip, c.len() - 1));
                    st.detail = crate::app::Detail::Effect;
                    st.detail_open = true;
                }
                ui.close();
            }
        }
    });
}

/// Knobs for one effect. `reduction` is the strip's live gain reduction, dB.
pub fn editor(
    ui: &mut egui::Ui,
    fx: &mut Effect,
    tracks: &[String],
    reduction: f32,
    rate: f32,
    tempo: f32,
) {
    match fx {
        Effect::Eq { .. } => {
            let snapshot = fx.clone();
            eq_curve(ui, &snapshot, rate);
            eq_knobs(ui, fx);
        }
        Effect::Filter {
            mode,
            cutoff,
            resonance,
            ..
        } => {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("fx-filter-mode")
                    .width(100.0)
                    .selected_text(mode.name())
                    .show_ui(ui, |ui| {
                        for m in FilterMode::ALL {
                            ui.selectable_value(mode, m, m.name());
                        }
                    });
                ui.add(
                    Knob::new(cutoff, 20.0, 20000.0, 3000.0, "Cutoff")
                        .unit("Hz")
                        .decimals(0)
                        .log(),
                );
                knob(ui, resonance, 0.0, 1.0, 0.2, "Resonance");
            });
        }
        Effect::Drive {
            amount,
            curve,
            tone,
            mix,
            ..
        } => {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("fx-curve")
                    .width(80.0)
                    .selected_text(curve.name())
                    .show_ui(ui, |ui| {
                        for c in Curve::ALL {
                            ui.selectable_value(curve, c, c.name());
                        }
                    });
                knob(ui, amount, 0.0, 1.0, 0.3, "Amount");
                ui.add(
                    Knob::new(tone, 500.0, 20000.0, 9000.0, "Tone")
                        .unit("Hz")
                        .decimals(0)
                        .log(),
                );
                knob(ui, mix, 0.0, 1.0, 1.0, "Mix");
            });
        }
        Effect::Chorus {
            rate: r,
            depth,
            mix,
            ..
        } => {
            ui.horizontal(|ui| {
                ui.add(
                    Knob::new(r, 0.02, 8.0, 0.4, "Rate")
                        .unit("Hz")
                        .decimals(2)
                        .log(),
                );
                ui.add(
                    Knob::new(depth, 0.0, 20.0, 4.0, "Depth")
                        .unit("ms")
                        .decimals(1),
                );
                knob(ui, mix, 0.0, 1.0, 0.4, "Mix");
            });
        }
        Effect::Delay {
            beats,
            feedback,
            mix,
            pingpong,
            tone,
            ..
        } => {
            ui.horizontal(|ui| {
                ui.add(
                    Knob::new(beats, 0.0625, 4.0, 0.75, "Time")
                        .unit("beats")
                        .decimals(3)
                        .log(),
                );
                egui::ComboBox::from_id_salt("fx-delay-note")
                    .width(70.0)
                    .selected_text("Note")
                    .show_ui(ui, |ui| {
                        for (name, b) in [
                            ("1/16", 0.25),
                            ("1/8", 0.5),
                            ("Dotted 1/8", 0.75),
                            ("1/4", 1.0),
                            ("Dotted 1/4", 1.5),
                            ("1/2", 2.0),
                            ("1/8 triplet", 1.0 / 3.0),
                        ] {
                            if ui.selectable_label(false, name).clicked() {
                                *beats = b;
                            }
                        }
                    });
                knob(ui, feedback, 0.0, 0.95, 0.35, "Feedback");
                knob(ui, mix, 0.0, 1.0, 0.25, "Mix");
                ui.add(
                    Knob::new(tone, 500.0, 20000.0, 4500.0, "Tone")
                        .unit("Hz")
                        .decimals(0)
                        .log(),
                );
                if widgets::toggle(ui, *pingpong, "Ping-pong", ACCENT).clicked() {
                    *pingpong = !*pingpong;
                }
            });
            ui.label(
                egui::RichText::new(format!(
                    "{:.0} ms at {:.0} bpm",
                    *beats * 60000.0 / tempo.max(1.0),
                    tempo
                ))
                .color(FAINT)
                .small(),
            );
        }
        Effect::Reverb {
            size,
            decay,
            damp,
            predelay,
            mix,
            lowcut,
            ..
        } => {
            ui.horizontal(|ui| {
                knob(ui, size, 0.0, 1.0, 0.7, "Size");
                ui.add(
                    Knob::new(decay, 0.2, 20.0, 2.8, "Decay")
                        .unit("s")
                        .decimals(2)
                        .log(),
                );
                ui.add(
                    Knob::new(damp, 500.0, 20000.0, 5000.0, "Damping")
                        .unit("Hz")
                        .decimals(0)
                        .log(),
                );
                ui.add(
                    Knob::new(predelay, 0.0, 200.0, 20.0, "Pre-delay")
                        .unit("ms")
                        .decimals(0),
                );
                ui.add(
                    Knob::new(lowcut, 20.0, 1000.0, 180.0, "Low cut")
                        .unit("Hz")
                        .decimals(0)
                        .log(),
                );
                knob(ui, mix, 0.0, 1.0, 0.3, "Mix");
            });
        }
        Effect::Compressor {
            threshold,
            ratio,
            attack,
            release,
            makeup,
            sidechain,
            ..
        } => {
            ui.horizontal(|ui| {
                ui.add(
                    Knob::new(threshold, -60.0, 0.0, -18.0, "Threshold")
                        .unit("dB")
                        .decimals(1),
                );
                ui.add(Knob::new(ratio, 1.0, 20.0, 4.0, "Ratio").decimals(1).log());
                ui.add(
                    Knob::new(attack, 0.1, 200.0, 10.0, "Attack")
                        .unit("ms")
                        .decimals(1)
                        .log(),
                );
                ui.add(
                    Knob::new(release, 10.0, 2000.0, 120.0, "Release")
                        .unit("ms")
                        .decimals(0)
                        .log(),
                );
                ui.add(
                    Knob::new(makeup, 0.0, 24.0, 0.0, "Makeup")
                        .unit("dB")
                        .decimals(1),
                );
                gr_meter(ui, reduction);
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Sidechain").color(FAINT));
                let text = sidechain.clone().unwrap_or_else(|| "Its own signal".into());
                egui::ComboBox::from_id_salt("fx-sidechain")
                    .width(140.0)
                    .selected_text(text)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(sidechain, None, "Its own signal");
                        for t in tracks {
                            ui.selectable_value(sidechain, Some(t.clone()), t);
                        }
                    });
                ui.label(
                    egui::RichText::new("Duck to another track (a kick pumping a pad)")
                        .color(FAINT)
                        .small(),
                );
            });
        }
        Effect::Limiter {
            ceiling,
            gain,
            release,
            ..
        } => {
            ui.horizontal(|ui| {
                ui.add(
                    Knob::new(ceiling, -12.0, 0.0, -1.0, "Ceiling")
                        .unit("dB")
                        .decimals(1),
                );
                ui.add(
                    Knob::new(gain, 0.0, 24.0, 0.0, "Gain")
                        .unit("dB")
                        .decimals(1),
                );
                ui.add(
                    Knob::new(release, 5.0, 500.0, 80.0, "Release")
                        .unit("ms")
                        .decimals(0)
                        .log(),
                );
                gr_meter(ui, reduction);
            });
        }
        Effect::Width { amount, .. } => {
            ui.horizontal(|ui| {
                ui.add(Knob::new(amount, 0.0, 2.0, 1.0, "Width").decimals(2));
                ui.label(
                    egui::RichText::new("0 mono, 1 as is, 2 twice as wide")
                        .color(FAINT)
                        .small(),
                );
            });
        }
        Effect::Crush {
            bits, rate: r, mix, ..
        } => {
            ui.horizontal(|ui| {
                ui.add(Knob::new(bits, 1.0, 16.0, 10.0, "Bits").decimals(1));
                ui.add(
                    Knob::new(r, 500.0, 48000.0, 16000.0, "Rate")
                        .unit("Hz")
                        .decimals(0)
                        .log(),
                );
                knob(ui, mix, 0.0, 1.0, 1.0, "Mix");
            });
        }
    }
}

fn eq_knobs(ui: &mut egui::Ui, fx: &mut Effect) {
    let Effect::Eq {
        low_db,
        low_hz,
        mid_db,
        mid_hz,
        mid_q,
        high_db,
        high_hz,
        cut_hz,
        ..
    } = fx
    else {
        return;
    };
    ui.horizontal(|ui| {
        widgets::group(ui, "Low shelf", |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    Knob::new(low_db, -18.0, 18.0, 0.0, "Gain")
                        .unit("dB")
                        .decimals(1),
                );
                ui.add(
                    Knob::new(low_hz, 20.0, 1000.0, 120.0, "Freq")
                        .unit("Hz")
                        .decimals(0)
                        .log(),
                );
            });
        });
        widgets::group(ui, "Bell", |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    Knob::new(mid_db, -18.0, 18.0, 0.0, "Gain")
                        .unit("dB")
                        .decimals(1),
                );
                ui.add(
                    Knob::new(mid_hz, 80.0, 12000.0, 1000.0, "Freq")
                        .unit("Hz")
                        .decimals(0)
                        .log(),
                );
                ui.add(Knob::new(mid_q, 0.2, 8.0, 0.8, "Q").decimals(2).log());
            });
        });
        widgets::group(ui, "High shelf", |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    Knob::new(high_db, -18.0, 18.0, 0.0, "Gain")
                        .unit("dB")
                        .decimals(1),
                );
                ui.add(
                    Knob::new(high_hz, 1000.0, 18000.0, 6000.0, "Freq")
                        .unit("Hz")
                        .decimals(0)
                        .log(),
                );
            });
        });
        widgets::group(ui, "High-pass", |ui| {
            ui.add(
                Knob::new(cut_hz, 0.0, 500.0, 0.0, "Cut")
                    .unit("Hz")
                    .decimals(0),
            )
            .on_hover_text("0 = off");
        });
    });
}

fn eq_curve(ui: &mut egui::Ui, fx: &Effect, rate: f32) {
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width().min(620.0), 120.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(3), BG0);
    let g = rect.shrink(6.0);
    let (lo, hi) = (20.0f32.ln(), 20000.0f32.ln());
    let x_of = |hz: f32| g.left() + (hz.ln() - lo) / (hi - lo) * g.width();
    let y_of = |db: f32| g.center().y - db.clamp(-20.0, 20.0) / 20.0 * g.height() * 0.5;
    for db in [-12.0, -6.0, 6.0, 12.0] {
        p.hline(g.x_range(), y_of(db), Stroke::new(1.0, theme::line(8)));
        p.text(
            pos2(g.left() + 2.0, y_of(db)),
            Align2::LEFT_CENTER,
            format!("{db:+.0}"),
            theme::font_body(9.0),
            FAINT,
        );
    }
    p.hline(g.x_range(), y_of(0.0), Stroke::new(1.0, theme::line(24)));
    for (hz, name) in [
        (50.0f32, "50"),
        (100.0, "100"),
        (500.0, "500"),
        (1000.0, "1k"),
        (5000.0, "5k"),
        (10000.0, "10k"),
    ] {
        p.vline(x_of(hz), g.y_range(), Stroke::new(1.0, theme::line(8)));
        p.text(
            pos2(x_of(hz) + 2.0, g.bottom()),
            Align2::LEFT_BOTTOM,
            name,
            theme::font_body(9.0),
            FAINT,
        );
    }
    let n = 160;
    let pts: Vec<egui::Pos2> = (0..n)
        .map(|i| {
            let hz = (lo + (hi - lo) * i as f32 / (n - 1) as f32).exp();
            pos2(x_of(hz), y_of(eq_response(fx, hz, rate)))
        })
        .collect();
    let zero = y_of(0.0);
    let mut mesh = egui::Mesh::default();
    let c = theme::with_alpha(ACCENT, 34);
    for w in pts.windows(2) {
        let b = mesh.vertices.len() as u32;
        mesh.colored_vertex(w[0], c);
        mesh.colored_vertex(w[1], c);
        mesh.colored_vertex(pos2(w[1].x, zero), c);
        mesh.colored_vertex(pos2(w[0].x, zero), c);
        mesh.add_triangle(b, b + 1, b + 2);
        mesh.add_triangle(b, b + 2, b + 3);
    }
    let clip = p.with_clip_rect(g);
    clip.add(Shape::mesh(mesh));
    clip.add(Shape::line(
        pts,
        Stroke::new(1.8, if fx.is_on() { ACCENT } else { DIM }),
    ));
}

/// A horizontal gain-reduction bar, 0..-24 dB.
fn gr_meter(ui: &mut egui::Ui, reduction: f32) {
    ui.vertical(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(120.0, 10.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(r, CornerRadius::same(2), BG0);
        let d = (-reduction / 24.0).clamp(0.0, 1.0);
        p.rect_filled(
            egui::Rect::from_min_size(
                pos2(r.right() - r.width() * d, r.top()),
                vec2(r.width() * d, r.height()),
            ),
            CornerRadius::same(2),
            WARN,
        );
        ui.label(
            egui::RichText::new(format!("Reduction {:.1} dB", reduction))
                .color(DIM)
                .small(),
        );
    });
}
