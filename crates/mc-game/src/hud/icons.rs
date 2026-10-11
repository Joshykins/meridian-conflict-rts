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
        IconKind::Damper => {
            // The field, a ring broken on the diagonals; four chevrons on the axes pulling
            // in on the point at its middle (see `icons.wgsl`).
            let q = PI / 4.0;
            for i in 0..4 {
                let a = q + i as f32 * FRAC_PI_2;
                ui.arc(
                    c,
                    r * 0.76 * 0.9,
                    a + 0.24,
                    a + FRAC_PI_2 - 0.24,
                    line,
                    color,
                );
            }
            for d in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
                arrow_head(ui, c + d * r * 0.2 * 0.9, -d, r * 0.26 * 0.9, line, color);
            }
            ui.disc(c, r * 0.13, color);
        }
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
        IconKind::Elevator => elevator(ui, c, r, color),
        IconKind::WingedTitan => winged_titan(ui, c, r, color),
        IconKind::GraviticEngine => gravitic_engine(ui, c, r, color),
        IconKind::AirStaging => air_staging(ui, c, r, color),
        IconKind::SiegeWalker => siege_walker(ui, c, r, color),
        IconKind::AssaultWalker => assault_walker(ui, c, r, color),
        IconKind::Salvage | IconKind::SalvageCarrier | IconKind::SalvageDrone => {
            salvage(ui, kind, c, r, color, cut)
        }
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
/// materials as a mine does) with its domain's mark: a reach ring for a structure, a hull for a
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
    tier5_frame(ui, c, r, color);
}

/// A space elevator from the front, as `icons.wgsl` draws it (`sd_elevator`): its cable
/// coming down out of the sky with a platform on it, into the clamp jaws of two tower
/// flanks and the one between them. Given in the renderer's icon square (y up), placed in
/// a box of half-size `r`; framed as a tier 5 where there is room.
pub fn elevator(ui: &mut Ui, c: Vec2, r: f32, color: Color) {
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
    bx(ui, 0.0, 0.42, 0.045, 0.54);
    bx(ui, 0.0, 0.5, 0.26, 0.07);
    bx(ui, 0.0, -0.8, 0.82, 0.07);
    for side in [-1.0, 1.0] {
        let tower = [(0.52, -0.76), (0.8, -0.76), (0.6, 0.08), (0.42, 0.08)];
        fill_outline(ui, &tower.map(|(x, y)| at(side * x, y)), color);
        limb(ui, (side * 0.47, 0.02), (side * 0.13, -0.16), 0.07);
    }
    let middle = [(-0.17, -0.76), (0.17, -0.76), (0.08, -0.3), (-0.08, -0.3)];
    fill_outline(ui, &middle.map(|(x, y)| at(x, y)), color);
    tier5_frame(ui, c, r, color);
}

/// A titan's figure, framed, for a card: the winged titan's or the walking one's.
pub fn figure(ui: &mut Ui, kind: IconKind, c: Vec2, r: f32, color: Color) {
    match kind {
        IconKind::WingedTitan => winged_titan(ui, c, r, color),
        IconKind::GraviticEngine => gravitic_engine(ui, c, r, color),
        _ => titan(ui, c, r, color),
    }
}

/// A tier 5's corner brackets round its figure, where there is room for them.
fn tier5_frame(ui: &mut Ui, c: Vec2, r: f32, color: Color) {
    if r >= 9.0 {
        let frame = Rect::new(c.x - r * 1.05, c.y - r * 1.02, r * 2.1, r * 2.36);
        ui.brackets(
            frame,
            r * 0.32,
            [color[0], color[1], color[2], color[3] * 0.8],
        );
    }
}

/// Draws a figure given in the renderer's icon square (y up) into a box of half-size `r`
/// round `c`, with the same parts `icons.wgsl` builds its shapes from.
struct Pen<'a, 'u> {
    ui: &'a mut Ui<'u>,
    c: Vec2,
    k: f32,
    color: Color,
}

impl<'a, 'u> Pen<'a, 'u> {
    fn new(ui: &'a mut Ui<'u>, c: Vec2, r: f32, color: Color) -> Self {
        Self {
            ui,
            c,
            k: r * 0.9,
            color,
        }
    }

    fn at(&self, (x, y): (f32, f32)) -> Vec2 {
        self.c + Vec2::new(x, -y) * self.k
    }

    /// `sd_box`: centre and half-size.
    fn rect(&mut self, (x, y): (f32, f32), (hw, hh): (f32, f32)) {
        let (a, b) = (self.at((x - hw, y + hh)), self.at((x + hw, y - hh)));
        self.ui
            .fill(Rect::new(a.x, a.y, b.x - a.x, b.y - a.y), self.color);
    }

