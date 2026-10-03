//! A core mine being placed in another's reach (`orders::mine_reach`). Inside a
//! mine in sight it cannot go: that mine's circle lights up red, a bar shows how
//! far in the new one is, and the nearest open site is marked. Inside a mine the
//! side or an ally has only planned it may go, with a warning: that plan's circle
//! and whose it is show in amber.

use super::{ground, on_screen, overview_height, smooth, Scene};
use crate::hud::HEALTHY;
use crate::orders::mine_reach::Crowding;
use crate::ui::{palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use std::f32::consts::TAU;

/// Points round a circle on the ground.
const RING_POINTS: usize = 180;

/// What stands in the way of the mine under the pointer.
pub(super) struct Verdict {
    /// A mine in sight it would stand in the reach of: it cannot go.
    pub(super) crowd: Option<Crowding>,
    /// A planned mine it would: it may go, warned.
    pub(super) warn: Option<Crowding>,
    /// The nearest site out of every mine's reach, while it cannot go.
    pub(super) open: Option<Vec2>,
}

impl Verdict {
    pub(super) fn of(s: &Scene, blueprint: mc_data::BlueprintId, at: Vec2) -> Verdict {
        let reach = s
            .blueprints
            .unit(blueprint)
            .mine
            .map_or(0.0, |m| m.reach.to_f32());
        let crowd = crate::orders::mine_reach::standing(s.view, s.blueprints, reach, at);
        let warn = if crowd.is_none() {
            crate::orders::mine_reach::planned(s.view, s.blueprints, blueprint, at, None)
        } else {
            None
        };
        Verdict {
            crowd,
            warn,
            open: crowd.and(s.placing_open),
        }
    }

    /// The ghost's territory tone when something is in its way.
    pub(super) fn tone(&self) -> Option<u32> {
        if self.crowd.is_some() {
            Some(palette::BAD)
        } else if self.warn.is_some() {
            Some(palette::WARN)
        } else {
            None
        }
    }
}

/// Whose a mine is, for the cards: "your", "Kara's", "an enemy".
pub(crate) fn whose(view: &crate::game::View, owner: u8) -> String {
    let team = |p: u8| view.status.players.get(p as usize).map_or(p, |s| s.team);
    if owner == view.local {
        "your".to_owned()
    } else if team(owner) != team(view.local) {
        "an enemy".to_owned()
    } else {
        view.status
            .players
            .get(owner as usize)
            .map_or_else(|| "an ally's".to_owned(), |p| format!("{}'s", p.name))
    }
}

/// A circle on the ground, `width` points wide.
fn ring(ui: &mut Ui, s: &Scene, centre: Vec2, r: f32, width: f32, color: crate::ui::Color) {
    let scale = ui.s;
    let pts: Vec<Option<Vec2>> = (0..=RING_POINTS)
        .map(|i| {
            let a = (i % RING_POINTS) as f32 / RING_POINTS as f32 * TAU;
            ground(s, scale, centre + Vec2::from_angle(a) * r)
        })
        .collect();
    smooth(ui, s, &pts, width, color);
}

/// A circle on the ground in dashes marching round it, `on` of every `period` points.
#[expect(
    clippy::too_many_arguments,
    reason = "one ring's look: where, how big, how wide, its colour and its dashes"
)]
fn dashed_ring(
    ui: &mut Ui,
    s: &Scene,
    centre: Vec2,
    r: f32,
    width: f32,
    color: crate::ui::Color,
    phase: f32,
    (on, period): (f32, f32),
) {
    let pts: Vec<glam::Vec3> = (0..=RING_POINTS)
        .map(|i| {
            let a = (i % RING_POINTS) as f32 / RING_POINTS as f32 * TAU;
            let p = centre + Vec2::from_angle(a) * r;
            p.extend(overview_height(s.map, p) + 1.5)
        })
        .collect();
    super::dashed_by(ui, s, &pts, width, color, phase, None, on, period);
}

