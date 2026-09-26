//! The mixer: a strip per track, then the return buses, then the master.
//! Each strip has its inserts (add, bypass, reorder, open), sends to every
//! bus, pan, how it follows the battle's intensity (its layer, with the gain
//! the engine is applying right now), a fader and a stereo meter with peak
//! hold and gain reduction.

use crate::app::{chain_mut, Detail, Strip, Studio};
use crate::songops;
use crate::theme::{self, ACCENT, BG0, BG2, DIM, FAINT, TEXT, WARN};
use crate::transport::truncate;
use crate::widgets::{self, db_text, fader, meter_stereo, Knob};
use eframe::egui::{
    self, pos2, vec2, Align2, CornerRadius, Id, Rect, Sense, Shape, Stroke, StrokeKind,
};
use mc_music::song::{Layer, Send};

#[derive(Default)]
pub struct State {
    rename: String,
}

const STRIP_W: f32 = 104.0;

pub fn show(ui: &mut egui::Ui, st: &mut Studio) {
    let h = ui.available_height();
    egui::ScrollArea::horizontal().id_salt("mixer").auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.add_space(4.0);
            for t in 0..st.song.tracks.len() {
                strip(ui, st, Strip::Track(t), h);
            }
            divider(ui, h);
            for b in 0..st.song.buses.len() {
                strip(ui, st, Strip::Bus(b), h);
            }
            ui.vertical(|ui| {
                ui.add_space(8.0);
                if ui.small_button("+ Bus").on_hover_text("A return bus: tracks send to it, it has its own inserts (a shared reverb)").clicked() {
                    let name = songops::unique_name("Bus", |n| st.song.bus(n).is_some());
                    st.song.buses.push(mc_music::song::Bus { name, db: 0.0, effects: Vec::new(), mute: false });
                }
            });
            divider(ui, h);
            strip(ui, st, Strip::Master, h);
        });
    });
}

fn divider(ui: &mut egui::Ui, h: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(10.0, h - 8.0), Sense::hover());
    ui.painter()
        .vline(r.center().x, r.y_range(), Stroke::new(1.0, theme::line(24)));
}

