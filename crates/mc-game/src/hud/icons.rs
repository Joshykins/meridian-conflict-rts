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
    }
    let pips = tech.min(4);
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
    }
}
