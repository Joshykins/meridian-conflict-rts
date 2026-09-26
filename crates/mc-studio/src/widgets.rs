//! Custom controls drawn with the painter: knobs, faders, meters, envelope
//! graphs, a two-handle range slider and the small icon and toggle buttons.
//! egui's stock sliders are too wide and too plain for a dense mixer, and a
//! knob has to answer to a vertical drag, Shift for fine and a double-click to
//! reset the way every other audio tool's do.

use crate::theme::{self, ACCENT, ACCENT_DEEP, BAD, BG0, BG3, DIM, FAINT, TEXT, WARN};
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, Id, Mesh, Pos2, Rect, Response, Sense, Shape,
    Stroke, StrokeKind, Ui, Vec2, Widget,
};
use mc_music::patch::Env;

// -- knob ------------------------------------------------------------------------

/// How a knob maps its value to its travel.
#[derive(Clone, Copy, PartialEq)]
pub enum Scale {
    Linear,
    /// Equal travel per octave: frequencies, times.
    Log,
}

pub struct Knob<'a> {
    value: &'a mut f32,
    min: f32,
    max: f32,
    default: f32,
    label: &'a str,
    unit: &'a str,
    decimals: usize,
    scale: Scale,
    integer: bool,
    bipolar: bool,
    diameter: f32,
    colour: Color32,
}

impl<'a> Knob<'a> {
    pub fn new(value: &'a mut f32, min: f32, max: f32, default: f32, label: &'a str) -> Knob<'a> {
        Knob {
            value,
            min,
            max,
            default,
            label,
            unit: "",
            decimals: 2,
            scale: Scale::Linear,
            integer: false,
            bipolar: min < 0.0 && max > 0.0,
            diameter: 30.0,
            colour: ACCENT,
        }
    }
    pub fn unit(mut self, unit: &'a str) -> Self {
        self.unit = unit;
        self
    }
    pub fn decimals(mut self, d: usize) -> Self {
        self.decimals = d;
        self
    }
    pub fn log(mut self) -> Self {
        self.scale = Scale::Log;
        self
    }
    pub fn integer(mut self) -> Self {
        self.integer = true;
        self.decimals = 0;
        self
    }
    pub fn size(mut self, d: f32) -> Self {
        self.diameter = d;
        self
    }
    pub fn colour(mut self, c: Color32) -> Self {
        self.colour = c;
        self
    }

    fn norm(&self, v: f32) -> f32 {
        match self.scale {
            Scale::Log if self.min > 0.0 => {
                ((v.max(self.min) / self.min).ln() / (self.max / self.min).ln()).clamp(0.0, 1.0)
            }
            _ => ((v - self.min) / (self.max - self.min)).clamp(0.0, 1.0),
        }
    }
    fn denorm(&self, n: f32) -> f32 {
        let n = n.clamp(0.0, 1.0);
        let v = match self.scale {
            Scale::Log if self.min > 0.0 => self.min * (self.max / self.min).powf(n),
            _ => self.min + (self.max - self.min) * n,
        };
        if self.integer {
            v.round()
        } else {
            v
        }
    }
}

pub fn format_value(v: f32, decimals: usize, unit: &str) -> String {
    if unit == "Hz" && v >= 1000.0 {
        return format!("{:.1} kHz", v / 1000.0);
    }
    if unit == "s" && v < 1.0 {
        return format!("{:.0} ms", v * 1000.0);
    }
    let sep = if unit.is_empty() || unit == "%" {
        ""
    } else {
        " "
    };
    format!("{v:.decimals$}{sep}{unit}")
}

/// Vertical movement this frame; on the frame a drag is recognised, all of it
/// since the button went down, so the control does not lag the pointer.
fn drag_dy(ui: &Ui, resp: &Response) -> f32 {
    if resp.drag_started() {
        ui.input(
            |i| match (i.pointer.interact_pos(), i.pointer.press_origin()) {
                (Some(a), Some(b)) => a.y - b.y,
                _ => 0.0,
            },
        )
    } else {
        resp.drag_delta().y
    }
}