    /// `sd_taper` (`sd_segment` when the radii match): a limb from `a` to `b`.
    fn limb(&mut self, a: (f32, f32), b: (f32, f32), ra: f32, rb: f32) {
        let (pa, pb) = (self.at(a), self.at(b));
        let n = (pb - pa).normalize_or_zero().perp();
        let (ra, rb) = (ra * self.k, rb * self.k);
        fill_outline(
            self.ui,
            &[pa + n * ra, pb + n * rb, pb - n * rb, pa - n * ra],
            self.color,
        );
        self.ui.disc(pa, ra, self.color);
        self.ui.disc(pb, rb, self.color);
    }

    /// `sd_poly4`.
    fn quad(&mut self, points: [(f32, f32); 4]) {
        let points = points.map(|p| self.at(p));
        fill_outline(self.ui, &points, self.color);
    }
}

/// A tier-5 winged titan from the front, as `icons.wgsl` draws it (`sd_winged_titan`):
/// hovering, three blade feathers fanned off each shoulder vane, guns hung at its sides,
/// legs trailing to points. Framed as a tier 5 where there is room.
pub fn winged_titan(ui: &mut Ui, c: Vec2, r: f32, color: Color) {
    let mut pen = Pen::new(ui, c, r, color);
    pen.rect((0.0, 0.52), (0.08, 0.09));
    for s in [-1.0, 1.0] {
        pen.quad([(0.0, 0.42), (s * 0.22, 0.42), (s * 0.13, 0.0), (0.0, 0.0)]);
        pen.quad([
            (0.0, 0.02),
            (s * 0.14, 0.02),
            (s * 0.08, -0.16),
            (0.0, -0.16),
        ]);
        pen.limb((s * 0.18, 0.34), (s * 0.4, 0.5), 0.08, 0.08);
        for (a, tip) in [
            ((0.34, 0.5), (0.56, 0.98)),
            ((0.4, 0.48), (0.88, 0.78)),
            ((0.4, 0.42), (0.94, 0.4)),
        ] {
            pen.limb((s * a.0, a.1), (s * tip.0, tip.1), 0.085, 0.03);
        }
        pen.limb((s * 0.26, 0.34), (s * 0.3, 0.04), 0.06, 0.06);
        pen.rect((s * 0.31, -0.14), (0.06, 0.17));
    }
    pen.limb((-0.08, -0.14), (-0.15, -0.88), 0.08, 0.025);
    pen.limb((0.08, -0.14), (0.16, -0.66), 0.08, 0.025);
    tier5_frame(ui, c, r, color);
}

/// A tier-5 gravitic engine from the front, as `icons.wgsl` draws it
/// (`sd_gravitic_engine`): a crown of shards leaning in to a point over its well, on a
/// broken ground line. Framed as a tier 5 where there is room.
pub fn gravitic_engine(ui: &mut Ui, c: Vec2, r: f32, color: Color) {
    let mut pen = Pen::new(ui, c, r, color);
    pen.limb((0.0, -0.36), (0.0, 0.98), 0.1, 0.02);
    for s in [-1.0, 1.0] {
        pen.rect((s * 0.5, -0.66), (0.36, 0.07));
        pen.limb((s * 0.34, -0.58), (s * 0.1, 0.8), 0.1, 0.02);
        pen.limb((s * 0.68, -0.58), (s * 0.24, 0.5), 0.09, 0.02);
    }
    pen.limb((0.0, -0.5), (0.0, -0.5), 0.1, 0.1);
    tier5_frame(ui, c, r, color);
}

/// An air staging base, as `icons.wgsl` draws it (case 42): a bunker in the ground and a
/// V over it pointing down into it.
pub fn air_staging(ui: &mut Ui, c: Vec2, r: f32, color: Color) {
    let mut pen = Pen::new(ui, c, r, color);
    pen.rect((0.0, -0.46), (0.78, 0.22));
    for s in [-1.0, 1.0] {
        pen.limb((s * 0.52, 0.62), (0.0, 0.04), 0.13, 0.13);
    }
}

