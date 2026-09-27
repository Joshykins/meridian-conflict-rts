//! The siege chart: the Progenitor, every front flowing toward the defenders
//! in its domain's colour, the harbor, the node sites and the landing zones,
//! each held zone in its defender's colour. Survival's set-up and the co-op
//! lobby both draw it.

use super::marks::*;
use super::Theatre;
use crate::audio::Sfx;
use crate::setup::TEAM_COLORS;
use crate::ui::{id, ink, palette, preview, rgb, type_scale, Color, Rect, Ui};
use glam::Vec2;
use mc_data::survival::Domain;
use mc_sim::SurvivalRules;
use std::f32::consts::TAU;

/// A landing zone someone deploys at.
pub struct Holder {
    /// Index into the theatre's spawns.
    pub spawn: usize,
    pub color: [f32; 3],
    /// This machine's commander.
    pub you: bool,
    pub name: String,
}

/// What the chart shows.
pub struct Siege<'a> {
    pub theatre: &'a Theatre,
    /// Keys the chart's fade-in: a new theatre fades up.
    pub key: usize,
    pub rules: &'a SurvivalRules,
    /// A fronts chip under the pointer: its lanes light up.
    pub hover_domain: Option<Domain>,
    pub holders: &'a [Holder],
    /// Clicking a free zone, or its number key, moves your commander there.
    pub can_pick: bool,
    /// Image slot for the chart, and which theatre is drawn in it now.
    pub slot: usize,
}

/// Where the chart's zone markers were drawn, in canvas points (tests click them).
pub type Markers = Vec<Vec2>;

/// A front as drawn: from the engine, down its path, and the last leg to your zone.
struct Route {
    pts: Vec<Vec2>,
    /// Cumulative length at each point.
    at: Vec<f32>,
    /// Index of the path's first and last points in `pts`.
    first: usize,
    last: usize,
    /// Length on the ground, path and last leg, metres.
    metres: f32,
}

impl Route {
    fn new(pts: Vec<Vec2>, first: usize, last: usize, metres: f32) -> Route {
        let mut at = vec![0.0];
        for w in pts.windows(2) {
            at.push(at.last().unwrap() + w[0].distance(w[1]));
        }
        Route {
            pts,
            at,
            first,
            last,
            metres,
        }
    }

    fn total(&self) -> f32 {
        *self.at.last().unwrap_or(&0.0)
    }

    /// Point and heading `s` along.
    fn sample(&self, s: f32) -> (Vec2, Vec2) {
        let i = self
            .at
            .partition_point(|&a| a <= s)
            .clamp(1, self.pts.len() - 1);
        let (a, b) = (self.pts[i - 1], self.pts[i]);
        let len = self.at[i] - self.at[i - 1];
        let t = if len > 0.0 {
            (s - self.at[i - 1]) / len
        } else {
            0.0
        };
        (a + (b - a) * t.clamp(0.0, 1.0), (b - a).normalize_or_zero())
    }

    fn distance(&self, p: Vec2) -> f32 {
        self.pts
            .windows(2)
            .map(|w| {
                let e = w[1] - w[0];
                let t = ((p - w[0]).dot(e) / e.length_squared().max(1e-4)).clamp(0.0, 1.0);
                p.distance(w[0] + e * t)
            })
            .fold(f32::MAX, f32::min)
    }
}

fn dashed(ui: &mut Ui, a: Vec2, b: Vec2, dash: f32, gap: f32, phase: f32, t: f32, color: Color) {
    let len = a.distance(b);
    if len < 0.5 {
        return;
    }
    let d = (b - a) / len;
    let mut s = phase.rem_euclid(dash + gap) - (dash + gap);
    while s < len {
        let (s0, s1) = (s.max(0.0), (s + dash).min(len));
        if s1 > s0 {
            ui.stroke(a + d * s0, a + d * s1, t, color);
        }
        s += dash + gap;
    }
}

