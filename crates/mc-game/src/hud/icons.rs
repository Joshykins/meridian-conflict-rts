//! Vector symbols for the HUD: the strategic icon of every unit class (the same
//! shapes the renderer draws over distant units, so a tile reads like the
//! battlefield does) and the glyphs on the order card.

use crate::ui::{Color, Rect, Ui};
use glam::Vec2;
use mc_data::IconKind;
use std::f32::consts::{FRAC_PI_2, TAU};

fn polygon(centre: Vec2, r: f32, sides: usize, turn: f32) -> Vec<Vec2> {
    (0..sides)
        .map(|i| centre + Vec2::from_angle(turn + TAU * i as f32 / sides as f32) * r)
        .collect()
}

fn fill_polygon(ui: &mut Ui, centre: Vec2, points: &[Vec2], color: Color) {
    for i in 0..points.len() {
        ui.triangle(centre, points[i], points[(i + 1) % points.len()], color);
    }
}

fn outline(ui: &mut Ui, points: &[Vec2], thickness: f32, color: Color) {
    for i in 0..points.len() {
        ui.stroke(points[i], points[(i + 1) % points.len()], thickness, color);
    }
}

/// Fills a simple polygon, convex or not, by clipping ears off it.
fn fill_outline(ui: &mut Ui, points: &[Vec2], color: Color) {
    let cross = |a: Vec2, b: Vec2, c: Vec2| (b - a).perp_dot(c - a);
    let winding: f32 = (0..points.len())
        .map(|i| points[i].perp_dot(points[(i + 1) % points.len()]))
        .sum();
    let mut left: Vec<usize> = (0..points.len()).collect();
    while left.len() > 3 {
        let n = left.len();
        let ear = (0..n).find(|&i| {
            let (a, b, c) = (points[left[(i + n - 1) % n]], points[left[i]], points[left[(i + 1) % n]]);
            cross(a, b, c) * winding > 0.0
                && left.iter().all(|&j| {
                    let q = points[j];
                    q == a || q == b || q == c
                        || !(cross(a, b, q) * winding >= 0.0 && cross(b, c, q) * winding >= 0.0 && cross(c, a, q) * winding >= 0.0)
                })
        });
        let Some(i) = ear else { break };
        ui.triangle(points[left[(i + n - 1) % n]], points[left[i]], points[left[(i + 1) % n]], color);
        left.remove(i);
    }
    if left.len() == 3 {
        ui.triangle(points[left[0]], points[left[1]], points[left[2]], color);
    }
}

/// A symbol outline given nose-up in the renderer's [-1, 1] icon square (y up), placed
/// in a box of half-size `r` on screen (y down). The world icons in `icons.wgsl` use the same points.
fn airframe(c: Vec2, r: f32, points: &[(f32, f32)]) -> Vec<Vec2> {
    points.iter().map(|&(x, y)| c + Vec2::new(x, -y) * r * 0.9).collect()
}

const FIGHTER: [(f32, f32); 16] = [
    (0.0, 0.92), (0.12, 0.5), (0.14, 0.2), (0.78, -0.34), (0.78, -0.52), (0.16, -0.4), (0.36, -0.76), (0.36, -0.88),
    (0.0, -0.78), (-0.36, -0.88), (-0.36, -0.76), (-0.16, -0.4), (-0.78, -0.52), (-0.78, -0.34), (-0.14, 0.2), (-0.12, 0.5),
];
const FLYING_WING: [(f32, f32); 12] = [
    (0.0, 0.56), (1.0, -0.18), (1.0, -0.4), (0.7, -0.54), (0.46, -0.34), (0.22, -0.54),
    (0.0, -0.36), (-0.22, -0.54), (-0.46, -0.34), (-0.7, -0.54), (-1.0, -0.4), (-1.0, -0.18),
];

fn square(c: Vec2, half_w: f32, half_h: f32) -> Rect {
    Rect::new(c.x - half_w, c.y - half_h, half_w * 2.0, half_h * 2.0)
}