fn strip(ui: &mut egui::Ui, st: &mut Studio, strip: Strip, h: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(STRIP_W, h - 8.0), Sense::hover());
    let selected = match strip {
        Strip::Track(t) => st.sel.track == t,
        _ => st.sel.fx.is_some_and(|f| f.0 == strip),
    };
    let p = ui.painter();
    p.rect(
        rect,
        CornerRadius::same(3),
        if selected { theme::BG3 } else { BG2 },
        Stroke::new(
            1.0,
            if selected {
                theme::with_alpha(ACCENT, 140)
            } else {
                theme::line(14)
            },
        ),
        StrokeKind::Inside,
    );
    let (name, colour) = match strip {
        Strip::Track(t) => (
            st.song.tracks[t].name.clone(),
            theme::rgb(st.song.tracks[t].colour),
        ),
        Strip::Bus(b) => (st.song.buses[b].name.clone(), theme::mix(DIM, ACCENT, 0.2)),
        Strip::Master => ("Master".to_string(), ACCENT),
    };
    p.rect_filled(
        Rect::from_min_size(rect.min, vec2(rect.width(), 3.0)),
        CornerRadius::same(1),
        colour,
    );
    let mut ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(vec2(6.0, 6.0)))
            .layout(egui::Layout::top_down(egui::Align::Center)),
    );
    let ui = &mut ui;
    ui.spacing_mut().item_spacing.y = 3.0;
    ui.add_space(2.0);
    let title = ui.add(
        egui::Label::new(
            egui::RichText::new(truncate(&name, 12))
                .font(theme::font_semi(13.5))
                .color(TEXT),
        )
        .sense(Sense::click()),
    );
    if title.clicked() {
        if let Strip::Track(t) = strip {
            st.sel.track = t;
        }
    }
    if title.secondary_clicked() {
        st.mixer.rename = name.clone();
    }
    title.context_menu(|ui| {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut st.mixer.rename).desired_width(110.0));
            if ui.button("Rename").clicked() {
                let n = st.mixer.rename.trim().to_string();
                match strip {
                    Strip::Track(t) => songops::rename_track(&mut st.song, t, &n),
                    Strip::Bus(b) => songops::rename_bus(&mut st.song, b, &n),
                    Strip::Master => {}
                }
                ui.close();
            }
        });
        if let Strip::Bus(b) = strip {
            if ui.button("Delete bus").clicked() {
                songops::delete_bus(&mut st.song, b);
                ui.close();
            }
        }
    });
    let kind = match strip {
        Strip::Track(t) => st.song.tracks[t].instrument.kind_name(),
        Strip::Bus(_) => "Bus",
        Strip::Master => "Output",
    };
    ui.label(egui::RichText::new(kind).color(FAINT).small());
    inserts(ui, st, strip);
    if let Strip::Track(t) = strip {
        sends(ui, st, t);
    } else {
        ui.add_space(4.0);
    }
    // Pan (tracks) and the layer (tracks).
    if let Strip::Track(t) = strip {
        ui.add(
            Knob::new(&mut st.song.tracks[t].pan, -1.0, 1.0, 0.0, "Pan")
                .decimals(2)
                .size(26.0),
        );
        let live = st.meters.layers.get(t).copied().unwrap_or(1.0);
        let intensity = st.status.intensity;
        layer_editor(
            ui,
            Id::new(("layer", t)),
            &mut st.song.tracks[t].layer,
            live,
            intensity,
        );
    }
    // Fader and meter fill the rest.
    let bottom_h = 46.0;
    let avail = ui.available_rect_before_wrap();
    let area = Rect::from_min_max(
        avail.min,
        pos2(
            avail.right(),
            (avail.bottom() - bottom_h).max(avail.top() + 60.0),
        ),
    );
    ui.allocate_rect(area, Sense::hover());
    let fader_r = Rect::from_min_max(
        pos2(area.left() + 8.0, area.top()),
        pos2(area.center().x + 6.0, area.bottom()),
    );
    let meter_r = Rect::from_min_max(
        pos2(area.center().x + 12.0, area.top() + 6.0),
        pos2(area.right() - 8.0, area.bottom() - 6.0),
    );
    let (peak, gr) = match strip {
        Strip::Track(t) => st
            .meters
            .tracks
            .get(t)
            .map(|m| (m.peak, m.reduction))
            .unwrap_or_default(),
        Strip::Bus(b) => st
            .meters
            .buses
            .get(b)
            .map(|m| (m.peak, m.reduction))
            .unwrap_or_default(),
        Strip::Master => (st.meters.master.peak, st.meters.master.reduction),
    };
    let has_dyn = match strip {
        Strip::Track(t) => st.song.tracks[t].effects.iter().any(is_dynamics),
        Strip::Bus(b) => st.song.buses[b].effects.iter().any(is_dynamics),
        Strip::Master => st.song.master.effects.iter().any(is_dynamics),
    };
    let db = match strip {
        Strip::Track(t) => &mut st.song.tracks[t].db,
        Strip::Bus(b) => &mut st.song.buses[b].db,
        Strip::Master => &mut st.song.master.db,
    };
    fader(ui, fader_r, Id::new(("fader", format!("{strip:?}"))), db);
    let db_now = *db;
    meter_stereo(
        ui,
        meter_r,
        Id::new(("meter", format!("{strip:?}"))),
        peak,
        has_dyn.then_some(gr),
    );
    // Readouts and mute/solo.
    let peak_db = widgets::lin_db(peak[0].max(peak[1]));
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(db_text(db_now))
                .font(theme::font_semi(12.5))
                .color(TEXT),
        );
        if peak_db > -60.0 {
            ui.label(
                egui::RichText::new(format!("{peak_db:.0}"))
                    .color(if peak_db > 0.0 { theme::BAD } else { FAINT })
                    .small(),
            );
        }
    });
    ui.horizontal(|ui| match strip {
        Strip::Track(t) => {
            let tr = &mut st.song.tracks[t];
            if widgets::toggle(ui, tr.mute, "M", WARN)
                .on_hover_text("Mute")
                .clicked()
            {
                tr.mute = !tr.mute;
            }
            if widgets::toggle(ui, tr.solo, "S", ACCENT)
                .on_hover_text("Solo")
                .clicked()
            {
                tr.solo = !tr.solo;
            }
        }
        Strip::Bus(b) => {
            let bus = &mut st.song.buses[b];
            if widgets::toggle(ui, bus.mute, "M", WARN)
                .on_hover_text("Mute")
                .clicked()
            {
                bus.mute = !bus.mute;
            }
        }
        Strip::Master => {}
    });
}