/// The chart in `area`; the zone picked (a free one clicked or keyed), if any.
pub fn chart(
    ui: &mut Ui,
    view: &Siege,
    drawn: &mut Option<usize>,
    markers: &mut Markers,
    area: Rect,
) -> Option<usize> {
    let t = view.theatre;
    if *drawn != Some(view.key) {
        ui.o.set_image(
            view.slot,
            preview::SIZE,
            preview::SIZE,
            &preview::render(&t.map, t.climate),
        );
        *drawn = Some(view.key);
    }
    let map = t.map.clone();
    let layout = t.layout.clone();
    let side = area.w.min(area.h - 84.0);
    let frame = Rect::new(area.x + (area.w - side) * 0.5, area.y, side, side);
    let origin = Vec2::new(frame.x, frame.y);
    let place = |p: (f32, f32)| origin + preview::locate(&map, [p.0, p.1], side);
    let shown = ui.ease(id("survival-chart-shown", view.key), 1.0, 5.0);
    ui.fill(frame, ink(0.85));
    // A little darker than skirmish's chart: the fronts are what should read.
    let tone = 0.42 * shown;
    ui.image(
        view.slot,
        [0.0, 0.0, preview::SIZE as f32, preview::SIZE as f32],
        frame,
        [tone, tone, tone, 1.0],
    );
    ui.fill(frame, ink(0.38));
    for k in 1..4 {
        let f = k as f32 / 4.0;
        ui.vline(
            frame.x + frame.w * f,
            frame.y,
            frame.h,
            rgb(palette::LINE, 0.06),
        );
        ui.hline(
            frame.x,
            frame.y + frame.h * f,
            frame.w,
            rgb(palette::LINE, 0.06),
        );
    }
    ui.frame(frame, rgb(palette::LINE, 0.25));
    ui.brackets(frame.inset(-6.0), 14.0, rgb(palette::ACCENT, 0.7));
    let size_m = map.info().size_metres().to_f32();
    let km_pts = frame.w / (size_m[0].max(size_m[1]) / 1000.0);
    let bar_km = if size_m[0] > 30_000.0 { 10.0 } else { 2.0 };
    ui.fill(
        Rect::new(frame.x + 14.0, frame.bottom() - 16.0, km_pts * bar_km, 2.0),
        rgb(palette::TEXT, 0.8),
    );
    ui.text(
        frame.x + 14.0,
        frame.bottom() - 28.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 0.8),
        &format!("{bar_km:.0} km"),
    );
    ui.text_right(
        frame.right() - 12.0,
        frame.y + 16.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 0.6),
        "N",
    );
    ui.stroke(
        Vec2::new(frame.right() - 16.0, frame.y + 44.0),
        Vec2::new(frame.right() - 16.0, frame.y + 26.0),
        1.2,
        rgb(palette::TEXT, 0.6),
    );

    let starts = map.start_positions();
    let zone_world = |i: usize| -> Option<(f32, f32)> {
        let s = layout.spawns.get(i)?;
        let p = starts.get(s.start as usize)?.to_f32();
        Some((p[0], p[1]))
    };
    let holder = |i: usize| view.holders.iter().find(|h| h.spawn == i);
    // The fronts run on to the defender nearest the engine, as the waves do.
    let lead = view
        .holders
        .iter()
        .filter_map(|h| Some((zone_world(h.spawn)?, h)))
        .min_by(|a, b| {
            let d = |p: (f32, f32)| (p.0 - layout.engine.0).hypot(p.1 - layout.engine.1);
            d(a.0).total_cmp(&d(b.0))
        });
    let you = lead.map_or(layout.engine, |(p, _)| p);
    let lead_name = lead.map_or("the defenders", |(_, h)| {
        layout
            .spawns
            .get(h.spawn)
            .map_or("the defenders", |s| s.name.as_str())
    });
    let eng = place(layout.engine);

    // Pointer handling first, drawing after: markers sit on top of the fronts
    // and take the pointer before them.
    let mut tip: Option<Tip> = None;
    let mut spawn_res = Vec::new();
    let mut pick_spawn = None;
    // A zone another commander holds cannot be taken.
    let free = |i: usize| view.can_pick && holder(i).is_none();
    markers.clear();
    for i in 0..layout.spawns.len() {
        let Some(w) = zone_world(i) else {
            spawn_res.push(Default::default());
            markers.push(Vec2::ZERO);
            continue;
        };
        let p = place(w);
        markers.push(p + ui.shift);
        let res = ui.interact(
            id("survival-zone", i),
            Rect::new(p.x - 18.0, p.y - 18.0, 36.0, 36.0),
            true,
        );
        if res.clicked && free(i) {
            pick_spawn = Some(i);
        }
        spawn_res.push(res);
    }
    let engine_res = ui.interact_with(
        id("survival-engine", 0),
        Rect::new(eng.x - 20.0, eng.y - 20.0, 40.0, 40.0),
        true,
        false,
    );
    let harbor_res = layout.harbor.map(|h| {
        ui.interact_with(
            id("survival-harbor", 0),
            Rect::new(place(h).x - 12.0, place(h).y - 12.0, 24.0, 24.0),
            true,
            false,
        )
    });
    let site_res: Vec<_> = layout
        .node_sites
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let p = place(n.at);
            ui.interact_with(
                id("survival-site", i),
                Rect::new(p.x - 9.0, p.y - 9.0, 18.0, 18.0),
                true,
                false,
            )
        })
        .collect();

    // Keys 1-9 pick a zone.
    if ui.mem.editing.is_none() && ui.mem.popup.is_none() && ui.interactive {
        if let Some(d) = ui.input.typed.chars().filter_map(|c| c.to_digit(10)).next() {
            let i = d as usize;
            if (1..=layout.spawns.len()).contains(&i) && free(i - 1) {
                pick_spawn = Some(i - 1);
            }
        }
    }

    // The fronts, and which one the pointer is on.
    let routes: Vec<Route> = layout
        .fronts
        .iter()
        .map(|f| {
            let mut pts = vec![eng];
            if f.domain == Domain::Naval {
                if let Some(h) = layout.harbor {
                    if place(h).distance(place(f.path[0])) > 1.0 {
                        pts.push(place(h));
                    }
                }
            }
            let first = pts.len();
            pts.extend(f.path.iter().map(|&p| place(p)));
            let last = pts.len() - 1;
            pts.push(place(you));
            let mut metres = 0.0;
            let world: Vec<Vec2> = f
                .path
                .iter()
                .map(|&(x, y)| Vec2::new(x, y))
                .chain([Vec2::new(you.0, you.1)])
                .collect();
            for w in world.windows(2) {
                metres += w[0].distance(w[1]);
            }
            Route::new(pts, first, last, metres)
        })
        .collect();
    let over_marker = spawn_res.iter().any(|r| r.hovered)
        || engine_res.hovered
        || harbor_res.is_some_and(|r| r.hovered)
        || site_res.iter().any(|r| r.hovered);
    let cursor = ui.cursor - ui.shift;
    let hovered_front =
        (ui.interactive && ui.mem.popup.is_none() && !over_marker && frame.contains(cursor))
            .then(|| {
                routes
                    .iter()
                    .enumerate()
                    .map(|(i, r)| (i, r.distance(cursor)))
                    .filter(|(_, d)| *d < 8.0)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(i, _)| i)
            })
            .flatten();

    let tick = ui.time;
    for (i, (f, route)) in layout.fronts.iter().zip(&routes).enumerate() {
        let on = view.rules.has(f.domain);
        let on_k = ui.ease(id("survival-front-on", i), if on { 1.0 } else { 0.0 }, 8.0);
        let lit = hovered_front == Some(i) || view.hover_domain == Some(f.domain);
        let lit_k = ui.ease(
            id("survival-front-lit", i),
            if lit { 1.0 } else { 0.0 },
            14.0,
        );
        let base = 0.16 + 0.34 * on_k + 0.4 * lit_k;
        let width = 1.3 + 1.2 * lit_k;
        if lit_k > 0.01 {
            for w in route.pts.windows(2) {
                ui.stroke(w[0], w[1], 8.0, dcol(f.domain, 0.12 * lit_k));
            }
        }
        for (k, w) in route.pts.windows(2).enumerate() {
            let leg = k >= route.last;
            let stub = k < route.first;
            let a = if stub { base * 0.5 } else { base };
            let c = dcol(f.domain, a);
            let phase = if on { tick * 8.0 } else { 0.0 };
            match (f.domain, leg) {
                (_, true) => dashed(ui, w[0], w[1], 5.0, 5.0, phase, width, c),
                (Domain::Air, false) => dashed(ui, w[0], w[1], 12.0, 6.0, phase, width, c),
                (Domain::Naval, false) => {
                    let n = (w[1] - w[0]).normalize_or_zero().perp() * 1.8;
                    ui.stroke(w[0] + n, w[1] + n, width * 0.8, c);
                    ui.stroke(w[0] - n, w[1] - n, width * 0.8, c);
                }
                (Domain::Land, false) => ui.stroke(w[0], w[1], width, c),
            }
        }
        // Chevrons flowing from the engine toward your zone.
        let total = route.total();
        let spacing = 26.0;
        let speed = match f.domain {
            Domain::Land => 16.0,
            Domain::Air => 30.0,
            Domain::Naval => 11.0,
        };
        let offset = if on {
            (tick * speed).rem_euclid(spacing)
        } else {
            spacing * 0.5
        };
        let mut s = offset;
        while s < total {
            let (p, d) = route.sample(s);
            let n = d.perp();
            let fade = (s / 30.0).min(1.0) * ((total - s) / 30.0).clamp(0.0, 1.0);
            let a = if on {
                (0.55 + 0.45 * lit_k) * fade
            } else {
                0.14 * fade
            } * (0.25 + 0.75 * on_k.max(0.3));
            let size = 4.0 + 1.5 * lit_k;
            ui.stroke(p - d * size + n * size, p, 1.6, dcol(f.domain, a));
            ui.stroke(p - d * size - n * size, p, 1.6, dcol(f.domain, a));
            s += spacing;
        }
        // A tag half way down the path: the domain glyph and the front's name.
        let mid_s = (route.at[route.first] + route.at[route.last]) * 0.5;
        let (p, _) = route.sample(mid_s);
        let ta = 0.45 + 0.4 * on_k + 0.15 * lit_k;
        ui.disc(p, 9.0 + 1.5 * lit_k, ink(0.85));
        ui.arc(p, 9.0 + 1.5 * lit_k, 0.0, TAU, 1.2, dcol(f.domain, ta));
        domain_glyph(ui, f.domain, p, 5.0, dcol(f.domain, ta));
        let (st, name) = ui.fitted(type_scale::MICRO, &f.name, 130.0);
        let nw = ui.text_width(st, &name);
        let lx = if p.x + 14.0 + nw > frame.right() - 6.0 {
            p.x - 14.0 - nw
        } else {
            p.x + 14.0
        };
        ui.fill(
            Rect::new(lx - 4.0, p.y - 8.0, nw + 8.0, 16.0),
            ink(0.55 * ta),
        );
        ui.text(lx, p.y, st, rgb(palette::TEXT, ta), &name);
        if hovered_front == Some(i) {
            let zone = lead_name;
            let what = match f.domain {
                Domain::Land => "Ground forces march this road",
                Domain::Air => "Aircraft fly this corridor",
                Domain::Naval => "Ships sail this lane out of the harbor",
            };
            tip = Some(Tip {
                at: cursor,
                title: f.name.clone(),
                accent: dcol(f.domain, 1.0),
                lines: vec![
                    format!(
                        "{} front  \u{b7}  {} to {zone}",
                        f.domain.label(),
                        km(route.metres)
                    ),
                    if on {
                        what.to_owned()
                    } else {
                        format!(
                            "Off: no {} attacks this match",
                            f.domain.label().to_lowercase()
                        )
                    },
                ],
                glow: lit_k.max(0.6),
            });
        }
    }

    // Node sites.
    let nodes_on = view.rules.nodes > 0;
    let nodes_k = ui.ease(
        id("survival-nodes-on", 0),
        if nodes_on { 1.0 } else { 0.0 },
        8.0,
    );
    for (i, (n, res)) in layout.node_sites.iter().zip(&site_res).enumerate() {
        let p = place(n.at);
        let a = 0.35 + 0.55 * nodes_k + 0.1 * res.glow;
        let fill = if n.domain == Domain::Naval {
            dcol(Domain::Naval, 0.75 * a)
        } else {
            engine(0.35 * a)
        };
        diamond(ui, p, 5.0 + 1.5 * res.glow, fill, engine(a));
        if res.hovered {
            tip = Some(Tip {
                at: p,
                title: n.name.clone(),
                accent: engine(1.0),
                lines: vec![
                    format!("{} Shaper site", n.domain.label()),
                    if nodes_on {
                        "The Progenitor may raise a Shaper here".to_owned()
                    } else {
                        "Shapers are off".to_owned()
                    },
                ],
                glow: res.glow,
            });
            let _ = i;
        }
    }

    // Harbor.
    if let (Some(h), Some(res)) = (layout.harbor, harbor_res) {
        let p = place(h);
        let a = if view.rules.has(Domain::Naval) {
            0.95
        } else {
            0.4
        };
        ui.disc(p, 9.0, ink(0.85));
        ui.arc(p, 9.0, 0.0, TAU, 1.2, dcol(Domain::Naval, a));
        anchor(ui, p, 5.5, dcol(Domain::Naval, a));
        if res.hovered {
            tip = Some(Tip {
                at: p,
                title: "Harbor".into(),
                accent: dcol(Domain::Naval, 1.0),
                lines: vec!["The engine's slipways: every ship it prints launches here".into()],
                glow: res.glow,
            });
        }
    }

    // The engine.
    engine_mark(ui, eng, 11.0 + 1.5 * engine_res.glow, 1.0, true);
    let (lx, ly) = (eng.x, eng.y - 40.0);
    let label = "The Progenitor";
    let lw = ui.text_width(type_scale::MICRO, label);
    let lx = (lx - lw * 0.5).clamp(frame.x + 6.0, (frame.right() - lw - 6.0).max(frame.x + 6.0));
    let ly = if ly < frame.y + 12.0 {
        eng.y + 42.0
    } else {
        ly
    };
    ui.fill(Rect::new(lx - 5.0, ly - 8.0, lw + 10.0, 16.0), ink(0.7));
    ui.text(lx, ly, type_scale::MICRO, engine(1.0), label);
    if engine_res.hovered {
        tip = Some(Tip {
            at: eng,
            title: "The Progenitor".into(),
            accent: engine(1.0),
            lines: vec!["Prints every round's attack in its bays and sends it down the fronts. It cannot be destroyed: outlast it.".into()],
            glow: engine_res.glow,
        });
    }

    // Landing zones.
    for (i, res) in spawn_res.iter().enumerate() {
        let Some(w) = zone_world(i) else { continue };
        if let Some(t) = zone(ui, view, i, place(w), res, view.can_pick) {
            tip = Some(t);
        }
    }
    if pick_spawn.is_some() {
        ui.audio.play(Sfx::Select);
    }
    if let Some(t) = &tip {
        draw_tip(ui, t, frame);
    }

    // Under the chart: the theatre, where you deploy, and the legend.
    let y = frame.bottom() + 26.0;
    let end = ui.text(
        frame.x,
        y,
        type_scale::ITEM,
        rgb(palette::TEXT, 1.0),
        map.name(),
    );
    let end = ui.text(
        end + 14.0,
        y + 1.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &format!(
            "{:.1} \u{d7} {:.1} km",
            size_m[0] / 1000.0,
            size_m[1] / 1000.0
        ),
    );
    let yours = view.holders.iter().find(|h| h.you).or(view.holders.first());
    if let Some((h, s)) = yours.and_then(|h| Some((h, layout.spawns.get(h.spawn)?))) {
        let x = end + 22.0;
        let w = frame.right() - x;
        let head = format!("Deploying at {}  \u{b7}  ", s.name);
        let hw = ui.text_width(type_scale::MICRO, &head);
        let c = h.color;
        ui.text(
            x,
            y + 1.0,
            type_scale::MICRO,
            [c[0], c[1], c[2], 1.0],
            &head,
        );
        ui.text_fit_left(
            x + hw,
            y + 1.0,
            (w - hw).max(0.0),
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            &s.blurb,
        );
    }
    legend(ui, frame.x, y + 30.0, frame.w);
    pick_spawn
}