/// Knob travel, radians either side of straight up.
const SWEEP: f32 = 2.35;

fn arc_points(c: Pos2, r: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    let n = (((a1 - a0).abs() / 0.12).ceil() as usize).max(2);
    (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            pos2(c.x + r * a.sin(), c.y - r * a.cos())
        })
        .collect()
}

impl Widget for Knob<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let d = self.diameter;
        let w = (d + 16.0).max(44.0);
        let (rect, mut resp) = ui.allocate_exact_size(vec2(w, d + 15.0), Sense::click_and_drag());
        let id = resp.id;
        let knob_rect = Rect::from_center_size(
            pos2(rect.center().x, rect.top() + d * 0.5 + 1.0),
            vec2(d, d),
        );
        if resp.double_clicked() {
            *self.value = self.default;
            resp.mark_changed();
        } else if resp.drag_started() || resp.dragged() {
            if resp.drag_started() {
                let n = self.norm(*self.value);
                ui.data_mut(|m| m.insert_temp(id, n));
            }
            let fine = ui.input(|i| i.modifiers.shift);
            let dy = drag_dy(ui, &resp);
            let mut n: f32 = ui
                .data(|m| m.get_temp(id))
                .unwrap_or_else(|| self.norm(*self.value));
            n = (n - dy / if fine { 1600.0 } else { 160.0 }).clamp(0.0, 1.0);
            ui.data_mut(|m| m.insert_temp(id, n));
            let v = self.denorm(n);
            if v != *self.value {
                *self.value = v;
                resp.mark_changed();
            }
        }
        if ui.is_rect_visible(rect) {
            let p = ui.painter();
            let c = knob_rect.center();
            let r = d * 0.5;
            let n = self.norm(*self.value);
            let active = resp.dragged() || resp.hovered();
            p.circle_filled(c, r - 3.0, if active { theme::BG4 } else { BG3 });
            p.circle_stroke(c, r - 3.0, Stroke::new(1.0, theme::line(28)));
            p.add(Shape::line(
                arc_points(c, r, -SWEEP, SWEEP),
                Stroke::new(2.5, theme::line(26)),
            ));
            let a = -SWEEP + 2.0 * SWEEP * n;
            let from = if self.bipolar {
                -SWEEP + 2.0 * SWEEP * self.norm(0.0)
            } else {
                -SWEEP
            };
            if (a - from).abs() > 0.01 {
                let (a0, a1) = if a > from { (from, a) } else { (a, from) };
                p.add(Shape::line(
                    arc_points(c, r, a0, a1),
                    Stroke::new(2.5, self.colour),
                ));
            }
            let tip = pos2(c.x + (r - 5.0) * a.sin(), c.y - (r - 5.0) * a.cos());
            let base = pos2(c.x + (r * 0.25) * a.sin(), c.y - (r * 0.25) * a.cos());
            p.line_segment([base, tip], Stroke::new(2.0, TEXT));
            let text = if resp.dragged() || resp.hovered() {
                format_value(*self.value, self.decimals, self.unit)
            } else {
                self.label.to_string()
            };
            let colour = if resp.dragged() {
                ACCENT
            } else if resp.hovered() {
                TEXT
            } else {
                DIM
            };
            p.text(
                pos2(rect.center().x, rect.bottom() - 1.0),
                Align2::CENTER_BOTTOM,
                text,
                theme::font_body(11.0),
                colour,
            );
        }
        resp.on_hover_text(format!(
            "{}: {}",
            self.label,
            format_value(*self.value, self.decimals, self.unit)
        ))
    }
}

pub fn knob(
    ui: &mut Ui,
    value: &mut f32,
    min: f32,
    max: f32,
    default: f32,
    label: &str,
) -> Response {
    ui.add(Knob::new(value, min, max, default, label))
}

pub fn knob_i32(
    ui: &mut Ui,
    value: &mut i32,
    min: i32,
    max: i32,
    default: i32,
    label: &str,
) -> Response {
    let mut v = *value as f32;
    let r = ui.add(Knob::new(&mut v, min as f32, max as f32, default as f32, label).integer());
    *value = v.round() as i32;
    r
}