fn is_dynamics(fx: &mc_music::Effect) -> bool {
    matches!(
        fx,
        mc_music::Effect::Compressor { .. } | mc_music::Effect::Limiter { .. }
    )
}

fn inserts(ui: &mut egui::Ui, st: &mut Studio, strip: Strip) {
    let n = crate::app::chain(&st.song, strip)
        .map(|c| c.len())
        .unwrap_or(0);
    let w = ui.available_width();
    let mut action: Option<(usize, i32)> = None;
    for i in 0..n {
        let (r, resp) = ui.allocate_exact_size(vec2(w, 18.0), Sense::click());
        let fx = &crate::app::chain(&st.song, strip).unwrap()[i];
        let on = fx.is_on();
        let selected = st.sel.fx == Some((strip, i));
        let p = ui.painter();
        p.rect(
            r,
            CornerRadius::same(2),
            if selected {
                theme::with_alpha(ACCENT, 60)
            } else if resp.hovered() {
                theme::BG4
            } else {
                BG0
            },
            Stroke::new(1.0, theme::line(14)),
            StrokeKind::Inside,
        );
        let dot = Rect::from_center_size(pos2(r.left() + 8.0, r.center().y), vec2(8.0, 8.0));
        p.circle_filled(dot.center(), 3.0, if on { ACCENT } else { FAINT });
        p.text(
            pos2(r.left() + 16.0, r.center().y),
            Align2::LEFT_CENTER,
            fx.name(),
            theme::font_body(11.5),
            if on { TEXT } else { FAINT },
        );
        if resp.hovered() {
            // Reorder and remove, shown on hover.
            for (k, (icon, what)) in [
                (widgets::Icon::Up, -1),
                (widgets::Icon::Down, 1),
                (widgets::Icon::Close, 0),
            ]
            .into_iter()
            .enumerate()
            {
                let ir = Rect::from_center_size(
                    pos2(r.right() - 8.0 - k as f32 * 13.0, r.center().y),
                    vec2(12.0, 14.0),
                );
                widgets::draw_icon(p, ir, icon, DIM);
                if resp.clicked() && ir.contains(resp.interact_pointer_pos().unwrap_or_default()) {
                    action = Some((i, if what == 0 { 99 } else { what }));
                }
            }
        }
        if resp.clicked() && action.is_none() {
            let pos = resp.interact_pointer_pos().unwrap_or_default();
            if dot.expand(3.0).contains(pos) {
                action = Some((i, 50));
            } else {
                st.sel.fx = Some((strip, i));
                st.detail = Detail::Effect;
                st.detail_open = true;
            }
        }
    }
    if let (Some((i, what)), Some(c)) = (action, chain_mut(&mut st.song, strip)) {
        match what {
            50 => {
                let on = c[i].is_on();
                c[i].set_on(!on);
            }
            99 => {
                c.remove(i);
                st.sel.fx = None;
            }
            d => {
                let j = i as i32 + d;
                if j >= 0 && (j as usize) < c.len() {
                    c.swap(i, j as usize);
                    if st.sel.fx == Some((strip, i)) {
                        st.sel.fx = Some((strip, j as usize));
                    }
                }
            }
        }
    }
    crate::effects::add_menu(ui, st, strip);
}