/// A four-legged siege walker in profile, as `icons.wgsl` draws it (`sd_siege_walker`):
/// legs arched over a slung hull, the beam head low at the front, a rocket pack on its back.
pub fn siege_walker(ui: &mut Ui, c: Vec2, r: f32, color: Color) {
    let mut pen = Pen::new(ui, c, r, color);
    pen.rect((-0.04, 0.16), (0.44, 0.17));
    pen.quad([(0.34, 0.24), (0.66, 0.16), (0.64, -0.06), (0.34, -0.02)]);
    pen.rect((0.82, 0.06), (0.16, 0.05));
    pen.rect((-0.2, 0.38), (0.18, 0.08));
    for s in [-1.0, 1.0] {
        pen.limb((s * 0.3, 0.22), (s * 0.56, 0.56), 0.08, 0.08);
        pen.limb((s * 0.56, 0.56), (s * 0.82, -0.72), 0.07, 0.05);
        pen.limb((s * 0.12, 0.08), (s * 0.3, -0.3), 0.07, 0.07);
        pen.limb((s * 0.3, -0.3), (s * 0.34, -0.72), 0.06, 0.05);
    }
}

/// A reverse-kneed assault walker in profile, as `icons.wgsl` draws it
/// (`sd_assault_walker`): a hunched hull with a launcher hump, a low pointed head and the
/// gun arm out in front.
pub fn assault_walker(ui: &mut Ui, c: Vec2, r: f32, color: Color) {
    let mut pen = Pen::new(ui, c, r, color);
    pen.quad([(-0.46, 0.34), (0.24, 0.4), (0.36, 0.06), (-0.4, 0.04)]);
    pen.quad([(0.2, 0.34), (0.6, 0.26), (0.58, 0.16), (0.24, 0.12)]);
    pen.quad([(-0.4, 0.3), (-0.08, 0.36), (0.02, 0.6), (-0.32, 0.58)]);
    pen.rect((0.36, 0.0), (0.26, 0.12));
    pen.rect((0.76, 0.05), (0.16, 0.035));
    pen.rect((0.76, -0.06), (0.16, 0.035));
    for (o, lift) in [(-0.16, 0.0), (0.06, 0.12)] {
        let knee = (o + 0.3, -0.36 + lift);
        let ankle = (o - 0.02, -0.66 + lift);
        pen.limb((o, 0.02), knee, 0.1, 0.1);
        pen.limb(knee, ankle, 0.08, 0.08);
        let (x, y) = ankle;
        pen.quad([
            (x - 0.1, y + 0.02),
            (x + 0.08, y + 0.02),
            (x + 0.22, y - 0.14),
            (x - 0.1, y - 0.14),
        ]);
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
    /// A capital ship jumps: streaks running into a point, and the ring it comes out of.
    Warp,
    /// A factory's batch: a block of units held behind a gate.
    Batch,
    /// A prefab: lots of three sizes laid out together, seen from above.
    Prefab,
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
        Glyph::Batch => {
            // Two rows of three, waiting behind the gate on the right.
            let s = r * 0.34;
            for (dx, dy) in [
                (-0.85, -0.42),
                (-0.3, -0.42),
                (0.25, -0.42),
                (-0.85, 0.42),
                (-0.3, 0.42),
                (0.25, 0.42),
            ] {
                let p = c + Vec2::new(dx * r, dy * r);
                ui.fill(Rect::new(p.x - s * 0.5, p.y - s * 0.5, s, s), color);
            }
            ui.stroke(
                c + Vec2::new(r * 0.8, -r * 0.9),
                c + Vec2::new(r * 0.8, r * 0.9),
                t,
                color,
            );
        }
        Glyph::Prefab => {
            // A big lot, a small one beside it and a long one under both, outlined.
            let lots = [
                (-1.0, -1.0, 0.25, 0.25),
                (0.45, -1.0, 1.0, -0.2),
                (-1.0, 0.45, 1.0, 1.0),
            ];
            for (x0, y0, x1, y1) in lots {
                let (a, b) = (c + Vec2::new(x0, y0) * r, c + Vec2::new(x1, y1) * r);
                let pts = [a, Vec2::new(b.x, a.y), b, Vec2::new(a.x, b.y)];
                ui.polyline(&pts, t, color, true);
            }
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
        Glyph::Warp => {
            // Three streaks of light running in from the lower left, and the ring the
            // ship comes out of at the far end, the jump's arrow through it.
            let dir = Vec2::new(1.0, -1.0).normalize();
            let side = dir.perp();
            let exit = c + dir * r * 0.42;
            for (off, from) in [(-0.34, -0.55), (0.0, -1.0), (0.34, -0.55)] {
                let a = c + dir * r * from + side * r * off;
                let b = c + dir * r * 0.05 + side * r * off * 0.45;
                ui.stroke(a, b, t, color);
            }
            ui.arc(exit, r * 0.42, 0.0, TAU, t, color);
            arrow_head(ui, exit + dir * r * 0.18, dir, r * 0.3, t, color);
        }
    }
}
