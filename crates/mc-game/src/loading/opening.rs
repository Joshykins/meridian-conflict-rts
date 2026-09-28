//! The opening of a run: the first loading screen, over the black of a new
//! window, before the front end comes up. A wireframe valley runs out to the
//! horizon with the meridian down its floor; the game's M is traced in over
//! it and the name is set either side of the line. Overlay only, so the
//! splash (which draws no scene) and the first renderer draw the same thing.
//! Presentation only: every float here ends on the screen.

use super::{ease_in_out, ease_out};
use crate::ui::emblem::monogram;
use crate::ui::{self, palette, rgb, Color, Rect, Ui};
use glam::{Vec2, Vec3};
use mc_render::Face;

/// When the last of the intro has arrived. The window is not handed over
/// before it, so no stall lands mid-reveal.
pub(super) const INTRO: f32 = 3.3;
/// The earliest the screen lifts, so the name is there to be read.
pub(super) const LINGER: f32 = 4.0;
/// What the loading line says while the front end comes up underneath.
pub(super) const STAGE: &str = "Opening command";

// When each part arrives, in seconds of the screen's own clock.
const LINE: (f32, f32) = (0.0, 0.8);
const LAND: (f32, f32) = (0.35, 2.3);
const TRACE: (f32, f32) = (0.9, 2.0);
const BAR: (f32, f32) = (1.3, 1.9);
const FILL: (f32, f32) = (1.7, 2.6);
const TITLE: (f32, f32) = (2.1, 2.9);
const CREDIT: (f32, f32) = (2.6, 3.3);
const STATUS: (f32, f32) = (2.7, 3.3);
/// Rings run out from the meridian across the land this often, once it is all up.
const PING_EVERY: f32 = 6.0;
const PING_SPEED: f32 = 650.0;
/// A light runs down the meridian toward the eye this often.
const PULSE_EVERY: f32 = 2.6;

/// The eye's height over the valley floor, and its speed up the valley.
const EYE: f32 = 70.0;
const DRIFT: f32 = 16.0;
/// The wireframe's cell, and the nearest and farthest rows drawn, metres.
const GRID: f32 = 40.0;
const NEAR: f32 = 140.0;
const FAR: f32 = 3200.0;
/// The ranges beyond the grid, drawn as bare ridgelines.
const RANGES: [(f32, f32); 2] = [(3400.0, 0.16), (4700.0, 0.09)];
/// Deliberate cap (cosmetic): the most wire segments in a frame. Rows are
/// drawn near to far, so a very wide screen loses the faintest far lines
/// rather than overflowing the overlay's vertex budget.
const MOST_SEGMENTS: usize = 11_000;

/// The left half of the program icon's M (`emblem::monogram::outline`), in
/// its 1000-unit design square; the right half mirrors it.
fn m_half() -> [Vec2; 8] {
    monogram::outline(false)[0]
}
/// The half as convex quads, for filling.
const M_QUADS: [[usize; 4]; 3] = [[0, 1, 2, 3], [0, 3, 6, 7], [3, 4, 5, 6]];
const M_MIDDLE: Vec2 = Vec2::new(500.0, 505.0);
/// The M's top and foot, design units.
fn m_span() -> (f32, f32) {
    let half = m_half();
    (half[2].y, half[0].y)
}
/// The meridian's bar through the M: its half-width and its ends.
const M_BAR: (f32, f32, f32) = (17.0, 150.0, 860.0);
/// The corner the outline is traced out from: the foot of the V.
const TRACE_FROM: usize = 5;
const STEEL: u32 = 0xF2F2F0;
const STEEL_FOOT: u32 = 0xAEB4BC;
const HOT: u32 = 0xFFE4CF;
const WIRE: u32 = 0xC4CFDA;

fn phase(t: f32, (from, to): (f32, f32)) -> f32 {
    ((t - from) / (to - from)).clamp(0.0, 1.0)
}