pub fn knob_u32(
    ui: &mut Ui,
    value: &mut u32,
    min: u32,
    max: u32,
    default: u32,
    label: &str,
) -> Response {
    let mut v = *value as f32;
    let r = ui.add(Knob::new(&mut v, min as f32, max as f32, default as f32, label).integer());
    *value = v.round().max(0.0) as u32;
    r
}

// -- meters ----------------------------------------------------------------------

pub const METER_FLOOR: f32 = -60.0;
pub const METER_CEIL: f32 = 6.0;

pub fn db_norm(db: f32) -> f32 {
    ((db - METER_FLOOR) / (METER_CEIL - METER_FLOOR)).clamp(0.0, 1.0)
}

pub fn lin_db(g: f32) -> f32 {
    if g <= 1e-6 {
        -120.0
    } else {
        20.0 * g.log10()
    }
}

/// The meter's colour at a level: grey-blue low, warming through the accent
/// near full scale, red over it.
fn meter_colour(n: f32) -> Color32 {
    let hot = db_norm(-12.0);
    let zero = db_norm(0.0);
    if n >= zero {
        BAD
    } else if n >= hot {
        theme::mix(ACCENT_DEEP, ACCENT, (n - hot) / (zero - hot))
    } else {
        theme::mix(
            Color32::from_rgb(0x3A, 0x3E, 0x48),
            ACCENT_DEEP,
            (n / hot).powf(2.2),
        )
    }
}

/// Fills `rect` from the bottom to `level` (0..1) with the meter gradient.
fn gradient_bar(p: &egui::Painter, rect: Rect, level: f32, vertical: bool) {
    if level <= 0.0 {
        return;
    }
    let stops = [
        0.0,
        db_norm(-24.0),
        db_norm(-12.0),
        db_norm(-3.0),
        db_norm(0.0),
        1.0,
    ];
    let mut mesh = Mesh::default();
    let mut add = |n: f32| {
        let (a, b) = if vertical {
            let y = rect.bottom() - rect.height() * n;
            (pos2(rect.left(), y), pos2(rect.right(), y))
        } else {
            let x = rect.left() + rect.width() * n;
            (pos2(x, rect.top()), pos2(x, rect.bottom()))
        };
        let c = meter_colour(n);
        mesh.colored_vertex(a, c);
        mesh.colored_vertex(b, c);
    };
    add(0.0);
    for &s in &stops[1..] {
        if s >= level {
            break;
        }
        add(s);
    }
    add(level.min(1.0));
    let quads = mesh.vertices.len() / 2 - 1;
    for q in 0..quads as u32 {
        let i = q * 2;
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i + 1, i + 3, i + 2);
    }
    p.add(Shape::mesh(mesh));
}

/// Falling levels and held peaks, per meter, kept in egui memory.
#[derive(Clone, Copy, Default)]
struct MeterState {
    level: [f32; 2],
    hold: [f32; 2],
    hold_t: [f64; 2],
    gr: f32,
}

/// Levels (in 0..1 of the scale) after ballistics: instant rise, 24 dB/s fall,
/// peaks held 1.5 s.
fn ballistics(ui: &Ui, id: Id, peak: [f32; 2], gr: f32) -> MeterState {
    let now = ui.input(|i| i.time);
    let dt = ui.input(|i| i.stable_dt).min(0.1);
    let mut s: MeterState = ui.data(|m| m.get_temp(id)).unwrap_or_default();
    for c in 0..2 {
        let db = lin_db(peak[c]);
        let n = db_norm(db);
        let fall = 24.0 * dt / (METER_CEIL - METER_FLOOR);
        s.level[c] = n.max(s.level[c] - fall);
        if n >= s.hold[c] || now - s.hold_t[c] > 1.5 {
            if n >= s.hold[c] {
                s.hold[c] = n;
                s.hold_t[c] = now;
            } else {
                s.hold[c] = (s.hold[c] - fall * 1.5).max(s.level[c]);
            }
        }
    }
    s.gr = if gr < s.gr {
        gr
    } else {
        s.gr + (gr - s.gr) * (dt * 8.0).min(1.0)
    };
    ui.data_mut(|m| m.insert_temp(id, s));
    s
}