/// The strategic icon of a unit class in a box of half-size `r`, with tech pips under it.
/// `cut` is the colour of whatever is behind the icon, for the shapes that have a hole.
pub fn strategic(ui: &mut Ui, kind: IconKind, tech: u8, c: Vec2, r: f32, color: Color, cut: Color) {
    let line = (r * 0.2).max(1.4);
    // Pointing up: screen y runs down.
    let up = -FRAC_PI_2;
    match kind {
        IconKind::Commander => {
            ui.disc(c, r * 0.42, color);
            ui.arc(c, r * 0.78, 0.0, TAU, line, color);
        }
        IconKind::Engineer => {
            let (h, t) = (r * 0.62, line);
            ui.fill(Rect::new(c.x - h, c.y - h, h * 2.0, t), color);
            ui.fill(Rect::new(c.x - h, c.y + h - t, h * 2.0, t), color);
            ui.fill(Rect::new(c.x - h, c.y - h, t, h * 2.0), color);
            ui.fill(Rect::new(c.x + h - t, c.y - h, t, h * 2.0), color);
        }
        IconKind::Bot => fill_polygon(
            ui,
            c + Vec2::Y * r * 0.12,
            &polygon(c + Vec2::Y * r * 0.12, r * 0.9, 3, up),
            color,
        ),
        IconKind::Tank => {
            // The tank in profile, as the renderer draws it: track run, hull, turret, gun. Screen y runs down.
            let run = Rect::new(c.x - r * 0.8, c.y + r * 0.16, r * 1.6, r * 0.48);
            ui.fill(
                Rect::new(run.x + run.h * 0.5, run.y, run.w - run.h, run.h),
                color,
            );
            ui.disc(
                Vec2::new(run.x + run.h * 0.5, run.y + run.h * 0.5),
                run.h * 0.5,
                color,
            );
            ui.disc(
                Vec2::new(run.x + run.w - run.h * 0.5, run.y + run.h * 0.5),
                run.h * 0.5,
                color,
            );
            ui.fill(
                Rect::new(c.x - r * 0.64, c.y - r * 0.08, r * 1.2, r * 0.3),
                color,
            );
            ui.fill(
                Rect::new(c.x - r * 0.48, c.y - r * 0.5, r * 0.64, r * 0.44),
                color,
            );
            ui.fill(
                Rect::new(c.x + r * 0.08, c.y - r * 0.39, r * 0.84, r * 0.14),
                color,
            );
        }
        IconKind::Artillery => {
            outline(ui, &polygon(c, r * 0.85, 4, up), line, color);
            ui.disc(c, r * 0.22, color);
        }
        IconKind::AntiAir => fill_polygon(
            ui,
            c - Vec2::Y * r * 0.12,
            &polygon(c - Vec2::Y * r * 0.12, r * 0.9, 3, -up),
            color,
        ),
        IconKind::Scout => {
            let (tip, l, rr) = (
                c + Vec2::new(0.0, -r * 0.6),
                c + Vec2::new(-r * 0.75, r * 0.5),
                c + Vec2::new(r * 0.75, r * 0.5),
            );
            ui.stroke(l, tip, line * 1.3, color);
            ui.stroke(tip, rr, line * 1.3, color);
        }
        IconKind::Factory => {
            let h = r * 0.78;
            ui.fill(Rect::new(c.x - h, c.y - h, h * 2.0, h * 0.8), color);
            ui.fill(Rect::new(c.x - h, c.y - h, h * 0.55, h * 2.0), color);
            ui.fill(Rect::new(c.x + h * 0.45, c.y - h, h * 0.55, h * 2.0), color);
        }
        IconKind::Extractor => {
            ui.disc(c, r * 0.72, color);
            ui.fill(square(c, r * 0.13, r * 0.52), cut);
            ui.fill(square(c, r * 0.52, r * 0.13), cut);
        }
        IconKind::Power => fill_polygon(ui, c, &polygon(c, r * 0.8, 6, 0.0), color),
        IconKind::Storage => ui.fill(square(c, r * 0.72, r * 0.46), color),
        IconKind::Defense => {
            outline(ui, &polygon(c, r * 0.8, 6, 0.0), line, color);
            ui.disc(c, r * 0.22, color);
        }
        IconKind::Intel => {
            ui.arc(c, r * 0.62, 0.0, TAU, line * 1.2, color);
            ui.disc(c, r * 0.14, color);
        }
        IconKind::Wall => ui.fill(square(c, r * 0.5, r * 0.5), color),
        IconKind::Shield => {
            let tip = c + Vec2::new(0.0, -r * 0.88);
            let bl = c + Vec2::new(-r * 0.2, r * 0.78);
            let br = c + Vec2::new(r * 0.2, r * 0.78);
            ui.triangle(tip, bl, br, color);
            ui.arc(
                c + Vec2::new(0.0, -r * 0.18),
                r * 0.7,
                3.5,
                5.9,
                line * 1.3,
                color,
            );
        }
        // Aircraft from above, nose up; the outline is the role (see `icons.wgsl`).
        IconKind::Fighter => fill_outline(ui, &airframe(c, r, &FIGHTER), color),
        IconKind::Bomber => fill_outline(ui, &airframe(c, r, &FLYING_WING), color),
        IconKind::Airbase => {
            // The bunker in the ground, and a V over it pointing down into it. Screen y runs down.
            ui.fill(Rect::new(c.x - r * 0.78, c.y + r * 0.24, r * 1.56, r * 0.44), color);
            let apex = c + Vec2::new(0.0, -r * 0.04);
            for side in [-1.0, 1.0] {
                ui.stroke(c + Vec2::new(side * r * 0.52, -r * 0.62), apex, r * 0.26, color);
            }
            ui.disc(apex, r * 0.13, color);
        }
        IconKind::Gunship => {
            // Crossed rotor blades over the body, tail boom and tail rotor.
            let hub = c - Vec2::Y * r * 0.09;
            for d in [Vec2::new(1.0, 1.0), Vec2::new(1.0, -1.0)] {
                let d = d.normalize() * r * 0.81;
                ui.stroke(hub - d, hub + d, r * 0.15, color);
            }
            fill_polygon(ui, hub, &polygon(hub, r * 0.2, 12, 0.0).iter().map(|&q| hub + (q - hub) * Vec2::new(1.0, 1.64)).collect::<Vec<_>>(), color);
            ui.fill(Rect::new(c.x - r * 0.054, c.y + r * 0.13, r * 0.108, r * 0.54), color);
            ui.fill(Rect::new(c.x - r * 0.22, c.y + r * 0.59, r * 0.44, r * 0.11), color);
        }
        IconKind::Transport => {
            // Capital transport: wedge bow over a broad stern and drive shoulders.
            fill_polygon(ui, c, &[
                c + Vec2::new(-0.10, -0.88) * r,
                c + Vec2::new(0.10, -0.88) * r,
                c + Vec2::new(0.40, 0.0) * r,
                c + Vec2::new(0.32, 0.80) * r,
                c + Vec2::new(-0.32, 0.80) * r,
                c + Vec2::new(-0.40, 0.0) * r,
            ], color);
            for side in [-1.0, 1.0] {
                ui.fill(Rect::new(c.x + (side * 0.47 - 0.15) * r, c.y, r * 0.30, r * 0.78), color);
            }
        }
        IconKind::Ship => {
            // A warship in profile: flared hull, bridge and mast, the deck gun forward. Screen y runs down.
            fill_polygon(
                ui,
                c + Vec2::new(0.0, r * 0.3),
                &[
                    c + Vec2::new(-r * 0.86, r * 0.14),
                    c + Vec2::new(r * 0.9, r * 0.14),
                    c + Vec2::new(r * 0.62, r * 0.46),
                    c + Vec2::new(-r * 0.62, r * 0.46),
                ],
                color,
            );
            ui.fill(Rect::new(c.x - r * 0.48, c.y - r * 0.16, r * 0.68, r * 0.3), color);
            ui.fill(Rect::new(c.x - r * 0.15, c.y - r * 0.58, r * 0.1, r * 0.44), color);
            ui.fill(Rect::new(c.x + r * 0.31, c.y - r * 0.04, r * 0.22, r * 0.18), color);
            ui.fill(Rect::new(c.x + r * 0.42, c.y - r * 0.02, r * 0.4, r * 0.07), color);
        }
        IconKind::Submarine => {
            // A long hull low in the water and its sail.
            let hull = Rect::new(c.x - r * 0.9, c.y + r * 0.0, r * 1.8, r * 0.36);
            ui.fill(Rect::new(hull.x + hull.h * 0.5, hull.y, hull.w - hull.h, hull.h), color);
            ui.disc(Vec2::new(hull.x + hull.h * 0.5, hull.y + hull.h * 0.5), hull.h * 0.5, color);
            ui.disc(Vec2::new(hull.x + hull.w - hull.h * 0.5, hull.y + hull.h * 0.5), hull.h * 0.5, color);
            ui.fill(Rect::new(c.x, c.y - r * 0.36, r * 0.3, r * 0.4), color);
        }
    }
    let pips = tech.min(5);
    for i in 0..pips {
        let x = c.x + (i as f32 - (pips as f32 - 1.0) * 0.5) * r * 0.42;
        ui.fill(
            Rect::new(x - r * 0.15, c.y + r * 1.08, r * 0.3, r * 0.16),
            color,
        );
    }
}