/// The marks on the ground: the circle in the way, the bar out of it and the
/// nearest open site; or the planned circle being overlapped.
pub(super) fn draw(ui: &mut Ui, s: &Scene, at: Vec2, v: &Verdict, far: f32, time: f32) {
    let scale = ui.s;
    let pulse = 0.5 + 0.5 * (time * 4.5).sin();
    if let Some(c) = v.crowd {
        // The circle it must keep out of, breathing red, a softer edge just inside.
        ring(
            ui,
            s,
            c.at,
            c.keep,
            2.2 + far * 1.8,
            rgb(palette::BAD, 0.55 + 0.35 * pulse),
        );
        ring(
            ui,
            s,
            c.at,
            c.keep * 0.985,
            5.0 + far * 3.0,
            rgb(palette::BAD, 0.12 + 0.08 * pulse),
        );
        // The mine in the way, ringed where it stands.
        if let Some(p) = ground(s, scale, c.at) {
            ui.arc(p, 10.0 + 4.0 * pulse, 0.0, TAU, 2.0, rgb(palette::BAD, 0.9));
            ui.disc(p, 3.0, rgb(palette::BAD, 0.9));
        }
        // How far in: a bar from the ghost straight out to the edge (the card says by how much).
        let out = (at - c.at).try_normalize().unwrap_or(Vec2::X);
        let edge = c.at + out * c.keep;
        if let (Some(a), Some(b)) = (ground(s, scale, at), ground(s, scale, edge)) {
            ui.stroke(a, b, 2.4, rgb(palette::BAD, 0.85));
            let d = (b - a).try_normalize().unwrap_or(Vec2::X);
            let n = d.perp();
            ui.triangle(
                b,
                b - d * 9.0 + n * 5.0,
                b - d * 9.0 - n * 5.0,
                rgb(palette::BAD, 0.95),
            );
        }
        if let Some(open) = v.open {
            draw_open(ui, s, at, open, time);
        }
    } else if let Some(w) = v.warn {
        // The plan's circle in marching amber dashes, and whose it is.
        dashed_ring(
            ui,
            s,
            w.at,
            w.keep,
            2.0 + far * 1.6,
            rgb(palette::WARN, 0.6 + 0.3 * pulse),
            time * 24.0,
            (12.0, 18.0),
        );
        if let Some(p) = ground(s, scale, w.at) {
            ui.arc(
                p,
                9.0 + 3.0 * pulse,
                0.0,
                TAU,
                1.6,
                rgb(palette::WARN, 0.85),
            );
            let text = format!("{} planned mine", capitalised(&whose(s.view, w.owner)));
            pill(ui, p + Vec2::new(0.0, -26.0), &text, palette::WARN);
        }
    }
}

/// The nearest open site: a line of chevrons from the ghost and a marked lot.
fn draw_open(ui: &mut Ui, s: &Scene, from: Vec2, open: Vec2, time: f32) {
    let scale = ui.s;
    let (Some(a), Some(b)) = (ground(s, scale, from), ground(s, scale, open)) else {
        return;
    };
    let length = a.distance(b);
    if length < 8.0 {
        return;
    }
    let d = (b - a) / length;
    let n = d.perp();
    let tone = |k: f32| rgb(HEALTHY, k);
    // Chevrons running from the ghost to the site.
    let step = 22.0;
    let count = ((length - 18.0) / step).floor().max(0.0) as usize;
    for i in 0..count {
        let t = (i as f32 + (time * 1.6).fract()) * step + 9.0;
        if t > length - 14.0 {
            continue;
        }
        let fade = (t / 40.0).min(1.0) * ((length - 14.0 - t) / 30.0).clamp(0.0, 1.0);
        let c = a + d * t;
        let col = tone(0.75 * fade);
        ui.stroke(c - d * 4.0 + n * 5.0, c + d * 2.0, 2.0, col);
        ui.stroke(c - d * 4.0 - n * 5.0, c + d * 2.0, 2.0, col);
    }
    // The lot itself: a square with corner ticks, breathing.
    let pulse = 0.5 + 0.5 * (time * 3.0).sin();
    let r = 9.0 + 2.0 * pulse;
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let corner = b + Vec2::new(sx, sy) * r;
        ui.stroke(corner, corner - Vec2::new(sx * 5.0, 0.0), 2.0, tone(0.95));
        ui.stroke(corner, corner - Vec2::new(0.0, sy * 5.0), 2.0, tone(0.95));
    }
    ui.disc(b, 2.5, tone(0.95));
    let metres = from.distance(open);
    // The label past the lot, along the way there, clear of the chevrons and the card.
    let text = format!("Nearest open site  \u{b7}  {metres:.0} m");
    let half = ui.text_width(type_scale::CAPTION, &text) * 0.5 + 9.0;
    pill(
        ui,
        b + d * (22.0 + half * d.x.abs() + 10.0 * d.y.abs()),
        &text,
        HEALTHY,
    );
}