/// A vertical stereo meter in `rect`, with peak hold, and an optional
/// gain-reduction strip on its right when `gr` is `Some`.
pub fn meter_stereo(ui: &Ui, rect: Rect, id: Id, peak: [f32; 2], gr: Option<f32>) {
    let s = ballistics(ui, id, peak, gr.unwrap_or(0.0));
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(1), BG0);
    let gr_w = if gr.is_some() { 4.0 } else { 0.0 };
    let inner = rect.shrink(1.0);
    let bars_w = inner.width() - gr_w - if gr.is_some() { 1.0 } else { 0.0 };
    let bw = (bars_w - 1.0) / 2.0;
    for c in 0..2 {
        let r = Rect::from_min_size(
            pos2(inner.left() + c as f32 * (bw + 1.0), inner.top()),
            vec2(bw, inner.height()),
        );
        gradient_bar(p, r, s.level[c], true);
        if s.hold[c] > 0.01 {
            let y = r.bottom() - r.height() * s.hold[c];
            p.hline(
                r.x_range(),
                y,
                Stroke::new(1.0, meter_colour(s.hold[c]).gamma_multiply(1.3)),
            );
        }
    }
    let zero = inner.bottom() - inner.height() * db_norm(0.0);
    p.hline(
        inner.left()..=inner.left() + bars_w,
        zero,
        Stroke::new(1.0, theme::line(50)),
    );
    if gr.is_some() {
        let r = Rect::from_min_max(pos2(inner.right() - gr_w, inner.top()), inner.max);
        let depth = (-s.gr / 24.0).clamp(0.0, 1.0);
        if depth > 0.0 {
            p.rect_filled(
                Rect::from_min_size(r.min, vec2(r.width(), r.height() * depth)),
                CornerRadius::ZERO,
                WARN,
            );
        }
    }
}

/// A thin horizontal stereo meter for the transport bar.
pub fn meter_horizontal(ui: &Ui, rect: Rect, id: Id, peak: [f32; 2]) {
    let s = ballistics(ui, id, peak, 0.0);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(1), BG0);
    let inner = rect.shrink(1.0);
    let bh = (inner.height() - 1.0) / 2.0;
    for c in 0..2 {
        let r = Rect::from_min_size(
            pos2(inner.left(), inner.top() + c as f32 * (bh + 1.0)),
            vec2(inner.width(), bh),
        );
        gradient_bar(p, r, s.level[c], false);
        if s.hold[c] > 0.01 {
            let x = r.left() + r.width() * s.hold[c];
            p.vline(x, r.y_range(), Stroke::new(1.5, meter_colour(s.hold[c])));
        }
    }
    for db in [-48.0, -36.0, -24.0, -12.0, -6.0, 0.0] {
        let x = inner.left() + inner.width() * db_norm(db);
        p.vline(
            x,
            inner.y_range(),
            Stroke::new(1.0, Color32::from_black_alpha(120)),
        );
    }
}

// -- fader -----------------------------------------------------------------------

pub const FADER_MIN: f32 = -72.0;
pub const FADER_MAX: f32 = 12.0;

/// Fader travel for a level in dB: linear in dB, the bottom 2% is silence.
pub fn fader_pos(db: f32) -> f32 {
    if db <= FADER_MIN {
        0.0
    } else {
        0.02 + 0.98 * ((db - FADER_MIN) / (FADER_MAX - FADER_MIN)).clamp(0.0, 1.0)
    }
}

pub fn fader_db(pos: f32) -> f32 {
    if pos <= 0.02 {
        -96.0
    } else {
        FADER_MIN + (FADER_MAX - FADER_MIN) * ((pos - 0.02) / 0.98).clamp(0.0, 1.0)
    }
}

pub fn db_text(db: f32) -> String {
    if db <= -95.0 {
        "-inf".into()
    } else {
        format!("{db:+.1}")
    }
}

