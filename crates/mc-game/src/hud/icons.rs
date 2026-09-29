//! Vector symbols for the HUD: the strategic icon of every unit class (the same
//! shapes the renderer draws over distant units, so a tile reads like the
//! battlefield does) and the glyphs on the order card.

use crate::ui::{Color, Rect, Ui};
use glam::Vec2;
use mc_data::IconKind;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

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
            let (a, b, c) = (
                points[left[(i + n - 1) % n]],
                points[left[i]],
                points[left[(i + 1) % n]],
            );
            cross(a, b, c) * winding > 0.0
                && left.iter().all(|&j| {
                    let q = points[j];
                    q == a
                        || q == b
                        || q == c
                        || !(cross(a, b, q) * winding >= 0.0
                            && cross(b, c, q) * winding >= 0.0
                            && cross(c, a, q) * winding >= 0.0)
                })
        });
        let Some(i) = ear else { break };
        ui.triangle(
            points[left[(i + n - 1) % n]],
            points[left[i]],
            points[left[(i + 1) % n]],
            color,
        );
        left.remove(i);
    }
    if left.len() == 3 {
        ui.triangle(points[left[0]], points[left[1]], points[left[2]], color);
    }
}

/// A symbol outline given nose-up in the renderer's [-1, 1] icon square (y up), placed
/// in a box of half-size `r` on screen (y down). The world icons in `icons.wgsl` use the same points.
fn airframe(c: Vec2, r: f32, points: &[(f32, f32)]) -> Vec<Vec2> {
    points
        .iter()
        .map(|&(x, y)| c + Vec2::new(x, -y) * r * 0.9)
        .collect()
}