fn smooth(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

fn alpha(mut c: Color, a: f32) -> Color {
    c[3] = a;
    c
}

// -- the land ------------------------------------------------------------------------

fn hash(x: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (z as u32).wrapping_mul(0xD816_3841);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}

fn noise(x: f32, z: f32) -> f32 {
    let (x0, z0) = (x.floor(), z.floor());
    let (fx, fz) = (x - x0, z - z0);
    let (ux, uz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    let (i, k) = (x0 as i32, z0 as i32);
    let near = hash(i, k) + (hash(i + 1, k) - hash(i, k)) * ux;
    let far = hash(i, k + 1) + (hash(i + 1, k + 1) - hash(i, k + 1)) * ux;
    near + (far - near) * uz
}

/// Sharp-crested ranges, about 0..1.
fn ridges(x: f32, z: f32) -> f32 {
    let (mut sum, mut amp, mut freq, mut weight) = (0.0, 0.55, 1.0, 1.0);
    for _ in 0..5 {
        let n = 1.0 - (noise(x * freq, z * freq) * 2.0 - 1.0).abs();
        let n = n * n * weight;
        weight = n.clamp(0.0, 1.0);
        sum += n * amp;
        amp *= 0.5;
        freq *= 2.03;
    }
    sum
}

/// The land, metres: a valley down the meridian (x = 0) between ranges that
/// climb the further out they are.
fn height(x: f32, z: f32) -> f32 {
    let side = x.abs();
    let wall = smooth(90.0, 900.0, side);
    let lift = 60.0 + 320.0 * smooth(300.0, 1800.0, side);
    let floor = 5.0 * noise(x / 90.0 + 7.0, z / 90.0) + 3.0 * (z / 260.0).sin();
    floor + wall * ridges(x / 520.0 + 17.3, z / 520.0) * lift + wall * wall * 30.0
}

/// The far ranges, taller and plainer.
fn far_height(x: f32, z: f32) -> f32 {
    60.0 + ridges(x / 900.0 + 3.1, z / 900.0) * 700.0 * smooth(150.0, 1500.0, x.abs())
}

struct View {
    centre: f32,
    horizon: f32,
    focal: f32,
    size: Vec2,
    /// How far up the valley the eye has come, metres.
    eye: f32,
}

impl View {
    /// A point on the land, `z` metres up the valley from the eye's start.
    fn project(&self, p: Vec3) -> Vec2 {
        let z = p.z - self.eye;
        Vec2::new(
            self.centre + self.focal * p.x / z,
            self.horizon - self.focal * (p.y - EYE) / z,
        )
    }
}

/// The highest line drawn so far across the screen, drawing near to far:
/// anything further that falls below it is behind a nearer ridge.
struct Skyline {
    top: Vec<f32>,
}

const BUCKET: f32 = 2.0;

impl Skyline {
    fn new(width: f32) -> Skyline {
        Skyline {
            top: vec![f32::INFINITY; (width / BUCKET) as usize + 2],
        }
    }

    fn clear(&self, p: Vec2) -> bool {
        if !p.x.is_finite() || p.x < 0.0 {
            return true;
        }
        self.top
            .get((p.x / BUCKET) as usize)
            .is_none_or(|&top| p.y <= top + 0.75)
    }

    fn raise(&mut self, a: Vec2, b: Vec2) {
        let (a, b) = if a.x <= b.x { (a, b) } else { (b, a) };
        let last = self.top.len() as f32 - 1.0;
        let (i0, i1) = ((a.x / BUCKET).floor(), (b.x / BUCKET).ceil());
        if i1 < 0.0 || i0 > last {
            return;
        }
        for i in i0.max(0.0) as usize..=i1.min(last) as usize {
            let x = (i as f32 + 0.5) * BUCKET;
            let u = if b.x - a.x > 1e-3 {
                ((x - a.x) / (b.x - a.x)).clamp(0.0, 1.0)
            } else {
                0.5
            };
            let y = a.y + (b.y - a.y) * u;
            self.top[i] = self.top[i].min(y);
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Grid,
    Meridian,
    /// A far ridgeline at this opacity.
    Range(f32),
}

/// Draws the wireframe with its hidden lines taken out.
struct Wire<'v> {
    view: &'v View,
    sky: Skyline,
    drawn: usize,
    /// How far either side of the meridian the land is drawn yet, metres.
    reveal: f32,
    /// The reveal once it has reached the edges of the screen, metres.
    full: f32,
    /// How far down the screen the meridian is drawn yet.
    down: f32,
    /// The ring running out across the land, metres from the meridian.
    ping: Option<f32>,
}

impl Wire<'_> {
    /// Whether a point is drawn: in sight, and reached by the reveal. `x`
    /// is its distance across, metres.
    fn shown(&self, q: Vec2, x: f32, kind: Kind) -> bool {
        self.sky.clear(q)
            && match kind {
                Kind::Meridian => q.y <= self.down,
                Kind::Grid => x.abs() <= self.reveal,
                // The ranges come as the reveal reaches the far edge of the grid.
                Kind::Range(_) => x.abs() * FAR / RANGES[1].0 <= self.reveal,
            }
    }

    /// The straight line between two points on the land, where it can be seen.
    fn segment(&mut self, ui: &mut Ui, (a, pa): (Vec3, Vec2), (b, pb): (Vec3, Vec2), kind: Kind) {
        let size = self.view.size;
        let off = |p: Vec2| (p.x < -8.0, p.x > size.x + 8.0, p.y > size.y + 8.0);
        let (oa, ob) = (off(pa), off(pb));
        if (oa.0 && ob.0) || (oa.1 && ob.1) || (oa.2 && ob.2) || self.drawn >= MOST_SEGMENTS {
            return;
        }
        let at = |u: f32| pa.lerp(pb, u);
        let shown = |u: f32| self.shown(at(u), a.x + (b.x - a.x) * u, kind);
        // Where it goes in or out of sight, between a seen and an unseen sample.
        let edge = |mut seen: f32, mut hidden: f32| {
            for _ in 0..5 {
                let mid = (seen + hidden) * 0.5;
                if shown(mid) {
                    seen = mid;
                } else {
                    hidden = mid;
                }
            }
            seen
        };
        let steps = (pa.distance(pb) / 6.0).ceil().clamp(1.0, 48.0) as usize;
        let mut runs: Vec<(f32, f32)> = Vec::new();
        let mut open: Option<f32> = None;
        let mut last = 0.0;
        for i in 0..=steps {
            let u = i as f32 / steps as f32;
            match (shown(u), open) {
                (true, None) => open = Some(if i == 0 { 0.0 } else { edge(u, last) }),
                (false, Some(from)) => {
                    runs.push((from, edge(last, u)));
                    open = None;
                }
                _ => {}
            }
            last = u;
        }
        runs.extend(open.map(|from| (from, 1.0)));
        for (from, to) in runs {
            self.run(ui, a.lerp(b, (from + to) * 0.5), at(from), at(to), kind);
        }
    }

    /// One stretch in sight, coloured for where it lies.
    fn run(&mut self, ui: &mut Ui, mid: Vec3, from: Vec2, to: Vec2, kind: Kind) {
        let view = self.view;
        let z = mid.z - view.eye;
        let screen = from.lerp(to, 0.5);
        // Faint toward the foot of the screen, where the loading line sits.
        let foot = 1.0 - 0.8 * smooth(view.size.y - 280.0, view.size.y - 40.0, screen.y);
        let px = 1.0 / ui.s;
        let (color, thickness) = match kind {
            Kind::Range(a) => (rgb(WIRE, a), px),
            Kind::Meridian => {
                let fog = (1.0 - z / FAR).clamp(0.0, 1.0).powf(0.7) * smooth(NEAR, NEAR + 80.0, z);
                let a = fog * foot;
                let glow = (900.0 / z * 5.0).clamp(2.5, 11.0);
                let accent = rgb(palette::ACCENT, 0.0);
                ui.ribbon(
                    from,
                    to,
                    glow,
                    glow,
                    &[
                        (-1.0, 0.0, accent),
                        (0.0, 0.0, alpha(accent, 0.30 * a)),
                        (1.0, 0.0, accent),
                    ],
                );
                let wide = 1.1 + 1.4 * smooth(1200.0, 200.0, z);
                (rgb(palette::ACCENT, 0.95 * a), wide)
            }
            Kind::Grid => {
                let fog = (1.0 - z / FAR).clamp(0.0, 1.0).powf(1.2) * smooth(NEAR, NEAR + 120.0, z);
                let crest = smooth(40.0, 320.0, mid.y);
                let line = (-mid.x.abs() / 170.0).exp();
                let front = if self.reveal < self.full {
                    (-((self.reveal - mid.x.abs()) / (GRID * 1.5)).powi(2)).exp()
                } else {
                    0.0
                };
                let ping = self.ping.map_or(0.0, |r| {
                    (-((mid.x.abs() - r) / 80.0).powi(2)).exp() * (1.0 - r / 2400.0).max(0.0)
                });
                let warm = (line * 0.8 + ping + front).min(1.0);
                let a = fog * foot * (0.26 + 0.22 * crest + 0.55 * warm);
                let color = mix(rgb(WIRE, 1.0), rgb(palette::ACCENT, 1.0), warm);
                let color = mix(color, rgb(HOT, 1.0), (front + ping - 0.6).max(0.0));
                let wide = 1.0 + 0.5 * smooth(900.0, 250.0, z);
                (alpha(color, a.min(1.0)), wide * px)
            }
        };
        if color[3] < 0.01 {
            return;
        }
        self.drawn += 1;
        ui.stroke(from, to, thickness, color);
    }

    /// The valley, row by row from the eye outward.
    fn land(&mut self, ui: &mut Ui) {
        let view = self.view;
        let first = ((view.eye + NEAR) / GRID).ceil() as i32;
        let last = ((view.eye + FAR) / GRID).floor() as i32;
        let mut before: Option<(i32, Vec<(Vec3, Vec2)>)> = None;
        for k in first..=last {
            let z = k as f32 * GRID;
            let reach = (z - view.eye) * (view.size.x * 0.5 + 60.0) / view.focal;
            let n = (reach / GRID).ceil() as i32;
            let row: Vec<(Vec3, Vec2)> = (-n..=n)
                .map(|j| {
                    let x = j as f32 * GRID;
                    let p = Vec3::new(x, height(x, z), z);
                    (p, view.project(p))
                })
                .collect();
            // The lines running out, back to the row before (always the narrower).
            if let Some((from, prev)) = &before {
                for (i, &near) in prev.iter().enumerate() {
                    let j = from + i as i32;
                    let kind = if j == 0 { Kind::Meridian } else { Kind::Grid };
                    self.segment(ui, near, row[(j + n) as usize], kind);
                }
            }
            for pair in row.windows(2) {
                self.segment(ui, pair[0], pair[1], Kind::Grid);
            }
            for pair in row.windows(2) {
                self.sky.raise(pair[0].1, pair[1].1);
            }
            before = Some((-n, row));
        }
        // The ranges beyond, behind all of it.
        for (distance, a) in RANGES {
            let z = view.eye + distance;
            let reach = distance * (view.size.x * 0.5 + 60.0) / view.focal;
            let n = (reach / 60.0).ceil() as i32;
            let line: Vec<(Vec3, Vec2)> = (-n..=n)
                .map(|j| {
                    let x = j as f32 * 60.0;
                    let p = Vec3::new(x, far_height(x, z), z);
                    (p, view.project(p))
                })
                .collect();
            for pair in line.windows(2) {
                self.segment(ui, pair[0], pair[1], Kind::Range(a));
            }
            for pair in line.windows(2) {
                self.sky.raise(pair[0].1, pair[1].1);
            }
        }
    }
}

// -- the screen ----------------------------------------------------------------------

/// The opening's own state: the loading line's words as they change.
pub(super) struct Opening {
    step: &'static str,
    was: &'static str,
    changed: f32,
}

/// Where the parts sit, in points.
struct Layout {
    view: View,
    /// The M: its middle and points per design unit.
    m_at: Vec2,
    m_k: f32,
    title_y: f32,
}

impl Opening {
    pub(super) fn new() -> Opening {
        Opening {
            step: "",
            was: "",
            changed: 0.0,
        }
    }

    /// Draws the screen `t` seconds into its own clock: `lift` 0..1 as it
    /// goes, `bar` how far the loading has got, `step` what it is doing.
    pub(super) fn draw(
        &mut self,
        ui: &mut Ui,
        t: f32,
        lift: f32,
        bar: f32,
        step: Option<&'static str>,
    ) {
        let size = ui.size;
        let surge = ease_in_out(lift);
        let layout = Layout {
            view: View {
                centre: size.x * 0.5,
                horizon: size.y * 0.62,
                focal: size.y * 0.9,
                size,
                eye: t * DRIFT + surge * surge * 420.0,
            },
            m_at: Vec2::new(size.x * 0.5, size.y * 0.31),
            m_k: size.y * 0.18 / (m_span().1 - m_span().0) * (1.0 + 0.05 * surge),
            title_y: size.y * 0.49,
        };
        // Black that fades out evenly to the eye as the screen lifts.
        let fade = ui.fade;
        ui.fade = 1.0;
        ui.fill(Rect::new(0.0, 0.0, size.x, size.y), ui::ink(fade));
        ui.fade = fade;
        self.sky(ui, &layout, t);
        self.land(ui, &layout, t);
        self.meridian(ui, &layout, t);
        self.monogram(ui, &layout, t);
        self.title(ui, &layout, t);
        self.status(ui, t, bar, step);
    }

    fn sky(&self, ui: &mut Ui, l: &Layout, t: f32) {
        let (w, horizon, cx) = (l.view.size.x, l.view.horizon, l.view.centre);
        let land = ease_in_out(phase(t, LAND));
        ui.gradient_v(
            Rect::new(0.0, 0.0, w, horizon),
            rgb(0x000000, 1.0),
            rgb(0x0C0605, land),
        );
        // First light, behind the M where the meridian meets the horizon.
        let r = l.view.size.y * 0.55;
        let centre = Vec2::new(cx, horizon);
        ui.ribbon_cap(centre, Vec2::new(0.0, -1.0), r, &falloff(0.26 * land));
        ui.ribbon_cap(centre, Vec2::new(0.0, 1.0), r * 0.45, &falloff(0.16 * land));
        // Haze along the horizon, which the far rows of the land fade into.
        let haze = rgb(palette::ACCENT_DEEP, 0.10 * land);
        ui.gradient_v(
            Rect::new(0.0, horizon - 26.0, w, 26.0),
            alpha(haze, 0.0),
            haze,
        );
        ui.gradient_v(Rect::new(0.0, horizon, w, 40.0), haze, alpha(haze, 0.0));
        // The horizon line, out from the meridian as the land comes.
        let reach = land * w * 0.3;
        let glow = rgb(palette::ACCENT, 0.45);
        ui.gradient_h(
            Rect::new(cx - reach, horizon, reach, 1.0),
            alpha(glow, 0.0),
            glow,
        );
        ui.gradient_h(Rect::new(cx, horizon, reach, 1.0), glow, alpha(glow, 0.0));
        // The spark the meridian is struck from.
        let spark = 1.0 - phase(t, (0.0, 0.9));
        if spark > 0.0 && t > 0.0 {
            glow_dot(
                ui,
                Vec2::new(cx, horizon),
                46.0 * (1.2 - spark),
                spark * spark,
            );
        }
    }

    fn land(&self, ui: &mut Ui, l: &Layout, t: f32) {
        let (size, horizon) = (l.view.size, l.view.horizon);
        let ping = (t >= INTRO).then(|| (t - INTRO) % PING_EVERY * PING_SPEED);
        let full = FAR * (size.x * 0.5 + 60.0) / l.view.focal;
        let mut wire = Wire {
            view: &l.view,
            sky: Skyline::new(size.x),
            drawn: 0,
            reveal: ease_in_out(phase(t, LAND)).powf(1.4) * (full + GRID * 3.0),
            full,
            down: horizon + ease_out(phase(t, LINE)) * (size.y - horizon),
            ping,
        };
        wire.land(ui);
        // A light runs down the meridian, out of the distance toward the eye.
        if t > INTRO - 0.7 {
            let u = (t - (INTRO - 0.7)) % PULSE_EVERY / PULSE_EVERY;
            let z = FAR * (1.0 - u).powi(2) + NEAR * 2.0;
            let world = l.view.eye + z;
            let p = l.view.project(Vec3::new(0.0, height(0.0, world), world));
            let fade = smooth(0.0, 0.15, u) * (1.0 - smooth(size.y - 260.0, size.y - 60.0, p.y));
            if wire.sky.clear(p) && fade > 0.0 {
                glow_dot(ui, p, (1500.0 / z * 6.0).clamp(5.0, 26.0), fade * 0.8);
            }
        }
        // The downward head of the line as it is first drawn.
        let down = phase(t, LINE);
        if t > 0.0 && down < 1.0 {
            glow_dot(
                ui,
                Vec2::new(l.view.centre, wire.down),
                14.0,
                1.0 - down * down,
            );
        }
        // Dark edges and foot, for the words.
        let (w, h) = (size.x, size.y);
        ui.scrim(Rect::new(0.0, 0.0, w * 0.2, h), 0.7, 0.0, true);
        ui.scrim(Rect::new(w * 0.8, 0.0, w * 0.2, h), 0.0, 0.7, true);
        ui.scrim(Rect::new(0.0, 0.0, w, h * 0.14), 0.6, 0.0, false);
        ui.scrim(Rect::new(0.0, h - 230.0, w, 230.0), 0.0, 0.9, false);
    }

    /// The meridian above the land: from the top of the screen, through the
    /// M and between the words of the name, to the rule under them.
    fn meridian(&self, ui: &mut Ui, l: &Layout, t: f32) {
        let (cx, horizon) = (l.view.centre, l.view.horizon);
        let top = horizon - ease_out(phase(t, LINE)) * horizon;
        let end = l.title_y + 32.0;
        let (from, to) = (top, end.max(top));
        if to > from {
            let accent = rgb(palette::ACCENT, 1.0);
            // Out of the dark at the top of the screen, brightest through the name.
            let shade = |y: f32| 0.25 + 0.65 * smooth(0.0, l.m_at.y, y);
            ui.gradient_v(
                Rect::new(cx - 0.75, from, 1.5, to - from),
                alpha(accent, shade(from)),
                alpha(accent, shade(to)),
            );
            let halo = alpha(accent, 0.12);
            ui.gradient_h(
                Rect::new(cx - 7.0, from, 6.25, to - from),
                alpha(halo, 0.0),
                halo,
            );
            ui.gradient_h(
                Rect::new(cx + 0.75, from, 6.25, to - from),
                halo,
                alpha(halo, 0.0),
            );
        }
        // Until the land reaches it, the line runs on down to the horizon.
        let rest = 1.0 - phase(t, (LAND.0, LAND.0 + 0.8));
        if rest > 0.0 && horizon > end.max(top) {
            let y = end.max(top);
            ui.fill(
                Rect::new(cx - 0.75, y, 1.5, horizon - y),
                rgb(palette::ACCENT, 0.9 * rest),
            );
        }
        if t > 0.0 && phase(t, LINE) < 1.0 {
            glow_dot(ui, Vec2::new(cx, top), 16.0, 1.0 - phase(t, LINE).powi(2));
        }
    }

    /// The M, traced round from the foot of its V, then filled with steel;
    /// the meridian's bar grows through it.
    fn monogram(&self, ui: &mut Ui, l: &Layout, t: f32) {
        let at = |d: Vec2, mirror: bool| {
            let d = if mirror {
                Vec2::new(1000.0 - d.x, d.y)
            } else {
                d
            };
            l.m_at + (d - M_MIDDLE) * l.m_k
        };
        let design = m_half();
        let (top, foot) = m_span();
        let halves = [false, true].map(|mirror| design.map(|d| at(d, mirror)));
        let fill = ease_in_out(phase(t, FILL));
        let trace = phase(t, TRACE);

        if fill > 0.0 {
            for half in &halves {
                for quad in M_QUADS {
                    let corners = quad.map(|i| half[i]);
                    let colors = quad.map(|i| {
                        let depth = (design[i].y - top) / (foot - top);
                        let c = mix(rgb(STEEL, 1.0), rgb(STEEL_FOOT, 1.0), depth);
                        alpha(c, fill * ui.fade)
                    });
                    let px = corners.map(|p| <[f32; 2]>::from((p + ui.shift) * ui.s));
                    ui.o.quad_shaded(px, colors);
                }
            }
        }

        // The outline: traced both ways round from the V, meeting at the stem.
        let traced = ease_in_out(trace);
        let rim = 1.0 - 0.7 * fill;
        if traced > 0.0 {
            let hot = rgb(HOT, rim);
            let glow = rgb(palette::ACCENT, 0.0);
            let glow_stops = [
                (-1.0, 0.0, glow),
                (0.0, 0.0, alpha(glow, 0.30 * rim)),
                (1.0, 0.0, glow),
            ];
            for half in &halves {
                let around: f32 = (0..8).map(|i| half[i].distance(half[(i + 1) % 8])).sum();
                for forward in [true, false] {
                    let path = along(half, TRACE_FROM, forward, traced * around * 0.5);
                    for pair in path.windows(2) {
                        ui.ribbon(pair[0], pair[1], 8.0, 8.0, &glow_stops);
                    }
                    ui.polyline(&path, 1.4, hot, false);
                    if trace < 1.0 {
                        if let Some(&head) = path.last() {
                            glow_dot(ui, head, 16.0, 1.0 - trace * trace);
                        }
                    }
                }
            }
        }

        // The meridian's bar, out from the middle of the V.
        let grow = ease_out(phase(t, BAR));
        if grow > 0.0 {
            let (half, top, foot) = M_BAR;
            let mid = at(Vec2::new(500.0, 505.0), false).y;
            let (y0, y1) = (
                mid + (at(Vec2::new(500.0, top), false).y - mid) * grow,
                mid + (at(Vec2::new(500.0, foot), false).y - mid) * grow,
            );
            let hw = (half * l.m_k * grow).max(0.75);
            let cx = l.m_at.x;
            let breath = 0.85 + 0.15 * (t * 1.7).sin() * phase(t, (INTRO, INTRO + 1.0));
            let accent = rgb(palette::ACCENT, 1.0);
            let halo = alpha(accent, 0.30 * breath * grow);
            let reach = 70.0 * l.m_k;
            let clear = alpha(halo, 0.0);
            let across = [
                (-1.0, 0.0, clear),
                (-0.45, 0.0, alpha(halo, 0.3 * halo[3])),
                (0.0, 0.0, halo),
                (0.45, 0.0, alpha(halo, 0.3 * halo[3])),
                (1.0, 0.0, clear),
            ];
            let (a, b) = (Vec2::new(cx, y0), Vec2::new(cx, y1));
            ui.ribbon(a, b, reach, reach, &across);
            let round = [
                (0.0, 0.0, halo),
                (0.55, 0.0, alpha(halo, 0.3 * halo[3])),
                (1.0, 0.0, clear),
            ];
            ui.ribbon_cap(a, Vec2::new(0.0, -1.0), reach, &round);
            ui.ribbon_cap(b, Vec2::new(0.0, 1.0), reach, &round);
            ui.fill(Rect::new(cx - hw, y0, hw * 2.0, y1 - y0), accent);
        }
    }

    /// The name either side of the meridian, a rule under it, the credit.
    fn title(&self, ui: &mut Ui, l: &Layout, t: f32) {
        let (cx, y) = (l.view.centre, l.title_y);
        let bold = ui::style(Face::Bold, 44.0, 1.0);
        let light = ui::style(Face::Light, 44.0, 1.0);
        let gap = 15.0;
        let e = ease_out(phase(t, TITLE));
        if e > 0.0 {
            let left = ui.text_width(bold, "Meridian");
            let right = ui.text_width(light, "Conflict");
            let slide = (1.0 - e) * 22.0;
            let saved = ui.fade;
            ui.fade *= e;
            let text = rgb(palette::TEXT, 1.0);
            // In toward the meridian from either side.
            ui.text(cx - gap - left - slide, y, bold, text, "Meridian");
            ui.text(cx + gap + slide, y, light, text, "Conflict");
            ui.fade = saved;
            // The rule, out from the meridian to the ends of the words.
            let rule = y + 32.0;
            let line = rgb(palette::LINE, 0.28 * e);
            let (l_len, r_len) = ((left + 6.0) * e, (right + 6.0) * e);
            ui.hline(cx - gap - l_len, rule, l_len, line);
            ui.hline(cx + gap, rule, r_len, line);
            ui.vline(cx - gap - l_len, rule - 3.0, 7.0, line);
            ui.vline(cx + gap + r_len, rule - 3.0, 7.0, line);
            ui.disc(Vec2::new(cx, rule + 0.5), 2.5, rgb(palette::ACCENT, e));
        }
        let credit = ease_out(phase(t, CREDIT));
        if credit > 0.0 {
            ui.text_centred(
                cx,
                y + 58.0 + (1.0 - credit) * 8.0,
                ui::style(Face::Medium, 15.0, 1.4),
                rgb(palette::DIM, credit),
                "By Joshowaaah",
            );
        }
    }

    /// What is loading and how far it has got: a bar filling out from the meridian.
    fn status(&mut self, ui: &mut Ui, t: f32, bar: f32, step: Option<&'static str>) {
        let step = step.unwrap_or("Ready");
        if step != self.step {
            self.was = self.step;
            self.step = step;
            self.changed = t;
        }
        let shown = ease_out(phase(t, STATUS));
        if shown <= 0.0 {
            return;
        }
        let (w, h) = (ui.size.x, ui.size.y);
        let (cx, y) = (w * 0.5, h - 64.0);
        let saved = ui.fade;
        ui.fade *= shown;

        let half = 210.0;
        ui.hline(cx - half, y, half * 2.0, rgb(palette::LINE, 0.14));
        let filled = bar.clamp(0.0, 1.0) * half;
        let accent = rgb(palette::ACCENT, 1.0);
        ui.fill(Rect::new(cx - filled, y - 0.5, filled * 2.0, 2.0), accent);
        for side in [-1.0, 1.0] {
            glow_dot(ui, Vec2::new(cx + side * filled, y + 0.5), 9.0, 0.7);
        }

        let words = ui::style(Face::Medium, 13.0, 0.6);
        // The old words rise out before the new ones rise in, so the two never overlap.
        let swap = ((t - self.changed) / 0.4).clamp(0.0, 1.0);
        let (out, into) = (ease_out(swap * 2.0), ease_out(swap * 2.0 - 1.0));
        if out < 1.0 && !self.was.is_empty() {
            ui.text_centred(
                cx,
                y - 22.0 - out * 8.0,
                words,
                rgb(palette::DIM, 1.0 - out),
                self.was,
            );
        }
        let into = if self.was.is_empty() { swap } else { into };
        ui.text_centred(
            cx,
            y - 22.0 + (1.0 - into) * 8.0,
            words,
            rgb(palette::DIM, into),
            self.step,
        );
        ui.text_right(
            w - 48.0,
            y,
            ui::type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &format!(
                "{}   \u{b7}   {}",
                crate::build_label(),
                env!("CARGO_PKG_VERSION")
            ),
        );
        ui.fade = saved;
    }
}

/// A soft round glow's stops, from `peak` in the middle to nothing at the rim.
fn falloff(peak: f32) -> Vec<(f32, f32, Color)> {
    let dawn = rgb(palette::ACCENT_DEEP, 0.0);
    (0..=16)
        .map(|i| {
            let k = i as f32 / 16.0;
            let a = peak * (-k * k * 5.0).exp() * (1.0 - k * k);
            (k, 0.0, alpha(dawn, a))
        })
        .collect()
}

/// A point of light: a hot core in a soft orange bloom.
fn glow_dot(ui: &mut Ui, at: Vec2, radius: f32, strength: f32) {
    if strength <= 0.0 {
        return;
    }
    let glow = rgb(palette::ACCENT, 0.0);
    let stops = [
        (0.0, 0.0, alpha(glow, 0.55 * strength)),
        (0.25, 0.0, alpha(glow, 0.22 * strength)),
        (1.0, 0.0, glow),
    ];
    ui.ribbon_cap(at, Vec2::new(0.0, -1.0), radius, &stops);
    ui.ribbon_cap(at, Vec2::new(0.0, 1.0), radius, &stops);
    ui.disc(at, (radius * 0.12).clamp(1.2, 3.0), rgb(HOT, strength));
}

/// The first `length` of the way round the closed outline `pts`, from corner
/// `start`, forward or backward.
fn along(pts: &[Vec2; 8], start: usize, forward: bool, length: f32) -> Vec<Vec2> {
    let n = pts.len();
    let mut path = vec![pts[start]];
    let (mut at, mut left) = (start, length);
    for _ in 0..n {
        let next = if forward {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        };
        let step = pts[at].distance(pts[next]);
        if step >= left {
            path.push(pts[at].lerp(pts[next], left / step.max(1e-3)));
            break;
        }
        path.push(pts[next]);
        left -= step;
        at = next;
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_outline_is_traced_whole_from_both_ways() {
        let half = m_half();
        let around: f32 = (0..8).map(|i| half[i].distance(half[(i + 1) % 8])).sum();
        let a = along(&half, TRACE_FROM, true, around * 0.5);
        let b = along(&half, TRACE_FROM, false, around * 0.5);
        let (ea, eb) = (a[a.len() - 1], b[b.len() - 1]);
        assert!(ea.distance(eb) < 0.5, "the two traces meet: {ea} {eb}");
    }

    #[test]
    fn the_valley_floor_is_under_the_eye() {
        for z in (0..40).map(|i| i as f32 * 97.0) {
            assert!(
                height(0.0, z) < EYE * 0.5,
                "floor at {z}: {}",
                height(0.0, z)
            );
        }
    }

    #[test]
    fn the_m_is_filled_by_convex_quads() {
        for quad in M_QUADS {
            let p = quad.map(|i| m_half()[i]);
            for i in 0..4 {
                let (a, b, c) = (p[i], p[(i + 1) % 4], p[(i + 2) % 4]);
                assert!(
                    (b - a).perp_dot(c - b) >= -1e-3,
                    "quad {quad:?} turns one way"
                );
            }
        }
    }
}