/// A vertical fader over `rect`, dB. Double-click returns to 0 dB.
pub fn fader(ui: &mut Ui, rect: Rect, id: Id, db: &mut f32) -> Response {
    let mut resp = ui.interact(rect, id, Sense::click_and_drag());
    let travel = rect.height() - 12.0;
    if resp.double_clicked() {
        *db = 0.0;
        resp.mark_changed();
    } else if resp.drag_started() || resp.dragged() {
        if resp.drag_started() {
            ui.data_mut(|m| m.insert_temp(id, fader_pos(*db)));
        }
        let fine = ui.input(|i| i.modifiers.shift);
        let mut pos: f32 = ui.data(|m| m.get_temp(id)).unwrap_or(fader_pos(*db));
        pos = (pos - drag_dy(ui, &resp) / travel * if fine { 0.1 } else { 1.0 }).clamp(0.0, 1.0);
        ui.data_mut(|m| m.insert_temp(id, pos));
        let v = (fader_db(pos) * 10.0).round() / 10.0;
        if v != *db {
            *db = v;
            resp.mark_changed();
        }
    }
    let p = ui.painter();
    let cx = rect.center().x;
    p.rect_filled(
        Rect::from_center_size(pos2(cx, rect.center().y), vec2(3.0, travel)),
        CornerRadius::same(1),
        BG0,
    );
    for mark in [12.0, 6.0, 0.0, -6.0, -12.0, -24.0, -48.0] {
        let y = rect.bottom() - 6.0 - travel * fader_pos(mark);
        let w = if mark == 0.0 { 7.0 } else { 4.0 };
        p.hline(
            cx - 5.0 - w..=cx - 5.0,
            y,
            Stroke::new(1.0, theme::line(if mark == 0.0 { 90 } else { 40 })),
        );
    }
    let y = rect.bottom() - 6.0 - travel * fader_pos(*db);
    let lit = pos2(cx, y);
    p.rect_filled(
        Rect::from_min_max(pos2(cx - 1.5, y), pos2(cx + 1.5, rect.bottom() - 6.0)),
        CornerRadius::same(1),
        theme::with_alpha(ACCENT, 110),
    );
    let cap = Rect::from_center_size(lit, vec2(22.0, 11.0));
    let hot = resp.dragged() || resp.hovered();
    p.rect(
        cap,
        CornerRadius::same(2),
        if hot { theme::BG4 } else { BG3 },
        Stroke::new(1.0, theme::line(70)),
        StrokeKind::Inside,
    );
    p.hline(
        cap.x_range().shrink(4.0),
        lit.y,
        Stroke::new(1.5, if resp.dragged() { ACCENT } else { TEXT }),
    );
    resp.on_hover_text(format!("{} dB", db_text(*db)))
}

// -- envelope --------------------------------------------------------------------

/// Share of the graph width each timed stage may take, and the longest time it shows.
const ENV_SEG: f32 = 0.28;
const ENV_MAX: f32 = 10.0;

fn env_x(t: f32) -> f32 {
    (t.max(0.0) / ENV_MAX).sqrt().min(1.0) * ENV_SEG
}
fn env_t(x: f32) -> f32 {
    let r = (x / ENV_SEG).clamp(0.0, 1.0);
    (r * r * ENV_MAX).max(0.0005)
}