const FIGHTER: [(f32, f32); 16] = [
    (0.0, 0.92),
    (0.12, 0.5),
    (0.14, 0.2),
    (0.78, -0.34),
    (0.78, -0.52),
    (0.16, -0.4),
    (0.36, -0.76),
    (0.36, -0.88),
    (0.0, -0.78),
    (-0.36, -0.88),
    (-0.36, -0.76),
    (-0.16, -0.4),
    (-0.78, -0.52),
    (-0.78, -0.34),
    (-0.14, 0.2),
    (-0.12, 0.5),
];
/// The sensor ship's outline, nose up (`icons.wgsl` case 35 has the same points).
const SENSOR_SHIP: [(f32, f32); 13] = [
    (0.0, 0.78),
    (0.6, 0.52),
    (0.94, 0.38),
    (0.94, 0.14),
    (0.24, 0.1),
    (0.18, -0.62),
    (0.24, -0.92),
    (-0.24, -0.92),
    (-0.18, -0.62),
    (-0.24, 0.1),
    (-0.94, 0.14),
    (-0.94, 0.38),
    (-0.6, 0.52),
];
/// A capital warship from above; its spinal gun is a slot cut down the middle.
const WARSHIP: [(f32, f32); 15] = [
    (0.0, 0.96),
    (0.16, 0.62),
    (0.2, 0.12),
    (0.36, 0.08),
    (0.36, -0.12),
    (0.24, -0.16),
    (0.3, -0.56),
    (0.3, -0.9),
    (-0.3, -0.9),
    (-0.3, -0.56),
    (-0.24, -0.16),
    (-0.36, -0.12),
    (-0.36, 0.08),
    (-0.2, 0.12),
    (-0.16, 0.62),
];
/// A torpedo bomber's gull wing; the body, tail and torpedo are drawn over it.
const GULL_WING: [(f32, f32); 10] = [
    (0.0, 0.62),
    (0.34, 0.46),
    (0.96, 0.6),
    (0.96, 0.42),
    (0.34, 0.26),
    (0.0, 0.36),
    (-0.34, 0.26),
    (-0.96, 0.42),
    (-0.96, 0.6),
    (-0.34, 0.46),
];
const FLYING_WING: [(f32, f32); 12] = [
    (0.0, 0.56),
    (1.0, -0.18),
    (1.0, -0.4),
    (0.7, -0.54),
    (0.46, -0.34),
    (0.22, -0.54),
    (0.0, -0.36),
    (-0.22, -0.54),
    (-0.46, -0.34),
    (-0.7, -0.54),
    (-1.0, -0.4),
    (-1.0, -0.18),
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
        IconKind::TorpedoBomber => {
            // A gull-winged plane with a finned torpedo under it (see `icons.wgsl`).
            let at = |x: f32, y: f32| c + Vec2::new(x, -y) * r * 0.9;
            let u = r * 0.9;
            fill_outline(ui, &airframe(c, r, &GULL_WING), color);
            ui.stroke(at(0.0, 0.1), at(0.0, 0.9), u * 0.24, color);
            ui.disc(at(0.0, 0.9), u * 0.12, color);
            ui.disc(at(0.0, 0.1), u * 0.12, color);
            let tail = at(-0.3, 0.15);
            ui.fill(Rect::new(tail.x, tail.y, u * 0.6, u * 0.14), color);
            ui.stroke(at(-0.42, -0.5), at(0.52, -0.5), u * 0.28, color);
            ui.disc(at(-0.42, -0.5), u * 0.14, color);
            ui.disc(at(0.52, -0.5), u * 0.14, color);
            let fin = at(-0.68, -0.28);
            ui.fill(Rect::new(fin.x, fin.y, u * 0.12, u * 0.44), color);
        }
        IconKind::Silo => {
            // A missile standing in an open tube, fins at its foot. Screen y runs down.
            ui.fill(
                Rect::new(c.x - r * 0.12, c.y - r * 0.44, r * 0.24, r * 0.9),
                color,
            );
            ui.stroke(
                c - Vec2::Y * r * 0.46,
                c - Vec2::Y * r * 0.74,
                r * 0.16,
                color,
            );
            ui.stroke(
                c + Vec2::new(-r * 0.26, r * 0.44),
                c + Vec2::new(r * 0.26, r * 0.44),
                r * 0.14,
                color,
            );
            for side in [-1.0, 1.0] {
                ui.fill(
                    Rect::new(
                        c.x + side * r * 0.5 - r * 0.08,
                        c.y - r * 0.08,
                        r * 0.16,
                        r * 0.88,
                    ),
                    color,
                );
            }
            ui.fill(
                Rect::new(c.x - r * 0.58, c.y + r * 0.68, r * 1.16, r * 0.16),
                color,
            );
        }
        IconKind::AntiNuke => {
            // A missile rising out of a shield's bowl toward the mark where it meets its warhead.
            ui.arc(c - Vec2::Y * r * 0.05, r * 0.66, 0.0, PI, r * 0.18, color);
            ui.stroke(
                c + Vec2::Y * r * 0.42,
                c - Vec2::Y * r * 0.3,
                r * 0.18,
                color,
            );
            ui.stroke(
                c - Vec2::Y * r * 0.3,
                c - Vec2::Y * r * 0.44,
                r * 0.1,
                color,
            );
            ui.stroke(
                c + Vec2::new(-r * 0.16, -r * 0.62),
                c + Vec2::new(r * 0.16, -r * 0.9),
                r * 0.12,
                color,
            );
            ui.stroke(
                c + Vec2::new(-r * 0.16, -r * 0.9),
                c + Vec2::new(r * 0.16, -r * 0.62),
                r * 0.12,
                color,
            );
        }
        IconKind::Gunship => {
            // A straight wing with a pod on each tip, body and tail (see `icons.wgsl`).
            let at = |x: f32, y: f32| c + Vec2::new(x, -y) * r * 0.9;
            let mut capsule = |a: Vec2, b: Vec2, radius: f32| {
                ui.stroke(a, b, radius * 2.0, color);
                ui.disc(a, radius, color);
                ui.disc(b, radius, color);
            };
            capsule(at(0.0, -0.5), at(0.0, 0.62), r * 0.9 * 0.15);
            for side in [-1.0, 1.0] {
                capsule(
                    at(side * 0.66, -0.26),
                    at(side * 0.66, 0.44),
                    r * 0.9 * 0.14,
                );
            }
            let (wing, tail) = (at(-0.62, 0.12), at(-0.3, -0.58));
            ui.fill(
                Rect::new(wing.x, wing.y, r * 0.9 * 1.24, r * 0.9 * 0.2),
                color,
            );
            ui.fill(
                Rect::new(tail.x, tail.y, r * 0.9 * 0.6, r * 0.9 * 0.16),
                color,
            );
        }
        IconKind::SensorShip => {
            // A hammerhead of sensors over a slim hull, the eye-line cut across the head.
            fill_outline(ui, &airframe(c, r, &SENSOR_SHIP), color);
            let (hw, hh) = (0.58 * 0.9 * r, 0.05 * 0.9 * r);
            ui.fill(
                Rect::new(c.x - hw, c.y - 0.36 * 0.9 * r - hh, hw * 2.0, hh * 2.0),
                cut,
            );
        }
        IconKind::Warship => {
            // A long spine, pointed prow, flank sponsons, and the spinal gun cut down the middle.
            fill_outline(ui, &airframe(c, r, &WARSHIP), color);
            let (hw, hh) = (0.06 * 0.9 * r, 0.6 * 0.9 * r);
            ui.fill(
                Rect::new(c.x - hw, c.y + 0.02 * 0.9 * r - hh, hw * 2.0, hh * 2.0),
                cut,
            );
        }
        IconKind::Transport => {
            // Capital transport: wedge bow over a broad stern and drive shoulders.
            fill_polygon(
                ui,
                c,
                &[
                    c + Vec2::new(-0.10, -0.88) * r,
                    c + Vec2::new(0.10, -0.88) * r,
                    c + Vec2::new(0.40, 0.0) * r,
                    c + Vec2::new(0.32, 0.80) * r,
                    c + Vec2::new(-0.32, 0.80) * r,
                    c + Vec2::new(-0.40, 0.0) * r,
                ],
                color,
            );
            for side in [-1.0, 1.0] {
                ui.fill(
                    Rect::new(c.x + (side * 0.47 - 0.15) * r, c.y, r * 0.30, r * 0.78),
                    color,
                );
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
            ui.fill(
                Rect::new(c.x - r * 0.48, c.y - r * 0.16, r * 0.68, r * 0.3),
                color,
            );
            ui.fill(
                Rect::new(c.x - r * 0.15, c.y - r * 0.58, r * 0.1, r * 0.44),
                color,
            );
            ui.fill(
                Rect::new(c.x + r * 0.31, c.y - r * 0.04, r * 0.22, r * 0.18),
                color,
            );
            ui.fill(
                Rect::new(c.x + r * 0.42, c.y - r * 0.02, r * 0.4, r * 0.07),
                color,
            );
        }
        IconKind::Titan => titan(ui, c, r, color),
        IconKind::Salvage
        | IconKind::SalvageBoat
        | IconKind::SalvageCarrier
        | IconKind::SalvageDrone => salvage(ui, kind, c, r, color, cut),
        IconKind::Submarine => {
            // A long hull low in the water and its sail.
            let hull = Rect::new(c.x - r * 0.9, c.y + r * 0.0, r * 1.8, r * 0.36);
            ui.fill(
                Rect::new(hull.x + hull.h * 0.5, hull.y, hull.w - hull.h, hull.h),
                color,
            );
            ui.disc(
                Vec2::new(hull.x + hull.h * 0.5, hull.y + hull.h * 0.5),
                hull.h * 0.5,
                color,
            );
            ui.disc(
                Vec2::new(hull.x + hull.w - hull.h * 0.5, hull.y + hull.h * 0.5),
                hull.h * 0.5,
                color,
            );
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

/// The salvage icons, as `icons.wgsl` draws them: the Extractor's disc (salvage feeds
/// mass as a mine does) with its domain's mark: a reach ring for a structure, a hull for a
/// boat, wings and a tail for a carrier, nothing for a drone. Given in the renderer's icon
/// square (y up), placed in a box of half-size `r`; `cut` fills the disc's cross.
fn salvage(ui: &mut Ui, kind: IconKind, c: Vec2, r: f32, color: Color, cut: Color) {
    let k = r * 0.9;
    let at = |x: f32, y: f32| c + Vec2::new(x, -y) * k;
    // `sd_ore_disc`.
    let disc = |ui: &mut Ui, y: f32, rr: f32| {
        let o = at(0.0, y);
        ui.disc(o, rr * k, color);
        ui.fill(square(o, 0.12 * k, rr * 0.8 * k), cut);
        ui.fill(square(o, rr * 0.8 * k, 0.12 * k), cut);
    };
    let limb = |ui: &mut Ui, a: (f32, f32), b: (f32, f32), w: f32| {
        let (pa, pb) = (at(a.0, a.1), at(b.0, b.1));
        ui.stroke(pa, pb, w * 2.0 * k, color);
        ui.disc(pa, w * k, color);
        ui.disc(pb, w * k, color);
    };
    match kind {
        IconKind::Salvage => {
            ui.arc(c, 0.8 * k, 0.0, TAU, 0.14 * k, color);
            disc(ui, 0.0, 0.5);
        }
        IconKind::SalvageBoat => {
            let hull = [
                (-0.894, -0.39),
                (0.894, -0.39),
                (0.66, -0.65),
                (-0.66, -0.65),
            ];
            fill_outline(ui, &hull.map(|(x, y)| at(x, y)), color);
            disc(ui, 0.2, 0.5);
        }
        IconKind::SalvageCarrier => {
            for side in [-1.0, 1.0] {
                limb(ui, (side * 0.45, -0.05), (side * 0.95, -0.3), 0.1);
            }
            limb(ui, (0.0, -0.45), (0.0, -0.8), 0.09);
            disc(ui, 0.05, 0.52);
        }
        // The drone: the disc alone.
        _ => disc(ui, 0.0, 0.6),
    }
}

/// A tier-5 titan from the front, as `icons.wgsl` draws it (`sd_titan`): a giant
/// mid-stride, the rotary rail cluster on its right arm, the long bore on its left.
/// Given in the renderer's icon square (y up), placed in a box of half-size `r`.
pub fn titan(ui: &mut Ui, c: Vec2, r: f32, color: Color) {
    let k = r * 0.9;
    let at = |x: f32, y: f32| c + Vec2::new(x, -y) * k;
    let bx = |ui: &mut Ui, x: f32, y: f32, hw: f32, hh: f32| {
        let (a, b) = (at(x - hw, y + hh), at(x + hw, y - hh));
        ui.fill(Rect::new(a.x, a.y, b.x - a.x, b.y - a.y), color);
    };
    let limb = |ui: &mut Ui, a: (f32, f32), b: (f32, f32), w: f32| {
        let (pa, pb) = (at(a.0, a.1), at(b.0, b.1));
        ui.stroke(pa, pb, w * 2.0 * k, color);
        ui.disc(pa, w * k, color);
        ui.disc(pb, w * k, color);
    };
    let torso = [
        (-0.46, 0.52),
        (0.46, 0.52),
        (0.34, 0.22),
        (0.2, 0.02),
        (-0.2, 0.02),
        (-0.34, 0.22),
    ];
    fill_outline(ui, &torso.map(|(x, y)| at(x, y)), color);
    bx(ui, 0.0, 0.66, 0.1, 0.1);
    bx(ui, -0.36, 0.6, 0.12, 0.08);
    bx(ui, 0.36, 0.6, 0.12, 0.08);
    limb(ui, (0.44, 0.46), (0.64, 0.22), 0.09);
    limb(ui, (-0.44, 0.46), (-0.64, 0.22), 0.09);
    bx(ui, 0.68, 0.04, 0.14, 0.2);
    limb(ui, (0.68, -0.16), (0.68, -0.38), 0.065);
    fill_outline(
        ui,
        &[
            at(-0.82, 0.26),
            at(-0.52, 0.26),
            at(-0.62, -0.44),
            at(-0.72, -0.44),
        ],
        color,
    );
    bx(ui, 0.0, 0.0, 0.17, 0.08);
    limb(ui, (-0.1, -0.02), (-0.27, -0.38), 0.1);
    limb(ui, (-0.27, -0.38), (-0.33, -0.8), 0.085);
    bx(ui, -0.35, -0.85, 0.16, 0.055);
    limb(ui, (0.1, -0.02), (0.24, -0.3), 0.1);
    limb(ui, (0.24, -0.3), (0.29, -0.66), 0.085);
    bx(ui, 0.31, -0.72, 0.14, 0.055);
    // The tier-5 frame, where there is room for it.
    if r >= 9.0 {
        let frame = Rect::new(c.x - r * 1.05, c.y - r * 1.02, r * 2.1, r * 2.36);
        ui.brackets(
            frame,
            r * 0.32,
            [color[0], color[1], color[2], color[3] * 0.8],
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
    Formation,
    GroundAttack,
    Bombard,
    FireAtWill,
    HoldFire,
    HoldPosition,
    Dive,
    Surface,
    Guard,
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
            let (a, b) = (
                c + Vec2::new(-r * 0.7, r * 0.2),
                c + Vec2::new(r * 0.7, -r * 0.2),
            );
            ui.arc(c, r * 0.8, 0.3, 2.6, t, color);
            ui.arc(c, r * 0.8, 3.45, 5.75, t, color);
            ui.disc(a, t * 1.5, color);
            ui.disc(b, t * 1.5, color);
            arrow_head(
                ui,
                c + Vec2::from_angle(2.6) * r * 0.8,
                Vec2::from_angle(2.6 + FRAC_PI_2),
                r * 0.35,
                t,
                color,
            );
            arrow_head(
                ui,
                c + Vec2::from_angle(5.75) * r * 0.8,
                Vec2::from_angle(5.75 + FRAC_PI_2),
                r * 0.35,
                t,
                color,
            );
        }
        Glyph::Formation => {
            for (dx, dy) in [
                (-0.6, 0.45),
                (0.0, 0.45),
                (0.6, 0.45),
                (-0.3, -0.15),
                (0.3, -0.15),
                (0.0, -0.7),
            ] {
                ui.disc(c + Vec2::new(dx * r, dy * r), t * 1.25, color);
            }
        }
        Glyph::GroundAttack => {
            reticle(ui, c + Vec2::new(0.0, -r * 0.2), r * 0.8);
            ui.stroke(
                c + Vec2::new(-r, r * 0.8),
                c + Vec2::new(r, r * 0.8),
                t,
                color,
            );
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
            ui.stroke(
                c + Vec2::new(-r * 0.85, -r * 0.85),
                c + Vec2::new(r * 0.85, r * 0.85),
                t,
                color,
            );
        }
        Glyph::Dive | Glyph::Surface => {
            // The sea's surface, a hull under or on it, and which way it is going.
            let sea = c.y - r * 0.35;
            ui.stroke(
                Vec2::new(c.x - r * 0.9, sea),
                Vec2::new(c.x + r * 0.9, sea),
                t,
                color,
            );
            let (hull, dir) = if glyph == Glyph::Dive {
                (c.y + r * 0.55, Vec2::Y)
            } else {
                (sea - t * 1.5, -Vec2::Y)
            };
            ui.fill(Rect::new(c.x - r * 0.6, hull - t, r * 1.2, t * 2.0), color);
            ui.fill(
                Rect::new(c.x - r * 0.08, hull - t * 3.0, r * 0.3, t * 2.0),
                color,
            );
            let from = if glyph == Glyph::Dive {
                sea + t * 2.5
            } else {
                c.y + r * 0.85
            };
            let tip = Vec2::new(c.x + r * 0.7, from + dir.y * r * 0.55);
            ui.stroke(Vec2::new(c.x + r * 0.7, from), tip, t, color);
            arrow_head(ui, tip, dir, r * 0.25, t, color);
        }
        Glyph::HoldPosition => {
            // A spot staked out: a square with a pin at its middle.
            let k = r * 0.75;
            let corners = [
                Vec2::new(-k, -k),
                Vec2::new(k, -k),
                Vec2::new(k, k),
                Vec2::new(-k, k),
            ];
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
        Glyph::Land => {
            // A broad hull coming straight down onto the ground, legs out under it.
            let ground = c.y + r * 0.8;
            ui.stroke(
                Vec2::new(c.x - r * 0.95, ground),
                Vec2::new(c.x + r * 0.95, ground),
                t,
                color,
            );
            let hull = c.y - r * 0.05;
            ui.stroke(
                Vec2::new(c.x - r * 0.75, hull),
                Vec2::new(c.x + r * 0.75, hull),
                t * 1.6,
                color,
            );
            for s in [-1.0, 1.0] {
                let hip = Vec2::new(c.x + s * r * 0.5, hull);
                ui.stroke(
                    hip,
                    Vec2::new(c.x + s * r * 0.7, ground - t * 1.4),
                    t,
                    color,
                );
            }
            let tip = Vec2::new(c.x, hull - t * 1.6);
            ui.stroke(Vec2::new(c.x, c.y - r * 0.95), tip, t, color);
            arrow_head(ui, tip, Vec2::Y, r * 0.3, t, color);
        }
        Glyph::Unload => {
            // The hull on the ground, its ramp let down, and an arrow walking off it.
            let ground = c.y + r * 0.75;
            ui.stroke(
                Vec2::new(c.x - r * 0.95, ground),
                Vec2::new(c.x + r * 0.95, ground),
                t,
                color,
            );
            let deck = c.y + r * 0.1;
            ui.stroke(
                Vec2::new(c.x - r * 0.9, deck),
                Vec2::new(c.x + r * 0.05, deck),
                t * 1.6,
                color,
            );
            ui.stroke(
                Vec2::new(c.x + r * 0.05, deck),
                Vec2::new(c.x + r * 0.55, ground),
                t,
                color,
            );
            let from = Vec2::new(c.x - r * 0.1, c.y - r * 0.55);
            let tip = Vec2::new(c.x + r * 0.9, c.y - r * 0.05);
            ui.stroke(from, tip, t, color);
            arrow_head(ui, tip, (tip - from).normalize(), r * 0.32, t, color);
        }
        Glyph::Board => {
            // The hull on the ground, its ramp let down, and an arrow walking up into it.
            let ground = c.y + r * 0.75;
            ui.stroke(
                Vec2::new(c.x - r * 0.95, ground),
                Vec2::new(c.x + r * 0.95, ground),
                t,
                color,
            );
            let deck = c.y + r * 0.1;
            ui.stroke(
                Vec2::new(c.x - r * 0.9, deck),
                Vec2::new(c.x + r * 0.05, deck),
                t * 1.6,
                color,
            );
            ui.stroke(
                Vec2::new(c.x + r * 0.05, deck),
                Vec2::new(c.x + r * 0.55, ground),
                t,
                color,
            );
            let from = Vec2::new(c.x + r * 0.9, c.y - r * 0.05);
            let tip = Vec2::new(c.x - r * 0.1, c.y - r * 0.55);
            ui.stroke(from, tip, t, color);
            arrow_head(ui, tip, (tip - from).normalize(), r * 0.32, t, color);
        }
        Glyph::TakeOff => {
            // A broad hull rising off the ground, legs folding, an arrow up over it.
            let ground = c.y + r * 0.8;
            ui.stroke(
                Vec2::new(c.x - r * 0.95, ground),
                Vec2::new(c.x + r * 0.95, ground),
                t,
                color,
            );
            let hull = c.y + r * 0.2;
            ui.stroke(
                Vec2::new(c.x - r * 0.75, hull),
                Vec2::new(c.x + r * 0.75, hull),
                t * 1.6,
                color,
            );
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