/// A small frosted label centred on `at`, an edge of `tone` down its left.
fn pill(ui: &mut Ui, at: Vec2, text: &str, tone: u32) {
    let w = ui.text_width(type_scale::CAPTION, text) + 18.0;
    let r = Rect::new(at.x - w * 0.5, at.y - 10.0, w, 20.0);
    ui.frost(r, 0.9);
    ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(tone, 1.0));
    ui.text(
        r.x + 10.0,
        r.y + 3.0,
        type_scale::CAPTION,
        rgb(tone, 1.0),
        text,
    );
}

fn capitalised(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

/// The card by the ghost when it cannot go: why, and what to do. `c` is the ghost
/// on screen, points.
/// The card sits on the ghost's side away from the mine in the way, where the bar
/// and the way to the open site do not run.
pub(super) fn blocked_card(ui: &mut Ui, s: &Scene, c: Vec2, crowd: &Crowding, open: bool) {
    let lines = [
        (
            format!("Inside {} mine's reach", whose(s.view, crowd.owner)),
            palette::BAD,
        ),
        (
            format!("{:.0} m too close", crowd.short.ceil()),
            palette::TEXT,
        ),
        (
            if open {
                "Nearest open site marked in green".to_owned()
            } else {
                "Mines keep out of each other's reach".to_owned()
            },
            palette::DIM,
        ),
    ];
    let w = 240.0;
    let outward = ground(s, ui.s, crowd.at).is_none_or(|mine| c.x < mine.x);
    let x = if outward { c.x + 40.0 } else { c.x - 40.0 - w };
    card(
        ui,
        s,
        Rect::new(x, c.y - 28.0, w, 56.0),
        palette::BAD,
        &lines,
    );
}

/// The strip over the ghost's readout (the cursor's hint is under it) when it
/// overlaps a planned mine.
pub(super) fn warn_card(ui: &mut Ui, s: &Scene, c: Vec2, warn: &Crowding) {
    let lines = [
        (
            format!("Overlaps {} planned mine", whose(s.view, warn.owner)),
            palette::WARN,
        ),
        (
            "Whichever is begun second will be refused".to_owned(),
            palette::DIM,
        ),
    ];
    card(
        ui,
        s,
        Rect::new(c.x + 40.0, c.y - 74.0, 252.0, 40.0),
        palette::WARN,
        &lines,
    );
}

fn card(ui: &mut Ui, s: &Scene, r: Rect, edge: u32, lines: &[(String, u32)]) {
    if !on_screen(s.camera.viewport / ui.s, Vec2::new(r.x, r.y), 300.0) {
        return;
    }
    ui.frost(r, 0.9);
    ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(edge, 1.0));
    for (i, (text, tone)) in lines.iter().enumerate() {
        ui.text(
            r.x + 10.0,
            r.y + 5.0 + i as f32 * 16.0,
            type_scale::CAPTION,
            rgb(*tone, 1.0),
            text,
        );
    }
}