/// An ADSR drawn as its shape with three draggable handles: the attack peak
/// (time), the decay corner (time and sustain level) and the release end
/// (time). Times use a square-root scale so both 2 ms and 4 s are reachable.
pub fn envelope(ui: &mut Ui, id: Id, env: &mut Env, size: Vec2, colour: Color32) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::hover());
    let g = rect.shrink2(vec2(6.0, 8.0));
    let w = g.width();
    let xa = g.left() + env_x(env.a) * w;
    let xd = xa + env_x(env.d) * w;
    let xs = xd + 0.16 * w;
    let xr = xs + env_x(env.r) * w;
    let ys = g.bottom() - g.height() * env.s.clamp(0.0, 1.0);
    let handles = [pos2(xa, g.top()), pos2(xd, ys), pos2(xr, g.bottom())];
    let mut dragging = None;
    for (i, h) in handles.iter().enumerate() {
        let r = ui.interact(
            Rect::from_center_size(*h, vec2(14.0, 14.0)),
            id.with(i),
            Sense::drag(),
        );
        if r.dragged() {
            dragging = Some(i);
            let pos = r.interact_pointer_pos().unwrap_or(*h);
            match i {
                0 => env.a = env_t((pos.x - g.left()) / w),
                1 => {
                    env.d = env_t((pos.x - xa) / w);
                    env.s = ((g.bottom() - pos.y) / g.height()).clamp(0.0, 1.0);
                }
                _ => env.r = env_t((pos.x - xs) / w),
            }
            resp.mark_changed();
        }
        if r.hovered() && dragging.is_none() {
            dragging = Some(i + 10);
        }
    }
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(3), BG0);
    for k in 1..4 {
        let y = g.bottom() - g.height() * k as f32 / 4.0;
        p.hline(g.x_range(), y, Stroke::new(1.0, theme::line(10)));
    }
    let pts = vec![
        pos2(g.left(), g.bottom()),
        handles[0],
        handles[1],
        pos2(xs, ys),
        handles[2],
    ];
    let mut fill = pts.clone();
    fill.push(pos2(g.left(), g.bottom()));
    // The shape is not convex; fill it as a fan of thin quads under each segment.
    let mut mesh = Mesh::default();
    let under = theme::with_alpha(colour, 36);
    for s in pts.windows(2) {
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(s[0], under);
        mesh.colored_vertex(s[1], under);
        mesh.colored_vertex(pos2(s[1].x, g.bottom()), under);
        mesh.colored_vertex(pos2(s[0].x, g.bottom()), under);
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    p.add(Shape::mesh(mesh));
    p.add(Shape::line(pts, Stroke::new(1.5, colour)));
    p.vline(xs, g.y_range(), Stroke::new(1.0, theme::line(16)));
    for (i, h) in handles.iter().enumerate() {
        let hot = dragging == Some(i) || dragging == Some(i + 10);
        p.circle_filled(
            *h,
            if hot { 5.0 } else { 3.5 },
            if hot { TEXT } else { colour },
        );
    }
    let label = match dragging {
        Some(0) | Some(10) => format!("Attack {}", format_value(env.a, 3, "s")),
        Some(1) | Some(11) => format!(
            "Decay {}  Sustain {:.0}%",
            format_value(env.d, 3, "s"),
            env.s * 100.0
        ),
        Some(2) | Some(12) => format!("Release {}", format_value(env.r, 3, "s")),
        _ => String::new(),
    };
    if !label.is_empty() {
        p.text(
            rect.right_top() + vec2(-6.0, 4.0),
            Align2::RIGHT_TOP,
            label,
            theme::font_body(11.0),
            TEXT,
        );
    }
    resp
}

// -- small controls --------------------------------------------------------------

/// A two-handle 0..1 range slider (a section's intensity range).
pub fn range_slider(ui: &mut Ui, id: Id, range: &mut (f32, f32), width: f32) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(width, 20.0), Sense::hover());
    let track = rect.shrink2(vec2(6.0, 0.0));
    let x = |v: f32| track.left() + track.width() * v;
    let p = ui.painter();
    p.rect_filled(
        Rect::from_center_size(rect.center(), vec2(track.width(), 4.0)),
        CornerRadius::same(2),
        BG0,
    );
    p.rect_filled(
        Rect::from_min_max(
            pos2(x(range.0), rect.center().y - 2.0),
            pos2(x(range.1), rect.center().y + 2.0),
        ),
        CornerRadius::same(2),
        ACCENT,
    );
    for i in 0..2 {
        let v = if i == 0 { range.0 } else { range.1 };
        let h = Rect::from_center_size(pos2(x(v), rect.center().y), vec2(10.0, 16.0));
        let r = ui.interact(h, id.with(i), Sense::drag());
        if r.dragged() {
            if let Some(pos) = r.interact_pointer_pos() {
                let nv = (((pos.x - track.left()) / track.width()).clamp(0.0, 1.0) * 100.0).round()
                    / 100.0;
                if i == 0 {
                    range.0 = nv.min(range.1);
                } else {
                    range.1 = nv.max(range.0);
                }
                resp.mark_changed();
            }
        }
        let hot = r.hovered() || r.dragged();
        ui.painter().rect(
            h,
            CornerRadius::same(2),
            if hot { TEXT } else { theme::BG4 },
            Stroke::new(1.0, ACCENT),
            StrokeKind::Inside,
        );
    }
    resp
}