/// The glyphs of the order card.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Glyph {
    Move,
    Attack,
    AttackMove,
    Stop,
    Assist,
    Reclaim,
    Upgrade,
    Repeat,
    Pause,
    Play,
    Patrol,
    Orbit,
    Formation,
    GroundAttack,
    Bombard,
    FireAtWill,
    HoldFire,
    HoldPosition,
    Dive,
    Surface,
    Guard,
    Launch,
    /// A lift ship sets down.
    Land,
    /// A lift ship lets its hold out down the ramp.
    Unload,
    /// Land units walk up a lift ship's ramp into its hold.
    Board,
    /// A lift ship rises off the ground.
    TakeOff,
}

/// Two strokes meeting at `tip`, opening away from `dir`.
pub fn arrow_head(ui: &mut Ui, tip: Vec2, dir: Vec2, size: f32, t: f32, color: Color) {
    let side = dir.perp();
    ui.stroke(tip, tip - dir * size + side * size * 0.8, t, color);
    ui.stroke(tip, tip - dir * size - side * size * 0.8, t, color);
}

pub fn glyph(ui: &mut Ui, glyph: Glyph, c: Vec2, r: f32, color: Color) {
    let t = (r * 0.17).max(1.4);
    let reticle = |ui: &mut Ui, c: Vec2, r: f32| {
        ui.arc(c, r * 0.6, 0.0, TAU, t, color);
        for d in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
            ui.stroke(c + d * r * 0.32, c + d * r, t, color);
        }
    };
    match glyph {
        Glyph::Move => {
            let (from, to) = (
                c + Vec2::new(-r * 0.75, r * 0.75),
                c + Vec2::new(r * 0.75, -r * 0.75),
            );
            ui.stroke(from, to, t, color);
            arrow_head(ui, to, (to - from).normalize(), r * 0.55, t, color);
            ui.disc(from, t * 1.1, color);
        }
        Glyph::Attack => {
            reticle(ui, c, r);
            ui.disc(c, t * 0.9, color);
        }
        Glyph::AttackMove => {
            reticle(ui, c + Vec2::new(r * 0.3, -r * 0.3), r * 0.7);
            let (from, to) = (c + Vec2::new(-r, r), c + Vec2::new(-r * 0.25, r * 0.25));
            ui.stroke(from, to, t, color);
            arrow_head(ui, to, (to - from).normalize(), r * 0.4, t, color);
        }
        Glyph::Stop => {
            outline(ui, &polygon(c, r * 0.95, 8, TAU / 16.0), t, color);
            ui.fill(square(c, r * 0.32, r * 0.32), color);
        }
        Glyph::Assist => {
            ui.arc(c, r * 0.85, 0.0, TAU, t, color);
            ui.fill(square(c, r * 0.45, t * 0.5), color);
            ui.fill(square(c, t * 0.5, r * 0.45), color);
        }
        Glyph::Reclaim => {
            // Three arrows chasing each other round a ring.
            for i in 0..3 {
                let a = TAU * i as f32 / 3.0 - FRAC_PI_2;
                ui.arc(c, r * 0.78, a + 0.25, a + TAU / 3.0 - 0.35, t, color);
                let end = a + TAU / 3.0 - 0.35;
                let tip = c + Vec2::from_angle(end + 0.22) * r * 0.78;
                arrow_head(
                    ui,
                    tip,
                    Vec2::from_angle(end + FRAC_PI_2),
                    r * 0.38,
                    t,
                    color,
                );
            }
        }
        Glyph::Upgrade => {
            for dy in [-0.35, 0.3] {
                let tip = c + Vec2::new(0.0, (dy - 0.4) * r);
                ui.stroke(tip + Vec2::new(-r * 0.7, r * 0.6), tip, t * 1.2, color);
                ui.stroke(tip, tip + Vec2::new(r * 0.7, r * 0.6), t * 1.2, color);
            }
        }
        Glyph::Repeat => {
            ui.arc(
                c,
                r * 0.75,
                -FRAC_PI_2 + 0.9,
                -FRAC_PI_2 + TAU - 0.1,
                t,
                color,
            );
            let tip = c + Vec2::from_angle(-FRAC_PI_2 + 0.75) * r * 0.75;
            arrow_head(
                ui,
                tip,
                Vec2::from_angle(-FRAC_PI_2 + 0.75 - FRAC_PI_2),
                r * 0.42,
                t,
                color,
            );
        }
        Glyph::Pause => {
            ui.fill(
                Rect::new(c.x - r * 0.55, c.y - r * 0.7, r * 0.38, r * 1.4),
                color,
            );
            ui.fill(
                Rect::new(c.x + r * 0.17, c.y - r * 0.7, r * 0.38, r * 1.4),
                color,
            );
        }
        Glyph::Play => ui.triangle(
            c + Vec2::new(-r * 0.5, -r * 0.75),
            c + Vec2::new(r * 0.75, 0.0),
            c + Vec2::new(-r * 0.5, r * 0.75),
            color,
        ),
        Glyph::Patrol => {
            // Two posts and the loop between them.
            let (a, b) = (c + Vec2::new(-r * 0.7, r * 0.2), c + Vec2::new(r * 0.7, -r * 0.2));
            ui.arc(c, r * 0.8, 0.3, 2.6, t, color);
            ui.arc(c, r * 0.8, 3.45, 5.75, t, color);
            ui.disc(a, t * 1.5, color);
            ui.disc(b, t * 1.5, color);
            arrow_head(ui, c + Vec2::from_angle(2.6) * r * 0.8, Vec2::from_angle(2.6 + FRAC_PI_2), r * 0.35, t, color);
            arrow_head(ui, c + Vec2::from_angle(5.75) * r * 0.8, Vec2::from_angle(5.75 + FRAC_PI_2), r * 0.35, t, color);
        }
        Glyph::Orbit => {
            ui.arc(c, r * 0.85, 0.2, TAU - 0.5, t, color);
            let end = TAU - 0.5;
            arrow_head(ui, c + Vec2::from_angle(end) * r * 0.85, Vec2::from_angle(end + FRAC_PI_2), r * 0.38, t, color);
            ui.disc(c, t * 1.3, color);
        }
        Glyph::Formation => {
            for (dx, dy) in [(-0.6, 0.45), (0.0, 0.45), (0.6, 0.45), (-0.3, -0.15), (0.3, -0.15), (0.0, -0.7)] {
                ui.disc(c + Vec2::new(dx * r, dy * r), t * 1.25, color);
            }
        }
        Glyph::GroundAttack => {
            reticle(ui, c + Vec2::new(0.0, -r * 0.2), r * 0.8);
            ui.stroke(c + Vec2::new(-r, r * 0.8), c + Vec2::new(r, r * 0.8), t, color);
        }
        Glyph::Bombard => {
            // A spread of hits inside a broken ring.
            for i in 0..6 {
                let a = TAU * i as f32 / 6.0;
                ui.arc(c, r * 0.9, a, a + 0.6, t, color);
            }
            for (dx, dy) in [(-0.35, -0.2), (0.3, -0.3), (0.05, 0.35)] {
                ui.disc(c + Vec2::new(dx * r, dy * r), t * 1.3, color);
            }
        }
        Glyph::FireAtWill => {
            reticle(ui, c, r);
            ui.disc(c, t * 1.4, color);
        }
        Glyph::HoldFire => {
            ui.arc(c, r * 0.6, 0.0, TAU, t, color);
            ui.stroke(c + Vec2::new(-r * 0.85, -r * 0.85), c + Vec2::new(r * 0.85, r * 0.85), t, color);
        }
        Glyph::Dive | Glyph::Surface => {
            // The sea's surface, a hull under or on it, and which way it is going.
            let sea = c.y - r * 0.35;
            ui.stroke(Vec2::new(c.x - r * 0.9, sea), Vec2::new(c.x + r * 0.9, sea), t, color);
            let (hull, dir) = if glyph == Glyph::Dive { (c.y + r * 0.55, Vec2::Y) } else { (sea - t * 1.5, -Vec2::Y) };
            ui.fill(Rect::new(c.x - r * 0.6, hull - t, r * 1.2, t * 2.0), color);
            ui.fill(Rect::new(c.x - r * 0.08, hull - t * 3.0, r * 0.3, t * 2.0), color);
            let from = if glyph == Glyph::Dive { sea + t * 2.5 } else { c.y + r * 0.85 };
            let tip = Vec2::new(c.x + r * 0.7, from + dir.y * r * 0.55);
            ui.stroke(Vec2::new(c.x + r * 0.7, from), tip, t, color);
            arrow_head(ui, tip, dir, r * 0.25, t, color);
        }
        Glyph::HoldPosition => {
            // A spot staked out: a square with a pin at its middle.
            let k = r * 0.75;
            let corners = [Vec2::new(-k, -k), Vec2::new(k, -k), Vec2::new(k, k), Vec2::new(-k, k)];
            for i in 0..4 {
                ui.stroke(c + corners[i], c + corners[(i + 1) % 4], t, color);
            }
            ui.disc(c, t * 1.6, color);
        }
        Glyph::Guard => {
            // The area watched: a ring with four ticks pointing in, the spot held at its middle.
            ui.arc(c, r * 0.9, 0.0, TAU, t, color);
            for i in 0..4 {
                let d = Vec2::from_angle(TAU * i as f32 / 4.0 + FRAC_PI_2 / 2.0);
                ui.stroke(c + d * r * 0.9, c + d * r * 0.55, t, color);
            }
            ui.disc(c, t * 1.7, color);
        }
        Glyph::Launch => {
            // The ground with a tunnel mouth in it, and a dart climbing away out of it.
            let ground = c.y + r * 0.7;
            ui.stroke(Vec2::new(c.x - r * 0.95, ground), Vec2::new(c.x - r * 0.35, ground), t, color);
            ui.stroke(Vec2::new(c.x + r * 0.15, ground), Vec2::new(c.x + r * 0.95, ground), t, color);
            let from = Vec2::new(c.x - r * 0.1, ground - t);
            let tip = Vec2::new(c.x + r * 0.7, c.y - r * 0.7);
            ui.stroke(from, tip, t, color);
            arrow_head(ui, tip, (tip - from).normalize(), r * 0.35, t, color);
        }
        Glyph::Land => {
            // A broad hull coming straight down onto the ground, legs out under it.
            let ground = c.y + r * 0.8;
            ui.stroke(Vec2::new(c.x - r * 0.95, ground), Vec2::new(c.x + r * 0.95, ground), t, color);
            let hull = c.y - r * 0.05;
            ui.stroke(Vec2::new(c.x - r * 0.75, hull), Vec2::new(c.x + r * 0.75, hull), t * 1.6, color);
            for s in [-1.0, 1.0] {
                let hip = Vec2::new(c.x + s * r * 0.5, hull);
                ui.stroke(hip, Vec2::new(c.x + s * r * 0.7, ground - t * 1.4), t, color);
            }
            let tip = Vec2::new(c.x, hull - t * 1.6);
            ui.stroke(Vec2::new(c.x, c.y - r * 0.95), tip, t, color);
            arrow_head(ui, tip, Vec2::Y, r * 0.3, t, color);
        }
        Glyph::Unload => {
            // The hull on the ground, its ramp let down, and an arrow walking off it.
            let ground = c.y + r * 0.75;
            ui.stroke(Vec2::new(c.x - r * 0.95, ground), Vec2::new(c.x + r * 0.95, ground), t, color);
            let deck = c.y + r * 0.1;
            ui.stroke(Vec2::new(c.x - r * 0.9, deck), Vec2::new(c.x + r * 0.05, deck), t * 1.6, color);
            ui.stroke(Vec2::new(c.x + r * 0.05, deck), Vec2::new(c.x + r * 0.55, ground), t, color);
            let from = Vec2::new(c.x - r * 0.1, c.y - r * 0.55);
            let tip = Vec2::new(c.x + r * 0.9, c.y - r * 0.05);
            ui.stroke(from, tip, t, color);
            arrow_head(ui, tip, (tip - from).normalize(), r * 0.32, t, color);
        }
        Glyph::Board => {
            // The hull on the ground, its ramp let down, and an arrow walking up into it.
            let ground = c.y + r * 0.75;
            ui.stroke(Vec2::new(c.x - r * 0.95, ground), Vec2::new(c.x + r * 0.95, ground), t, color);
            let deck = c.y + r * 0.1;
            ui.stroke(Vec2::new(c.x - r * 0.9, deck), Vec2::new(c.x + r * 0.05, deck), t * 1.6, color);
            ui.stroke(Vec2::new(c.x + r * 0.05, deck), Vec2::new(c.x + r * 0.55, ground), t, color);
            let from = Vec2::new(c.x + r * 0.9, c.y - r * 0.05);
            let tip = Vec2::new(c.x - r * 0.1, c.y - r * 0.55);
            ui.stroke(from, tip, t, color);
            arrow_head(ui, tip, (tip - from).normalize(), r * 0.32, t, color);
        }
        Glyph::TakeOff => {
            // A broad hull rising off the ground, legs folding, an arrow up over it.
            let ground = c.y + r * 0.8;
            ui.stroke(Vec2::new(c.x - r * 0.95, ground), Vec2::new(c.x + r * 0.95, ground), t, color);
            let hull = c.y + r * 0.2;
            ui.stroke(Vec2::new(c.x - r * 0.75, hull), Vec2::new(c.x + r * 0.75, hull), t * 1.6, color);
            for s in [-1.0, 1.0] {
                let hip = Vec2::new(c.x + s * r * 0.5, hull);
                ui.stroke(hip, Vec2::new(c.x + s * r * 0.3, hull + r * 0.3), t, color);
            }
            let tip = Vec2::new(c.x, c.y - r * 0.95);
            ui.stroke(Vec2::new(c.x, hull - t * 1.6), tip, t, color);
            arrow_head(ui, tip, -Vec2::Y, r * 0.3, t, color);
        }
    }
}