fn sends(ui: &mut egui::Ui, st: &mut Studio, t: usize) {
    if st.song.buses.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        for b in 0..st.song.buses.len() {
            let bus = st.song.buses[b].name.clone();
            let track = &mut st.song.tracks[t];
            let idx = track.sends.iter().position(|s| s.bus == bus);
            let mut db = idx.map(|i| track.sends[i].db).unwrap_or(-60.0);
            let label = truncate(&bus, 7);
            let r = ui.add(
                Knob::new(&mut db, -60.0, 6.0, -12.0, &label)
                    .unit("dB")
                    .decimals(1)
                    .size(24.0)
                    .colour(theme::mix(ACCENT, TEXT, 0.3)),
            );
            if r.changed() {
                match idx {
                    Some(i) if db <= -59.9 => {
                        track.sends.remove(i);
                    }
                    Some(i) => track.sends[i].db = db,
                    None if db > -59.9 => track.sends.push(Send { bus, db }),
                    None => {}
                }
            }
        }
    });
}

/// The layer as a small graph over intensity 0..1: drag the three handles
/// (silent below, full from, fading out above); the dot is the gain the
/// engine applies right now at the current intensity.
fn layer_editor(ui: &mut egui::Ui, id: Id, layer: &mut Layer, live: f32, intensity: f32) {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::hover());
    let g = rect.shrink2(vec2(4.0, 5.0));
    let x_of = |v: f32| g.left() + g.width() * v.clamp(0.0, 1.0);
    let y_of = |gain: f32| g.bottom() - g.height() * gain.clamp(0.0, 1.0);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(2), BG0);
    let pts: Vec<egui::Pos2> = (0..=40)
        .map(|i| {
            let x = i as f32 / 40.0;
            pos2(x_of(x), y_of(layer.gain(x)))
        })
        .collect();
    p.add(Shape::line(
        pts,
        Stroke::new(1.3, theme::with_alpha(ACCENT, 200)),
    ));
    p.vline(
        x_of(intensity),
        g.y_range(),
        Stroke::new(1.0, theme::line(50)),
    );
    p.circle_filled(pos2(x_of(intensity), y_of(live)), 3.0, TEXT);
    let handles = [
        (layer.from, 0.0, 0),
        (layer.full, 1.0, 1),
        (layer.until, 1.0, 2),
    ];
    for (v, gy, k) in handles {
        let c = pos2(x_of(v), y_of(gy));
        let hr = ui.interact(
            Rect::from_center_size(c, vec2(10.0, 12.0)),
            id.with(k),
            Sense::drag(),
        );
        if hr.dragged() {
            if let Some(pos) = hr.interact_pointer_pos() {
                let nv = (((pos.x - g.left()) / g.width()).clamp(0.0, 1.0) * 100.0).round() / 100.0;
                match k {
                    0 => {
                        layer.from = nv.min(layer.full);
                    }
                    1 => {
                        layer.full = nv.max(layer.from).min(layer.until);
                    }
                    _ => {
                        layer.until = nv.max(layer.full);
                    }
                }
            }
        }
        let hot = hr.hovered() || hr.dragged();
        p.rect_filled(
            Rect::from_center_size(c, vec2(5.0, 9.0)),
            CornerRadius::same(1),
            if hot { TEXT } else { ACCENT },
        );
    }
    resp.on_hover_text(format!(
        "Layer: silent below {:.2}, full from {:.2}{}. Now {:.0}% at intensity {:.2}",
        layer.from,
        layer.full,
        if layer.until < 1.0 {
            format!(", fading out above {:.2}", layer.until)
        } else {
            String::new()
        },
        live * 100.0,
        intensity
    ));
}