/// A compact latching button: filled with `on_colour` when on.
pub fn toggle(ui: &mut Ui, on: bool, text: &str, on_colour: Color32) -> Response {
    let font = theme::font_semi(12.5);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, TEXT);
    let size = vec2((galley.size().x + 12.0).max(20.0), 20.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let (fill, fg, stroke) = if on {
        (on_colour, Color32::from_rgb(0x10, 0x10, 0x12), on_colour)
    } else if resp.hovered() {
        (theme::BG4, TEXT, theme::line(60))
    } else {
        (BG3, DIM, theme::line(18))
    };
    p.rect(
        rect,
        CornerRadius::same(2),
        fill,
        Stroke::new(1.0, stroke),
        StrokeKind::Inside,
    );
    p.galley(rect.center() - galley.size() * 0.5, galley, fg);
    resp
}

#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    Play,
    Stop,
    Home,
    Loop,
    Metronome,
    Undo,
    Redo,
    Plus,
    Close,
    Up,
    Down,
    Speaker,
    SpeakerOff,
}

pub fn draw_icon(p: &egui::Painter, rect: Rect, icon: Icon, c: Color32) {
    let ctr = rect.center();
    let s = rect.height().min(rect.width()) * 0.3;
    let st = Stroke::new(1.6, c);
    match icon {
        Icon::Play => {
            p.add(Shape::convex_polygon(
                vec![
                    ctr + vec2(-s * 0.8, -s),
                    ctr + vec2(s, 0.0),
                    ctr + vec2(-s * 0.8, s),
                ],
                c,
                Stroke::NONE,
            ));
        }
        Icon::Stop => {
            p.rect_filled(
                Rect::from_center_size(ctr, vec2(s * 1.6, s * 1.6)),
                CornerRadius::same(1),
                c,
            );
        }
        Icon::Home => {
            p.vline(ctr.x - s, (ctr.y - s)..=(ctr.y + s), Stroke::new(2.0, c));
            p.add(Shape::convex_polygon(
                vec![
                    ctr + vec2(s, -s),
                    ctr + vec2(s, s),
                    ctr + vec2(-s * 0.7, 0.0),
                ],
                c,
                Stroke::NONE,
            ));
        }
        Icon::Loop => {
            let r = Rect::from_center_size(ctr, vec2(s * 2.2, s * 1.4));
            p.rect_stroke(r, CornerRadius::same(3), st, StrokeKind::Middle);
            p.add(Shape::convex_polygon(
                vec![
                    pos2(r.right() - 1.0, r.top() - 3.0),
                    pos2(r.right() + 2.5, r.top()),
                    pos2(r.right() - 1.0, r.top() + 3.0),
                ],
                c,
                Stroke::NONE,
            ));
        }
        Icon::Metronome => {
            p.add(Shape::closed_line(
                vec![
                    ctr + vec2(-s * 0.8, s),
                    ctr + vec2(-s * 0.35, -s),
                    ctr + vec2(s * 0.35, -s),
                    ctr + vec2(s * 0.8, s),
                ],
                st,
            ));
            p.line_segment(
                [ctr + vec2(0.0, s * 0.6), ctr + vec2(s * 0.8, -s * 0.9)],
                st,
            );
        }
        Icon::Undo | Icon::Redo => {
            let dir = if icon == Icon::Undo { -1.0 } else { 1.0 };
            let pts = arc_points(ctr + vec2(0.0, s * 0.2), s * 0.9, -1.6 * dir, 1.4 * dir);
            let end = pts[0];
            p.add(Shape::line(pts, st));
            p.add(Shape::convex_polygon(
                vec![
                    end + vec2(0.0, -3.5),
                    end + vec2(3.5 * dir * -1.0, 0.0),
                    end + vec2(0.0, 3.5),
                ],
                c,
                Stroke::NONE,
            ));
        }
        Icon::Plus => {
            p.hline((ctr.x - s)..=(ctr.x + s), ctr.y, st);
            p.vline(ctr.x, (ctr.y - s)..=(ctr.y + s), st);
        }
        Icon::Close => {
            p.line_segment([ctr + vec2(-s, -s) * 0.8, ctr + vec2(s, s) * 0.8], st);
            p.line_segment([ctr + vec2(s, -s) * 0.8, ctr + vec2(-s, s) * 0.8], st);
        }
        Icon::Speaker | Icon::SpeakerOff => {
            let body = vec![
                ctr + vec2(-s * 1.1, -s * 0.4),
                ctr + vec2(-s * 0.5, -s * 0.4),
                ctr + vec2(s * 0.1, -s),
                ctr + vec2(s * 0.1, s),
                ctr + vec2(-s * 0.5, s * 0.4),
                ctr + vec2(-s * 1.1, s * 0.4),
            ];
            p.add(Shape::convex_polygon(body, c, Stroke::NONE));
            if icon == Icon::Speaker {
                p.add(Shape::line(
                    arc_points(ctr + vec2(s * 0.2, 0.0), s * 0.6, 0.6, 2.5),
                    st,
                ));
                p.add(Shape::line(
                    arc_points(ctr + vec2(s * 0.2, 0.0), s * 1.05, 0.6, 2.5),
                    st,
                ));
            } else {
                p.line_segment(
                    [
                        ctr + vec2(s * 0.5, -s * 0.45),
                        ctr + vec2(s * 1.3, s * 0.45),
                    ],
                    st,
                );
                p.line_segment(
                    [
                        ctr + vec2(s * 1.3, -s * 0.45),
                        ctr + vec2(s * 0.5, s * 0.45),
                    ],
                    st,
                );
            }
        }
        Icon::Up | Icon::Down => {
            let d = if icon == Icon::Up { -1.0 } else { 1.0 };
            p.add(Shape::line(
                vec![
                    ctr + vec2(-s * 0.8, -d * s * 0.4),
                    ctr + vec2(0.0, d * s * 0.5),
                    ctr + vec2(s * 0.8, -d * s * 0.4),
                ],
                st,
            ));
        }
    }
}