/// Landing zone `i` at `p`: in its holder's colour, pulsing when it is yours.
fn zone(
    ui: &mut Ui,
    view: &Siege,
    i: usize,
    p: Vec2,
    res: &crate::ui::Response,
    can_pick: bool,
) -> Option<Tip> {
    let layout = &view.theatre.layout;
    let held = view.holders.iter().find(|h| h.spawn == i);
    let mine = held.is_some_and(|h| h.you);
    let sel = ui.ease(
        id("survival-zone-sel", i),
        if held.is_some() { 1.0 } else { 0.0 },
        10.0,
    );
    let color = match held {
        Some(h) => [h.color[0], h.color[1], h.color[2], 1.0],
        None => rgb(palette::TEXT, 0.75 + 0.25 * res.glow),
    };
    let r = 13.0 + 2.0 * res.glow + 2.0 * sel;
    ui.disc(p, r, ink(0.85));
    ui.arc(p, r, 0.0, TAU, 1.2 + 1.2 * sel, color);
    if mine {
        let pulse = (ui.time * 0.8).fract();
        ui.arc(
            p,
            r + 1.0 + 18.0 * pulse,
            0.0,
            TAU,
            1.4,
            [color[0], color[1], color[2], 0.8 * (1.0 - pulse)],
        );
        // Corner ticks: the zone you hold.
        ui.brackets(
            Rect::new(p.x - r - 7.0, p.y - r - 7.0, 2.0 * r + 14.0, 2.0 * r + 14.0),
            5.0,
            [color[0], color[1], color[2], 0.9 * sel],
        );
    }
    ui.text_centred(
        p.x + 0.5,
        p.y,
        type_scale::VALUE,
        rgb(palette::TEXT, 1.0),
        &format!("{}", i + 1),
    );
    let name = &layout.spawns[i].name;
    let nw = ui.text_width(type_scale::CAPTION, name);
    let ny = p.y - r - 14.0;
    ui.fill(
        Rect::new(p.x - nw * 0.5 - 5.0, ny - 8.0, nw + 10.0, 16.0),
        ink(0.6),
    );
    ui.text_centred(
        p.x,
        ny,
        type_scale::CAPTION,
        if held.is_some() {
            color
        } else {
            rgb(palette::TEXT, 0.8)
        },
        name,
    );
    (res.glow > 0.05).then(|| Tip {
        at: p,
        title: name.clone(),
        accent: if held.is_some() {
            color
        } else {
            rgb(palette::ACCENT, 1.0)
        },
        lines: vec![
            layout.spawns[i].blurb.clone(),
            match held {
                Some(h) if h.you => "Your landing zone".into(),
                Some(h) => format!("{} deploys here", h.name),
                None if can_pick => format!("Click or press {} to deploy here", i + 1),
                None => "Nobody deploys here".into(),
            },
        ],
        glow: res.glow,
    })
}