/// A square button with a drawn icon; `active` fills it with the accent.
pub fn icon_button(ui: &mut Ui, icon: Icon, active: bool, size: f32, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let p = ui.painter();
    let (fill, fg) = if active {
        (ACCENT, Color32::from_rgb(0x12, 0x12, 0x14))
    } else if resp.hovered() {
        (theme::BG4, TEXT)
    } else {
        (BG3, DIM)
    };
    p.rect(
        rect,
        CornerRadius::same(2),
        fill,
        Stroke::new(1.0, theme::line(if active { 0 } else { 18 })),
        StrokeKind::Inside,
    );
    draw_icon(p, rect, icon, fg);
    if tip.is_empty() {
        resp
    } else {
        resp.on_hover_text(tip)
    }
}

/// A tiny flat icon (no box) for list rows.
pub fn tiny_icon(ui: &mut Ui, icon: Icon, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
    let c = if resp.hovered() { TEXT } else { FAINT };
    draw_icon(ui.painter(), rect, icon, c);
    resp.on_hover_text(tip)
}

/// A dim caption above a group of controls.
pub fn caption(ui: &mut Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(theme::font_semi(12.5))
            .color(DIM),
    );
}

/// A group box: a slightly lighter panel with a hairline and a caption.
pub fn group<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(theme::BG2)
        .stroke(Stroke::new(1.0, theme::line(16)))
        .corner_radius(CornerRadius::same(3))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                if !title.is_empty() {
                    caption(ui, title);
                }
                add(ui)
            })
            .inner
        })
        .inner
}