/// The key under the chart, `w` wide: an item that would run past it starts a new line.
fn legend(ui: &mut Ui, x: f32, y: f32, w: f32) {
    let mut cx = x;
    let mut y = y;
    let mut item = |ui: &mut Ui, cx: &mut f32, label: &str, draw: &dyn Fn(&mut Ui, Vec2)| {
        if *cx > x && *cx + 22.0 + ui.text_width(type_scale::MICRO, label) > x + w {
            *cx = x;
            y += 20.0;
        }
        draw(ui, Vec2::new(*cx + 8.0, y));
        *cx = ui.text(
            *cx + 22.0,
            y,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            label,
        ) + 20.0;
    };
    item(ui, &mut cx, "Progenitor", &|ui, c| {
        engine_mark(ui, c, 6.0, 1.0, false)
    });
    for d in Domain::ALL {
        let label = format!("{} Front", d.label());
        item(ui, &mut cx, &label, &|ui, c| {
            ui.stroke(c - Vec2::X * 9.0, c + Vec2::X * 9.0, 1.4, dcol(d, 0.5));
            ui.stroke(
                c + Vec2::new(-1.0, -4.0),
                c + Vec2::new(3.0, 0.0),
                1.6,
                dcol(d, 1.0),
            );
            ui.stroke(
                c + Vec2::new(-1.0, 4.0),
                c + Vec2::new(3.0, 0.0),
                1.6,
                dcol(d, 1.0),
            );
        });
    }
    item(ui, &mut cx, "Shaper Site", &|ui, c| {
        diamond(ui, c, 5.0, engine(0.35), engine(0.9))
    });
    item(ui, &mut cx, "Harbor", &|ui, c| {
        anchor(ui, c, 6.0, dcol(Domain::Naval, 1.0))
    });
    let mine = TEAM_COLORS[0];
    item(ui, &mut cx, "Landing Zone", &|ui, c| {
        ui.arc(c, 6.0, 0.0, TAU, 1.8, [mine[0], mine[1], mine[2], 1.0])
    });
}
